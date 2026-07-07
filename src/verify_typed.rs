//! Phase 0 spike for the A.2.1 typed operand-stack verifier pass.
//!
//! This is a standalone proof of concept, not yet wired into
//! [`crate::verify`]. It demonstrates the load-bearing mechanism of the
//! planned pass (see `tmp/A21_typed_verifier_pass_plan.md`): an abstract
//! operand stack that tracks the flat *shape* (composite kind and byte
//! size) of every value, so a compiler-baked flat-field offset can be
//! validated against the byte size of the composite it is applied to
//! rather than trusted and deferred to a runtime `debug_assert!`.
//!
//! The spike covers only straight-line op sequences and the two building
//! blocks that prove the idea: `NewComposite` (produces a flat body of a
//! known size) and the flat `GetField` forms (consume a flat body and
//! read a scalar or a nested composite at a baked offset). Control flow,
//! signatures/seeding, the wire enrichment, and the remaining ops are the
//! subject of Phases 1 onward.

use crate::bytecode::{NewCompositeOperand, Op, StructField};
use crate::value_layout::{CompositeKind, ScalarKind};
use alloc::vec::Vec;

/// An abstract value on the operand stack. The spike carries scalars and
/// flat composite bodies; the full pass adds reference handles and a
/// `Top` join element (see the plan).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbsVal {
    /// A fixed-size scalar of the given kind.
    Scalar(ScalarKind),
    /// A flat composite body.
    Flat {
        /// The composite variant the body re-wraps as.
        kind: CompositeKind,
        /// The body's byte length.
        size: u32,
    },
}

impl AbsVal {
    /// Byte size this abstract value occupies inside a packed flat body.
    fn packed_size(&self, word_bytes: usize, float_bytes: usize) -> u32 {
        match self {
            AbsVal::Scalar(k) => k.size_in_bytes(word_bytes, float_bytes) as u32,
            AbsVal::Flat { size, .. } => *size,
        }
    }
}

/// A decidable MUST-REJECT reason found by the typed pass. Every variant
/// is a load-time rejection, never a runtime fault (per meta-spec C3).
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
    /// A field access was applied to a value that is not a composite.
    ExpectedComposite {
        /// Instruction index of the access.
        ip: usize,
    },
    /// A `NewComposite` baked byte size disagrees with the packed size of
    /// the values it consumes.
    NewCompositeSizeMismatch {
        /// Instruction index of the construction.
        ip: usize,
        /// The baked byte size carried by the operand.
        baked: u32,
        /// The size recomputed from the consumed values.
        computed: u32,
    },
}

/// Spike interpreter: walk a straight-line op slice maintaining an
/// abstract operand stack, and return the final stack (or the first
/// MUST-REJECT reason). The abstract stack height is the operand depth,
/// so underflow subsumes the current scalar depth pass.
pub fn spike_check(
    ops: &[Op],
    word_bytes: usize,
    float_bytes: usize,
) -> Result<Vec<AbsVal>, TypedError> {
    let mut stack: Vec<AbsVal> = Vec::new();
    for (ip, op) in ops.iter().enumerate() {
        match op {
            // A constant load pushes a scalar. The spike treats every
            // constant as a word-width integer; the full pass reads the
            // constant's kind from the pool.
            Op::Const(_) => stack.push(AbsVal::Scalar(ScalarKind::Int)),

            // Construct a flat composite: pop `count` values, recompute
            // the packed size, and require it equal the baked `byte_size`
            // (the NewComposite half of finding B1). Enum bodies pad the
            // payload to the largest variant, so their exact size check is
            // deferred to Phase 3 with the enum-layout table; the spike
            // checks the padding-free kinds.
            Op::NewComposite(NewCompositeOperand::Flat {
                kind,
                count,
                byte_size,
            }) => {
                let mut computed: u32 = 0;
                for _ in 0..*count {
                    let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
                    computed = computed.saturating_add(v.packed_size(word_bytes, float_bytes));
                }
                if !matches!(kind, CompositeKind::Enum) && computed != u32::from(*byte_size) {
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

            // Read a flat scalar field: the baked offset plus the scalar
            // width must fit within the composite body (findings B1/B2).
            Op::GetField(StructField::Flat { offset, kind }) => {
                let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
                let AbsVal::Flat { size, .. } = v else {
                    return Err(TypedError::ExpectedComposite { ip });
                };
                let need = usize::from(*offset) + kind.size_in_bytes(word_bytes, float_bytes);
                if need > size as usize {
                    return Err(TypedError::OffsetOutOfBounds {
                        ip,
                        offset: usize::from(*offset),
                        need,
                        size,
                    });
                }
                stack.push(AbsVal::Scalar(*kind));
            }

            // Read a nested flat composite field: offset + nested size must
            // fit within the parent body. This is exactly the check that is
            // only a release-elided `debug_assert!` in `flat_value::nested_view`
            // today, promoted here to a load-time MUST-REJECT (finding B1).
            Op::GetField(StructField::FlatNested {
                offset,
                size: nested,
                variant,
            }) => {
                let v = stack.pop().ok_or(TypedError::StackUnderflow { ip })?;
                let AbsVal::Flat { size: parent, .. } = v else {
                    return Err(TypedError::ExpectedComposite { ip });
                };
                let need = usize::from(*offset) + usize::from(*nested);
                if need > parent as usize {
                    return Err(TypedError::OffsetOutOfBounds {
                        ip,
                        offset: usize::from(*offset),
                        need,
                        size: parent,
                    });
                }
                stack.push(AbsVal::Flat {
                    kind: *variant,
                    size: u32::from(*nested),
                });
            }

            // Every other op is out of scope for the Phase 0 spike.
            _ => {}
        }
    }
    Ok(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    // A struct of two word-width integers packs to 16 bytes at the host
    // width (word_bytes = 8): two Const pushes then a flat NewComposite.
    fn two_int_struct(byte_size: u16) -> Vec<Op> {
        vec![
            Op::Const(0),
            Op::Const(0),
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size,
            }),
        ]
    }

    #[test]
    fn valid_flat_field_access_accepts() {
        let mut ops = two_int_struct(16);
        // Second field at offset 8, Int (8 bytes): 8 + 8 = 16 <= 16. Valid.
        ops.push(Op::GetField(StructField::Flat {
            offset: 8,
            kind: ScalarKind::Int,
        }));
        let r = spike_check(&ops, 8, 8);
        assert!(r.is_ok(), "valid flat access should verify: {:?}", r);
    }

    #[test]
    fn out_of_bounds_flat_field_offset_rejects() {
        let mut ops = two_int_struct(16);
        // Offset 12, Int (8 bytes): 12 + 8 = 20 > 16. MUST-REJECT (B1/B2).
        ops.push(Op::GetField(StructField::Flat {
            offset: 12,
            kind: ScalarKind::Int,
        }));
        assert!(
            matches!(
                spike_check(&ops, 8, 8),
                Err(TypedError::OffsetOutOfBounds { .. })
            ),
            "an out-of-bounds baked offset must be rejected at load time"
        );
    }

    #[test]
    fn nested_flat_field_out_of_bounds_rejects() {
        // Parent body 16 bytes; a nested composite of 12 bytes at offset 8
        // would read to byte 20 > 16. This is the `nested_view` debug_assert
        // promoted to a MUST-REJECT (finding B1).
        let mut ops = two_int_struct(16);
        ops.push(Op::GetField(StructField::FlatNested {
            offset: 8,
            size: 12,
            variant: CompositeKind::Struct,
        }));
        assert!(
            matches!(
                spike_check(&ops, 8, 8),
                Err(TypedError::OffsetOutOfBounds { .. })
            ),
            "an out-of-bounds nested-composite offset must be rejected"
        );
    }

    #[test]
    fn newcomposite_size_mismatch_rejects() {
        // Baked byte_size 24 but two Int fields pack to 16. MUST-REJECT.
        assert!(
            matches!(
                spike_check(&two_int_struct(24), 8, 8),
                Err(TypedError::NewCompositeSizeMismatch { .. })
            ),
            "a NewComposite size disagreeing with its elements must be rejected"
        );
    }

    #[test]
    fn field_access_on_scalar_rejects() {
        // A field read applied to a bare scalar is ill-typed. The scalar
        // depth pass cannot catch this; the typed pass does.
        let ops = vec![
            Op::Const(0),
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }),
        ];
        assert!(
            matches!(
                spike_check(&ops, 8, 8),
                Err(TypedError::ExpectedComposite { .. })
            ),
            "a field access on a scalar must be rejected"
        );
    }

    #[test]
    fn stack_underflow_rejects() {
        // A field read with nothing on the stack underflows: the abstract
        // stack height subsumes the operand-depth pass.
        let ops = vec![Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        })];
        assert!(
            matches!(
                spike_check(&ops, 8, 8),
                Err(TypedError::StackUnderflow { .. })
            ),
            "operand-stack underflow must be rejected"
        );
    }
}
