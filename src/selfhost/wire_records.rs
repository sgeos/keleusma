//! Fixed-record field decoding through Keleusma. Schema validation is separate.

use super::wire_decode::transport;
use super::wire_framing::{command, get_word, set_word, with_compiled_wire};
use super::*;
use crate::wire_schema as schema;

/// A fixed schema record whose fields were decoded by Keleusma.
///
/// Tags, names and ranges have not yet been validated against other tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireSchemaRecord {
    /// Decoded `NAMES` record.
    NameRef(schema::NameRef),
    /// Decoded `CONSTS` record.
    ConstRecord(schema::ConstRecord),
    /// Decoded `STRUCT_AUX` record.
    StructAux(schema::StructAux),
    /// Decoded `ENUM_AUX` record.
    EnumAux(schema::EnumAux),
    /// Decoded `SHAPES` record.
    ShapeRecord(schema::ShapeRecord),
    /// Decoded `SIGNATURES` record.
    SignatureRecord(schema::SignatureRecord),
    /// Decoded `STRUCT_TEMPLATES` record.
    StructTemplateRecord(schema::StructTemplateRecord),
    /// Decoded `ENUM_VARIANTS` record.
    EnumVariantRecord(schema::EnumVariantRecord),
    /// Decoded `ENUM_LAYOUTS` record.
    EnumLayoutRecord(schema::EnumLayoutRecord),
    /// Decoded `DATA_SLOTS` record.
    DataSlotRecord(schema::DataSlotRecord),
    /// Decoded `SHARED_LAYOUT` record.
    SharedSlotRecord(schema::SharedSlotRecord),
    /// Decoded `PRIVATE_COMPOSITE` record.
    PrivateCompositeRecord(schema::PrivateCompositeRecord),
    /// Decoded `DATA_INIT` record.
    DataInitRecord(schema::DataInitRecord),
    /// Decoded `CHUNKS` record.
    ChunkRecord(schema::ChunkRecord),
    /// Decoded `NATIVES` record.
    NativeRecord(schema::NativeRecord),
    /// Decoded `HEADER` record.
    HeaderRecord(schema::HeaderRecord),
    /// Decoded `NATIVE_RETURNS` record.
    NativeReturnRecord(schema::NativeReturnRecord),
}

impl WireSchemaRecord {
    /// The schema region kind associated with this record.
    pub fn kind(&self) -> u16 {
        match self {
            Self::NameRef(_) => schema::kind::NAMES,
            Self::ConstRecord(_) => schema::kind::CONSTS,
            Self::StructAux(_) => schema::kind::STRUCT_AUX,
            Self::EnumAux(_) => schema::kind::ENUM_AUX,
            Self::ShapeRecord(_) => schema::kind::SHAPES,
            Self::SignatureRecord(_) => schema::kind::SIGNATURES,
            Self::StructTemplateRecord(_) => schema::kind::STRUCT_TEMPLATES,
            Self::EnumVariantRecord(_) => schema::kind::ENUM_VARIANTS,
            Self::EnumLayoutRecord(_) => schema::kind::ENUM_LAYOUTS,
            Self::DataSlotRecord(_) => schema::kind::DATA_SLOTS,
            Self::SharedSlotRecord(_) => schema::kind::SHARED_LAYOUT,
            Self::PrivateCompositeRecord(_) => schema::kind::PRIVATE_COMPOSITE,
            Self::DataInitRecord(_) => schema::kind::DATA_INIT,
            Self::ChunkRecord(_) => schema::kind::CHUNKS,
            Self::NativeRecord(_) => schema::kind::NATIVES,
            Self::HeaderRecord(_) => schema::kind::HEADER,
            Self::NativeReturnRecord(_) => schema::kind::NATIVE_RETURNS,
        }
    }
}

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire records: {detail}"),
    }
}

fn byte(n: i64) -> Result<u8, SelfHostError> {
    u8::try_from(n).map_err(|_| error("byte field out of range"))
}
fn short(n: i64) -> Result<u16, SelfHostError> {
    u16::try_from(n).map_err(|_| error("short field out of range"))
}
fn word(n: i64) -> Result<u32, SelfHostError> {
    u32::try_from(n).map_err(|_| error("word field out of range"))
}

// Constructs typed fields, without byte offsets or endian operations.
// The u64 payload uses all bits of its signed transport word.
fn materialize(kind: u16, fields: &[i64]) -> Result<WireSchemaRecord, SelfHostError> {
    Ok(match (kind, fields) {
        (schema::kind::NAMES, [offset, length]) => WireSchemaRecord::NameRef(schema::NameRef {
            offset: word(*offset)?,
            length: word(*length)?,
        }),
        (schema::kind::CONSTS, [tag, flags, aux, payload]) => {
            WireSchemaRecord::ConstRecord(schema::ConstRecord {
                tag: short(*tag)?,
                flags: short(*flags)?,
                aux: word(*aux)?,
                payload: *payload as u64,
            })
        }
        (schema::kind::STRUCT_AUX, [type_name, field_names_first]) => {
            WireSchemaRecord::StructAux(schema::StructAux {
                type_name: word(*type_name)?,
                field_names_first: word(*field_names_first)?,
            })
        }
        (schema::kind::ENUM_AUX, [type_name, variant, discriminant]) => {
            WireSchemaRecord::EnumAux(schema::EnumAux {
                type_name: word(*type_name)?,
                variant: word(*variant)?,
                discriminant: *discriminant,
            })
        }
        (schema::kind::SHAPES, [tag, kind, reserved, size]) => {
            WireSchemaRecord::ShapeRecord(schema::ShapeRecord {
                tag: short(*tag)?,
                kind: byte(*kind)?,
                reserved: byte(*reserved)?,
                size: word(*size)?,
            })
        }
        (schema::kind::SIGNATURES, [params_first, params_count, ret, resume]) => {
            WireSchemaRecord::SignatureRecord(schema::SignatureRecord {
                params_first: word(*params_first)?,
                params_count: word(*params_count)?,
                ret: word(*ret)?,
                resume: word(*resume)?,
            })
        }
        (schema::kind::STRUCT_TEMPLATES, [type_name, field_names_first, field_count, reserved]) => {
            WireSchemaRecord::StructTemplateRecord(schema::StructTemplateRecord {
                type_name: word(*type_name)?,
                field_names_first: word(*field_names_first)?,
                field_count: word(*field_count)?,
                reserved: word(*reserved)?,
            })
        }
        (schema::kind::ENUM_VARIANTS, [name, reserved, disc]) => {
            WireSchemaRecord::EnumVariantRecord(schema::EnumVariantRecord {
                name: word(*name)?,
                reserved: word(*reserved)?,
                disc: *disc,
            })
        }
        (schema::kind::ENUM_LAYOUTS, [type_name, variants_first, variants_count, min_payload]) => {
            WireSchemaRecord::EnumLayoutRecord(schema::EnumLayoutRecord {
                type_name: word(*type_name)?,
                variants_first: word(*variants_first)?,
                variants_count: word(*variants_count)?,
                min_payload: word(*min_payload)?,
            })
        }
        (schema::kind::DATA_SLOTS, [name, visibility, reserved, run]) => {
            WireSchemaRecord::DataSlotRecord(schema::DataSlotRecord {
                name: word(*name)?,
                visibility: byte(*visibility)?,
                reserved: byte(*reserved)?,
                run: short(*run)?,
            })
        }
        (schema::kind::SHARED_LAYOUT, [offset, kind, reserved, len, first_slot, run, stride]) => {
            WireSchemaRecord::SharedSlotRecord(schema::SharedSlotRecord {
                offset: word(*offset)?,
                kind: byte(*kind)?,
                reserved: byte(*reserved)?,
                len: short(*len)?,
                first_slot: word(*first_slot)?,
                run: short(*run)?,
                stride: short(*stride)?,
            })
        }
        (schema::kind::PRIVATE_COMPOSITE, [slot, reserved, offset]) => {
            WireSchemaRecord::PrivateCompositeRecord(schema::PrivateCompositeRecord {
                slot: short(*slot)?,
                reserved: short(*reserved)?,
                offset: word(*offset)?,
            })
        }
        (schema::kind::DATA_INIT, [first, count]) => {
            WireSchemaRecord::DataInitRecord(schema::DataInitRecord {
                first: word(*first)?,
                count: word(*count)?,
            })
        }
        (
            schema::kind::CHUNKS,
            [
                name,
                consts_first,
                consts_count,
                templates_first,
                templates_count,
                param_types_first,
                param_types_count,
                debug_first,
                debug_len,
                op_byte_offset,
                op_record_count,
                local_count,
                param_count,
                block_type,
            ],
        ) => WireSchemaRecord::ChunkRecord(schema::ChunkRecord {
            name: word(*name)?,
            consts_first: word(*consts_first)?,
            consts_count: word(*consts_count)?,
            templates_first: word(*templates_first)?,
            templates_count: word(*templates_count)?,
            param_types_first: word(*param_types_first)?,
            param_types_count: word(*param_types_count)?,
            debug_first: word(*debug_first)?,
            debug_len: word(*debug_len)?,
            op_byte_offset: word(*op_byte_offset)?,
            op_record_count: word(*op_record_count)?,
            local_count: short(*local_count)?,
            param_count: byte(*param_count)?,
            block_type: byte(*block_type)?,
        }),
        (schema::kind::NATIVES, [name, reserved]) => {
            WireSchemaRecord::NativeRecord(schema::NativeRecord {
                name: word(*name)?,
                reserved: word(*reserved)?,
            })
        }
        (
            schema::kind::HEADER,
            [
                entry_point,
                word_bits_log2,
                addr_bits_log2,
                float_bits_log2,
                flags,
                wcet_cycles,
                wcmu_bytes,
                shared_data_bytes,
                private_data_bytes,
                schema_hash,
                reserved,
            ],
        ) => WireSchemaRecord::HeaderRecord(schema::HeaderRecord {
            entry_point: word(*entry_point)?,
            word_bits_log2: byte(*word_bits_log2)?,
            addr_bits_log2: byte(*addr_bits_log2)?,
            float_bits_log2: byte(*float_bits_log2)?,
            flags: byte(*flags)?,
            wcet_cycles: word(*wcet_cycles)?,
            wcmu_bytes: word(*wcmu_bytes)?,
            shared_data_bytes: word(*shared_data_bytes)?,
            private_data_bytes: word(*private_data_bytes)?,
            schema_hash: word(*schema_hash)?,
            reserved: word(*reserved)?,
        }),
        (schema::kind::NATIVE_RETURNS, [shape, reserved]) => {
            WireSchemaRecord::NativeReturnRecord(schema::NativeReturnRecord {
                shape: word(*shape)?,
                reserved: word(*reserved)?,
            })
        }
        _ => return Err(error("unexpected record kind or field count")),
    })
}

fn decode_records(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    kind: u16,
    bytes: &[u8],
) -> Result<Vec<WireSchemaRecord>, SelfHostError> {
    set_word(vm, shared, wire_slots::WARG, i64::from(kind))?;
    set_word(
        vm,
        shared,
        wire_slots::WARG + 1,
        i64::try_from(bytes.len()).map_err(|_| error("table length exceeds word range"))?,
    )?;
    let stride = command(vm, shared, 218)?;
    if stride == 0 || stride > 65536 {
        return Err(error("invalid record window"));
    }
    let count = get_word(vm, shared, wire_slots::FIN)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| error("record allocation failed"))?;
    for record in bytes.chunks_exact(stride) {
        transport(vm, shared, 0, record)?;
        set_word(vm, shared, wire_slots::WARG + 1, record.len() as i64)?;
        let fields_count = command(vm, shared, 219)?;
        if fields_count == 0 || fields_count > 14 {
            return Err(error("invalid field count"));
        }
        let mut fields = [0; 14];
        for (i, field) in fields.iter_mut().take(fields_count).enumerate() {
            *field = match vm
                .get_shared(shared, wire_slots::FIN + i)
                .map_err(|_| error("record field transport failed"))?
            {
                Value::Int(n) => n,
                _ => return Err(error("non-integer record field")),
            };
        }
        if result.len() >= count {
            return Err(error("record count exceeded"));
        }
        result.push(materialize(kind, &fields[..fields_count])?);
    }
    if result.len() != count || command(vm, shared, 219)? != 0 {
        return Err(error("record stream did not finish at the declared count"));
    }
    Ok(result)
}

/// Decode one fixed-record schema table through Keleusma.
///
/// The stage selects the stride, checks divisibility and unpacks every field.
/// The host supplies only the region kind and raw table bytes, transports one
/// record at a time and constructs the corresponding typed records. Byte pools
/// and unknown region kinds are refused. Reserved fields and unknown tags are
/// retained as the reference record reader retains them. Semantic tag checks,
/// cross-table validation and constant-tree reconstruction remain separate.
///
/// # Errors
/// Refuses unsupported kinds, malformed table lengths, allocation failures and
/// invalid stage responses. No table-size limit is imposed by the byte window.
pub fn wire_schema_records_via_kel(
    kind: u16,
    bytes: &[u8],
) -> Result<Vec<WireSchemaRecord>, SelfHostError> {
    with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
        decode_records(vm, shared, kind, bytes)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use keleusma_wire::WireRecord;

    fn reference(kind: u16, bytes: &[u8]) -> Option<WireSchemaRecord> {
        Some(match kind {
            schema::kind::NAMES => WireSchemaRecord::NameRef(schema::NameRef::read_record(bytes)?),
            schema::kind::CONSTS => {
                WireSchemaRecord::ConstRecord(schema::ConstRecord::read_record(bytes)?)
            }
            schema::kind::STRUCT_AUX => {
                WireSchemaRecord::StructAux(schema::StructAux::read_record(bytes)?)
            }
            schema::kind::ENUM_AUX => {
                WireSchemaRecord::EnumAux(schema::EnumAux::read_record(bytes)?)
            }
            schema::kind::SHAPES => {
                WireSchemaRecord::ShapeRecord(schema::ShapeRecord::read_record(bytes)?)
            }
            schema::kind::SIGNATURES => {
                WireSchemaRecord::SignatureRecord(schema::SignatureRecord::read_record(bytes)?)
            }
            schema::kind::STRUCT_TEMPLATES => WireSchemaRecord::StructTemplateRecord(
                schema::StructTemplateRecord::read_record(bytes)?,
            ),
            schema::kind::ENUM_VARIANTS => {
                WireSchemaRecord::EnumVariantRecord(schema::EnumVariantRecord::read_record(bytes)?)
            }
            schema::kind::ENUM_LAYOUTS => {
                WireSchemaRecord::EnumLayoutRecord(schema::EnumLayoutRecord::read_record(bytes)?)
            }
            schema::kind::DATA_SLOTS => {
                WireSchemaRecord::DataSlotRecord(schema::DataSlotRecord::read_record(bytes)?)
            }
            schema::kind::SHARED_LAYOUT => {
                WireSchemaRecord::SharedSlotRecord(schema::SharedSlotRecord::read_record(bytes)?)
            }
            schema::kind::PRIVATE_COMPOSITE => WireSchemaRecord::PrivateCompositeRecord(
                schema::PrivateCompositeRecord::read_record(bytes)?,
            ),
            schema::kind::DATA_INIT => {
                WireSchemaRecord::DataInitRecord(schema::DataInitRecord::read_record(bytes)?)
            }
            schema::kind::CHUNKS => {
                WireSchemaRecord::ChunkRecord(schema::ChunkRecord::read_record(bytes)?)
            }
            schema::kind::NATIVES => {
                WireSchemaRecord::NativeRecord(schema::NativeRecord::read_record(bytes)?)
            }
            schema::kind::HEADER => {
                WireSchemaRecord::HeaderRecord(schema::HeaderRecord::read_record(bytes)?)
            }
            schema::kind::NATIVE_RETURNS => WireSchemaRecord::NativeReturnRecord(
                schema::NativeReturnRecord::read_record(bytes)?,
            ),
            _ => return None,
        })
    }

    fn stride(kind: u16) -> Option<usize> {
        Some(match kind {
            schema::kind::NAMES => schema::NameRef::STRIDE,
            schema::kind::CONSTS => schema::ConstRecord::STRIDE,
            schema::kind::STRUCT_AUX => schema::StructAux::STRIDE,
            schema::kind::ENUM_AUX => schema::EnumAux::STRIDE,
            schema::kind::SHAPES => schema::ShapeRecord::STRIDE,
            schema::kind::SIGNATURES => schema::SignatureRecord::STRIDE,
            schema::kind::STRUCT_TEMPLATES => schema::StructTemplateRecord::STRIDE,
            schema::kind::ENUM_VARIANTS => schema::EnumVariantRecord::STRIDE,
            schema::kind::ENUM_LAYOUTS => schema::EnumLayoutRecord::STRIDE,
            schema::kind::DATA_SLOTS => schema::DataSlotRecord::STRIDE,
            schema::kind::SHARED_LAYOUT => schema::SharedSlotRecord::STRIDE,
            schema::kind::PRIVATE_COMPOSITE => schema::PrivateCompositeRecord::STRIDE,
            schema::kind::DATA_INIT => schema::DataInitRecord::STRIDE,
            schema::kind::CHUNKS => schema::ChunkRecord::STRIDE,
            schema::kind::NATIVES => schema::NativeRecord::STRIDE,
            schema::kind::HEADER => schema::HeaderRecord::STRIDE,
            schema::kind::NATIVE_RETURNS => schema::NativeReturnRecord::STRIDE,
            _ => return None,
        })
    }

    #[test]
    fn all_u16_identifiers_obey_the_seventeen_record_kind_contract() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            let mut accepted = 0;
            for kind in 0..=u16::MAX {
                set_word(vm, shared, wire_slots::WARG, i64::from(kind))?;
                set_word(vm, shared, wire_slots::WARG + 1, 0)?;
                let actual = command(vm, shared, 218);
                match (actual, stride(kind)) {
                    (Ok(actual), Some(expected)) => {
                        assert_eq!(actual, expected, "kind {kind}");
                        assert_eq!(command(vm, shared, 219)?, 0);
                        accepted += 1;
                    }
                    (Err(_), None) => {}
                    (a, b) => panic!("kind {kind}: {a:?} versus {b:?}"),
                }
            }
            assert_eq!(accepted, 17);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn every_record_field_matches_derived_reader_for_full_width_patterns() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            let mut compared = 0;
            for kind in 0x10..=0x23 {
                let Some(width) = stride(kind) else { continue };
                let mut bytes = Vec::new();
                for seed in 0..=255u8 {
                    for at in 0..width {
                        bytes.push(seed.wrapping_add((at as u8).wrapping_mul(37)));
                    }
                }
                for value in [0, 255] {
                    bytes.extend(core::iter::repeat_n(value, width));
                }
                let decoded = decode_records(vm, shared, kind, &bytes)?;
                assert_eq!(decoded.len(), 258);
                for (actual, raw) in decoded.iter().zip(bytes.chunks_exact(width)) {
                    assert_eq!(
                        actual,
                        &reference(kind, raw).unwrap(),
                        "kind {kind}, bytes {raw:?}"
                    );
                    assert_eq!(actual.kind(), kind);
                    compared += 1;
                }
                for len in [1, width - 1, width + 1] {
                    assert!(decode_records(vm, shared, kind, &bytes[..len]).is_err());
                }
            }
            assert_eq!(compared, 17 * 258);
            std::eprintln!("compared {compared} fixed schema records");
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn record_retry_restart_and_completion_preserve_protocol_state() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            assert!(command(vm, shared, 219).is_err());
            set_word(
                vm,
                shared,
                wire_slots::WARG,
                i64::from(schema::kind::CHUNKS),
            )?;
            set_word(vm, shared, wire_slots::WARG + 1, 96)?;
            assert_eq!(command(vm, shared, 218)?, 48);
            assert_eq!(get_word(vm, shared, wire_slots::FIN)?, 2);
            for cmd in [0, 2, 188, 195, 203, 209, 214, 215, 216, 217] {
                assert!(command(vm, shared, cmd).is_err(), "command {cmd}");
            }
            let record: Vec<_> = (0..48).collect();
            transport(vm, shared, 0, &record)?;
            for length in [-1, 0, 47, 49, 65537] {
                set_word(vm, shared, wire_slots::WARG + 1, length)?;
                assert!(command(vm, shared, 219).is_err());
            }
            set_word(vm, shared, wire_slots::WARG + 1, 48)?;
            assert_eq!(command(vm, shared, 219)?, 14);
            assert_eq!(command(vm, shared, 219)?, 14);
            assert_eq!(command(vm, shared, 219)?, 0);
            assert!(command(vm, shared, 219).is_err());
            set_word(vm, shared, wire_slots::WARG + 1, 48)?;
            assert_eq!(command(vm, shared, 218)?, 48);
            set_word(vm, shared, wire_slots::WARG + 1, -1)?;
            assert!(command(vm, shared, 218).is_err());
            assert!(command(vm, shared, 219).is_err());
            assert_eq!(
                decode_records(vm, shared, schema::kind::CHUNKS, &record)?.len(),
                1
            );
            Ok(())
        })
        .unwrap();
    }
}
