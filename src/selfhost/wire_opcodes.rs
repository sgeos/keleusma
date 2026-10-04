//! Typed instruction transport. All byte packing and pool decisions are in Keleusma.

use super::wire_framing::{command, read_window, set_word, with_compiled_wire};
use super::*;

/// The two instruction sections, emitted in chunk and instruction order.
pub struct WireOpcodeSections {
    /// Four-byte instruction records including their parity bits.
    pub opcodes: Vec<u8>,
    /// Eight-byte operand entries including tags and parity bytes.
    pub operands: Vec<u8>,
}

/// Marshal enum discriminants and typed fields without constructing wire bytes.
/// The ordinary form is zero. Access variants are scalar, boxed and nested.
/// Construction variants are flat and boxed. These private transport tags are
/// not the wire sentinels, pool tags or bytecode instruction identifiers.
fn fields(op: &Op) -> [i64; 5] {
    let [form, a, b, c] = match op {
        Op::Add
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
        | Op::EndIf
        | Op::Stream
        | Op::Reset
        | Op::Return
        | Op::Yield
        | Op::Dup
        | Op::Len
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::CheckedAdd
        | Op::CheckedSub
        | Op::CheckedNeg
        | Op::CheckedMod
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Shl
        | Op::Shr => [0; 4],
        Op::WordToFixed(n)
        | Op::FixedToWord(n)
        | Op::FixedMul(n)
        | Op::FixedDiv(n)
        | Op::CheckedMul(n)
        | Op::CheckedDiv(n)
        | Op::PushImmediate(n)
        | Op::PopN(n) => [0, i64::from(*n), 0, 0],
        Op::Const(n)
        | Op::GetLocal(n)
        | Op::SetLocal(n)
        | Op::BoundsCheck(n)
        | Op::If(n)
        | Op::Else(n)
        | Op::Loop(n)
        | Op::EndLoop(n)
        | Op::Break(n)
        | Op::BreakIf(n)
        | Op::IsStruct(n)
        | Op::Trap(n) => [0, i64::from(*n), 0, 0],
        Op::GetData(n) | Op::SetData(n) => [0, i64::from(*n), 0, 0],
        Op::Call(a, b) | Op::CallVerifiedNative(a, b) | Op::CallExternalNative(a, b) => {
            [0, i64::from(*a), i64::from(*b), 0]
        }
        Op::GetDataIndexed(a, b) | Op::SetDataIndexed(a, b) => [0, i64::from(*a), i64::from(*b), 0],
        Op::IsEnum(a, b, c) => [0, i64::from(*a), i64::from(*b), i64::from(*c)],
        Op::GetField(StructField::Flat { offset, kind })
        | Op::GetTupleField(TupleField::Flat { offset, kind })
        | Op::GetEnumField(EnumField::Flat { offset, kind }) => {
            [1, i64::from(*offset), i64::from(kind.to_tag()), 0]
        }
        Op::GetField(StructField::Boxed { name_const }) => [2, i64::from(*name_const), 0, 0],
        Op::GetTupleField(TupleField::Boxed { index })
        | Op::GetEnumField(EnumField::Boxed { index }) => [2, i64::from(*index), 0, 0],
        Op::GetField(StructField::FlatNested {
            offset,
            size,
            variant,
        })
        | Op::GetTupleField(TupleField::FlatNested {
            offset,
            size,
            variant,
        })
        | Op::GetEnumField(EnumField::FlatNested {
            offset,
            size,
            variant,
        }) => [
            3,
            i64::from(*offset),
            i64::from(*size),
            i64::from(variant.to_tag()),
        ],
        Op::GetIndex(ArrayElem::Flat { kind }) => [1, i64::from(kind.to_tag()), 0, 0],
        Op::GetIndex(ArrayElem::Boxed) => [2, 0, 0, 0],
        Op::GetIndex(ArrayElem::FlatNested { size, variant }) => {
            [3, i64::from(*size), 0, i64::from(variant.to_tag())]
        }
        Op::NewComposite(NewCompositeOperand::Flat {
            kind,
            count,
            byte_size,
        }) => [
            4,
            i64::from(kind.to_tag()),
            i64::from(*count),
            i64::from(*byte_size),
        ],
        Op::NewComposite(NewCompositeOperand::Boxed { kind, count, meta }) => [
            5,
            i64::from(kind.to_tag()),
            i64::from(*count),
            i64::from(*meta),
        ],
    };
    [
        i64::from(crate::wire_format::opcode_id_of(op).0),
        form,
        a,
        b,
        c,
    ]
}

/// Encode every instruction variant through the Keleusma wire stage.
///
/// The host supplies typed semantic fields and collects the returned records.
/// Keleusma selects inline or pooled representation, derives pool ordinals,
/// packs fields, writes sentinels and computes parity. No reference encoder runs.
///
/// # Errors
/// Refuses data-slot operands beyond 24 bits, pool-index overflow, allocation
/// failures, execution failures and unexpected stage output.
pub fn wire_opcode_sections_via_kel(module: &Module) -> Result<WireOpcodeSections, SelfHostError> {
    let error = |detail: &str| SelfHostError::Unsupported {
        detail: format!("wire opcodes: {detail}"),
    };
    let count = module
        .chunks
        .iter()
        .try_fold(0usize, |n, c| n.checked_add(c.ops.len()))
        .ok_or_else(|| error("instruction count overflow"))?;
    let len = count
        .checked_mul(4)
        .filter(|&n| u32::try_from(n).is_ok())
        .ok_or_else(|| error("instruction section exceeds u32"))?;
    let mut opcodes = Vec::new();
    opcodes
        .try_reserve_exact(len)
        .map_err(|_| error("instruction allocation failed"))?;
    let mut operands = Vec::new();
    let stage = compile_src(&read_stage("kel/wire.kel"));
    with_compiled_wire(stage, |vm, shared| {
        set_word(vm, shared, wire_slots::WARG, 0)?;
        if command(vm, shared, 193)? != 0 {
            return Err(error("unexpected stream start"));
        }
        for op in module.chunks.iter().flat_map(|c| &c.ops) {
            for (i, value) in fields(op).into_iter().enumerate() {
                set_word(vm, shared, wire_slots::FIN + i, value)?;
            }
            let wrote = command(vm, shared, 194)?;
            if !matches!(wrote, 4 | 12) {
                return Err(error("unexpected record length"));
            }
            let mut bytes = [0; 12];
            read_window(vm, shared, &mut bytes[..wrote])?;
            opcodes.extend_from_slice(&bytes[..4]);
            if wrote == 12 {
                operands
                    .try_reserve(8)
                    .map_err(|_| error("operand allocation failed"))?;
                operands.extend_from_slice(&bytes[4..]);
            }
        }
        Ok(WireOpcodeSections { opcodes, operands })
    })
}

/// Assemble an unsigned artifact through Keleusma without reference artifact bytes.
/// The auxiliary helper's documented record-form limits still apply. This is not
/// yet the shipping serializer. Auxiliary record-form limits still apply.
///
/// # Errors
/// Refuses signing or encryption flags and propagates section production,
/// framing, capacity and allocation errors.
pub fn wire_module_artifact_via_kel(module: &Module) -> Result<Vec<u8>, SelfHostError> {
    if module.flags
        & (crate::wire_format::FLAG_REQUIRES_SIGNATURE | crate::wire_format::FLAG_ENCRYPTED)
        != 0
    {
        return Err(SelfHostError::Unsupported {
            detail: "wire artifact: signing and encryption require their dedicated encoders".into(),
        });
    }
    let sections = wire_opcode_sections_via_kel(module)?;
    let auxiliary = wire_auxiliary_from_module_via_kel(module)?;
    wire_artifact_from_sections_via_kel(module, &sections.opcodes, &sections.operands, &auxiliary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire_format::{OpcodeId, OpcodeRecord, OperandPoolEntry, decode_op, encode_op};

    #[test]
    fn unsigned_module_encoder_refuses_security_extensions() {
        let mut module = compile_src("fn main() -> Word { 7 }");
        for flags in [2, 4, 6] {
            module.flags = flags;
            assert!(wire_module_artifact_via_kel(&module).is_err());
        }
        module.flags = 0;
        assert_eq!(
            wire_module_artifact_via_kel(&module).unwrap(),
            module.to_bytes().unwrap()
        );
        module.chunks.clear();
        let empty = wire_opcode_sections_via_kel(&module).unwrap();
        assert!(empty.opcodes.is_empty() && empty.operands.is_empty());
    }

    #[test]
    fn every_opcode_and_composite_form_matches_reference_bytes() {
        let mut ops = Vec::new();
        let table = crate::wire_format::opcode_table();
        assert_eq!(table.len(), 66);
        for &(_, id) in table {
            let pool = match id {
                5 | 6 => vec![OperandPoolEntry::from_u24_u24(0, 0)],
                43 => vec![OperandPoolEntry::from_u16_u16_u16(0, 0, 0)],
                _ => Vec::new(),
            };
            ops.push(
                decode_op(
                    OpcodeRecord::from_id_and_operand(OpcodeId(id), [0; 3]),
                    &pool,
                )
                .unwrap(),
            );
        }
        for n in [0, 1, 255, 256, 65535] {
            ops.extend([
                Op::Const(n),
                Op::GetLocal(n),
                Op::SetLocal(n),
                Op::BoundsCheck(n),
                Op::If(n),
                Op::Else(n),
                Op::Loop(n),
                Op::EndLoop(n),
                Op::Break(n),
                Op::BreakIf(n),
                Op::IsStruct(n),
                Op::Trap(n),
                Op::Call(n, 255),
                Op::CallVerifiedNative(n, 254),
                Op::CallExternalNative(n, 253),
                Op::IsEnum(n, 65535, n),
            ]);
        }
        for n in [0, 1, 127, 255] {
            ops.extend([
                Op::WordToFixed(n),
                Op::FixedToWord(n),
                Op::FixedMul(n),
                Op::FixedDiv(n),
                Op::CheckedMul(n),
                Op::CheckedDiv(n),
                Op::PushImmediate(n),
                Op::PopN(n),
                Op::GetTupleField(TupleField::Boxed { index: n }),
                Op::GetEnumField(EnumField::Boxed { index: n }),
            ]);
        }
        for n in [0, 1, 65535, 65536, 0xFFFFFF] {
            ops.extend([
                Op::GetData(n),
                Op::SetData(n),
                Op::GetDataIndexed(n, 0xFFFFFF),
                Op::SetDataIndexed(0xFFFFFF, n),
            ]);
        }
        for kind in (0..8).filter_map(ScalarKind::from_tag) {
            ops.extend([
                Op::GetField(StructField::Flat {
                    offset: 65535,
                    kind,
                }),
                Op::GetTupleField(TupleField::Flat { offset: 256, kind }),
                Op::GetEnumField(EnumField::Flat { offset: 255, kind }),
                Op::GetIndex(ArrayElem::Flat { kind }),
            ]);
        }
        ops.extend([
            Op::GetField(StructField::Boxed { name_const: 65535 }),
            Op::GetIndex(ArrayElem::Boxed),
        ]);
        for variant in (0..4).map(|n| CompositeKind::from_tag(n).unwrap()) {
            ops.extend([
                Op::GetField(StructField::FlatNested {
                    offset: 65535,
                    size: 65534,
                    variant,
                }),
                Op::GetTupleField(TupleField::FlatNested {
                    offset: 1,
                    size: 65535,
                    variant,
                }),
                Op::GetEnumField(EnumField::FlatNested {
                    offset: 256,
                    size: 257,
                    variant,
                }),
                Op::GetIndex(ArrayElem::FlatNested {
                    size: 65535,
                    variant,
                }),
            ]);
            for count in [0, 1, 62, 63, 255, 256, 65535] {
                ops.extend([
                    Op::NewComposite(NewCompositeOperand::Flat {
                        kind: variant,
                        count,
                        byte_size: 65535,
                    }),
                    Op::NewComposite(NewCompositeOperand::Boxed {
                        kind: variant,
                        count,
                        meta: 65534,
                    }),
                ]);
            }
        }
        let mut module = compile_src("fn main() -> Word { 7 }");
        module.chunks[0].ops = ops.clone();
        // A second chunk proves that the pool ordinal continues across chunks.
        module.chunks.push(module.chunks[0].clone());
        let actual = wire_opcode_sections_via_kel(&module).unwrap();
        let mut expected = Vec::new();
        let mut pool = Vec::new();
        for op in ops.iter().chain(&ops) {
            expected.extend_from_slice(&encode_op(op, &mut pool).unwrap().0);
        }
        assert_eq!(actual.opcodes, expected);
        assert_eq!(
            actual.operands,
            pool.iter().flat_map(|p| p.0).collect::<Vec<_>>()
        );
        for (bytes, op) in actual
            .opcodes
            .as_chunks::<4>()
            .0
            .iter()
            .zip(ops.iter().chain(&ops))
        {
            assert_eq!(
                &decode_op(OpcodeRecord::from_raw(*bytes), &pool).unwrap(),
                op
            );
        }
    }

    #[test]
    fn pool_boundaries_and_refusals_preserve_the_stream() {
        let stage = compile_src(&read_stage("kel/wire.kel"));
        with_compiled_wire(stage, |vm, shared| {
            fn raw(vm: &mut Vm<'_, '_>, shared: &mut [u8], row: [i64; 5]) -> i64 {
                for (i, value) in row.into_iter().enumerate() {
                    set_word(vm, shared, wire_slots::FIN + i, value).unwrap();
                }
                enter_wire(vm, shared, 194).unwrap()
            }
            fn begin(vm: &mut Vm<'_, '_>, shared: &mut [u8], count: i64) -> i64 {
                set_word(vm, shared, wire_slots::WARG, count).unwrap();
                enter_wire(vm, shared, 193).unwrap()
            }
            assert_eq!(raw(vm, shared, [8, 0, 0, 0, 0]), -271);
            for count in [-1, 0x1000001] {
                assert_eq!(begin(vm, shared, count), -270);
            }
            assert_eq!(begin(vm, shared, 65535), 0);
            assert_eq!(raw(vm, shared, [38, 3, 1, 2, 0]), 12);
            let mut last = [0; 12];
            read_window(vm, shared, &mut last)?;
            assert_eq!(&last[1..4], &[255, 255, 240]);
            for row in [
                [38, 3, 1, 2, 0],
                [69, 5, 0, 1, 0],
                [34, 0, 0, 0, 0],
                [0, 0, 65536, 0, 0],
                [3, 0, 0x1000000, 0, 0],
                [30, 0, 0, 256, 0],
                [38, 1, 0, 8, 0],
                [39, 2, 1, 0, 0],
                [69, 4, 4, 0, 0],
                [43, 0, -1, 0, 0],
                [8, 0, 1, 0, 0],
            ] {
                assert_eq!(raw(vm, shared, row), -271, "{row:?}");
                let mut unchanged = [0; 12];
                read_window(vm, shared, &mut unchanged)?;
                assert_eq!(unchanged, last);
            }
            assert_eq!(raw(vm, shared, [5, 0, 1, 2, 0]), 12);
            read_window(vm, shared, &mut last)?;
            assert_eq!(&last[1..4], &[0, 0, 1]);
            assert_eq!(begin(vm, shared, 0xFFFFFF), 0);
            assert_eq!(raw(vm, shared, [43, 0, 1, 2, 3]), 12);
            assert_eq!(raw(vm, shared, [5, 0, 1, 2, 0]), -271);
            assert_eq!(raw(vm, shared, [8, 0, 0, 0, 0]), 4);
            assert_eq!(begin(vm, shared, 0), 0);
            assert_eq!(raw(vm, shared, [5, 0, 1, 2, 0]), 12);
            read_window(vm, shared, &mut last)?;
            assert_eq!(&last[1..4], &[0, 0, 0]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn oversized_typed_data_operands_are_not_truncated() {
        let mut module = compile_src("fn main() -> Word { 7 }");
        for op in [
            Op::GetData(0x1000000),
            Op::SetData(u32::MAX),
            Op::GetDataIndexed(0, 0x1000000),
            Op::SetDataIndexed(u32::MAX, 0),
        ] {
            module.chunks[0].ops = vec![op];
            assert!(wire_opcode_sections_via_kel(&module).is_err());
        }
    }
}
