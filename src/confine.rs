//! Per-site composite confinement: *does the region this construction site
//! builds become unreachable once its enclosing iteration ends?*
//!
//! # Why this exists
//!
//! A memory planner that wants to give one construction site a single reused
//! slot needs to know that the previous occupant is dead. The
//! composite-region-reuse work calls such a site **confined**. This module
//! answers the question per site, over a chunk the caller already holds.
//!
//! # The standard: useful and sound, NOT complete
//!
//! A flow this analysis cannot establish is treated as escaping. That is the
//! sound direction, and it is the operator's stated standard — general
//! confinement analysis is not solved, and the goal is a predicate good enough
//! to be practically useful.
//!
//! # Three values, and why the third is not a nicety
//!
//! [`Confinement`](crate::confine::Confinement) has a third value,
//! [`CannotEstablish`](crate::confine::Confinement::CannotEstablish),
//! distinct from [`Escapes`](crate::confine::Confinement::Escapes).
//! **Soundness is identical either way**: a consumer must treat both as
//! "do not reuse". The distinction is *measurement*. Folded together, the
//! negative count moves for two unrelated reasons and it becomes impossible
//! to tell an analysis that improved from one that did not.
//!
//! # What a `Confined` verdict rests on
//!
//! [`route_of`](crate::confine::route_of) classifies **every** opcode by
//! what it can do to a composite's region, as an exhaustive `match`. A new
//! opcode is a compile error here, not a silently-confined site. The
//! classification agrees with the independently derived one in
//! `tests/composite_escape_routes.rs`, and a test asserts the agreement so
//! neither can drift.
//!
//! # What this module does NOT do
//!
//! It produces the predicate, not the transformation: no slot assignment
//! and no actual reuse. It gives no whole-module verdict. It does not
//! summarise callees — a composite passed to a `Call` yields
//! [`CannotEstablish`](crate::confine::Confinement::CannotEstablish), which
//! is where the next increment shows up as a measurable change.
//!
//! # Known imprecision, stated rather than discovered later
//!
//! An indexed read of an array whose elements are composites aliases the
//! whole array, because the element's own shape is what the operand records
//! and the array's identity is not tracked per element. That is sound and
//! it costs precision on programs that index arrays of structs.

use crate::bytecode::{ArrayElem, Chunk, EnumField, Op, StructField, TupleField};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;

/// What an opcode can do to a composite's region.
///
/// The question each variant answers is: *can this instruction make a
/// composite body readable after the scope that constructed it has ended, by
/// ALIASING the body's arena region rather than copying its bytes?*
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Route {
    /// Cannot hold a composite operand at all, or consumes one and produces a
    /// scalar. No region outlives anything through it.
    NoRegion,
    /// Moves a composite only within the scope's own dataflow: the operand
    /// stack, or a frame slot.
    ///
    /// Also covers an instruction that carries a region out of a scope while
    /// **ending** that scope, which is `Break` and `BreakIf`. The value leaves,
    /// but no later execution of the site can alias it, because the break
    /// guarantees there is no later iteration.
    WithinIteration,
    /// Moves a composite outward but **copies** the bytes, so the destination
    /// does not alias the source region.
    CopiesOut,
    /// Makes the constructed body itself readable beyond the scope.
    Escapes,
}

/// Classify one opcode. Exhaustive over the instruction set **by construction**:
/// there is no wildcard arm, so adding an opcode fails to compile here rather
/// than defaulting to a confined verdict.
pub fn route_of(op: &Op) -> Route {
    use Route::*;
    match op {
        // --- Escaping. ---
        // The host receives the `Value`, which carries the arena handle rather
        // than a copy.
        Op::Yield => Escapes,
        // A frame slot whose binding was declared OUTSIDE the scope keeps the
        // handle after the scope ends. The opcode cannot distinguish an inner
        // binding from an outer one — that is a property of the slot the
        // compiler assigned — so the opcode's own classification must be its
        // worst case. The per-site analysis below refines it with liveness.
        Op::SetLocal(_) => Escapes,
        // Hands the value to the caller's frame, or to the host at chunk exit.
        Op::Return => Escapes,
        // A native receives the composite. What it retains is the host's
        // affair: a trust boundary, not a route this analysis can close.
        Op::CallExternalNative(_, _) | Op::CallVerifiedNative(_, _) => Escapes,

        // --- Copying outward, but not aliasing. ---
        // `write_data_slot` packs a flat body into the persistent composite
        // pool at its baked offset, so no ephemeral handle is stored.
        Op::SetData(_) | Op::SetDataIndexed(_, _) => CopiesOut,
        // The flat path packs operands directly into the new allocation,
        // resolving any nested arena child in place.
        Op::NewComposite(_) => CopiesOut,

        // --- Within the scope only. ---
        Op::Call(_, _)
        | Op::Dup
        | Op::GetLocal(_)
        | Op::GetData(_)
        | Op::GetDataIndexed(_, _)
        | Op::PopN(_) => WithinIteration,
        // Projections. A scalar field copies its word out; a nested composite
        // field yields a view onto the parent's bytes, which aliases the parent
        // rather than creating a new escape.
        Op::GetField(_) | Op::GetIndex(_) | Op::GetTupleField(_) | Op::GetEnumField(_) => {
            WithinIteration
        }
        // `Break` transfers control and the whole operand stack goes with it,
        // so a composite sitting on the stack does cross the edge. It is not
        // escaping, and the reason is not that it cannot carry a region — it is
        // that it **ends the scope**, and the reuse hazard needs a next
        // iteration.
        Op::Break(_) | Op::BreakIf(_) => WithinIteration,

        // --- No region can leave through these. ---
        Op::Const(_) | Op::PushImmediate(_) | Op::BoundsCheck(_) => NoRegion,
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod | Op::Neg => NoRegion,
        // Comparisons admit composite operands and produce a boolean.
        Op::CmpEq | Op::CmpNe | Op::CmpLt | Op::CmpGt | Op::CmpLe | Op::CmpGe | Op::Not => NoRegion,
        Op::If(_) | Op::Else(_) | Op::EndIf | Op::Loop(_) | Op::EndLoop(_) => NoRegion,
        Op::Stream => NoRegion,
        // `Reset` ENDS the window rather than opening one: it reclaims the
        // ephemeral region and advances the epoch, which is what makes every
        // outstanding handle stale.
        Op::Reset => NoRegion,
        // Consume a composite, produce a scalar.
        Op::Len | Op::IsEnum(_, _, _) | Op::IsStruct(_) => NoRegion,
        Op::IntToFloat | Op::FloatToInt | Op::WordToByte | Op::ByteToWord => NoRegion,
        Op::WordToFixed(_) | Op::FixedToWord(_) | Op::FixedMul(_) | Op::FixedDiv(_) => NoRegion,
        Op::Trap(_) => NoRegion,
        Op::CheckedAdd | Op::CheckedSub | Op::CheckedMul(_) => NoRegion,
        Op::CheckedNeg | Op::CheckedDiv(_) | Op::CheckedMod => NoRegion,
        Op::BitAnd | Op::BitOr | Op::BitXor | Op::Shl | Op::Shr => NoRegion,
    }
}

/// The answer for one construction site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confinement {
    /// Every route out of the scope was considered and none carries the region
    /// past the scope's end.
    Confined,
    /// No demonstrable escape was found, but a flow could not be ruled out.
    ///
    /// **Treat exactly as [`Confinement::Escapes`] for soundness.** It is
    /// separate so that improvements to the analysis are visible as a shift
    /// from this value to [`Confinement::Confined`].
    CannotEstablish,
    /// A route was found that demonstrably carries the region out.
    Escapes,
}

impl Confinement {
    /// How strong a finding this is, ascending. Declared rather than left to
    /// the derived order so that reordering the variants cannot silently change
    /// which finding a site keeps.
    ///
    /// [`Confinement::Escapes`] outranks [`Confinement::CannotEstablish`]
    /// because it is actionable: it names a real route, where the other names
    /// only a limit of this analysis.
    fn severity(self) -> u8 {
        match self {
            Confinement::Confined => 0,
            Confinement::CannotEstablish => 1,
            Confinement::Escapes => 2,
        }
    }
}

/// The scope a site's confinement is judged against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// One iteration of the iterating loop whose `Op::Loop` is at this address.
    ///
    /// Only **iterating** loops appear here. `Op::Loop` also marks dispatch
    /// scopes — the lowering of `match` and of multi-clause `if` — whose bodies
    /// run once, so a composite built inside one is never reused and the
    /// question does not arise.
    Iteration {
        /// Address of the `Op::Loop` that opens the iterating scope.
        loop_ip: usize,
    },
    /// One invocation of the chunk, for a site with no enclosing iterating
    /// loop. Reuse across invocations is the corresponding hazard.
    Invocation,
}

/// Why a site is not confined. Recorded so a negative verdict can be acted on
/// rather than merely counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// Nothing carries the region out.
    None,
    /// The region reaches a `Yield` at this address.
    Yielded {
        /// Address of the `Yield`.
        ip: usize,
    },
    /// The region reaches a `Return` at this address.
    Returned {
        /// Address of the `Return`.
        ip: usize,
    },
    /// The region reaches a native call at this address, which is a trust
    /// boundary rather than a route that can be closed from here.
    HandedToNative {
        /// Address of the native call.
        ip: usize,
    },
    /// The region is written to a local slot that this analysis could not show
    /// dead at the scope boundary.
    StoredToLiveSlot {
        /// Address of the `SetLocal`.
        ip: usize,
        /// The slot written.
        slot: u16,
    },
    /// The region is passed to a Keleusma call, and no callee summary exists.
    PassedToCall {
        /// Address of the `Call`.
        ip: usize,
    },
    /// An opcode classified as escaping has no handler in the transfer
    /// function, so its route was not followed.
    ///
    /// This is reachable only by adding an escaping opcode without extending
    /// the analysis. It exists so that omission degrades the verdict rather
    /// than passing unnoticed.
    UnmodelledRoute {
        /// Address of the unhandled instruction.
        ip: usize,
    },
    /// The abstract interpretation could not be completed for this chunk.
    NotAnalysed,
}

/// One site's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SiteVerdict {
    /// Address of the `Op::NewComposite` that builds the region.
    pub ip: usize,
    /// What the verdict is relative to.
    pub scope: Scope,
    /// The answer.
    pub verdict: Confinement,
    /// The first route found, in the severity order of [`Confinement`].
    pub reason: Reason,
}

/// Answer the confinement question for every composite construction site in
/// `chunk`, in address order.
///
/// The chunk is expected to have passed structural verification; if the operand
/// stack cannot be tracked (which that verification rules out) every site is
/// reported [`Confinement::CannotEstablish`] with [`Reason::NotAnalysed`]
/// rather than being silently admitted.
pub fn chunk_confinement(chunk: &Chunk) -> Vec<SiteVerdict> {
    let scopes = site_scopes(chunk);
    if scopes.is_empty() {
        return Vec::new();
    }

    let mut escapes: BTreeMap<usize, (Confinement, Reason)> = BTreeMap::new();
    let mut note = |site: usize, verdict: Confinement, reason: Reason| {
        let slot = escapes
            .entry(site)
            .or_insert((Confinement::Confined, Reason::None));
        // Keep the most severe finding, so a site with both a real route and an
        // unestablished one reports the route.
        if verdict.severity() > slot.0.severity() {
            *slot = (verdict, reason);
        }
    };

    let ok = interpret(chunk, &scopes, &mut note);
    if !ok {
        return scopes
            .iter()
            .map(|(&ip, &scope)| SiteVerdict {
                ip,
                scope,
                verdict: Confinement::CannotEstablish,
                reason: Reason::NotAnalysed,
            })
            .collect();
    }

    scopes
        .iter()
        .map(|(&ip, &scope)| {
            let (verdict, reason) = escapes
                .get(&ip)
                .copied()
                .unwrap_or((Confinement::Confined, Reason::None));
            SiteVerdict {
                ip,
                scope,
                verdict,
                reason,
            }
        })
        .collect()
}

/// The construction sites and the scope each is judged against.
///
/// The dispatch discriminator: a `Op::Loop` scope containing an **unconditional**
/// `Break` targeting its own exit leaves in one pass and is therefore dispatch,
/// not iteration. A `for` range test compiles to a `BreakIf` and does not match.
fn site_scopes(chunk: &Chunk) -> BTreeMap<usize, Scope> {
    let ops = &chunk.ops;
    // Innermost enclosing iterating loop for every address, if any.
    let mut enclosing: Vec<Option<usize>> = vec![None; ops.len()];
    for (i, op) in ops.iter().enumerate() {
        let Op::Loop(exit) = op else { continue };
        let end = (*exit as usize).min(ops.len());
        if i + 1 >= end {
            continue;
        }
        let dispatch = ops[i + 1..end]
            .iter()
            .any(|o| matches!(o, Op::Break(t) if *t == *exit));
        if dispatch {
            continue;
        }
        // Later assignment wins, and loops are emitted outermost-first, so the
        // innermost enclosing loop is the one that lands last.
        for slot in enclosing.iter_mut().take(end).skip(i + 1) {
            *slot = Some(i);
        }
    }

    ops.iter()
        .enumerate()
        .filter(|(_, op)| matches!(op, Op::NewComposite(_)))
        .map(|(ip, _)| {
            let scope = match enclosing[ip] {
                Some(loop_ip) => Scope::Iteration { loop_ip },
                None => Scope::Invocation,
            };
            (ip, scope)
        })
        .collect()
}

/// The address range a scope's body occupies, as `[start, end)`.
fn scope_range(chunk: &Chunk, scope: Scope) -> (usize, usize) {
    match scope {
        Scope::Invocation => (0, chunk.ops.len()),
        Scope::Iteration { loop_ip } => match chunk.ops.get(loop_ip) {
            // `exit` addresses the instruction after `EndLoop`, so the body is
            // everything strictly between the `Loop` and that `EndLoop`.
            Some(Op::Loop(exit)) => {
                let end = (*exit as usize).min(chunk.ops.len());
                (loop_ip + 1, end.saturating_sub(1).max(loop_ip + 1))
            }
            _ => (0, chunk.ops.len()),
        },
    }
}

/// The set of sites a stack entry or local slot may alias. Empty means the
/// value holds no region built by any site in this chunk — which covers
/// scalars, parameters, and composites the caller built.
type Alias = BTreeSet<usize>;

#[derive(Clone, PartialEq, Eq)]
struct State {
    stack: Vec<Alias>,
    locals: Vec<Alias>,
}

/// Walk the chunk, reporting every route by which a site's region leaves its
/// scope. Returns `false` if the walk could not be completed.
fn interpret(
    chunk: &Chunk,
    scopes: &BTreeMap<usize, Scope>,
    note: &mut impl FnMut(usize, Confinement, Reason),
) -> bool {
    let state = State {
        stack: Vec::new(),
        locals: vec![Alias::new(); chunk.local_count as usize],
    };
    let mut breaks = Vec::new();
    walk(chunk, 0, chunk.ops.len(), state, &mut breaks, scopes, note).is_ok()
}

/// A walk that ran off the rails. Structural verification rules this out; it is
/// carried so a corrupt chunk produces a refusal rather than a panic or a
/// confident wrong answer.
struct Derailed;

/// Interpret `[start, end)`, returning the fall-through state or `None` if
/// every path leaves the region.
#[allow(clippy::too_many_arguments)]
fn walk(
    chunk: &Chunk,
    start: usize,
    end: usize,
    mut state: State,
    breaks: &mut Vec<State>,
    scopes: &BTreeMap<usize, Scope>,
    note: &mut impl FnMut(usize, Confinement, Reason),
) -> Result<Option<State>, Derailed> {
    let ops = &chunk.ops;
    let mut ip = start;
    while ip < end {
        let op = &ops[ip];
        match op {
            Op::Trap(_) => return Ok(None),
            Op::Return => {
                // `Return` PEEKS its value rather than popping it — the
                // verifier's own depth table gives it `(0, 0)` — so this reads
                // the top of the stack and does not disturb it. An empty stack
                // here means the chunk returns nothing, which carries no
                // region.
                let v = state.stack.last().cloned().unwrap_or_default();
                for &site in &v {
                    note(site, Confinement::Escapes, Reason::Returned { ip });
                }
                return Ok(None);
            }
            Op::Break(_) => {
                breaks.push(state);
                return Ok(None);
            }
            Op::BreakIf(_) => {
                // Pops its condition, then MAY leave. Both continuations are
                // live, so the state joins into the enclosing scope's exit set
                // and also falls through.
                apply(chunk, ip, op, &mut state, scopes, note)?;
                breaks.push(state.clone());
                ip += 1;
            }
            Op::If(target) => {
                apply(chunk, ip, op, &mut state, scopes, note)?;
                let target = *target as usize;
                if target > 0 && matches!(ops.get(target - 1), Some(Op::Else(_))) {
                    let Some(Op::Else(endif)) = ops.get(target - 1) else {
                        return Err(Derailed);
                    };
                    let endif = *endif as usize;
                    let then_end = walk(
                        chunk,
                        ip + 1,
                        target - 1,
                        state.clone(),
                        breaks,
                        scopes,
                        note,
                    )?;
                    let else_end = walk(chunk, target, endif, state, breaks, scopes, note)?;
                    match join(then_end, else_end) {
                        Some(joined) => state = joined,
                        None => return Ok(None),
                    }
                    ip = endif + 1;
                } else {
                    let skip = state.clone();
                    let then_end = walk(chunk, ip + 1, target, state, breaks, scopes, note)?;
                    match join(then_end, Some(skip)) {
                        Some(joined) => state = joined,
                        None => return Ok(None),
                    }
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                let exit = (*target as usize).min(ops.len());
                if exit == 0 || exit - 1 < ip + 1 {
                    return Err(Derailed);
                }
                // The lattice is the powerset of the site addresses and the
                // join only adds, so this ascending fixpoint terminates. The
                // cap is a backstop, not the termination argument.
                let mut head = state.clone();
                let cap = (head.locals.len() + 1) * (scopes.len() + 1) + 2;
                let mut body_end = None;
                let mut inner_breaks = Vec::new();
                for round in 0..=cap {
                    inner_breaks.clear();
                    body_end = walk(
                        chunk,
                        ip + 1,
                        exit - 1,
                        head.clone(),
                        &mut inner_breaks,
                        scopes,
                        note,
                    )?;
                    let Some(be) = &body_end else { break };
                    let widened = join_states(&head, be);
                    if widened == head {
                        break;
                    }
                    if round == cap {
                        return Err(Derailed);
                    }
                    head = widened;
                }
                // A `Break` out of this scope ends it, so its state is what
                // continues after the loop.
                //
                // A break targeting an OUTER scope is collected here too and
                // resumed after this loop, which it never reaches. That
                // over-approximates the state after the loop — more aliases,
                // never fewer — so it costs precision in the sound direction.
                // The typed verifier's walk has the same shape, deliberately.
                let mut after = body_end;
                for b in inner_breaks {
                    after = join(after, Some(b));
                }
                match after {
                    Some(s) => state = s,
                    None => return Ok(None),
                }
                ip = exit;
            }
            _ => {
                apply(chunk, ip, op, &mut state, scopes, note)?;
                ip += 1;
            }
        }
    }
    Ok(Some(state))
}

fn join(a: Option<State>, b: Option<State>) -> Option<State> {
    match (a, b) {
        (Some(x), Some(y)) => Some(join_states(&x, &y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn join_states(a: &State, b: &State) -> State {
    let unite = |x: &Vec<Alias>, y: &Vec<Alias>| -> Vec<Alias> {
        let n = x.len().max(y.len());
        (0..n)
            .map(|i| {
                let mut s = x.get(i).cloned().unwrap_or_default();
                if let Some(other) = y.get(i) {
                    s.extend(other.iter().copied());
                }
                s
            })
            .collect()
    };
    State {
        stack: unite(&a.stack, &b.stack),
        locals: unite(&a.locals, &b.locals),
    }
}

/// The transfer function for a straight-line instruction.
fn apply(
    chunk: &Chunk,
    ip: usize,
    op: &Op,
    state: &mut State,
    scopes: &BTreeMap<usize, Scope>,
    note: &mut impl FnMut(usize, Confinement, Reason),
) -> Result<(), Derailed> {
    // Pop and push counts come from the verifier's own depth table rather than
    // a second one that could drift from it.
    //
    // The split into `pops` and `pushes` is derived rather than tabulated, and
    // the STACK HEIGHT is exact whichever way it splits: `h - pops + pushes`
    // is `h + net` identically. The split only decides which entries feed the
    // result, and for every opcode where that matters — the calls, `Dup`, the
    // projections, `NewComposite`, `SetLocal`, `Yield` — `need` is the true
    // arity.
    let (need, net) = crate::verify::op_depth_effect(op, chunk);
    let pops = need.max(0) as usize;
    let pushes = (need + net).max(0) as usize;
    if state.stack.len() < pops {
        return Err(Derailed);
    }
    let taken: Vec<Alias> = state.stack.split_off(state.stack.len() - pops);
    let union: Alias = taken.iter().flatten().copied().collect();

    match op {
        Op::Yield => {
            for &site in &union {
                note(site, Confinement::Escapes, Reason::Yielded { ip });
            }
        }
        Op::CallExternalNative(_, _) | Op::CallVerifiedNative(_, _) => {
            for &site in &union {
                note(site, Confinement::Escapes, Reason::HandedToNative { ip });
            }
        }
        Op::Call(_, _) => {
            // No callee summary yet: a callee may retain or re-export any
            // composite argument. Sound, and deliberately visible as the
            // measurement that a summary would move.
            for &site in &union {
                note(
                    site,
                    Confinement::CannotEstablish,
                    Reason::PassedToCall { ip },
                );
            }
        }
        Op::SetLocal(slot) => {
            for &site in &union {
                let scope = scopes.get(&site).copied().unwrap_or(Scope::Invocation);
                let (s, e) = scope_range(chunk, scope);
                if !boundary_dead(chunk, s, e, *slot) {
                    note(
                        site,
                        Confinement::CannotEstablish,
                        Reason::StoredToLiveSlot { ip, slot: *slot },
                    );
                }
            }
        }
        // `Return` is the remaining escaping route and is handled by the walk,
        // which peeks rather than pops.
        Op::Return => {}
        other => {
            // BACKSTOP. Every route classified `Escapes` needs a handler
            // above; one without a handler would leave its sites reported
            // confined on the strength of a flow nothing followed.
            //
            // A new opcode is a compile error in `route_of`, which forces a
            // decision about its route — but not about this match, whose
            // catch-all arm would silently accept it. So the catch-all asks
            // the classification, and an escaping route with no handler
            // degrades to "cannot establish" rather than to silence.
            if route_of(other) == Route::Escapes {
                for &site in &union {
                    note(
                        site,
                        Confinement::CannotEstablish,
                        Reason::UnmodelledRoute { ip },
                    );
                }
            }
        }
    }

    // What the results may alias.
    //
    // The classification does the load-bearing work here. An opcode routed
    // `NoRegion` consumes any composite operand and produces a scalar, so its
    // result aliases nothing — which is what keeps `p.a + p.b` from reading as
    // the composite `p` and turning an ordinary field sum into a false escape
    // at the enclosing `Return`.
    let produced: Alias = if route_of(op) == Route::NoRegion {
        Alias::new()
    } else {
        match op {
            // A fresh region. Its operands were copied into it, so it aliases none
            // of them.
            Op::NewComposite(_) => {
                let mut s = Alias::new();
                s.insert(ip);
                s
            }
            // Copies bytes out; nothing is produced that aliases anything.
            Op::SetData(_) | Op::SetDataIndexed(_, _) => Alias::new(),
            // Reads a slot.
            Op::GetLocal(slot) => state.locals.get(*slot as usize).cloned().ok_or(Derailed)?,
            // The resumed value comes from the host, not from a site here.
            Op::Yield => Alias::new(),
            // A returned value may be an argument passed straight through.
            Op::Call(_, _) => union.clone(),
            // A native's result is the host's, built outside this chunk.
            Op::CallExternalNative(_, _) | Op::CallVerifiedNative(_, _) => Alias::new(),
            // A projection reads either a fixed-size scalar, which COPIES a
            // word out and aliases nothing, or a nested composite, which is a
            // VIEW onto the parent's bytes and therefore aliases the parent.
            //
            // The operand itself carries the distinction, baked by the
            // compiler, so this reads the answer rather than guessing it. The
            // pre-B28 boxed forms carry no shape and are treated as aliasing.
            Op::GetField(f) => project(&union, matches!(f, StructField::Flat { .. })),
            Op::GetTupleField(f) => project(&union, matches!(f, TupleField::Flat { .. })),
            Op::GetEnumField(f) => project(&union, matches!(f, EnumField::Flat { .. })),
            Op::GetIndex(e) => project(&union, matches!(e, ArrayElem::Flat { .. })),
            // Everything else forwards its operands, `Dup` being the case that
            // matters. Forwarding the union is the sound choice.
            _ => union.clone(),
        }
    };

    // An out-of-range slot would mean a dropped write or a read of a slot this
    // pass never modelled, and both UNDER-approximate: a region would go
    // untracked and the site could be reported confined on the strength of a
    // flow that was never followed. Structural verification rules the case out;
    // this refuses rather than defaulting.
    if let Op::SetLocal(slot) = op {
        let cell = state.locals.get_mut(*slot as usize).ok_or(Derailed)?;
        cell.clone_from(&union);
    }

    for _ in 0..pushes {
        state.stack.push(produced.clone());
    }
    Ok(())
}

/// The alias set a projection produces: nothing for a scalar read, and the
/// parent's own set for a nested composite, which the read is a view onto.
fn project(parent: &Alias, scalar: bool) -> Alias {
    if scalar { Alias::new() } else { parent.clone() }
}

/// Is `slot` dead at the boundary of the scope `[start, end)`?
///
/// Two conditions, both conservative in the sound direction:
///
/// 1. **No read outside the scope.** Any `GetLocal(slot)` elsewhere in the
///    chunk could observe the value after the scope ends.
/// 2. **No read before write inside the scope.** A read reaching a slot that
///    this pass through the scope has not yet written is reading the previous
///    iteration's value, which is exactly the cross-boundary carry in question.
///
/// A slot passing both is written and consumed entirely within one pass, so the
/// write does not carry the region past the boundary. This is the proof's B1r.
fn boundary_dead(chunk: &Chunk, start: usize, end: usize, slot: u16) -> bool {
    let ops = &chunk.ops;
    let read_outside = ops
        .iter()
        .enumerate()
        .any(|(i, o)| matches!(o, Op::GetLocal(s) if *s == slot) && !(start..end).contains(&i));
    if read_outside {
        return false;
    }
    !reads_before_write(chunk, start, end, slot, false).0
}

/// `(reads_before_write, defined_on_fall_through)`. `None` for the second means
/// no path falls out of the region.
fn reads_before_write(
    chunk: &Chunk,
    start: usize,
    end: usize,
    slot: u16,
    mut defined: bool,
) -> (bool, Option<bool>) {
    let ops = &chunk.ops;
    let mut rbw = false;
    let mut ip = start;
    while ip < end {
        match &ops[ip] {
            Op::GetLocal(s) if *s == slot && !defined => {
                return (true, None);
            }
            Op::SetLocal(s) if *s == slot => {
                defined = true;
                ip += 1;
            }
            Op::Trap(_) | Op::Return | Op::Break(_) => return (rbw, None),
            Op::If(target) => {
                let target = (*target as usize).min(ops.len());
                if target > 0
                    && matches!(ops.get(target - 1), Some(Op::Else(e)) if *e as usize <= ops.len())
                {
                    let Some(Op::Else(endif)) = ops.get(target - 1) else {
                        return (true, None);
                    };
                    let endif = (*endif as usize).min(ops.len());
                    let (a, ad) = reads_before_write(chunk, ip + 1, target - 1, slot, defined);
                    let (b, bd) = reads_before_write(chunk, target, endif, slot, defined);
                    rbw |= a || b;
                    defined = match (ad, bd) {
                        // Defined afterwards only if every path that continues
                        // defines it.
                        (Some(x), Some(y)) => x && y,
                        (Some(x), None) | (None, Some(x)) => x,
                        (None, None) => return (rbw, None),
                    };
                    ip = endif + 1;
                } else {
                    let (a, ad) = reads_before_write(chunk, ip + 1, target, slot, defined);
                    rbw |= a;
                    // The skip path keeps `defined` as it was, so the join is
                    // that path's value conjoined with the arm's.
                    defined = defined && ad.unwrap_or(true);
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                let exit = (*target as usize).min(ops.len());
                if exit == 0 || exit - 1 < ip + 1 {
                    return (true, None);
                }
                let (a, _) = reads_before_write(chunk, ip + 1, exit - 1, slot, defined);
                rbw |= a;
                // A loop may run zero times, so nothing it writes is guaranteed
                // afterwards.
                ip = exit;
            }
            _ => ip += 1,
        }
        if rbw {
            return (true, None);
        }
    }
    (rbw, Some(defined))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{BlockType, ConstValue};
    use crate::value_layout::CompositeKind;
    use alloc::string::String;

    fn chunk(ops: Vec<Op>) -> Chunk {
        Chunk {
            name: String::from("t"),
            ops,
            constants: vec![ConstValue::Int(0), ConstValue::Int(1)],
            struct_templates: Vec::new(),
            local_count: 4,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: Vec::new(),
            debug_pool: None,
        }
    }

    fn new_struct() -> Op {
        Op::NewComposite(crate::bytecode::NewCompositeOperand::Flat {
            kind: CompositeKind::Struct,
            count: 2,
            byte_size: 16,
        })
    }

    /// A `for`-shaped iterating loop whose body builds a composite, binds it,
    /// and reads a scalar field of it. `tail` is appended after the loop.
    ///
    /// The exit test is a `BreakIf`, which is what makes the scope ITERATING
    /// rather than dispatch.
    fn loop_program(tail: Vec<Op>) -> Chunk {
        loop_program_disposing(bind_and_read(), tail)
    }

    /// The ordinary disposition: bind the composite, read a scalar field of it,
    /// drop the word. Net effect on the stack is `-1`, the composite consumed.
    fn bind_and_read() -> Vec<Op> {
        vec![
            Op::SetLocal(1),
            Op::GetLocal(1),
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: crate::value_layout::ScalarKind::Int,
            }),
            Op::PopN(1),
        ]
    }

    /// `dispose` replaces the disposition of the constructed composite and must
    /// consume exactly it. Addresses are computed AFTER splicing, so a
    /// disposition of a different length does not silently invalidate the
    /// loop's jump targets — a mutation that corrupts the program proves
    /// nothing, and reads as a refusal rather than as the finding it was meant
    /// to demonstrate.
    fn loop_program_disposing(dispose: Vec<Op>, tail: Vec<Op>) -> Chunk {
        let mut body: Vec<Op> = vec![
            // exit test
            Op::GetLocal(0),
            Op::Const(1),
            Op::CmpGe,
            Op::BreakIf(0), // patched below
            // p = Struct { a, b }
            Op::Const(0),
            Op::Const(1),
            new_struct(),
        ];
        body.extend(dispose);
        body.extend([
            // i = i + 1
            Op::GetLocal(0),
            Op::Const(1),
            Op::Add,
            Op::SetLocal(0),
        ]);
        let mut ops: Vec<Op> = vec![Op::Const(0), Op::SetLocal(0), Op::Loop(0)];
        let loop_ip = ops.len() - 1;
        ops.extend(body);
        ops.push(Op::EndLoop(loop_ip as u16));
        let exit = ops.len() as u16;
        ops[loop_ip] = Op::Loop(exit);
        // Patch the exit test now that the address is known.
        for op in ops.iter_mut() {
            if matches!(op, Op::BreakIf(0)) {
                *op = Op::BreakIf(exit);
            }
        }
        ops.extend(tail);
        ops.push(Op::Const(0));
        ops.push(Op::Return);
        chunk(ops)
    }

    /// The construction site inside the loop body, whatever its address.
    fn loop_site(verdicts: &[SiteVerdict]) -> SiteVerdict {
        *verdicts
            .iter()
            .find(|v| matches!(v.scope, Scope::Iteration { .. }))
            .expect("the program has a site inside an iterating loop")
    }

    /// The baseline the two mutations below are measured against.
    ///
    /// A composite built and consumed inside one iteration, bound to a slot
    /// nothing reads afterwards. This is the shape of every ordinary
    /// per-iteration `let`, and admitting it is the whole point of the
    /// boundary-dead rule.
    #[test]
    fn a_per_iteration_binding_read_only_within_the_iteration_is_confined() {
        let c = loop_program(Vec::new());
        let v = loop_site(&chunk_confinement(&c));
        assert_eq!(
            (v.verdict, v.reason),
            (Confinement::Confined, Reason::None),
            "a composite bound inside the body and never read outside it is \
             confined; if this fails the boundary-dead rule has stopped \
             admitting the ordinary case"
        );
    }

    /// FALSIFIABILITY, mutation 1: add a read of the binding's slot AFTER the
    /// loop. The slot is then live across the boundary and the verdict must
    /// change.
    ///
    /// This is the mutation that makes the test above evidence rather than a
    /// constant. It differs from the baseline by three instructions and it
    /// compiles.
    #[test]
    fn a_slot_read_after_the_loop_defeats_the_boundary_dead_rule() {
        let c = loop_program(vec![
            Op::GetLocal(1),
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: crate::value_layout::ScalarKind::Int,
            }),
            Op::PopN(1),
        ]);
        let v = loop_site(&chunk_confinement(&c));
        assert_eq!(
            v.verdict,
            Confinement::CannotEstablish,
            "reading the slot after the loop makes the previous iteration's \
             region reachable, so the site must NOT be confined"
        );
        assert!(
            matches!(v.reason, Reason::StoredToLiveSlot { slot: 1, .. }),
            "the refusal must name the live slot, not some other route: {:?}",
            v.reason
        );
    }

    /// FALSIFIABILITY, mutation 2: yield the composite instead of binding it.
    /// The host holds the handle, so the verdict must be the strong one.
    #[test]
    fn yielding_the_composite_escapes_rather_than_merely_failing_to_establish() {
        // `Yield` pops the composite and pushes the host's reply; the `PopN`
        // drops that, so the disposition consumes exactly one value, as the
        // baseline's does.
        let c = loop_program_disposing(vec![Op::Yield, Op::PopN(1)], Vec::new());
        let v = loop_site(&chunk_confinement(&c));
        assert_eq!(
            v.verdict,
            Confinement::Escapes,
            "a yielded region demonstrably leaves, which is a stronger finding \
             than `CannotEstablish` and must not be reported as the weaker one; \
             reason was {:?}",
            v.reason
        );
        assert!(
            matches!(v.reason, Reason::Yielded { .. }),
            "the refusal must name the yield: {:?}",
            v.reason
        );
    }

    /// A dispatch scope is not an iteration, so a composite built in a `match`
    /// arm is judged against the invocation, not against a body that runs once.
    ///
    /// The discriminator is an UNCONDITIONAL `Break` targeting the scope's own
    /// exit. Conflating the two is an error this repository has made three
    /// times, and it produces a confident wrong answer rather than a refusal.
    #[test]
    fn a_dispatch_scope_is_not_treated_as_an_iteration() {
        let ops: Vec<Op> = vec![
            Op::Loop(7),
            Op::Const(0),
            Op::Const(1),
            new_struct(),
            Op::SetLocal(1),
            Op::Break(7),
            Op::EndLoop(0),
            Op::Const(0),
            Op::Return,
        ];
        let c = chunk(ops);
        let verdicts = chunk_confinement(&c);
        assert_eq!(verdicts.len(), 1);
        assert_eq!(
            verdicts[0].scope,
            Scope::Invocation,
            "an unconditional break to the scope's own exit makes it dispatch, \
             not iteration"
        );
    }

    /// A scalar field read copies a word out; it does not alias the parent. If
    /// it did, an ordinary field sum would read as the composite itself and
    /// the enclosing `Return` would report a false escape.
    #[test]
    fn a_scalar_field_read_does_not_alias_its_parent() {
        let field = Op::GetField(StructField::Flat {
            offset: 0,
            kind: crate::value_layout::ScalarKind::Int,
        });
        let ops: Vec<Op> = vec![
            Op::Const(0),
            Op::Const(1),
            new_struct(),
            Op::SetLocal(1),
            Op::GetLocal(1),
            field,
            Op::Return,
        ];
        let c = chunk(ops);
        let v = chunk_confinement(&c);
        assert_eq!(
            (v[0].verdict, v[0].reason),
            (Confinement::Confined, Reason::None),
            "returning a scalar field of a composite does not return the \
             composite"
        );
    }

    /// Every opcode the classification calls escaping has a handler in the
    /// transfer function, so none of them reaches the backstop.
    ///
    /// The backstop below `apply`'s explicit arms exists for an opcode added
    /// later without extending this analysis. **It cannot be exercised without
    /// adding an opcode**, so what is testable is the other half: that the
    /// handler set covers the escaping set as it stands. If a future route
    /// slips through, this test still passes and the backstop is what catches
    /// it — which is the division of labour it was written for.
    #[test]
    fn every_escaping_opcode_reaches_a_handler_rather_than_the_backstop() {
        // A composite is built, then consumed by the escaping opcode under
        // test. `Return` and the nested-field case are covered separately.
        let cases: Vec<(Op, Reason)> = vec![
            (Op::Yield, Reason::Yielded { ip: 3 }),
            (
                Op::CallExternalNative(0, 1),
                Reason::HandedToNative { ip: 3 },
            ),
            (
                Op::CallVerifiedNative(0, 1),
                Reason::HandedToNative { ip: 3 },
            ),
        ];
        for (consumer, expected) in cases {
            let ops: Vec<Op> = vec![
                Op::Const(0),
                Op::Const(1),
                new_struct(),
                consumer,
                Op::PopN(1),
                Op::Const(0),
                Op::Return,
            ];
            let c = chunk(ops);
            let v = chunk_confinement(&c);
            assert_eq!(
                v[0].reason, expected,
                "{consumer:?} must reach its own handler, not the backstop"
            );
            assert!(
                !matches!(v[0].reason, Reason::UnmodelledRoute { .. }),
                "{consumer:?} fell to the backstop, so its route is unfollowed"
            );
        }
    }

    /// The same program returning a NESTED composite field does alias, because
    /// the nested read is a view onto the parent's bytes.
    #[test]
    fn a_nested_composite_field_read_aliases_its_parent() {
        let field = Op::GetField(StructField::FlatNested {
            offset: 0,
            size: 8,
            variant: CompositeKind::Struct,
        });
        let ops: Vec<Op> = vec![
            Op::Const(0),
            Op::Const(1),
            new_struct(),
            Op::SetLocal(1),
            Op::GetLocal(1),
            field,
            Op::Return,
        ];
        let c = chunk(ops);
        let v = chunk_confinement(&c);
        assert_eq!(v[0].verdict, Confinement::Escapes);
        assert!(matches!(v[0].reason, Reason::Returned { .. }));
    }
}
