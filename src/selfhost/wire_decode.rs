//! Opcode semantics are decoded in Keleusma; Rust constructs typed values.

use super::wire_framing::{command, set_word, with_compiled_wire};
use super::*;

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire decode: {detail}"),
    }
}

fn byte(value: i64) -> Result<u8, SelfHostError> {
    u8::try_from(value).map_err(|_| error("semantic byte out of range"))
}
fn short(value: i64) -> Result<u16, SelfHostError> {
    u16::try_from(value).map_err(|_| error("semantic short out of range"))
}
fn wide(value: i64) -> Result<u32, SelfHostError> {
    u32::try_from(value).map_err(|_| error("semantic word out of range"))
}
fn scalar(value: i64) -> Result<ScalarKind, SelfHostError> {
    ScalarKind::from_tag(byte(value)?).ok_or_else(|| error("unavailable scalar kind"))
}
fn composite(value: i64) -> Result<CompositeKind, SelfHostError> {
    CompositeKind::from_tag(byte(value)?).ok_or_else(|| error("unknown composite kind"))
}

// No byte masks, sentinels, pool positions or endian operations belong here.
fn materialize([id, form, a, b, c]: [i64; 5]) -> Result<Op, SelfHostError> {
    if !matches!(id, 38..=41 | 69) && form != 0 {
        return Err(error("unexpected semantic form"));
    }
    Ok(match id {
        0 => Op::Const(short(a)?),
        1 => Op::GetLocal(short(a)?),
        2 => Op::SetLocal(short(a)?),
        3 => Op::GetData(wide(a)?),
        4 => Op::SetData(wide(a)?),
        5 => Op::GetDataIndexed(wide(a)?, wide(b)?),
        6 => Op::SetDataIndexed(wide(a)?, wide(b)?),
        7 => Op::BoundsCheck(short(a)?),
        8 => Op::Add,
        9 => Op::Sub,
        10 => Op::Mul,
        11 => Op::Div,
        12 => Op::Mod,
        13 => Op::Neg,
        14 => Op::CmpEq,
        15 => Op::CmpNe,
        16 => Op::CmpLt,
        17 => Op::CmpGt,
        18 => Op::CmpLe,
        19 => Op::CmpGe,
        20 => Op::Not,
        21 => Op::If(short(a)?),
        22 => Op::Else(short(a)?),
        23 => Op::EndIf,
        24 => Op::Loop(short(a)?),
        25 => Op::EndLoop(short(a)?),
        26 => Op::Break(short(a)?),
        27 => Op::BreakIf(short(a)?),
        28 => Op::Stream,
        29 => Op::Reset,
        30 => Op::Call(short(a)?, byte(b)?),
        31 => Op::Return,
        32 => Op::Yield,
        33 => Op::Dup,
        38 => Op::GetField(match form {
            1 => StructField::Flat {
                offset: short(a)?,
                kind: scalar(b)?,
            },
            2 => StructField::Boxed {
                name_const: short(a)?,
            },
            3 => StructField::FlatNested {
                offset: short(a)?,
                size: short(b)?,
                variant: composite(c)?,
            },
            _ => return Err(error("invalid struct access form")),
        }),
        39 => Op::GetIndex(match form {
            1 => ArrayElem::Flat { kind: scalar(a)? },
            2 => ArrayElem::Boxed,
            3 => ArrayElem::FlatNested {
                size: short(a)?,
                variant: composite(c)?,
            },
            _ => return Err(error("invalid array access form")),
        }),
        40 => Op::GetTupleField(match form {
            1 => TupleField::Flat {
                offset: short(a)?,
                kind: scalar(b)?,
            },
            2 => TupleField::Boxed { index: byte(a)? },
            3 => TupleField::FlatNested {
                offset: short(a)?,
                size: short(b)?,
                variant: composite(c)?,
            },
            _ => return Err(error("invalid tuple access form")),
        }),
        41 => Op::GetEnumField(match form {
            1 => EnumField::Flat {
                offset: short(a)?,
                kind: scalar(b)?,
            },
            2 => EnumField::Boxed { index: byte(a)? },
            3 => EnumField::FlatNested {
                offset: short(a)?,
                size: short(b)?,
                variant: composite(c)?,
            },
            _ => return Err(error("invalid enum access form")),
        }),
        42 => Op::Len,
        43 => Op::IsEnum(short(a)?, short(b)?, short(c)?),
        44 => Op::IsStruct(short(a)?),
        45 => Op::IntToFloat,
        46 => Op::FloatToInt,
        47 => Op::WordToByte,
        48 => Op::ByteToWord,
        49 => Op::WordToFixed(byte(a)?),
        50 => Op::FixedToWord(byte(a)?),
        51 => Op::FixedMul(byte(a)?),
        52 => Op::FixedDiv(byte(a)?),
        53 => Op::Trap(short(a)?),
        54 => Op::CheckedAdd,
        55 => Op::CheckedSub,
        56 => Op::CheckedMul(byte(a)?),
        57 => Op::CheckedNeg,
        58 => Op::CheckedDiv(byte(a)?),
        59 => Op::CheckedMod,
        60 => Op::PushImmediate(byte(a)?),
        61 => Op::PopN(byte(a)?),
        62 => Op::BitAnd,
        63 => Op::BitOr,
        64 => Op::BitXor,
        65 => Op::Shl,
        66 => Op::Shr,
        67 => Op::CallVerifiedNative(short(a)?, byte(b)?),
        68 => Op::CallExternalNative(short(a)?, byte(b)?),
        69 => Op::NewComposite(match form {
            4 => NewCompositeOperand::Flat {
                kind: composite(a)?,
                count: short(b)?,
                byte_size: short(c)?,
            },
            5 => NewCompositeOperand::Boxed {
                kind: composite(a)?,
                count: short(b)?,
                meta: short(c)?,
            },
            _ => return Err(error("invalid construction form")),
        }),
        _ => return Err(error("unknown semantic opcode identifier")),
    })
}

pub(super) fn transport(
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    offset: usize,
    bytes: &[u8],
) -> Result<(), SelfHostError> {
    if offset.checked_add(bytes.len()).is_none_or(|n| n > 65536) {
        return Err(error("transport window overflow"));
    }
    for (i, &value) in bytes.iter().enumerate() {
        vm.set_shared(shared, wire_slots::BYTES + offset + i, Value::Byte(value))
            .map_err(|_| error("byte transport failed"))?;
    }
    Ok(())
}

pub(super) fn decode_one(
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    record: &[u8; 4],
    operands: &[u8],
    pools: u32,
) -> Result<Op, SelfHostError> {
    transport(vm, shared, 0, record)?;
    set_word(vm, shared, wire_slots::WARG, i64::from(pools))?;
    set_word(
        vm,
        shared,
        wire_slots::WARG + 1,
        i64::from(cfg!(feature = "floats")),
    )?;
    match command(vm, shared, 214)? {
        0 => {}
        1 => {
            let index = match vm
                .get_shared(shared, wire_slots::FIN)
                .map_err(|_| error("pool request read failed"))?
            {
                Value::Int(n) => usize::try_from(n).map_err(|_| error("negative pool request"))?,
                _ => return Err(error("non-integer pool request")),
            };
            let entry = operands
                .as_chunks::<8>()
                .0
                .get(index)
                .ok_or_else(|| error("pool request out of bounds"))?;
            transport(vm, shared, 4, entry)?;
            if command(vm, shared, 215)? != 0 {
                return Err(error("unexpected pool decode result"));
            }
        }
        _ => return Err(error("unknown decode request")),
    }
    let mut fields = [0; 5];
    for (i, field) in fields.iter_mut().enumerate() {
        *field = match vm
            .get_shared(shared, wire_slots::FIN + i)
            .map_err(|_| error("semantic response read failed"))?
        {
            Value::Int(n) => n,
            _ => return Err(error("non-integer semantic response")),
        };
    }
    materialize(fields)
}

/// Decode instruction sections through the Keleusma wire stage.
///
/// Keleusma checks opcode and pool parity, selects operand representations,
/// validates used pool tags and indices, and returns semantic fields. All pool
/// entries are parity-checked, including unused entries, as in the reference.
/// Rust performs bounded byte transport and constructs typed instructions.
/// This is section decoding, not a complete module decoder.
///
/// # Errors
/// Rejects malformed section lengths, corrupt records, unknown identifiers,
/// invalid or feature-disabled kinds, bad pool references and stage failures.
pub fn wire_decode_opcode_sections_via_kel(
    opcodes: &[u8],
    operands: &[u8],
) -> Result<Vec<Op>, SelfHostError> {
    if !opcodes.len().is_multiple_of(4) || !operands.len().is_multiple_of(8) {
        return Err(error("misaligned opcode or operand section"));
    }
    let pools =
        u32::try_from(operands.len() / 8).map_err(|_| error("operand pool count exceeds u32"))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(opcodes.len() / 4)
        .map_err(|_| error("instruction allocation failed"))?;
    with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
        for entry in operands.as_chunks::<8>().0 {
            transport(vm, shared, 0, entry)?;
            set_word(vm, shared, wire_slots::WARG, 0)?;
            if command(vm, shared, 95)? != 1 {
                return Err(error("operand pool parity mismatch"));
            }
        }
        for record in opcodes.as_chunks::<4>().0 {
            result.push(decode_one(vm, shared, record, operands, pools)?);
        }
        Ok(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire_format::{OpcodeId, OpcodeRecord, OperandPoolEntry, decode_op, opcode_table};

    #[test]
    fn decoder_inventory_covers_every_canonical_identifier() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            assert_eq!(opcode_table().len(), 66);
            for &(name, id) in opcode_table() {
                let pool = match id {
                    5 | 6 => vec![OperandPoolEntry::from_u24_u24(0, 0)],
                    43 => vec![OperandPoolEntry::from_u16_u16_u16(0, 0, 0)],
                    _ => vec![],
                };
                let bytes: Vec<_> = pool.iter().flat_map(|e| e.0).collect();
                let record = OpcodeRecord::from_id_and_operand(OpcodeId(id), [0; 3]);
                assert_eq!(
                    decode_one(vm, shared, &record.0, &bytes, pool.len() as u32).unwrap(),
                    decode_op(record, &pool).unwrap(),
                    "{name}"
                );
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn byte_level_forms_and_failures_match_the_reference_decoder() {
        let entries = [
            OperandPoolEntry::from_u16_u16(65535, 257),
            OperandPoolEntry::from_u16_u16_u8(65535, 513, 1),
            OperandPoolEntry::from_u16_u16_u8(63, 99, 2),
            OperandPoolEntry::from_u16_u16_u16(1, 256, 65535),
            OperandPoolEntry::from_u24_u24(0xFFFFFF, 0x123456),
        ];
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            let mut accepted = 0usize;
            let mut refused = 0usize;
            for id in [0, 3, 5, 6, 8, 30, 34, 38, 39, 40, 41, 43, 49, 69, 127] {
                for high in 0..=255 {
                    for (pool_index, pool) in entries.iter().enumerate() {
                        for low in [0, 3, 5, 255] {
                            for middle in [0, 255] {
                                let operand = [low, middle, high];
                                let record = OpcodeRecord::from_id_and_operand(OpcodeId(id), operand);
                                let expected = decode_op(record, &[*pool]);
                                let actual = decode_one(vm, shared, &record.0, &pool.0, 1);
                                match (actual, expected) {
                                    (Ok(a), Ok(b)) => {
                                        assert_eq!(a, b, "id {id}, {operand:?}, pool {pool_index}");
                                        accepted += 1;
                                    }
                                    (Err(_), Err(_)) => refused += 1,
                                    (a, b) => panic!(
                                        "id {id}, {operand:?}, pool {pool_index}: {a:?} versus {b:?}"
                                    ),
                                }
                            }
                        }
                    }
                }
            }
            std::eprintln!("raw opcode comparisons accepted {accepted}, refused {refused}");
            assert!(accepted > 1000 && refused > 1000);
            let record = OpcodeRecord::from_id_and_operand(OpcodeId(30), [1, 2, 3]);
            for bit in 0..32 {
                let mut bytes = record.0;
                bytes[bit / 8] ^= 1 << (bit % 8);
                assert!(decode_one(vm, shared, &bytes, &[], 0).is_err());
            }
            let record = OpcodeRecord::from_id_and_operand(OpcodeId(38), [0, 0, 240]);
            for bit in 0..64 {
                let mut bytes = entries[0].0;
                bytes[bit / 8] ^= 1 << (bit % 8);
                assert!(decode_one(vm, shared, &record.0, &bytes, 1).is_err());
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn pool_request_widths_and_failed_retries_preserve_protocol_state() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            assert!(command(vm, shared, 215).is_err());
            for (id, operand, count, expected) in [
                (5, [255, 255, 255], 0x1000000, 0xFFFFFF),
                (38, [255, 255, 240], 65536, 65535),
                (69, [255, 255, 63], 65536, 65535),
            ] {
                let record = OpcodeRecord::from_id_and_operand(OpcodeId(id), operand);
                transport(vm, shared, 0, &record.0)?;
                set_word(vm, shared, wire_slots::WARG, count)?;
                set_word(vm, shared, wire_slots::WARG + 1, 1)?;
                assert_eq!(command(vm, shared, 214)?, 1);
                assert!(matches!(vm.get_shared(shared, wire_slots::FIN).unwrap(), Value::Int(n) if n == expected));
                set_word(vm, shared, wire_slots::WARG, count - 1)?;
                assert!(command(vm, shared, 214).is_err());
                assert!(command(vm, shared, 215).is_err());
            }
            let record = OpcodeRecord::from_id_and_operand(OpcodeId(5), [0; 3]);
            transport(vm, shared, 0, &record.0)?;
            set_word(vm, shared, wire_slots::WARG, 1)?;
            assert_eq!(command(vm, shared, 214)?, 1);
            transport(vm, shared, 4, &OperandPoolEntry::from_u16_u16(1, 2).0)?;
            assert!(command(vm, shared, 215).is_err());
            transport(vm, shared, 4, &OperandPoolEntry::from_u24_u24(3, 4).0)?;
            assert_eq!(command(vm, shared, 215)?, 0);
            assert!(command(vm, shared, 215).is_err());
            Ok(())
        }).unwrap();
    }
}
