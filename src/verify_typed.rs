//! Typed operand-stack verifier pass (A.2.1) — Phase 1: the abstract
//! interpreter framework.
//!
//! This is a standalone pass, not yet wired into [`crate::verify`]. It
//! walks a chunk's block-structured control flow over an *abstract operand
//! stack* whose slots carry the flat shape (composite kind and byte size)
//! of each value where known. The pass:
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
//! - validates a compiler-baked flat-field offset against the byte size of
//!   the composite it is applied to, wherever that size is statically known
//!   (findings B1/B2).
//!
//! Where a value's shape is not yet known — a parameter, a local, a call
//! result, a resume value, a composite constant — the slot is [`AbsVal::Top`]
//! and shape-dependent checks are conservatively skipped. Seeding those from
//! per-chunk signatures (so the checks fire everywhere) is Phase 2; extending
//! the offset validation to every access op and the wire-carried layout
//! tables is Phase 3/4. Nothing here changes runtime behaviour.

use crate::bytecode::{Chunk, ConstValue, NewCompositeOperand, Op, StructField};
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
    let ctx = Ctx {
        chunk,
        sig,
        wb: word_bytes,
        fb: float_bytes,
    };
    let mut breaks: Vec<Vec<AbsVal>> = Vec::new();
    interp_region(&ctx, 0, chunk.ops.len(), Vec::new(), &mut breaks).map(|_| ())
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
    /// Target word width in bytes (for scalar/composite sizing).
    wb: usize,
    /// Target float width in bytes.
    fb: usize,
}

/// A recursive abstract interpreter over ops `[start, end)`. The immutable
/// per-chunk context is carried in `ctx`; only the region bounds, the working
/// stack, and the break collector vary per call.
fn interp_region(
    ctx: &Ctx,
    start: usize,
    end: usize,
    mut stack: Vec<AbsVal>,
    breaks: &mut Vec<Vec<AbsVal>>,
) -> Result<Option<Vec<AbsVal>>, TypedError> {
    let ops = &ctx.chunk.ops;
    let mut ip = start;
    while ip < end {
        let op = &ops[ip];
        match op {
            Op::Trap(_) | Op::Return => return Ok(None),
            Op::Break(_) => {
                breaks.push(stack);
                return Ok(None);
            }
            Op::BreakIf(_) => {
                apply_op(ctx, op, &mut stack, ip)?;
                breaks.push(stack.clone());
                ip += 1;
            }
            Op::If(target) => {
                apply_op(ctx, op, &mut stack, ip)?;
                let target = *target as usize;
                if target > 0 && matches!(&ops[target - 1], Op::Else(_)) {
                    let endif = match &ops[target - 1] {
                        Op::Else(e) => *e as usize,
                        _ => unreachable!(),
                    };
                    let then_end = interp_region(ctx, ip + 1, target - 1, stack.clone(), breaks)?;
                    let else_end = interp_region(ctx, target, endif, stack, breaks)?;
                    match join_ends(then_end, else_end, ip)? {
                        Some(joined) => stack = joined,
                        None => return Ok(None),
                    }
                    ip = endif + 1;
                } else {
                    // No else: the then-arm merges with the fall-through path
                    // (the stack unchanged after popping the condition).
                    let skip = stack.clone();
                    let then_end = interp_region(ctx, ip + 1, target, stack, breaks)?;
                    match join_ends(then_end, Some(skip), ip)? {
                        Some(joined) => stack = joined,
                        None => return Ok(None),
                    }
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                let exit = *target as usize;
                let entry_height = stack.len();
                let mut loop_breaks: Vec<Vec<AbsVal>> = Vec::new();
                let body_end =
                    interp_region(ctx, ip + 1, exit - 1, stack.clone(), &mut loop_breaks)?;
                // Back-edge neutrality: the body's fall-through must restore
                // the exact entry stack (height and per-slot shape), so each
                // iteration begins in the same state and per-iteration stack
                // growth cannot escape the worst-case bound.
                if let Some(be) = &body_end
                    && *be != stack
                {
                    return Err(TypedError::LoopNotNeutral {
                        ip,
                        entry_height,
                        exit_height: be.len(),
                    });
                }
                // The code after the loop is reached via the break edges; if
                // there are none, fall back to the (neutral) body exit or the
                // entry stack, mirroring the depth pass.
                stack = match join_all(loop_breaks, ip)? {
                    Some(s) => s,
                    None => body_end.unwrap_or(stack),
                };
                ip = exit;
            }
            _ => {
                apply_op(ctx, op, &mut stack, ip)?;
                ip += 1;
            }
        }
    }
    Ok(Some(stack))
}

/// Join two region ends. `Some`/`Some` requires equal height and joins
/// per slot; a `None` (exiting) arm yields the other; both `None` yields
/// `None` (the merge is unreachable).
fn join_ends(
    then_end: Option<Vec<AbsVal>>,
    else_end: Option<Vec<AbsVal>>,
    ip: usize,
) -> Result<Option<Vec<AbsVal>>, TypedError> {
    match (then_end, else_end) {
        (Some(a), Some(b)) => {
            if a.len() != b.len() {
                return Err(TypedError::BranchHeightMismatch {
                    ip,
                    then_height: a.len(),
                    else_height: b.len(),
                });
            }
            let joined = a.iter().zip(b.iter()).map(|(x, y)| x.join(y)).collect();
            Ok(Some(joined))
        }
        (Some(a), None) => Ok(Some(a)),
        (None, Some(b)) => Ok(Some(b)),
        (None, None) => Ok(None),
    }
}

/// Join a set of break-edge stacks. Requires equal heights; `None` when the
/// set is empty.
fn join_all(stacks: Vec<Vec<AbsVal>>, ip: usize) -> Result<Option<Vec<AbsVal>>, TypedError> {
    let mut it = stacks.into_iter();
    let Some(mut acc) = it.next() else {
        return Ok(None);
    };
    for s in it {
        if s.len() != acc.len() {
            return Err(TypedError::BranchHeightMismatch {
                ip,
                then_height: acc.len(),
                else_height: s.len(),
            });
        }
        acc = acc.iter().zip(s.iter()).map(|(x, y)| x.join(y)).collect();
    }
    Ok(Some(acc))
}

/// Apply one non-control-flow op's effect to the abstract stack. Underflow
/// is decided from `op_depth_effect`'s required-operand count, so the height
/// discipline exactly matches the scalar depth pass. Shape is tracked
/// precisely for the ops that carry or consume it and conservatively (`Top`)
/// otherwise.
fn apply_op(ctx: &Ctx, op: &Op, stack: &mut Vec<AbsVal>, ip: usize) -> Result<(), TypedError> {
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

        // Seed a local read with its declared shape (Phase 2). Unseeded
        // locals are `Top`.
        Op::GetLocal(i) => stack.push(ctx.sig.local(*i as usize)),

        // A local write must store a value whose shape is compatible with the
        // slot's declared shape, so the seeded `GetLocal` reads stay
        // trustworthy. An unseeded slot or an unknown value defers.
        Op::SetLocal(i) => {
            let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
            if !shapes_compatible(&v, &ctx.sig.local(*i as usize)) {
                return Err(TypedError::LocalTypeMismatch {
                    ip,
                    slot: *i as usize,
                });
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
            // Enum bodies pad the payload to the largest variant, so their
            // exact size is validated in Phase 4 against the enum-layout
            // table. Non-enum flat bodies must pack exactly when every
            // element size is known.
            if all_known
                && !matches!(kind, CompositeKind::Enum)
                && computed != u32::from(*byte_size)
            {
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
        // Strings, composites, and options carry a shape the pass does not
        // yet compute; conservatively unknown.
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
