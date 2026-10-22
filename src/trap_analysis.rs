//! Bounded first-pass classification of explicit language traps.
//!
//! A trap-free verdict excludes language traps for the supported instruction
//! contracts. It does not remove runtime guards or certify host inputs, operand
//! shapes, storage lifetimes, resource availability or native implementations.
use crate::bytecode::{ArrayElem, EnumField, Module, Op, StructField, TrapKind, TupleField};
use alloc::format;
use alloc::string::String;

/// Maximum instructions per chunk in the first-pass trap-analysis profile.
pub const TRAP_OPCODE_CAP: usize = 2048;

/// An instruction location in a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrapLocation {
    /// Chunk ordinal in the module.
    pub chunk: usize,
    /// Instruction ordinal in the chunk.
    pub instruction: usize,
}

/// A completed classification, after validation and profile admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapVerdict {
    /// No supported instruction can raise an implicit language trap and no Trap occurs.
    TrapFree,
    /// An explicit Trap occurs, including in syntactically unreachable code.
    MayTrap {
        /// First explicit Trap in module order.
        first: TrapLocation,
    },
}

/// An instruction contract outside the first-pass proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedTrapOperation {
    /// Raw saturating Fixed division still faults on zero.
    FixedDivision,
    /// Only flat Word and Byte indexed values have total first-pass contracts.
    IndexedValue,
    /// Boxed field access can still fault internally.
    BoxedField,
    /// Native errors and result contracts require the later language profile.
    NativeCall,
}

/// Failure to establish a completed trap classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrapAnalysisError {
    /// Structural or typed validation rejected the subject.
    InvalidModule {
        /// Validation diagnostic. Different verifier implementations may differ in wording.
        detail: String,
    },
    /// A contract is outside the first-pass proof, even if no Trap appears.
    Unsupported {
        /// First unsupported instruction in module order.
        location: TrapLocation,
        /// Required extension to the proof profile.
        operation: UnsupportedTrapOperation,
    },
    /// The bounded opcode profile cannot hold a chunk.
    Capacity {
        /// Chunk ordinal.
        chunk: usize,
        /// Supported opcode count.
        limit: usize,
        /// Requested opcode count.
        actual: usize,
    },
    /// A verifier or classifier could not finish, or its protocol was invalid.
    Unavailable {
        /// Analysis diagnostic, never a completed subject verdict.
        detail: String,
    },
}
impl core::fmt::Display for TrapAnalysisError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "trap analysis: {self:?}")
    }
}
impl core::error::Error for TrapAnalysisError {}

pub(crate) fn preflight(module: &Module) -> Result<(), TrapAnalysisError> {
    // Guard shifts in the reference verifier before invoking it on public input.
    if !(3..=6).contains(&module.word_bits_log2)
        || !(3..=6).contains(&module.addr_bits_log2)
        || module.float_bits_log2 > 6
        || module
            .entry_point
            .is_some_and(|entry| entry >= module.chunks.len())
    {
        return Err(TrapAnalysisError::InvalidModule {
            detail: String::from("invalid module width or entry point"),
        });
    }
    for (chunk, body) in module.chunks.iter().enumerate() {
        if body.ops.len() > TRAP_OPCODE_CAP {
            return Err(TrapAnalysisError::Capacity {
                chunk,
                limit: TRAP_OPCODE_CAP,
                actual: body.ops.len(),
            });
        }
    }
    Ok(())
}

/// The trap class of one instruction, before module-level precedence is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstructionTrapClass {
    /// The instruction neither traps nor is outside the analysed profile.
    Clear,
    /// The instruction is a language trap.
    MayTrap,
    /// The instruction is outside the analysed profile.
    Unsupported(UnsupportedTrapOperation),
}

/// Classify one instruction. `classify_traps` applies this to every
/// instruction, and the Keleusma classifier's per-tag test derives its
/// expectation from it, so the two cannot drift through a hand-written table.
pub(crate) fn classify_instruction(
    op: &Op,
    location: TrapLocation,
) -> Result<InstructionTrapClass, TrapAnalysisError> {
    Ok(match op {
        Op::Trap(kind) => {
            if TrapKind::from_code(*kind).is_none() {
                return Err(TrapAnalysisError::InvalidModule {
                    detail: format!("unknown trap kind at {location:?}"),
                });
            }
            InstructionTrapClass::MayTrap
        }
        Op::PushImmediate(value) => {
            if *value > 19 {
                return Err(TrapAnalysisError::InvalidModule {
                    detail: format!("reserved immediate at {location:?}"),
                });
            }
            InstructionTrapClass::Clear
        }
        Op::FixedDiv(_) => {
            InstructionTrapClass::Unsupported(UnsupportedTrapOperation::FixedDivision)
        }
        Op::GetIndex(ArrayElem::Flat {
            kind: crate::value_layout::ScalarKind::Int | crate::value_layout::ScalarKind::Byte,
        }) => InstructionTrapClass::Clear,
        Op::GetIndex(_) => {
            InstructionTrapClass::Unsupported(UnsupportedTrapOperation::IndexedValue)
        }
        Op::GetField(StructField::Boxed { .. })
        | Op::GetTupleField(TupleField::Boxed { .. })
        | Op::GetEnumField(EnumField::Boxed { .. }) => {
            InstructionTrapClass::Unsupported(UnsupportedTrapOperation::BoxedField)
        }
        Op::CallVerifiedNative(..) | Op::CallExternalNative(..) => {
            InstructionTrapClass::Unsupported(UnsupportedTrapOperation::NativeCall)
        }
        // Exhaustive inventory. A new public opcode requires an explicit audit here.
        Op::Const(_)
        | Op::GetLocal(_)
        | Op::SetLocal(_)
        | Op::GetData(_)
        | Op::SetData(_)
        | Op::GetDataIndexed(..)
        | Op::SetDataIndexed(..)
        | Op::BoundsCheck(_)
        | Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Mod
        | Op::Neg
        | Op::CmpEq
        | Op::CmpNe
        | Op::CmpLt
        | Op::CmpGt
        | Op::CmpLe
        | Op::CmpGe
        | Op::Not
        | Op::If(_)
        | Op::Else(_)
        | Op::EndIf
        | Op::Loop(_)
        | Op::EndLoop(_)
        | Op::Break(_)
        | Op::BreakIf(_)
        | Op::Stream
        | Op::Reset
        | Op::Call(..)
        | Op::Return
        | Op::Yield
        | Op::Dup
        | Op::GetField(_)
        | Op::GetTupleField(_)
        | Op::GetEnumField(_)
        | Op::Len
        | Op::IsEnum(..)
        | Op::IsStruct(..)
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::WordToFixed(_)
        | Op::FixedToWord(_)
        | Op::FixedMul(_)
        | Op::CheckedAdd
        | Op::CheckedSub
        | Op::CheckedMul(_)
        | Op::CheckedNeg
        | Op::CheckedDiv(_)
        | Op::CheckedMod
        | Op::PopN(_)
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Shl
        | Op::Shr
        | Op::NewComposite(_) => InstructionTrapClass::Clear,
    })
}

/// Classify the entire validated module, including unreachable chunks and instructions.
/// Unsupported forms take precedence over a possible-trap verdict. This function
/// neither executes subject code nor creates an unchecked-execution certificate.
pub fn classify_traps(module: &Module) -> Result<TrapVerdict, TrapAnalysisError> {
    preflight(module)?;
    crate::verify::verify(module).map_err(|e| TrapAnalysisError::InvalidModule {
        detail: format!("{e:?}"),
    })?;
    let mut first = None;
    let mut unsupported = None;
    for (chunk, body) in module.chunks.iter().enumerate() {
        for (instruction, op) in body.ops.iter().enumerate() {
            let location = TrapLocation { chunk, instruction };
            match classify_instruction(op, location)? {
                InstructionTrapClass::Clear => {}
                InstructionTrapClass::MayTrap => {
                    first.get_or_insert(location);
                }
                InstructionTrapClass::Unsupported(operation) => {
                    unsupported.get_or_insert(TrapAnalysisError::Unsupported {
                        location,
                        operation,
                    });
                }
            }
        }
    }
    if let Some(error) = unsupported {
        return Err(error);
    }
    Ok(
        first.map_or(TrapVerdict::TrapFree, |first| TrapVerdict::MayTrap {
            first,
        }),
    )
}
