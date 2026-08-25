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
//! and no actual reuse. It gives no whole-module verdict — only a verdict
//! per site.
//!
//! # Callees
//!
//! [`module_confinement`](crate::confine::module_confinement) summarises what
//! each chunk does with each of its parameters, so a call that provably
//! cannot release its argument stops disqualifying the site. Two facts per
//! parameter, and both are load-bearing: whether the parameter can LEAK, and
//! whether the return value may ALIAS it. Recording only the first would force
//! every caller to assume every return aliases every argument, which is what
//! it already does with no summary at all.
//!
//! [`chunk_confinement`](crate::confine::chunk_confinement) keeps the
//! summary-free answer, in which every call is assumed to do both.
//!
//! **A summary that cannot be established answers conservatively**, and every
//! accessor defaults that way. A missing summary reading as a clean one would
//! turn this unsound in the direction hardest to notice, because the verdict
//! would improve.
//!
//! # Known imprecision, stated rather than discovered later
//!
//! An indexed read of an array whose elements are composites aliases the
//! whole array, because the element's own shape is what the operand records
//! and the array's identity is not tracked per element. That is sound and
//! it costs precision on programs that index arrays of structs.

use crate::bytecode::{ArrayElem, Chunk, EnumField, Module, Op, StructField, TupleField};
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

/// Something the walk observed a tracked region reach.
///
/// The walk reports events; what they MEAN is the caller's business. That
/// separation is what lets one walk answer both questions — is this site
/// confined, and does this parameter leak — without a second implementation
/// that would follow the escape routes slightly differently and pass its own
/// tests while doing so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    /// Reached a `Yield`.
    Yielded { ip: usize },
    /// Reached a `Return`.
    Returned { ip: usize },
    /// Reached a native call.
    HandedToNative { ip: usize },
    /// Written to a local slot.
    StoredToLocal { ip: usize, slot: u16 },
    /// Passed to a Keleusma call whose summary does not rule out a leak.
    PassedToCall { ip: usize },
    /// Reached an opcode classified as escaping with no handler.
    UnmodelledRoute { ip: usize },
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
/// `chunk`, with no knowledge of what its callees do.
///
/// Every Keleusma call is therefore assumed to leak and to re-export every
/// composite argument. For the answer that uses callee summaries, see
/// [`module_confinement`].
///
/// The chunk is expected to have passed structural verification; if the operand
/// stack cannot be tracked (which that verification rules out) every site is
/// reported [`CannotEstablish`](Confinement::CannotEstablish)
/// with [`Reason::NotAnalysed`] rather
/// than being silently admitted.
pub fn chunk_confinement(chunk: &Chunk) -> Vec<SiteVerdict> {
    chunk_confinement_with(chunk, &Summaries::default())
}

/// [`chunk_confinement`], using what is
/// known about the chunk's callees.
///
/// A callee with no entry, or a parameter beyond the entry's length, answers
/// conservatively, so passing [`Summaries::default`] is identical to
/// [`chunk_confinement`].
pub fn chunk_confinement_with(chunk: &Chunk, summaries: &Summaries) -> Vec<SiteVerdict> {
    let scopes = site_scopes(chunk);
    if scopes.is_empty() {
        return Vec::new();
    }

    let mut findings: BTreeMap<usize, (Confinement, Reason)> = BTreeMap::new();
    let mut note = |token: Token, event: Event| {
        // A summary run's parameters cannot appear here: this seeding tracks
        // construction sites only.
        let Token::Site(site) = token else { return };
        let Some((verdict, reason)) = site_meaning(chunk, &scopes, site, event) else {
            return;
        };
        let slot = findings
            .entry(site)
            .or_insert((Confinement::Confined, Reason::None));
        // Keep the most severe finding, so a site with both a real route and an
        // unestablished one reports the route.
        if verdict.severity() > slot.0.severity() {
            *slot = (verdict, reason);
        }
    };

    let ok = interpret(chunk, empty_locals(chunk), summaries, &mut note);
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
            let (verdict, reason) = findings
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

/// Answer the confinement question for every site in every chunk of `module`,
/// using summaries of what each chunk does with its parameters.
///
/// Returned per chunk, in chunk order.
pub fn module_confinement(module: &Module) -> Vec<Vec<SiteVerdict>> {
    let summaries = module_summaries(module);
    module
        .chunks
        .iter()
        .map(|c| chunk_confinement_with(c, &summaries))
        .collect()
}

/// What each chunk of `module` does with each of its parameters.
///
/// # Termination does not rest on the language's acyclicity guarantee
///
/// The call graph is acyclic by construction, and this does not rely on it. A
/// chunk is summarised only once every chunk it calls has a summary, and the
/// loop stops as soon as a round adds nothing. **A cycle simply never becomes
/// ready**, so it keeps the conservative no-summary answer rather than
/// recursing without bound. The round count is bounded by the number of chunks,
/// which makes termination checkable by inspection rather than by argument.
pub fn module_summaries(module: &Module) -> Summaries {
    let n = module.chunks.len();
    let mut per_chunk: Vec<Option<ChunkSummary>> = vec![None; n];
    let mut summaries = Summaries {
        per_chunk: per_chunk.clone(),
    };

    // Callees of each chunk, from the instruction stream. An out-of-range
    // index is left in: it can never become ready, so the caller stays
    // conservative, which is the right answer for a malformed module.
    let callees: Vec<Vec<usize>> = module
        .chunks
        .iter()
        .map(|c| {
            let mut v: Vec<usize> = c
                .ops
                .iter()
                .filter_map(|o| match o {
                    Op::Call(idx, _) => Some(*idx as usize),
                    _ => None,
                })
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        })
        .collect();

    for _round in 0..n {
        let mut progressed = false;
        for i in 0..n {
            if per_chunk[i].is_some() {
                continue;
            }
            // Ready when every callee other than itself is summarised. A
            // self-call is never ready, which is the cycle case.
            let ready = callees[i]
                .iter()
                .all(|&c| c != i && c < n && per_chunk[c].is_some());
            if !ready {
                continue;
            }
            per_chunk[i] = Some(summarize_chunk(&module.chunks[i], &summaries));
            summaries.per_chunk[i] = per_chunk[i].clone();
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    summaries
}

/// Summarise one chunk, given summaries for everything it calls.
fn summarize_chunk(chunk: &Chunk, summaries: &Summaries) -> ChunkSummary {
    let params = chunk.param_count as usize;
    if params == 0 {
        return ChunkSummary {
            leaks: Vec::new(),
            returns: Vec::new(),
        };
    }
    // The virtual machine makes a call's arguments the callee frame's first
    // `arg_count` local slots, so parameter `i` is slot `i`.
    let mut seed = empty_locals(chunk);
    if seed.len() < params {
        // Fewer slots than parameters is malformed; assume the worst.
        return ChunkSummary::opaque(params);
    }
    for (i, cell) in seed.iter_mut().enumerate().take(params) {
        cell.insert(Token::Param(i as u8));
    }

    let mut leaks = vec![false; params];
    let mut returns = vec![false; params];
    let mut note = |token: Token, event: Event| {
        let Token::Param(i) = token else { return };
        let i = i as usize;
        if i >= params {
            return;
        }
        match event {
            // The callee's frame dies when it returns, so a write into it
            // carries nothing past the call. This is where a parameter and a
            // construction site are answered by DIFFERENT rules, and why they
            // do not share a token space.
            Event::StoredToLocal { .. } => {}
            // Reaching the caller is not a leak — the caller sees the return
            // value and tracks what IT does with it. It does mean the return
            // value may alias this parameter.
            Event::Returned { .. } => returns[i] = true,
            Event::Yielded { .. }
            | Event::HandedToNative { .. }
            | Event::PassedToCall { .. }
            | Event::UnmodelledRoute { .. } => leaks[i] = true,
        }
    };

    if !interpret(chunk, seed, summaries, &mut note) {
        return ChunkSummary::opaque(params);
    }
    ChunkSummary { leaks, returns }
}

/// What an event means for a construction site, or `None` if it is harmless.
///
/// This is where a `SetLocal` is decided by LIVENESS: a write to a slot that is
/// dead at the site's scope boundary carries nothing past it.
fn site_meaning(
    chunk: &Chunk,
    scopes: &BTreeMap<usize, Scope>,
    site: usize,
    event: Event,
) -> Option<(Confinement, Reason)> {
    match event {
        Event::Yielded { ip } => Some((Confinement::Escapes, Reason::Yielded { ip })),
        Event::Returned { ip } => Some((Confinement::Escapes, Reason::Returned { ip })),
        Event::HandedToNative { ip } => Some((Confinement::Escapes, Reason::HandedToNative { ip })),
        Event::PassedToCall { ip } => {
            Some((Confinement::CannotEstablish, Reason::PassedToCall { ip }))
        }
        Event::UnmodelledRoute { ip } => {
            Some((Confinement::CannotEstablish, Reason::UnmodelledRoute { ip }))
        }
        Event::StoredToLocal { ip, slot } => {
            let scope = scopes.get(&site).copied().unwrap_or(Scope::Invocation);
            let (start, end) = scope_range(chunk, scope);
            if boundary_dead(chunk, start, end, slot) {
                None
            } else {
                Some((
                    Confinement::CannotEstablish,
                    Reason::StoredToLiveSlot { ip, slot },
                ))
            }
        }
    }
}

/// The construction sites and the scope each is judged against./// The construction sites and the scope each is judged against.
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

/// What one chunk does with each of its parameters.
///
/// Two facts per parameter, and both are needed. A summary carrying only
/// `leaks` would force every caller to treat every return value as aliasing
/// every argument, which is what it already does without a summary at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSummary {
    /// Per parameter: may this parameter's region become reachable after the
    /// call returns, other than through the return value?
    leaks: Vec<bool>,
    /// Per parameter: may the return value alias this parameter's region?
    returns: Vec<bool>,
}

impl ChunkSummary {
    /// The summary that assumes the worst of every parameter.
    fn opaque(param_count: usize) -> Self {
        ChunkSummary {
            leaks: vec![true; param_count],
            returns: vec![true; param_count],
        }
    }
}

/// Per-chunk summaries for one module, indexed by chunk.
///
/// **An absent or short entry answers conservatively.** A missing summary that
/// read as a clean one would turn a sound analysis unsound in the way hardest
/// to notice, because the verdict would IMPROVE. Every accessor therefore
/// defaults to `true`, and [`Summaries::default`] — no summaries at all — is
/// exactly the behaviour of the analysis before summaries existed.
#[derive(Debug, Clone, Default)]
pub struct Summaries {
    per_chunk: Vec<Option<ChunkSummary>>,
}

impl Summaries {
    /// May argument `arg` of a call to `callee` leak? Unknown means yes.
    fn leaks(&self, callee: usize, arg: usize) -> bool {
        match self.per_chunk.get(callee).and_then(|s| s.as_ref()) {
            Some(s) => s.leaks.get(arg).copied().unwrap_or(true),
            None => true,
        }
    }

    /// May the return value of a call to `callee` alias argument `arg`?
    /// Unknown means yes.
    fn returns(&self, callee: usize, arg: usize) -> bool {
        match self.per_chunk.get(callee).and_then(|s| s.as_ref()) {
            Some(s) => s.returns.get(arg).copied().unwrap_or(true),
            None => true,
        }
    }
}

/// What a tracked region originates from.
///
/// **Sites and parameters must not share a token space.** They are answered by
/// different rules — a site is judged against a scope with a liveness test, a
/// parameter is judged against the callee's whole invocation and its slot is
/// written by the CALLER during frame setup, so the liveness test would report
/// every parameter as live across its boundary. Making the distinction a type
/// rather than a numbering convention is what keeps the two rules from meeting
/// by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Token {
    /// A composite built at this address in the chunk under analysis.
    Site(usize),
    /// The chunk's parameter at this position, for a summary run.
    Param(u8),
}

/// The set of tokens a stack entry or local slot may alias. Empty means the
/// value holds no tracked region — which covers scalars and composites built
/// outside the chunk.
type Alias = BTreeSet<Token>;

#[derive(Clone, PartialEq, Eq)]
struct State {
    stack: Vec<Alias>,
    locals: Vec<Alias>,
}

/// Walk `chunk` from the top with `seed` locals, reporting every event a
/// tracked region reaches. Returns `false` if the walk could not be completed.
fn interpret(
    chunk: &Chunk,
    seed: Vec<Alias>,
    summaries: &Summaries,
    note: &mut impl FnMut(Token, Event),
) -> bool {
    let state = State {
        stack: Vec::new(),
        locals: seed,
    };
    let mut breaks = Vec::new();
    walk(
        chunk,
        0,
        chunk.ops.len(),
        state,
        &mut breaks,
        summaries,
        note,
    )
    .is_ok()
}

/// Locals seeded with nothing tracked, for a construction-site run.
fn empty_locals(chunk: &Chunk) -> Vec<Alias> {
    vec![Alias::new(); chunk.local_count as usize]
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
    summaries: &Summaries,
    note: &mut impl FnMut(Token, Event),
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
                for &t in &v {
                    note(t, Event::Returned { ip });
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
                apply(chunk, ip, op, &mut state, summaries, note)?;
                breaks.push(state.clone());
                ip += 1;
            }
            Op::If(target) => {
                apply(chunk, ip, op, &mut state, summaries, note)?;
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
                        summaries,
                        note,
                    )?;
                    let else_end = walk(chunk, target, endif, state, breaks, summaries, note)?;
                    match join(then_end, else_end) {
                        Some(joined) => state = joined,
                        None => return Ok(None),
                    }
                    ip = endif + 1;
                } else {
                    let skip = state.clone();
                    let then_end = walk(chunk, ip + 1, target, state, breaks, summaries, note)?;
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
                let cap = (head.locals.len() + 1) * (chunk.ops.len() + 1) + 2;
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
                        summaries,
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
                apply(chunk, ip, op, &mut state, summaries, note)?;
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
    summaries: &Summaries,
    note: &mut impl FnMut(Token, Event),
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
    // `taken` is in stack order, so `taken[i]` is argument `i` of a call.
    let taken: Vec<Alias> = state.stack.split_off(state.stack.len() - pops);
    let union: Alias = taken.iter().flatten().copied().collect();

    match op {
        Op::Yield => {
            for &t in &union {
                note(t, Event::Yielded { ip });
            }
        }
        Op::CallExternalNative(_, _) | Op::CallVerifiedNative(_, _) => {
            for &t in &union {
                note(t, Event::HandedToNative { ip });
            }
        }
        Op::Call(callee, _) => {
            // Only the arguments the summary cannot clear are reported. With
            // no summary every accessor answers `true`, so this is exactly the
            // pre-summary behaviour.
            for (arg, aliases) in taken.iter().enumerate() {
                if !summaries.leaks(*callee as usize, arg) {
                    continue;
                }
                for &t in aliases {
                    note(t, Event::PassedToCall { ip });
                }
            }
        }
        Op::SetLocal(slot) => {
            for &t in &union {
                note(t, Event::StoredToLocal { ip, slot: *slot });
            }
        }
        // `Return` is the remaining escaping route and is handled by the walk,
        // which peeks rather than pops.
        Op::Return => {}
        other => {
            // BACKSTOP. Every route classified `Escapes` needs a handler
            // above; one without a handler would leave its tokens reported
            // confined on the strength of a flow nothing followed.
            //
            // A new opcode is a compile error in `route_of`, which forces a
            // decision about its route — but not about this match, whose
            // catch-all arm would silently accept it. So the catch-all asks
            // the classification, and an escaping route with no handler
            // degrades to "cannot establish" rather than to silence.
            if route_of(other) == Route::Escapes {
                for &t in &union {
                    note(t, Event::UnmodelledRoute { ip });
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
            // A fresh region. Its operands were copied into it, so it aliases
            // none of them.
            Op::NewComposite(_) => {
                let mut s = Alias::new();
                s.insert(Token::Site(ip));
                s
            }
            // Copies bytes out; nothing is produced that aliases anything.
            Op::SetData(_) | Op::SetDataIndexed(_, _) => Alias::new(),
            // Reads a slot.
            Op::GetLocal(slot) => state.locals.get(*slot as usize).cloned().ok_or(Derailed)?,
            // The resumed value comes from the host, not from a token here.
            Op::Yield => Alias::new(),
            // The return value aliases only the arguments the summary says it
            // may. Without a summary that is all of them, as before.
            Op::Call(callee, _) => taken
                .iter()
                .enumerate()
                .filter(|(arg, _)| summaries.returns(*callee as usize, *arg))
                .flat_map(|(_, a)| a.iter().copied())
                .collect(),
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
    // untracked and the token could be reported confined on the strength of a
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
    use crate::bytecode::{BlockType, ConstValue, Module};
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

    /// A chunk with `params` parameters, taking `ops`.
    fn chunk_with_params(ops: Vec<Op>, params: u8) -> Chunk {
        let mut c = chunk(ops);
        c.param_count = params;
        c
    }

    fn module_of(chunks: Vec<Chunk>) -> Module {
        Module {
            chunks,
            signatures: Vec::new(),
            native_return_shapes: Vec::new(),
            native_names: Vec::new(),
            entry_point: None,
            data_layout: None,
            word_bits_log2: 6,
            addr_bits_log2: 6,
            float_bits_log2: 6,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            aux_arena_bytes: 0,
            persistent_composite_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            schema_hash: 0,
            enum_layouts: Vec::new(),
        }
    }

    /// A caller that builds a composite and passes it to chunk 1.
    fn caller() -> Chunk {
        chunk(vec![
            Op::Const(0),
            Op::Const(1),
            new_struct(),
            Op::Call(1, 1),
            Op::PopN(1),
            Op::Const(0),
            Op::Return,
        ])
    }

    /// The site in chunk 0, under `module_confinement`.
    fn caller_site(module: &Module) -> SiteVerdict {
        let all = module_confinement(module);
        let mut sites: Vec<SiteVerdict> = all[0].clone();
        assert_eq!(sites.len(), 1, "the caller builds exactly one composite");
        sites.pop().unwrap()
    }

    /// A callee that only reads a scalar field of its argument admits the
    /// caller's site. **This is the whole point of the summary**: without one,
    /// the call alone disqualifies every composite argument.
    #[test]
    fn a_callee_that_cannot_leak_its_argument_admits_the_callers_site() {
        let callee = chunk_with_params(
            vec![
                Op::GetLocal(0),
                Op::GetField(StructField::Flat {
                    offset: 0,
                    kind: crate::value_layout::ScalarKind::Int,
                }),
                Op::Return,
            ],
            1,
        );
        let m = module_of(vec![caller(), callee]);
        let v = caller_site(&m);
        assert_eq!(
            (v.verdict, v.reason),
            (Confinement::Confined, Reason::None),
            "the callee reads a scalar field and returns it; nothing carries \
             the argument's region anywhere"
        );
    }

    /// FALSIFIABILITY, and item 3 of the completion condition: a callee that
    /// DOES release the argument still refuses.
    ///
    /// This differs from the test above by replacing the projection with a
    /// `Yield`, which compiles and which flips the verdict. Without it, the
    /// test above would pass equally if summaries always answered "clean".
    #[test]
    fn a_callee_that_yields_its_argument_still_refuses_the_callers_site() {
        let callee = chunk_with_params(vec![Op::GetLocal(0), Op::Yield, Op::Return], 1);
        let m = module_of(vec![caller(), callee]);
        let v = caller_site(&m);
        assert_eq!(
            v.verdict,
            Confinement::CannotEstablish,
            "the callee hands the argument to the host, so the caller's site \
             must not be admitted"
        );
        assert!(
            matches!(v.reason, Reason::PassedToCall { .. }),
            "and the refusal must name the call: {:?}",
            v.reason
        );
    }

    /// A callee that RETURNS its argument makes the return value alias it, so
    /// what the caller then does with the return matters.
    ///
    /// Here the caller drops it, so the site is still confined — the point is
    /// that `returns` is tracked separately from `leaks` and does not by itself
    /// disqualify.
    #[test]
    fn returning_an_argument_is_recorded_separately_from_leaking_it() {
        let callee = chunk_with_params(vec![Op::GetLocal(0), Op::Return], 1);
        let m = module_of(vec![caller(), callee]);
        let s = module_summaries(&m);
        assert!(
            !s.leaks(1, 0),
            "returning to the caller is not a leak: the caller sees the value"
        );
        assert!(s.returns(1, 0), "but the return value does alias it");
        // The caller drops the result, so nothing escapes.
        assert_eq!(caller_site(&m).verdict, Confinement::Confined);
    }

    /// **TERMINATION DOES NOT REST ON THE LANGUAGE'S ACYCLICITY GUARANTEE.**
    ///
    /// The call graph cannot contain a cycle in a well-formed module, and this
    /// does not rely on that. A self-calling chunk never becomes ready, so it
    /// keeps the conservative no-summary answer instead of recursing without
    /// bound. If this test hangs rather than fails, the readiness rule stopped
    /// excluding cycles.
    #[test]
    fn a_cyclic_call_graph_stays_conservative_rather_than_recursing() {
        // Chunk 1 calls itself.
        let cyclic = chunk_with_params(vec![Op::GetLocal(0), Op::Call(1, 1), Op::Return], 1);
        let m = module_of(vec![caller(), cyclic]);
        let s = module_summaries(&m);
        assert!(
            s.leaks(1, 0) && s.returns(1, 0),
            "a chunk in a cycle must keep the conservative answer"
        );
        assert_eq!(
            caller_site(&m).verdict,
            Confinement::CannotEstablish,
            "and its caller must not be admitted on the strength of a summary \
             that was never computed"
        );
    }

    /// A callee outside the module answers conservatively rather than cleanly.
    ///
    /// Item 4 of the completion condition. An out-of-range chunk index is
    /// malformed, and the failure that matters is the one where a MISSING
    /// summary reads as a clean one — the verdict would improve, which is the
    /// hardest direction to notice.
    #[test]
    fn an_out_of_range_callee_is_conservative() {
        // The caller alone; chunk 1 does not exist.
        let m = module_of(vec![caller()]);
        let s = module_summaries(&m);
        assert!(s.leaks(1, 0), "an absent callee must be assumed to leak");
        assert!(s.leaks(99, 7), "and so must an absurd index");
        assert_eq!(
            caller_site(&m).verdict,
            Confinement::CannotEstablish,
            "a call to a chunk that is not there must not admit the site"
        );
    }

    /// Summaries are an ADDITION: analysing a chunk without them answers
    /// exactly as before.
    #[test]
    fn the_summary_free_answer_is_unchanged() {
        let callee = chunk_with_params(
            vec![
                Op::GetLocal(0),
                Op::GetField(StructField::Flat {
                    offset: 0,
                    kind: crate::value_layout::ScalarKind::Int,
                }),
                Op::Return,
            ],
            1,
        );
        let m = module_of(vec![caller(), callee]);
        let without = chunk_confinement(&m.chunks[0]);
        assert_eq!(
            without[0].verdict,
            Confinement::CannotEstablish,
            "with no summary every call is assumed to leak, as before"
        );
        assert_eq!(
            chunk_confinement_with(&m.chunks[0], &Summaries::default()),
            without,
            "and an empty summary table is identical to no table at all"
        );
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
