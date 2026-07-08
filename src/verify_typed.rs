//! Typed operand-stack verifier pass (A.2.1) — the abstract interpreter.
//!
//! This is a standalone pass, not yet wired into [`crate::verify`] (that is
//! Phase 6, gated on the conformance corpus). It walks a chunk's
//! block-structured control flow over an *abstract state* — an operand stack
//! and a local frame — whose slots carry the flat shape (composite kind and
//! byte size) of each value where known. The pass:
//!
//! - subsumes the scalar operand-depth pass (`verify::verify_stack_depth`):
//!   the abstract stack height is the depth, and a pop from an empty stack
//!   is a MUST-REJECT underflow;
//! - upgrades the depth pass's `max`-of-branch-depths to an **exact height
//!   join** at an `If`/`Else` merge, closing the branch-imbalance hole
//!   (findings B4/B5);
//! - requires **loop back-edge neutrality** — a loop body's fall-through
//!   must restore the exact entry stack (height and per-slot shape), so
//!   per-iteration stack growth cannot escape the worst-case bound
//!   (finding 3/B4);
//! - validates every compiler-baked flat-field offset and array stride
//!   against the byte size of the composite it is applied to (findings
//!   B1/B2), including across the local frame and a script-to-script call;
//! - validates the wire-carried layout tables — shared-slot offsets against
//!   the shared-data buffer (finding B6) and a flat-enum construction size
//!   against the declared enum body sizes (finding B8).
//!
//! Seeding comes from the module's per-chunk signature table (parameters,
//! return, resume) and the local frame is then tracked precisely. Where a
//! value's shape is still not known — an unseeded boundary such as a native
//! result or a Reentrant resume, a loop-written local, or a composite
//! constant — the slot is [`AbsVal::Top`] and shape-dependent checks defer
//! (they will become MUST-REJECT once seeding is complete and the pass is
//! wired in). Nothing here changes runtime behaviour.

use crate::bytecode::{
    ArrayElem, Chunk, ChunkSignature, ConstValue, EnumField, Module, NewCompositeOperand, Op,
    StructField, TupleField, WireShape,
};
use crate::value_layout::{CompositeKind, ScalarKind};
use crate::verify::op_depth_effect;
use alloc::vec::Vec;

/// An abstract value on the operand stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbsVal {
    /// A fixed-size scalar of the given kind.
    Scalar(ScalarKind),
    /// A flat composite body of a statically known size.
    Flat {
        /// The composite variant the body re-wraps as.
        kind: CompositeKind,
        /// The body's byte length.
        size: u32,
    },
    /// A value whose shape is not statically known at this phase (a
    /// parameter, local, call result, resume value, reference handle, or
    /// composite constant). The lattice top; joins with anything to `Top`.
    Top,
}

impl AbsVal {
    /// Byte size this value occupies inside a packed flat body, if known.
    fn packed_size(&self, word_bytes: usize, float_bytes: usize) -> Option<u32> {
        match self {
            AbsVal::Scalar(k) => Some(k.size_in_bytes(word_bytes, float_bytes) as u32),
            AbsVal::Flat { size, .. } => Some(*size),
            AbsVal::Top => None,
        }
    }

    /// Lattice join: equal values join to themselves, anything else to `Top`.
    fn join(&self, other: &AbsVal) -> AbsVal {
        if self == other {
            self.clone()
        } else {
            AbsVal::Top
        }
    }

    /// Lift a wire signature shape into an abstract value. An unknown tag
    /// (a wire from a newer producer, or a `Float` tag on a build without
    /// the `floats` feature) degrades to `Top`, which defers rather than
    /// falsely rejecting.
    fn from_wire(shape: &WireShape) -> AbsVal {
        match shape {
            WireShape::Top => AbsVal::Top,
            WireShape::Scalar { kind } => match ScalarKind::from_tag(*kind) {
                Some(k) => AbsVal::Scalar(k),
                None => AbsVal::Top,
            },
            WireShape::Flat { kind, size } => match CompositeKind::from_tag(*kind) {
                Some(k) => AbsVal::Flat {
                    kind: k,
                    size: *size,
                },
                None => AbsVal::Top,
            },
        }
    }
}

/// A decidable MUST-REJECT reason. Every variant is a load-time rejection,
/// never a runtime fault (meta-spec C3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedError {
    /// An op required more operands than the abstract stack held.
    StackUnderflow {
        /// Instruction index where the underflow was detected.
        ip: usize,
    },
    /// A flat field access reads past the end of the composite body.
    OffsetOutOfBounds {
        /// Instruction index of the access.
        ip: usize,
        /// The compiler-baked byte offset.
        offset: usize,
        /// Byte extent the access requires (offset plus read size).
        need: usize,
        /// Byte size of the composite body being accessed.
        size: u32,
    },
    /// A field access was applied to a value known to be a scalar.
    ExpectedComposite {
        /// Instruction index of the access.
        ip: usize,
    },
    /// A `NewComposite` baked byte size disagrees with the packed size of
    /// the (fully known) values it consumes.
    NewCompositeSizeMismatch {
        /// Instruction index of the construction.
        ip: usize,
        /// The baked byte size carried by the operand.
        baked: u32,
        /// The size recomputed from the consumed values.
        computed: u32,
    },
    /// The two arms of an `If`/`Else` leave the operand stack at different
    /// heights (a stack imbalance).
    BranchHeightMismatch {
        /// Instruction index of the `If`.
        ip: usize,
        /// Operand-stack height where the then-arm falls through.
        then_height: usize,
        /// Operand-stack height where the else-arm falls through.
        else_height: usize,
    },
    /// A loop body's fall-through does not restore the operand stack to its
    /// entry state (the back-edge is not stack-neutral), either in height or
    /// in a per-slot shape.
    LoopNotNeutral {
        /// Instruction index of the `Loop`.
        ip: usize,
        /// Operand-stack height at loop entry.
        entry_height: usize,
        /// Operand-stack height where the body falls through.
        exit_height: usize,
    },
    /// A `SetLocal` stores a value whose shape is incompatible with the
    /// slot's declared shape, which would make a later `GetLocal` seeding
    /// untrustworthy.
    LocalTypeMismatch {
        /// Instruction index of the `SetLocal`.
        ip: usize,
        /// The local slot being written.
        slot: usize,
    },
    /// A `Call` passes an argument whose shape is incompatible with the
    /// callee chunk's declared parameter shape (A.2.1 Phase 2b).
    CallArgMismatch {
        /// Instruction index of the `Call`.
        ip: usize,
        /// Callee chunk index.
        callee: usize,
        /// Zero-based argument position.
        arg: usize,
    },
    /// A flat array element access carries an element stride that does not
    /// evenly divide the array body, so the baked stride disagrees with the
    /// body size (A.2.1 Phase 3). The index bound remains a runtime trap; this
    /// validates only the element stride.
    ArrayStrideMismatch {
        /// Instruction index of the `GetIndex`.
        ip: usize,
        /// The baked per-element byte stride.
        element_size: u32,
        /// The array body's byte size.
        array_size: u32,
    },
    /// A shared data slot's byte extent (`offset + size`) reaches past the
    /// module's shared-data buffer (A.2.1 Phase 4, audit finding B6). The
    /// runtime reads and writes the slot at this offset, so an offset past the
    /// buffer is a memory-safety surface.
    SharedSlotOutOfBounds {
        /// Zero-based shared-slot index.
        slot: usize,
        /// The carried byte offset.
        offset: usize,
        /// The slot's byte size (scalar width or composite body length).
        size: usize,
        /// The shared-data buffer's byte length.
        buffer: u32,
    },
    /// A `NewComposite` that constructs a flat enum body carries a byte size
    /// that matches no declared enum body size (`word_bytes + min_payload`)
    /// in the module's enum-layout table (A.2.1 Phase 4, audit finding B8). A
    /// mutated `min_payload` no longer matches the baked construction sizes.
    EnumBodySizeMismatch {
        /// Instruction index of the `NewComposite`.
        ip: usize,
        /// The baked flat enum body size.
        baked: u32,
    },
}

/// Per-chunk signature that seeds the abstract stack where the op stream
/// alone cannot determine a value's shape. Phase 2 consumes it; the
/// compiler emitting it and the wire format carrying it (additively in the
/// auxiliary body) is the Phase 2b plumbing. Absent, every seed point is
/// [`AbsVal::Top`] and shape checks defer, which is exactly the Phase 1
/// behaviour.
#[derive(Clone, Debug, Default)]
pub struct ChunkSig {
    /// Abstract shape of each local slot, parameters first. A slot beyond
    /// the vector is treated as `Top`.
    pub locals: Vec<AbsVal>,
    /// Abstract shape a `Yield` resumes with (Stream and Reentrant chunks).
    pub resume: Option<AbsVal>,
}

impl ChunkSig {
    /// Build a seeding signature from a wire [`ChunkSignature`] (A.2.1 Phase
    /// 2b). The parameters seed the leading local slots (parameters occupy
    /// slots `0..param_count`); later locals are unseeded (`Top`) and refined
    /// by the pass. A `Top` resume records `None`, reproducing the unseeded
    /// resume behaviour.
    fn from_signature(sig: &ChunkSignature) -> ChunkSig {
        let locals = sig.params.iter().map(AbsVal::from_wire).collect();
        let resume = match &sig.resume {
            WireShape::Top => None,
            other => Some(AbsVal::from_wire(other)),
        };
        ChunkSig { locals, resume }
    }

    /// The shape of local slot `i`, or `Top` when unseeded.
    fn local(&self, i: usize) -> AbsVal {
        self.locals.get(i).cloned().unwrap_or(AbsVal::Top)
    }

    /// The shape a resume pushes, or `Top` when unseeded.
    fn resume_shape(&self) -> AbsVal {
        self.resume.clone().unwrap_or(AbsVal::Top)
    }
}

/// Check one chunk with no seeding (parameters, locals, and resume values
/// are `Top`). Equivalent to Phase 1.
pub fn typed_check_chunk(
    chunk: &Chunk,
    word_bytes: usize,
    float_bytes: usize,
) -> Result<(), TypedError> {
    typed_check_chunk_with_sig(chunk, &ChunkSig::default(), word_bytes, float_bytes)
}

/// Check one chunk, seeding local and resume shapes from `sig`. `Ok(())`
/// when the abstract interpretation completes with no MUST-REJECT, else the
/// first reason found.
pub fn typed_check_chunk_with_sig(
    chunk: &Chunk,
    sig: &ChunkSig,
    word_bytes: usize,
    float_bytes: usize,
) -> Result<(), TypedError> {
    check_chunk_seeded(chunk, sig, &[], &[], word_bytes, float_bytes)
}

/// Check every chunk of a module, seeding each from the module's per-chunk
/// signature table and resolving `Call`s against it (A.2.1 Phase 2b). A chunk
/// without a table entry (a shorter or empty `signatures` table) is checked
/// with an all-`Top` signature, reproducing the unseeded behaviour. `Ok(())`
/// when every chunk verifies, else the first reason found.
pub fn typed_check_module(
    module: &Module,
    word_bytes: usize,
    float_bytes: usize,
) -> Result<(), TypedError> {
    // Wire-table validation (A.2.1 Phase 4): the carried tables are trusted
    // only because verified. B6: every shared slot lies within the shared-data
    // buffer. B8 is enforced per-op below via the enum body-size cross-check.
    validate_data_layout(module, word_bytes, float_bytes)?;

    // The set of declared flat enum body sizes (`word_bytes + min_payload`),
    // against which a flat-enum `NewComposite` size is cross-checked (B8). A
    // mutated `min_payload` shifts this set away from the baked construction
    // sizes. Empty when the module declares no enums, in which case the check
    // defers.
    let enum_body_sizes: Vec<u32> = module
        .enum_layouts
        .iter()
        .map(|el| (word_bytes as u32).saturating_add(el.min_payload))
        .collect();

    let default_sig = ChunkSig::default();
    for (i, chunk) in module.chunks.iter().enumerate() {
        let seeded = module.signatures.get(i).map(ChunkSig::from_signature);
        let sig = seeded.as_ref().unwrap_or(&default_sig);
        check_chunk_seeded(
            chunk,
            sig,
            &module.signatures,
            &enum_body_sizes,
            word_bytes,
            float_bytes,
        )?;
    }
    Ok(())
}

/// Validate the shared-data slot layout (A.2.1 Phase 4, audit finding B6):
/// every shared slot's byte extent must lie within the module's shared-data
/// buffer. The runtime addresses each slot by its carried offset, so an offset
/// or size that overruns the buffer is a memory-safety surface promoted here
/// to a load-time MUST-REJECT.
fn validate_data_layout(
    module: &Module,
    word_bytes: usize,
    float_bytes: usize,
) -> Result<(), TypedError> {
    let Some(layout) = &module.data_layout else {
        return Ok(());
    };
    let buffer = module.shared_data_bytes;
    for (slot, sl) in layout.shared_layout.iter().enumerate() {
        // A composite slot's size is its carried body length; a scalar slot's
        // size follows from its kind tag at the module widths.
        let size = if sl.kind & crate::bytecode::SHARED_SLOT_COMPOSITE_FLAG != 0 {
            usize::from(sl.len)
        } else {
            match ScalarKind::from_tag(sl.kind) {
                Some(k) => k.size_in_bytes(word_bytes, float_bytes),
                // An undecodable kind tag cannot be sized; treat as a corrupt
                // table entry.
                None => {
                    return Err(TypedError::SharedSlotOutOfBounds {
                        slot,
                        offset: usize::from(sl.offset),
                        size: 0,
                        buffer,
                    });
                }
            }
        };
        let need = usize::from(sl.offset) + size;
        if need > buffer as usize {
            return Err(TypedError::SharedSlotOutOfBounds {
                slot,
                offset: usize::from(sl.offset),
                size,
                buffer,
            });
        }
    }
    Ok(())
}

/// Shared entry: interpret one chunk under a seeding signature, a module
/// signature table, and the set of declared flat enum body sizes (all empty
/// when the chunk is checked in isolation).
fn check_chunk_seeded(
    chunk: &Chunk,
    sig: &ChunkSig,
    module_sigs: &[ChunkSignature],
    enum_body_sizes: &[u32],
    word_bytes: usize,
    float_bytes: usize,
) -> Result<(), TypedError> {
    let ctx = Ctx {
        chunk,
        sig,
        module_sigs,
        enum_body_sizes,
        wb: word_bytes,
        fb: float_bytes,
    };
    // The local frame starts from the seed (parameters carry their declared
    // shape; every other slot is `Top`) and is tracked precisely thereafter
    // (A.2.1 Phase 2 residual — the "reconstruct locals from SetLocal
    // producers" option). A `GetLocal` reads the tracked shape, so a flat
    // access on a local-held composite is bounds-checked, not deferred.
    let locals: Vec<AbsVal> = (0..chunk.local_count as usize)
        .map(|i| sig.local(i))
        .collect();
    let state = AbsState {
        stack: Vec::new(),
        locals,
    };
    let mut breaks: Vec<AbsState> = Vec::new();
    interp_region(&ctx, 0, chunk.ops.len(), state, &mut breaks).map(|_| ())
}

/// Abstract state at a program point: the operand stack and the per-slot
/// shapes of the local frame. At a control-flow merge the stack must match in
/// height (a mismatch is a MUST-REJECT imbalance) and joins per slot; the
/// locals join per slot to `Top` on disagreement. Local slots are tracked so
/// a flat access on a local-held composite is validated rather than deferred;
/// soundness in a loop comes from invalidating (to `Top`) every slot the loop
/// body writes before the body is interpreted, since a prior iteration may
/// have overwritten it.
#[derive(Clone)]
struct AbsState {
    /// The abstract operand stack (top last).
    stack: Vec<AbsVal>,
    /// Per-slot abstract shapes of the local frame, indexed by slot.
    locals: Vec<AbsVal>,
}

/// Interpret ops `[start, end)` from `stack`. Returns `Ok(Some(exit_stack))`
/// when the region falls through and `Ok(None)` when every path exits via
/// `Break`, `Trap`, or `Return`. `breaks` collects the abstract stack at each
/// break edge that leaves the enclosing loop. Mirrors
/// `verify::verify_depth_region`'s control-flow shape, but over an abstract
/// stack rather than a scalar depth.
/// Immutable context threaded through the abstract interpreter: the chunk
/// under check, its seeding signature, and the module's target widths.
/// Folding these into one struct keeps the recursive interpreter's signature
/// small as Phase 2b adds the module-level signature table for cross-`Call`
/// seeding here.
struct Ctx<'a> {
    /// The chunk whose op stream is being interpreted.
    chunk: &'a Chunk,
    /// Seeds for the chunk's local slots and resume value.
    sig: &'a ChunkSig,
    /// The module's per-chunk signature table, indexed by callee chunk index,
    /// used to seed a `Call`'s result and check its arguments. Empty when the
    /// chunk is checked in isolation, in which case a `Call` result degrades
    /// to `Top` and arguments are not checked (A.2.1 Phase 2b).
    module_sigs: &'a [ChunkSignature],
    /// The module's declared flat enum body sizes (`word_bytes + min_payload`),
    /// against which a flat-enum `NewComposite` size is cross-checked (A.2.1
    /// Phase 4, audit finding B8). Empty when the chunk is checked in isolation
    /// or the module declares no enums, in which case the check defers.
    enum_body_sizes: &'a [u32],
    /// Target word width in bytes (for scalar/composite sizing).
    wb: usize,
    /// Target float width in bytes.
    fb: usize,
}

/// A recursive abstract interpreter over ops `[start, end)`. The immutable
/// per-chunk context is carried in `ctx`; the working state (operand stack and
/// local frame) and the break collector vary per call. Returns `Ok(Some(_))`
/// with the fall-through state, or `Ok(None)` when every path exits via
/// `Break`, `Trap`, or `Return`.
fn interp_region(
    ctx: &Ctx,
    start: usize,
    end: usize,
    mut state: AbsState,
    breaks: &mut Vec<AbsState>,
) -> Result<Option<AbsState>, TypedError> {
    let ops = &ctx.chunk.ops;
    let mut ip = start;
    while ip < end {
        let op = &ops[ip];
        match op {
            Op::Trap(_) | Op::Return => return Ok(None),
            Op::Break(_) => {
                breaks.push(state);
                return Ok(None);
            }
            Op::BreakIf(_) => {
                let AbsState { stack, locals } = &mut state;
                apply_op(ctx, op, stack, locals, ip)?;
                breaks.push(state.clone());
                ip += 1;
            }
            Op::If(target) => {
                {
                    let AbsState { stack, locals } = &mut state;
                    apply_op(ctx, op, stack, locals, ip)?;
                }
                let target = *target as usize;
                if target > 0 && matches!(&ops[target - 1], Op::Else(_)) {
                    let endif = match &ops[target - 1] {
                        Op::Else(e) => *e as usize,
                        _ => unreachable!(),
                    };
                    let then_end = interp_region(ctx, ip + 1, target - 1, state.clone(), breaks)?;
                    let else_end = interp_region(ctx, target, endif, state, breaks)?;
                    match join_ends(then_end, else_end, ip)? {
                        Some(joined) => state = joined,
                        None => return Ok(None),
                    }
                    ip = endif + 1;
                } else {
                    // No else: the then-arm merges with the fall-through path
                    // (the state unchanged after popping the condition).
                    let skip = state.clone();
                    let then_end = interp_region(ctx, ip + 1, target, state, breaks)?;
                    match join_ends(then_end, Some(skip), ip)? {
                        Some(joined) => state = joined,
                        None => return Ok(None),
                    }
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                let exit = *target as usize;
                let entry_height = state.stack.len();
                // Loop-head local shapes are a bounded ascending fixpoint: start
                // from the entry locals and widen by joining in the back-edge
                // (body fall-through) locals until stable. A slot with the same
                // shape on every iteration is proven; one that differs across
                // iterations widens to `Top` and defers. Starting from the
                // concrete entry locals (rather than invalidating every
                // body-written slot to `Top` up front) also validates the first
                // iteration precisely, which reads each slot's pre-loop value.
                // The lattice is finite and the join only moves toward `Top`, so
                // this converges (a slot changes at most once, concrete to
                // `Top`); a defensive cap forces convergence by invalidating
                // every body-written slot to `Top` — the sound over-
                // approximation — and doing one final pass. The operand stack is
                // not widened: back-edge neutrality (below) fixes its shapes
                // across iterations.
                let mut head = state.clone();
                let cap = head.locals.len() + 2;
                let mut loop_breaks: Vec<AbsState> = Vec::new();
                let mut body_end;
                let mut iters = 0usize;
                loop {
                    loop_breaks.clear();
                    body_end =
                        interp_region(ctx, ip + 1, exit - 1, head.clone(), &mut loop_breaks)?;
                    let Some(be) = &body_end else {
                        // The body always exits via Break/Trap/Return: no
                        // back-edge, so no fixpoint is needed.
                        break;
                    };
                    // Back-edge neutrality: the fall-through must restore the
                    // exact entry operand stack (height and per-slot shape), so
                    // per-iteration stack growth cannot escape the worst-case
                    // bound. Locals may change across iterations.
                    if be.stack != head.stack {
                        return Err(TypedError::LoopNotNeutral {
                            ip,
                            entry_height,
                            exit_height: be.stack.len(),
                        });
                    }
                    let widened = join_locals(&head.locals, &be.locals);
                    if widened == head.locals {
                        break; // stable
                    }
                    if iters >= cap {
                        invalidate_written_locals(ctx.chunk, ip + 1, exit - 1, &mut head.locals);
                        loop_breaks.clear();
                        body_end =
                            interp_region(ctx, ip + 1, exit - 1, head.clone(), &mut loop_breaks)?;
                        break;
                    }
                    head.locals = widened;
                    iters += 1;
                }
                // The code after the loop is reached via the break edges; if
                // there are none, fall back to the (neutral) body exit or the
                // fixpoint head, mirroring the depth pass.
                state = match join_all(loop_breaks, ip)? {
                    Some(s) => s,
                    None => body_end.unwrap_or(head),
                };
                ip = exit;
            }
            _ => {
                let AbsState { stack, locals } = &mut state;
                apply_op(ctx, op, stack, locals, ip)?;
                ip += 1;
            }
        }
    }
    Ok(Some(state))
}

/// Set to `Top` every local slot that any op in `[start, end)` writes via
/// `SetLocal`, so a loop body reads a body-written slot as unknown rather than
/// a stale pre-loop shape.
fn invalidate_written_locals(chunk: &Chunk, start: usize, end: usize, locals: &mut [AbsVal]) {
    for op in &chunk.ops[start..end] {
        if let Op::SetLocal(i) = op
            && let Some(slot) = locals.get_mut(*i as usize)
        {
            *slot = AbsVal::Top;
        }
    }
}

/// Join the operand stacks of two states: equal height required (else a
/// MUST-REJECT imbalance), then per-slot lattice join.
fn join_stacks(a: &[AbsVal], b: &[AbsVal], ip: usize) -> Result<Vec<AbsVal>, TypedError> {
    if a.len() != b.len() {
        return Err(TypedError::BranchHeightMismatch {
            ip,
            then_height: a.len(),
            else_height: b.len(),
        });
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| x.join(y)).collect())
}

/// Join two local frames per slot; a shorter frame is padded with `Top`.
fn join_locals(a: &[AbsVal], b: &[AbsVal]) -> Vec<AbsVal> {
    let n = a.len().max(b.len());
    (0..n)
        .map(|i| {
            let x = a.get(i).unwrap_or(&AbsVal::Top);
            let y = b.get(i).unwrap_or(&AbsVal::Top);
            x.join(y)
        })
        .collect()
}

/// Join two region ends (stack and locals). `Some`/`Some` joins both; a
/// `None` (exiting) arm yields the other; both `None` yields `None`.
fn join_ends(
    then_end: Option<AbsState>,
    else_end: Option<AbsState>,
    ip: usize,
) -> Result<Option<AbsState>, TypedError> {
    match (then_end, else_end) {
        (Some(a), Some(b)) => Ok(Some(AbsState {
            stack: join_stacks(&a.stack, &b.stack, ip)?,
            locals: join_locals(&a.locals, &b.locals),
        })),
        (Some(a), None) => Ok(Some(a)),
        (None, Some(b)) => Ok(Some(b)),
        (None, None) => Ok(None),
    }
}

/// Join a set of break-edge states. Requires equal stack heights; `None` when
/// the set is empty.
fn join_all(states: Vec<AbsState>, ip: usize) -> Result<Option<AbsState>, TypedError> {
    let mut it = states.into_iter();
    let Some(mut acc) = it.next() else {
        return Ok(None);
    };
    for s in it {
        acc = AbsState {
            stack: join_stacks(&acc.stack, &s.stack, ip)?,
            locals: join_locals(&acc.locals, &s.locals),
        };
    }
    Ok(Some(acc))
}

/// Apply one non-control-flow op's effect to the abstract stack. Underflow
/// is decided from `op_depth_effect`'s required-operand count, so the height
/// discipline exactly matches the scalar depth pass. Shape is tracked
/// precisely for the ops that carry or consume it and conservatively (`Top`)
/// otherwise.
fn apply_op(
    ctx: &Ctx,
    op: &Op,
    stack: &mut Vec<AbsVal>,
    locals: &mut [AbsVal],
    ip: usize,
) -> Result<(), TypedError> {
    let (req, net) = op_depth_effect(op, ctx.chunk);
    if (stack.len() as i32) < req {
        return Err(TypedError::StackUnderflow { ip });
    }
    match op {
        // Peek-and-push ops keep their operand; handle explicitly so the
        // generic path does not drop the peeked value's shape.
        Op::Dup => {
            let top = stack.last().cloned().unwrap_or(AbsVal::Top);
            stack.push(top);
        }
        Op::IsEnum(_, _, _) | Op::IsStruct(_) => stack.push(AbsVal::Scalar(ScalarKind::Bool)),

        // Read a local's tracked shape. A slot beyond the frame is `Top`.
        Op::GetLocal(i) => {
            let shape = locals.get(*i as usize).cloned().unwrap_or(AbsVal::Top);
            stack.push(shape);
        }

        // A local write must store a value whose shape is compatible with the
        // slot's declared (seed) shape, so a seeded `GetLocal` read stays
        // trustworthy; an unseeded slot (seed `Top`) accepts any value. The
        // tracked shape is then updated to the stored value, so a later
        // `GetLocal` sees exactly what was written.
        Op::SetLocal(i) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            if !shapes_compatible(&v, &ctx.sig.local(*i as usize)) {
                return Err(TypedError::LocalTypeMismatch {
                    ip,
                    slot: *i as usize,
                });
            }
            if let Some(slot) = locals.get_mut(*i as usize) {
                *slot = v;
            }
        }

        // A yield pops the output value and the resume pushes the input; the
        // resume's shape comes from the signature (Phase 2).
        Op::Yield => {
            stack.pop();
            stack.push(ctx.sig.resume_shape());
        }

        // A constant's shape comes from the pool: scalars are precise;
        // composite, string, and option constants are `Top` until Phase 3.
        Op::Const(idx) => stack.push(const_abs(ctx.chunk.constants.get(*idx as usize))),

        Op::NewComposite(NewCompositeOperand::Flat {
            kind,
            count,
            byte_size,
        }) => {
            let mut computed: u32 = 0;
            let mut all_known = true;
            for _ in 0..*count {
                let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
                match v.packed_size(ctx.wb, ctx.fb) {
                    Some(n) => computed = computed.saturating_add(n),
                    None => all_known = false,
                }
            }
            if matches!(kind, CompositeKind::Enum) {
                // An enum body pads its payload to the largest variant, so its
                // size is not the sum of the packed elements; instead it must
                // match a declared flat enum body size (`word_bytes +
                // min_payload`) from the module's enum-layout table (A.2.1
                // Phase 4, audit finding B8). The check defers when the table
                // is absent (an isolated chunk check or an enum-free module).
                if !ctx.enum_body_sizes.is_empty()
                    && !ctx.enum_body_sizes.contains(&u32::from(*byte_size))
                {
                    return Err(TypedError::EnumBodySizeMismatch {
                        ip,
                        baked: u32::from(*byte_size),
                    });
                }
            } else if all_known && computed != u32::from(*byte_size) {
                // A non-enum flat body must pack exactly when every element
                // size is known.
                return Err(TypedError::NewCompositeSizeMismatch {
                    ip,
                    baked: u32::from(*byte_size),
                    computed,
                });
            }
            stack.push(AbsVal::Flat {
                kind: *kind,
                size: u32::from(*byte_size),
            });
        }

        Op::GetField(StructField::Flat { offset, kind }) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            check_flat_scalar(&v, *offset, *kind, ctx.wb, ctx.fb, ip)?;
            stack.push(AbsVal::Scalar(*kind));
        }
        Op::GetField(StructField::FlatNested {
            offset,
            size,
            variant,
        }) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            check_flat_nested(&v, *offset, *size, ip)?;
            stack.push(AbsVal::Flat {
                kind: *variant,
                size: u32::from(*size),
            });
        }

        // Flat tuple-field access mirrors the struct case (a tuple is an
        // anonymous struct): a scalar field is bounds-checked at its offset, a
        // nested composite field at its offset and size (A.2.1 Phase 3).
        Op::GetTupleField(TupleField::Flat { offset, kind }) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            check_flat_scalar(&v, *offset, *kind, ctx.wb, ctx.fb, ip)?;
            stack.push(AbsVal::Scalar(*kind));
        }
        Op::GetTupleField(TupleField::FlatNested {
            offset,
            size,
            variant,
        }) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            check_flat_nested(&v, *offset, *size, ip)?;
            stack.push(AbsVal::Flat {
                kind: *variant,
                size: u32::from(*size),
            });
        }

        // Flat enum-payload access. The baked offset already includes the
        // leading discriminant word, so the bounds check is the same as a
        // struct field against the padded enum body (A.2.1 Phase 3).
        Op::GetEnumField(EnumField::Flat { offset, kind }) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            check_flat_scalar(&v, *offset, *kind, ctx.wb, ctx.fb, ip)?;
            stack.push(AbsVal::Scalar(*kind));
        }
        Op::GetEnumField(EnumField::FlatNested {
            offset,
            size,
            variant,
        }) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            check_flat_nested(&v, *offset, *size, ip)?;
            stack.push(AbsVal::Flat {
                kind: *variant,
                size: u32::from(*size),
            });
        }

        // Flat array element access pops the index and the array. The element
        // offset is `index * element_size` computed at run time, so the index
        // bound stays a runtime trap; the static check validates that the
        // baked element stride evenly divides the array body (A.2.1 Phase 3),
        // then pushes the element shape.
        Op::GetIndex(ArrayElem::Flat { kind }) => {
            let _index = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            let arr = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            let elem = kind.size_in_bytes(ctx.wb, ctx.fb) as u32;
            check_flat_array_stride(&arr, elem, ip)?;
            stack.push(AbsVal::Scalar(*kind));
        }
        Op::GetIndex(ArrayElem::FlatNested { size, variant }) => {
            let _index = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            let arr = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            check_flat_array_stride(&arr, u32::from(*size), ip)?;
            stack.push(AbsVal::Flat {
                kind: *variant,
                size: u32::from(*size),
            });
        }

        // A script-to-script call: pop the arguments, check each against the
        // callee's declared parameter shape, and push the callee's return
        // shape (A.2.1 Phase 2b). With no module signature table (an isolated
        // chunk check) the result degrades to `Top` and arguments are not
        // checked. The top-level underflow guard already ensured `n` operands
        // are present.
        Op::Call(callee, n) => {
            let argc = *n as usize;
            let mut args: Vec<AbsVal> = Vec::with_capacity(argc);
            for _ in 0..argc {
                args.push(stack.pop().ok_or(TypedError::StackUnderflow { ip })?);
            }
            // `args` is in reverse call order (last argument popped first);
            // reverse to index by declared parameter position.
            args.reverse();
            match ctx.module_sigs.get(*callee as usize) {
                Some(callee_sig) => {
                    for (arg, declared) in callee_sig.params.iter().enumerate() {
                        let expected = AbsVal::from_wire(declared);
                        if let Some(actual) = args.get(arg)
                            && !shapes_compatible(actual, &expected)
                        {
                            return Err(TypedError::CallArgMismatch {
                                ip,
                                callee: *callee as usize,
                                arg,
                            });
                        }
                    }
                    stack.push(AbsVal::from_wire(&callee_sig.ret));
                }
                None => stack.push(AbsVal::Top),
            }
        }

        // Generic height-correct effect for every other op: pop `req`,
        // push `net + req` unknown values.
        _ => {
            for _ in 0..req {
                stack.pop();
            }
            let produced = net + req;
            for _ in 0..produced.max(0) {
                stack.push(AbsVal::Top);
            }
        }
    }
    Ok(())
}

/// Validate a flat scalar field access against a composite of known size.
/// A `Top` operand defers (Phase 2 seeding), a scalar operand is a type
/// error, a `Flat` operand is bounds-checked.
fn check_flat_scalar(
    v: &AbsVal,
    offset: u16,
    kind: ScalarKind,
    wb: usize,
    fb: usize,
    ip: usize,
) -> Result<(), TypedError> {
    match v {
        AbsVal::Flat { size, .. } => {
            let need = usize::from(offset) + kind.size_in_bytes(wb, fb);
            if need > *size as usize {
                return Err(TypedError::OffsetOutOfBounds {
                    ip,
                    offset: usize::from(offset),
                    need,
                    size: *size,
                });
            }
            Ok(())
        }
        AbsVal::Scalar(_) => Err(TypedError::ExpectedComposite { ip }),
        AbsVal::Top => Ok(()),
    }
}

/// Validate a nested flat composite field access against a parent of known
/// size (the `nested_view` debug_assert promoted to a MUST-REJECT).
fn check_flat_nested(v: &AbsVal, offset: u16, size: u16, ip: usize) -> Result<(), TypedError> {
    match v {
        AbsVal::Flat { size: parent, .. } => {
            let need = usize::from(offset) + usize::from(size);
            if need > *parent as usize {
                return Err(TypedError::OffsetOutOfBounds {
                    ip,
                    offset: usize::from(offset),
                    need,
                    size: *parent,
                });
            }
            Ok(())
        }
        AbsVal::Scalar(_) => Err(TypedError::ExpectedComposite { ip }),
        AbsVal::Top => Ok(()),
    }
}

/// Validate a flat array element access: the baked element stride must evenly
/// divide the array body, so a corrupted element kind or size is a
/// MUST-REJECT. A zero-size element (a `Unit` array) reads no bytes and needs
/// no stride check. A `Top` array defers; a scalar is a type error.
fn check_flat_array_stride(arr: &AbsVal, element_size: u32, ip: usize) -> Result<(), TypedError> {
    match arr {
        AbsVal::Flat { size, .. } => {
            if element_size != 0 && *size % element_size != 0 {
                return Err(TypedError::ArrayStrideMismatch {
                    ip,
                    element_size,
                    array_size: *size,
                });
            }
            Ok(())
        }
        AbsVal::Scalar(_) => Err(TypedError::ExpectedComposite { ip }),
        AbsVal::Top => Ok(()),
    }
}

/// Whether a value's shape may be stored into a slot of the declared
/// shape. `Top` on either side defers. Two scalars are compatible
/// regardless of kind (they share the word/byte/float width the layout
/// cares about); two flat composites must match in kind and size; a scalar
/// and a composite never match.
fn shapes_compatible(value: &AbsVal, declared: &AbsVal) -> bool {
    match (value, declared) {
        (AbsVal::Top, _) | (_, AbsVal::Top) => true,
        (AbsVal::Scalar(_), AbsVal::Scalar(_)) => true,
        (AbsVal::Flat { kind: k1, size: s1 }, AbsVal::Flat { kind: k2, size: s2 }) => {
            k1 == k2 && s1 == s2
        }
        _ => false,
    }
}

/// Abstract shape of a constant-pool value.
fn const_abs(cv: Option<&ConstValue>) -> AbsVal {
    match cv {
        Some(ConstValue::Unit) => AbsVal::Scalar(ScalarKind::Unit),
        Some(ConstValue::Bool(_)) => AbsVal::Scalar(ScalarKind::Bool),
        Some(ConstValue::Int(_)) => AbsVal::Scalar(ScalarKind::Int),
        Some(ConstValue::Byte(_)) => AbsVal::Scalar(ScalarKind::Byte),
        Some(ConstValue::Fixed(_)) => AbsVal::Scalar(ScalarKind::Fixed),
        Some(ConstValue::Float(_)) => AbsVal::Scalar(ScalarKind::Float),
        // A static string pushes a fixed-size `Text` handle (A.2.1 Phase 3).
        Some(ConstValue::StaticStr(_)) => AbsVal::Scalar(ScalarKind::Text),
        // Composite constants (tuple, array, struct, enum) carry a flat shape
        // whose byte size needs the layout widths the constant pool does not
        // record; conservatively unknown until a later phase threads them.
        _ => AbsVal::Top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{BlockType, Chunk};
    use alloc::string::String;
    use alloc::vec;

    fn chunk(ops: Vec<Op>, constants: Vec<ConstValue>) -> Chunk {
        Chunk {
            name: String::from("t"),
            ops,
            constants,
            struct_templates: Vec::new(),
            local_count: 8,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: Vec::new(),
            debug_pool: None,
        }
    }

    // Two Int constants packed into a 16-byte flat struct.
    fn two_int_struct(byte_size: u16) -> Vec<Op> {
        vec![
            Op::Const(0),
            Op::Const(1),
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size,
            }),
        ]
    }

    fn ints() -> Vec<ConstValue> {
        vec![ConstValue::Int(0), ConstValue::Int(0)]
    }

    // A minimal module wrapping the given chunks and their parallel signature
    // table, at 64-bit word and float widths.
    fn module(chunks: Vec<Chunk>, signatures: Vec<ChunkSignature>) -> Module {
        Module {
            chunks,
            signatures,
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

    #[test]
    fn valid_flat_field_access_accepts() {
        let mut ops = two_int_struct(16);
        ops.push(Op::GetField(StructField::Flat {
            offset: 8,
            kind: ScalarKind::Int,
        }));
        ops.push(Op::PopN(1));
        assert!(typed_check_chunk(&chunk(ops, ints()), 8, 8).is_ok());
    }

    #[test]
    fn out_of_bounds_flat_field_offset_rejects() {
        let mut ops = two_int_struct(16);
        ops.push(Op::GetField(StructField::Flat {
            offset: 12,
            kind: ScalarKind::Int,
        }));
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    #[test]
    fn nested_flat_field_out_of_bounds_rejects() {
        let mut ops = two_int_struct(16);
        ops.push(Op::GetField(StructField::FlatNested {
            offset: 8,
            size: 12,
            variant: CompositeKind::Struct,
        }));
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    #[test]
    fn newcomposite_size_mismatch_rejects() {
        assert!(matches!(
            typed_check_chunk(&chunk(two_int_struct(24), ints()), 8, 8),
            Err(TypedError::NewCompositeSizeMismatch { .. })
        ));
    }

    // Phase 3: flat tuple-field access is bounds-checked like a struct field.
    #[test]
    fn tuple_field_out_of_bounds_rejects() {
        let mut ops = two_int_struct(16);
        ops.push(Op::GetTupleField(TupleField::Flat {
            offset: 12,
            kind: ScalarKind::Int,
        }));
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    #[test]
    fn tuple_field_in_bounds_accepts() {
        let mut ops = two_int_struct(16);
        ops.push(Op::GetTupleField(TupleField::Flat {
            offset: 8,
            kind: ScalarKind::Int,
        }));
        assert!(typed_check_chunk(&chunk(ops, ints()), 8, 8).is_ok());
    }

    // Phase 3: flat enum-payload access is bounds-checked against the padded
    // enum body (the baked offset already includes the discriminant word).
    #[test]
    fn enum_field_out_of_bounds_rejects() {
        let mut ops = two_int_struct(16);
        ops.push(Op::GetEnumField(EnumField::Flat {
            offset: 12,
            kind: ScalarKind::Int,
        }));
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    #[test]
    fn enum_nested_field_out_of_bounds_rejects() {
        let mut ops = two_int_struct(16);
        ops.push(Op::GetEnumField(EnumField::FlatNested {
            offset: 8,
            size: 12,
            variant: CompositeKind::Struct,
        }));
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    // Phase 3: a flat array element access whose baked stride evenly divides
    // the array body accepts; a stride that does not is rejected. The index
    // bound stays a runtime trap.
    #[test]
    fn array_index_valid_stride_accepts() {
        let mut ops = two_int_struct(16); // a 16-byte flat body [Flat]
        ops.push(Op::Const(0)); // index -> [Flat, Int]
        ops.push(Op::GetIndex(ArrayElem::Flat {
            kind: ScalarKind::Int, // stride 8; 16 % 8 == 0
        }));
        assert!(typed_check_chunk(&chunk(ops, ints()), 8, 8).is_ok());
    }

    #[test]
    fn array_index_bad_stride_rejects() {
        // A seeded 12-byte array with an 8-byte nested element: 12 % 8 != 0.
        let ops = vec![
            Op::GetLocal(0),
            Op::Const(0),
            Op::GetIndex(ArrayElem::FlatNested {
                size: 8,
                variant: CompositeKind::Struct,
            }),
        ];
        let sig = ChunkSig {
            locals: vec![AbsVal::Flat {
                kind: CompositeKind::Array,
                size: 12,
            }],
            resume: None,
        };
        assert!(matches!(
            typed_check_chunk_with_sig(&chunk(ops, ints()), &sig, 8, 8),
            Err(TypedError::ArrayStrideMismatch { .. })
        ));
    }

    // Phase 3: a static-string constant is a fixed-size `Text` scalar, so a
    // composite field access on it is a type error rather than a deferral.
    #[test]
    fn static_str_const_is_scalar_text() {
        let ops = vec![
            Op::Const(0),
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }),
        ];
        let consts = vec![ConstValue::StaticStr(String::from("hi"))];
        assert!(matches!(
            typed_check_chunk(&chunk(ops, consts), 8, 8),
            Err(TypedError::ExpectedComposite { .. })
        ));
    }

    // Phase 4 (B6): a shared data slot whose byte extent overruns the
    // shared-data buffer is rejected at the module level; an in-bounds slot
    // accepts.
    #[test]
    fn shared_slot_out_of_bounds_rejects() {
        use crate::bytecode::{DataLayout, SharedSlotLayout};
        let mut m = module(vec![chunk(vec![Op::Return], ints())], vec![]);
        m.shared_data_bytes = 16;
        m.data_layout = Some(DataLayout {
            slots: Vec::new(),
            // offset 12 + scalar Int size 8 = 20 > 16.
            shared_layout: vec![SharedSlotLayout {
                offset: 12,
                kind: ScalarKind::Int.to_tag(),
                len: 0,
            }],
            private_composite_layout: Vec::new(),
        });
        assert!(matches!(
            typed_check_module(&m, 8, 8),
            Err(TypedError::SharedSlotOutOfBounds { .. })
        ));
    }

    #[test]
    fn shared_slot_in_bounds_accepts() {
        use crate::bytecode::{DataLayout, SharedSlotLayout};
        let mut m = module(vec![chunk(vec![Op::Return], ints())], vec![]);
        m.shared_data_bytes = 16;
        m.data_layout = Some(DataLayout {
            slots: Vec::new(),
            shared_layout: vec![SharedSlotLayout {
                offset: 8, // 8 + 8 = 16 <= 16
                kind: ScalarKind::Int.to_tag(),
                len: 0,
            }],
            private_composite_layout: Vec::new(),
        });
        assert!(typed_check_module(&m, 8, 8).is_ok());
    }

    // Phase 4 (B8): a flat-enum `NewComposite` whose baked body size matches no
    // declared enum body size (`word_bytes + min_payload`) is rejected; a
    // matching size accepts. A mutated `min_payload` shifts the declared set
    // away from the baked construction size, which is how the mutation is
    // caught.
    fn enum_construct_chunk(byte_size: u16) -> Chunk {
        chunk(
            vec![
                Op::Const(0), // discriminant word
                Op::Const(1), // payload word
                Op::NewComposite(NewCompositeOperand::Flat {
                    kind: CompositeKind::Enum,
                    count: 2,
                    byte_size,
                }),
                Op::Return,
            ],
            ints(),
        )
    }

    fn module_with_enum(byte_size: u16, min_payload: u32) -> Module {
        use crate::bytecode::{EnumLayout, EnumVariantDisc};
        let mut m = module(vec![enum_construct_chunk(byte_size)], vec![]);
        m.enum_layouts = vec![EnumLayout {
            type_name: String::from("E"),
            variants: vec![EnumVariantDisc {
                name: String::from("A"),
                disc: 0,
            }],
            min_payload,
        }];
        m
    }

    #[test]
    fn enum_body_size_matches_layout_accepts() {
        // word_bytes 8 + min_payload 8 = 16; construct a 16-byte enum body.
        assert!(typed_check_module(&module_with_enum(16, 8), 8, 8).is_ok());
    }

    #[test]
    fn enum_body_size_mismatch_rejects() {
        // Declared body size is 16 (8 + 8); a 24-byte construction matches no
        // declared enum body size.
        assert!(matches!(
            typed_check_module(&module_with_enum(24, 8), 8, 8),
            Err(TypedError::EnumBodySizeMismatch { .. })
        ));
    }

    #[test]
    fn mutated_min_payload_rejects_construction() {
        // The bytecode was baked for min_payload 8 (a 16-byte body), but the
        // carried table's min_payload is mutated to 4 (a 12-byte body). The
        // 16-byte construction no longer matches the declared size.
        assert!(matches!(
            typed_check_module(&module_with_enum(16, 4), 8, 8),
            Err(TypedError::EnumBodySizeMismatch { .. })
        ));
    }

    #[test]
    fn field_access_on_scalar_rejects() {
        let ops = vec![
            Op::Const(0),
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }),
        ];
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::ExpectedComposite { .. })
        ));
    }

    #[test]
    fn stack_underflow_rejects() {
        let ops = vec![Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        })];
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::StackUnderflow { .. })
        ));
    }

    // Seeding a local's shape (Phase 2) makes a flat-field offset check fire
    // across the local boundary, which Phase 1 could only defer to `Top`.
    #[test]
    fn seeded_local_composite_offset_checks() {
        let ops = vec![
            Op::GetLocal(0),
            Op::GetField(StructField::Flat {
                offset: 8,
                kind: ScalarKind::Int,
            }),
        ];
        let c = chunk(ops, Vec::new());
        // Unseeded: the local is `Top`, so the check defers and accepts.
        assert!(typed_check_chunk(&c, 8, 8).is_ok());
        // Seeded with a 16-byte struct: the valid offset 8 accepts.
        let sig = ChunkSig {
            locals: vec![AbsVal::Flat {
                kind: CompositeKind::Struct,
                size: 16,
            }],
            resume: None,
        };
        assert!(typed_check_chunk_with_sig(&c, &sig, 8, 8).is_ok());
        // Seeded, an out-of-bounds offset now rejects.
        let bad = chunk(
            vec![
                Op::GetLocal(0),
                Op::GetField(StructField::Flat {
                    offset: 12,
                    kind: ScalarKind::Int,
                }),
            ],
            Vec::new(),
        );
        assert!(matches!(
            typed_check_chunk_with_sig(&bad, &sig, 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    // A local seeded as a scalar, then a field read on it, is a type error
    // the depth pass cannot see.
    #[test]
    fn seeded_local_scalar_field_access_rejects() {
        let ops = vec![
            Op::GetLocal(0),
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }),
        ];
        let sig = ChunkSig {
            locals: vec![AbsVal::Scalar(ScalarKind::Int)],
            resume: None,
        };
        assert!(matches!(
            typed_check_chunk_with_sig(&chunk(ops, Vec::new()), &sig, 8, 8),
            Err(TypedError::ExpectedComposite { .. })
        ));
    }

    // The resume value's shape is seeded, so a field access on a resumed
    // composite is validated.
    #[test]
    fn yield_resume_shape_is_seeded() {
        let sig = ChunkSig {
            locals: Vec::new(),
            resume: Some(AbsVal::Flat {
                kind: CompositeKind::Struct,
                size: 16,
            }),
        };
        let good = chunk(
            vec![
                Op::Const(0),
                Op::Yield,
                Op::GetField(StructField::Flat {
                    offset: 8,
                    kind: ScalarKind::Int,
                }),
            ],
            ints(),
        );
        assert!(typed_check_chunk_with_sig(&good, &sig, 8, 8).is_ok());
        let bad = chunk(
            vec![
                Op::Const(0),
                Op::Yield,
                Op::GetField(StructField::Flat {
                    offset: 12,
                    kind: ScalarKind::Int,
                }),
            ],
            ints(),
        );
        assert!(matches!(
            typed_check_chunk_with_sig(&bad, &sig, 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    // Storing a scalar into a slot declared as a composite is a mismatch
    // (Phase 2a residual): it would make a later GetLocal seeding wrong.
    #[test]
    fn setlocal_shape_mismatch_rejects() {
        let ops = vec![Op::Const(0), Op::SetLocal(0)];
        let sig = ChunkSig {
            locals: vec![AbsVal::Flat {
                kind: CompositeKind::Struct,
                size: 16,
            }],
            resume: None,
        };
        assert!(matches!(
            typed_check_chunk_with_sig(&chunk(ops, ints()), &sig, 8, 8),
            Err(TypedError::LocalTypeMismatch { .. })
        ));
    }

    // Storing a matching composite into the declared slot accepts.
    #[test]
    fn setlocal_matching_shape_accepts() {
        let ops = vec![
            Op::Const(0),
            Op::Const(1),
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size: 16,
            }),
            Op::SetLocal(0),
        ];
        let sig = ChunkSig {
            locals: vec![AbsVal::Flat {
                kind: CompositeKind::Struct,
                size: 16,
            }],
            resume: None,
        };
        assert!(typed_check_chunk_with_sig(&chunk(ops, ints()), &sig, 8, 8).is_ok());
    }

    // Phase 2 local tracking: a composite built and stored into an unseeded
    // local is read back with its tracked shape, so a flat field access with
    // an out-of-bounds offset on the local-held value is rejected (before,
    // the unseeded local read `Top` and the check deferred).
    #[test]
    fn tracked_local_composite_offset_out_of_bounds_rejects() {
        let mut ops = two_int_struct(16);
        ops.push(Op::SetLocal(0));
        ops.push(Op::GetLocal(0));
        ops.push(Op::GetField(StructField::Flat {
            offset: 12,
            kind: ScalarKind::Int,
        }));
        assert!(matches!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    // The same access at an in-bounds offset accepts.
    #[test]
    fn tracked_local_composite_valid_offset_accepts() {
        let mut ops = two_int_struct(16);
        ops.push(Op::SetLocal(0));
        ops.push(Op::GetLocal(0));
        ops.push(Op::GetField(StructField::Flat {
            offset: 8,
            kind: ScalarKind::Int,
        }));
        assert!(typed_check_chunk(&chunk(ops, ints()), 8, 8).is_ok());
    }

    // Phase 2 loop fixpoint: a loop-carried local whose shape is the same on
    // every iteration is proven, so a flat access on it inside the loop is
    // validated rather than deferred. Slot 0 holds a 16-byte struct before the
    // loop and is rewritten to the same shape each iteration; the fixpoint's
    // ascending join keeps it `Flat{16}`, so an in-bounds field access accepts.
    //
    // Body layout (Loop at 4, exit 14): 5 GetLocal(0), 6 GetField, 7 PopN,
    // 8-10 rebuild, 11 SetLocal(0), 12 Break(14); 13 EndLoop(5); 14 Return.
    fn loop_carried_local_chunk(field_offset: u16) -> Chunk {
        let mut ops = two_int_struct(16); // 0,1,2: build a struct
        ops.push(Op::SetLocal(0)); // 3: slot 0 = Flat{16}
        ops.push(Op::Loop(14)); // 4: body [5,13), EndLoop at 13
        ops.push(Op::GetLocal(0)); // 5: carried slot 0 -> [Flat{16}]
        ops.push(Op::GetField(StructField::Flat {
            offset: field_offset,
            kind: ScalarKind::Int,
        })); // 6
        ops.push(Op::PopN(1)); // 7: -> []
        ops.extend(two_int_struct(16)); // 8,9,10: rebuild -> [Flat]
        ops.push(Op::SetLocal(0)); // 11: slot 0 = Flat{16} again -> []
        ops.push(Op::Break(14)); // 12: exit
        ops.push(Op::EndLoop(5)); // 13: back-edge to 5
        ops.push(Op::Return); // 14
        chunk(ops, ints())
    }

    #[test]
    fn loop_carried_local_stable_shape_is_proven() {
        // In-bounds field (8 + 8 == 16) on the stable carried `Flat{16}`.
        assert!(
            typed_check_chunk(&loop_carried_local_chunk(8), 8, 8).is_ok(),
            "a stable loop-carried composite local must be proven and its field access accepted"
        );
    }

    #[test]
    fn loop_carried_local_out_of_bounds_rejected() {
        // Out-of-bounds field (12 + 8 = 20 > 16) on the same carried local; the
        // first iteration reads its concrete pre-loop shape and every iteration
        // keeps it, so the fixpoint rejects. The old invalidate-to-`Top`
        // approximation deferred (missed) this.
        assert!(matches!(
            typed_check_chunk(&loop_carried_local_chunk(12), 8, 8),
            Err(TypedError::OffsetOutOfBounds { .. })
        ));
    }

    // A loop body that returns to the entry height but with a different slot
    // shape is not neutral (Phase 1 residual: back-edge equality, not just
    // height). Modelled by a body that pops a seeded composite local and
    // pushes a scalar in its place across the back-edge.
    #[test]
    fn loop_non_neutral_by_shape_rejects() {
        // Entry stack seeded with one composite via GetLocal; the loop body
        // replaces it with a scalar (Const) at equal height.
        let ops = vec![
            Op::GetLocal(0), // 0: entry stack [Flat]
            Op::Loop(5),     // 1: body [2,4), exit=5, EndLoop at 4
            Op::PopN(1),     // 2: pop the Flat -> []
            Op::Const(0),    // 3: push a Scalar -> [Scalar] (height 1, != entry [Flat])
            Op::EndLoop(0),  // 4
            Op::Return,      // 5
        ];
        let sig = ChunkSig {
            locals: vec![AbsVal::Flat {
                kind: CompositeKind::Struct,
                size: 16,
            }],
            resume: None,
        };
        assert!(
            matches!(
                typed_check_chunk_with_sig(&chunk(ops, ints()), &sig, 8, 8),
                Err(TypedError::LoopNotNeutral { .. })
            ),
            "a loop back-edge that changes a slot's shape at equal height must be rejected"
        );
    }

    // If/Else with balanced arms (each leaves one value) accepts; the merge
    // height is consistent.
    #[test]
    fn balanced_if_else_accepts() {
        // Const cond; If -> [Const] ; Else -> [Const] ; EndIf ; PopN(1)
        // Layout: 0 Const(cond) 1 If(->4) 2 Const 3 Else(->6) 4 Const 5 (endif at 6) ...
        // Build with correct targets: If target points one past Else region.
        let ops = vec![
            Op::Const(0), // 0: cond
            Op::If(4),    // 1: pop cond; then-arm [2,3); ops[3] is Else
            Op::Const(0), // 2: then pushes one
            Op::Else(5),  // 3: else region [4,5), endif=5
            Op::Const(0), // 4: else pushes one
            Op::EndIf,    // 5
            Op::PopN(1),  // 6: consume the merged value
        ];
        assert!(
            typed_check_chunk(&chunk(ops, ints()), 8, 8).is_ok(),
            "balanced branches should verify"
        );
    }

    // If/Else where one arm leaves a value and the other does not: height
    // mismatch, MUST-REJECT.
    #[test]
    fn imbalanced_if_else_rejects() {
        let ops = vec![
            Op::Const(0), // 0: cond
            Op::If(4),    // 1: pop cond; ops[3] is Else
            Op::Const(0), // 2: then pushes one -> height 1
            Op::Else(4),  // 3: empty else region [4,4), endif=4 -> height 0
            Op::EndIf,    // 4
            Op::PopN(1),  // 5
        ];
        // then-arm leaves height 1, else-arm leaves height 0 -> mismatch.
        assert!(
            matches!(
                typed_check_chunk(&chunk(ops, ints()), 8, 8),
                Err(TypedError::BranchHeightMismatch { .. })
            ),
            "imbalanced branches must be rejected"
        );
    }

    // The stricter exact-join and loop-neutrality rules must not reject
    // balanced real compiler output. These programs exercise struct field
    // access, calls, an if/else expression, a bounded `for` loop, and a
    // productive `loop`/`yield` stream; every emitted chunk must pass. A
    // false reject here is a bug in the pass.
    #[cfg(feature = "compile")]
    #[test]
    fn accepts_balanced_real_programs() {
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let programs = [
            "struct P { x: Word, y: Word }\n\
             fn main() -> Word { let p = P { x: 1, y: 2 }; p.x + p.y }",
            "fn add(a: Word, b: Word) -> Word { a + b }\n\
             fn main() -> Word { add(3, 4) }",
            "fn main() -> Word { let x = 3; if x > 0 { x } else { 0 - x } }",
            "fn main() -> Word { for i in 0..4 { i } 0 }",
            "loop main(i: Word) -> Word { let n = yield i * 2; n }",
        ];
        for src in programs {
            let module =
                compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
            // The compiler bakes flat-composite sizes at the module's target
            // widths; verify at those same widths (they differ from the host's
            // under a narrow-word feature build).
            let wb = (1usize << module.word_bits_log2) / 8;
            let fb = (1usize << module.float_bits_log2) / 8;
            for c in &module.chunks {
                let r = typed_check_chunk(c, wb, fb);
                assert!(
                    r.is_ok(),
                    "balanced real chunk `{}` from `{}` should verify, got {:?}",
                    c.name,
                    src,
                    r
                );
            }
        }
    }

    // Phase 2b seeding must not turn a valid program into a false reject.
    // These programs pass composite values across parameters and calls, so
    // the module-level `typed_check_module` seeds parameters and call results
    // to concrete flat shapes (not `Top`) and fires the offset and argument
    // checks against them. Every program must still verify.
    #[cfg(feature = "compile")]
    #[test]
    fn typed_check_module_accepts_seeded_real_programs() {
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let programs = [
            // Struct passed by parameter, then flat field access on the param.
            "struct P { x: Word, y: Word }\n\
             fn sum(p: P) -> Word { p.x + p.y }\n\
             fn main() -> Word { sum(P { x: 1, y: 2 }) }",
            // Struct returned from a call, then field access on the result.
            "struct P { x: Word, y: Word }\n\
             fn mk(a: Word) -> P { P { x: a, y: a } }\n\
             fn main() -> Word { mk(3).y }",
            // Enum parameter matched in the callee.
            "enum E { A(Word), B }\n\
             fn f(e: E) -> Word { match e { E::A(v) => v, E::B => 0 } }\n\
             fn main() -> Word { f(E::A(5)) + f(E::B) }",
            // Nested composite parameter (a struct field that is a tuple).
            "struct Q { p: (Word, Word), z: Word }\n\
             fn g(q: Q) -> Word { q.z }\n\
             fn main() -> Word { g(Q { p: (1, 2), z: 3 }) }",
            // Tuple field access on a local (exercises GetTupleField).
            "fn main() -> Word { let t = (10, 20); t.0 + t.1 }",
            // Array indexing on a local (exercises GetIndex stride check).
            "fn main() -> Word { let a = [1, 2, 3, 4]; a[0] + a[3] }",
            // A Stream chunk resumes with its parameter's shape.
            "loop main(i: Word) -> Word { let n = yield i * 2; n }",
        ];
        for src in programs {
            let module =
                compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
            let wb = (1usize << module.word_bits_log2) / 8;
            let fb = (1usize << module.float_bits_log2) / 8;
            let r = typed_check_module(&module, wb, fb);
            assert!(
                r.is_ok(),
                "seeded module for `{}` should verify, got {:?}",
                src,
                r
            );
        }
    }

    // Phase 1 finding (B5), now fixed: the `match`/enum lowering compiled to a
    // dispatch `Loop` with one `Break` per arm and left the peeked scrutinee on
    // the operand stack in a failed arm, so the arms' break edges left
    // different stack heights (`[v]` vs `[e, 0]`). The scalar depth pass hid
    // this by taking the max; the typed pass's exact join over the break edges
    // detected it. The compiler now consumes the peeked scrutinee on both
    // branches of each refutable `IsEnum`/`IsStruct` test
    // (`emit_consume_peeked_scrutinee`), so the match lowering is balanced.
    // This test pins that the once-imbalanced program now verifies; a
    // regression that reintroduces the leak would fail it with a
    // `BranchHeightMismatch`.
    #[cfg(feature = "compile")]
    #[test]
    fn balanced_match_verifies_after_b5_fix() {
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "enum E { A(Word), B }\n\
                   fn f(e: E) -> Word { match e { E::A(v) => v, E::B => 0 } }\n\
                   fn main() -> Word { f(E::A(5)) }";
        let module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let wb = (1usize << module.word_bits_log2) / 8;
        let fb = (1usize << module.float_bits_log2) / 8;
        for c in &module.chunks {
            let r = typed_check_chunk(c, wb, fb);
            assert!(
                r.is_ok(),
                "match chunk `{}` should verify after the B5 balancing fix, got {:?}",
                c.name,
                r
            );
        }
    }

    // The `enum as Word` cast (`compile_enum_to_word`) is a separate dispatch
    // loop that also emitted a peeking `IsEnum` per variant and cleaned the
    // copy only on the match path, so a non-matching variant leaked it and each
    // arm's `Break` left a different height (the same B5 shape as the `match`
    // lowering). It now consumes the peek on both branches; this pins that a
    // compiled multi-variant cast verifies.
    #[cfg(feature = "compile")]
    #[test]
    fn balanced_enum_to_word_cast_verifies_after_b5_fix() {
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "enum Color { Red, Green, Blue }\n\
                   fn main() -> Word { Color::Blue as Word }";
        let module =
            compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let wb = (1usize << module.word_bits_log2) / 8;
        let fb = (1usize << module.float_bits_log2) / 8;
        for c in &module.chunks {
            let r = typed_check_chunk(c, wb, fb);
            assert!(
                r.is_ok(),
                "enum-to-word cast chunk `{}` should verify after the B5 balancing fix, got {:?}",
                c.name,
                r
            );
        }
    }

    // Phase 2b: a cross-call flat-composite field access validates. `mk`
    // returns a two-`Word` struct; `main` calls it and reads a field. The
    // module signature table seeds the call result's flat shape so the
    // `GetField` offset check fires (and passes) rather than deferring to
    // `Top`. The module must actually carry a populated signature table with a
    // flat return, else the seeding would be vacuous.
    #[cfg(feature = "compile")]
    #[test]
    fn cross_call_composite_access_validates() {
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "struct P { x: Word, y: Word }\n\
                   fn mk() -> P { P { x: 1, y: 2 } }\n\
                   fn main() -> Word { mk().x }";
        let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let wb = (1usize << m.word_bits_log2) / 8;
        let fb = (1usize << m.float_bits_log2) / 8;
        assert!(
            typed_check_module(&m, wb, fb).is_ok(),
            "a cross-call composite field access should verify under signature seeding"
        );
        assert_eq!(
            m.signatures.len(),
            m.chunks.len(),
            "the compiler must emit one signature per chunk"
        );
        assert!(
            m.signatures
                .iter()
                .any(|s| matches!(s.ret, WireShape::Flat { .. })),
            "`mk`'s struct return must be recorded as a flat shape"
        );
    }

    // Phase 2b: a `Call` whose argument shape is incompatible with the
    // callee's declared parameter shape is rejected. The callee (chunk 0)
    // declares one 16-byte struct parameter; the caller (chunk 1) passes a
    // scalar `Int`.
    #[test]
    fn call_arg_shape_mismatch_rejects() {
        let callee = chunk(vec![Op::Const(0), Op::Return], ints());
        let caller = chunk(vec![Op::Const(0), Op::Call(0, 1), Op::Return], ints());
        let sigs = vec![
            ChunkSignature {
                params: vec![WireShape::Flat {
                    kind: CompositeKind::Struct.to_tag(),
                    size: 16,
                }],
                ret: WireShape::Scalar {
                    kind: ScalarKind::Int.to_tag(),
                },
                resume: WireShape::Top,
            },
            ChunkSignature::default(),
        ];
        let m = module(vec![callee, caller], sigs);
        assert!(
            matches!(
                typed_check_module(&m, 8, 8),
                Err(TypedError::CallArgMismatch {
                    callee: 0,
                    arg: 0,
                    ..
                })
            ),
            "a scalar argument to a struct parameter must be rejected"
        );
    }

    // Phase 2b: a matching argument shape accepts, and the call result takes
    // the callee's return shape. The caller builds the struct the callee
    // expects, calls it, and the checker completes with no mismatch.
    #[test]
    fn call_arg_shape_match_accepts() {
        let callee = chunk(vec![Op::Const(0), Op::Return], ints());
        // Caller: build a 16-byte struct, then Call(0, 1).
        let mut ops = two_int_struct(16);
        ops.push(Op::Call(0, 1));
        ops.push(Op::Return);
        let caller = chunk(ops, ints());
        let sigs = vec![
            ChunkSignature {
                params: vec![WireShape::Flat {
                    kind: CompositeKind::Struct.to_tag(),
                    size: 16,
                }],
                ret: WireShape::Scalar {
                    kind: ScalarKind::Int.to_tag(),
                },
                resume: WireShape::Top,
            },
            ChunkSignature::default(),
        ];
        let m = module(vec![callee, caller], sigs);
        assert!(typed_check_module(&m, 8, 8).is_ok());
    }

    // A neutral loop body (pushes then pops) accepts; a non-neutral one
    // (pushes without popping) is rejected.
    #[test]
    fn loop_neutrality() {
        // Neutral: Loop -> [Const, PopN(1)] -> EndLoop ; then Break to exit.
        let neutral = vec![
            Op::Loop(5),    // 0: body [1,4), exit=5
            Op::Const(0),   // 1
            Op::PopN(1),    // 2
            Op::Break(5),   // 3: exit
            Op::EndLoop(0), // 4: back-edge (unreached; body exits via Break)
            Op::Return,     // 5
        ];
        assert!(
            typed_check_chunk(&chunk(neutral, ints()), 8, 8).is_ok(),
            "a stack-neutral loop body should verify"
        );

        // Non-neutral: body pushes and falls through without popping.
        let non_neutral = vec![
            Op::Loop(3),    // 0: body [1,2), exit=3, EndLoop at exit-1=2
            Op::Const(0),   // 1: pushes, never popped -> height 1
            Op::EndLoop(0), // 2: back-edge with height 1 != entry 0
            Op::Return,     // 3: exit
        ];
        assert!(
            matches!(
                typed_check_chunk(&chunk(non_neutral, ints()), 8, 8),
                Err(TypedError::LoopNotNeutral { .. })
            ),
            "a non-neutral loop body must be rejected"
        );
    }
}
