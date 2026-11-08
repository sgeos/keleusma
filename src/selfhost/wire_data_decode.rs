//! Data-layout decoding directed by Keleusma, over the field records of Encoding F.

use super::wire_directory::decode_directory;
use super::wire_framing::{set_word, with_compiled_wire};
use super::wire_records::decode_records;
use super::wire_semantics::{ARG, NameTables, byte_count, command, fields, name_lookup};
use super::*;
use crate::bytecode::{DataField, DataInit, DataLayout, SlotVisibility, data_kind_zero};
use crate::wire_schema::{ABSENT, DataFieldRecord, init_form, kind};

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire data layout: {detail}"),
    }
}
fn copy_name(name: &str) -> Result<String, SelfHostError> {
    let mut result = String::new();
    result
        .try_reserve_exact(name.len())
        .map_err(|_| error("field name allocation failed"))?;
    result.push_str(name);
    Ok(result)
}
fn expect(
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    cmd: i64,
    expected: usize,
) -> Result<(), SelfHostError> {
    if command(vm, shared, cmd)? != expected {
        return Err(error("unexpected data protocol response"));
    }
    Ok(())
}

/// Whether `kind` is a kind tag this build can represent, the reference decoder's test.
fn kind_known(kind: u8) -> bool {
    use crate::bytecode::SHARED_SLOT_COMPOSITE_FLAG;
    use crate::value_layout::{CompositeKind, ScalarKind};
    if kind & SHARED_SLOT_COMPOSITE_FLAG != 0 {
        CompositeKind::from_tag(kind & !SHARED_SLOT_COMPOSITE_FLAG).is_some()
    } else {
        ScalarKind::from_tag(kind).is_some()
    }
}

/// The twelve record fields of `r` in record order, then the two kind facts the stage reads:
/// whether the kind is known to this build and whether it has a zero.
fn field_row(r: &DataFieldRecord) -> [i64; 14] {
    [
        i64::from(r.name),
        i64::from(r.first_slot),
        i64::from(r.count),
        i64::from(r.offset),
        i64::from(r.stride),
        i64::from(r.elem_len),
        i64::from(r.kind),
        i64::from(r.visibility),
        i64::from(r.init),
        i64::from(r.reserved0),
        i64::from(r.reserved1),
        i64::from(r.init_const),
        i64::from(kind_known(r.kind)),
        i64::from(data_kind_zero(r.kind).is_some()),
    ]
}

/// Decode an optional data layout through Keleusma semantic operations.
///
/// The stage distinguishes absent from empty layouts, refuses an artifact carrying a retired
/// data-layout region, and validates every field record: its name index, visibility, reserved
/// bytes, count, tiling of the slot space, shared prefix, kind, element length, placement and
/// initialiser form. The host builds a field only from a record the stage accepted, and
/// decodes the explicit initialisers through the constant-pool decoder in one scope.
///
/// # Errors
/// Refuses a retired region, malformed field records, bad names, invalid initialiser
/// constants and stage or allocation failures.
pub fn wire_decode_data_layout_via_kel(bytes: &[u8]) -> Result<Option<DataLayout>, SelfHostError> {
    with_compiled_wire(
        compile_src(&read_stage("kel/wire.kel")),
        |codec, codec_shared| {
            let directory = decode_directory(codec, codec_shared, bytes)?;
            let region = |kind| {
                directory
                    .regions
                    .iter()
                    .find(|r| r.kind == kind)
                    .map(|r| r.bytes)
            };
            let records = region(kind::DATA_FIELDS);
            let names = NameTables {
                names: region(kind::NAMES),
                pool: region(kind::STRING_POOL),
            };
            let retired = kind::RETIRED_DATA_LAYOUT
                .iter()
                .any(|&k| region(k).is_some());
            let consts = region(kind::CONSTS).map_or(0, |c| c.len() / 16);
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    for (i, value) in [
                        byte_count(records)?,
                        byte_count(names.names)?,
                        byte_count(names.pool)?,
                        i64::from(retired),
                        i64::try_from(consts).map_err(|_| error("constant count"))?,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        set_word(vm, shared, ARG + i, value)?;
                    }
                    match command(vm, shared, 30)? {
                        0 => return Ok(None),
                        1 => {}
                        _ => return Err(error("unknown data presence result")),
                    }
                    let records = records.ok_or_else(|| error("field table absent"))?;
                    let records = decode_records(codec, codec_shared, kind::DATA_FIELDS, records)?;
                    let mut fields_out = Vec::new();
                    fields_out
                        .try_reserve_exact(records.len())
                        .map_err(|_| error("field allocation failed"))?;
                    let mut constants: Vec<(u32, u32)> = Vec::new();
                    for record in &records {
                        let WireSchemaRecord::DataFieldRecord(r) = record else {
                            return Err(error("unexpected field record"));
                        };
                        fields(vm, shared, &field_row(r))?;
                        expect(vm, shared, 31, 0)?;
                        let name = name_lookup(codec, codec_shared, vm, shared, names, r.name)?;
                        expect(vm, shared, 32, 0)?;
                        let init = match r.init {
                            init_form::ZERO => DataInit::Zero,
                            init_form::UNIT => DataInit::Unit,
                            _ => {
                                constants.push((r.init_const, 1));
                                // Replaced below, once every constant is decoded.
                                DataInit::Unit
                            }
                        };
                        fields_out.push(DataField {
                            name: copy_name(name)?,
                            visibility: if r.visibility == 1 {
                                SlotVisibility::Shared
                            } else {
                                SlotVisibility::Private
                            },
                            first_slot: r.first_slot,
                            count: r.count,
                            kind: r.kind,
                            elem_len: r.elem_len,
                            offset: (r.offset != ABSENT).then_some(r.offset),
                            stride: r.stride,
                            init,
                        });
                    }
                    expect(vm, shared, 33, 0)?;
                    if !constants.is_empty() {
                        let pools = wire_decode_constant_pools_via_kel(bytes, &constants)?;
                        let mut pools = pools.into_iter();
                        for (f, r) in fields_out.iter_mut().zip(&records) {
                            let WireSchemaRecord::DataFieldRecord(r) = r else {
                                return Err(error("unexpected field record"));
                            };
                            if r.init == init_form::CONST {
                                let value = pools
                                    .next()
                                    .and_then(|p| p.into_iter().next())
                                    .ok_or_else(|| error("initializer constant absent"))?;
                                f.init = DataInit::Const(value);
                            }
                        }
                    }
                    Ok(Some(DataLayout { fields: fields_out }))
                },
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::super::wire_framing::get_word;
    use super::super::wire_semantics::FIN;
    use super::*;

    /// A shared one-field record that the stage accepts, as twelve fields and two kind facts.
    fn shared_word() -> [i64; 14] {
        [0, 0, 2, 0, 8, 0, 3, 1, 0, 0, 0, i64::from(ABSENT), 1, 1]
    }

    #[test]
    fn data_protocol_validates_fields_and_refuses_retired_regions() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                // A retired region is refused even with no field table.
                for (i, value) in [-1, 8, 1, 1, 0].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                assert!(command(vm, shared, 30).is_err());
                // Absent and present.
                for (i, value) in [-1, 8, 1, 0, 0].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                expect(vm, shared, 30, 0)?;
                for (i, value) in [64, 8, 1, 0, 1].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                expect(vm, shared, 30, 1)?;
                assert_eq!(get_word(vm, shared, FIN)?, 2);
                assert!(command(vm, shared, 32).is_err(), "done before a check");
                assert!(
                    command(vm, shared, 33).is_err(),
                    "finish before every field"
                );
                fields(vm, shared, &shared_word())?;
                expect(vm, shared, 31, 0)?;
                expect(vm, shared, 32, 0)?;
                // A private field taking the zero of a kind without one is refused.
                let mut text = shared_word();
                text[1] = 2;
                text[2] = 1;
                text[3] = i64::from(ABSENT);
                text[4] = 0;
                text[6] = 6;
                text[7] = 2;
                text[13] = 0;
                fields(vm, shared, &text)?;
                assert!(command(vm, shared, 31).is_err());
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn data_protocol_refuses_each_broken_field_invariant() {
        let cases: &[(usize, i64, &str)] = &[
            (0, 9, "name index past the table"),
            (7, 0, "unknown visibility"),
            (9, 1, "reserved byte"),
            (10, 1, "reserved short"),
            (2, 0, "zero count"),
            (1, 1, "first slot not the running sum"),
            (2, 16_777_217, "past the slot space"),
            (12, 0, "unknown kind"),
            (5, 4, "scalar with a body length"),
            (3, i64::from(ABSENT), "shared field without an offset"),
            (8, 1, "shared field with a module initialiser"),
            (3, i64::from(u32::MAX) - 1, "last element offset overflows"),
        ];
        for &(column, value, what) in cases {
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    for (i, v) in [32, 8, 1, 0, 0].into_iter().enumerate() {
                        set_word(vm, shared, ARG + i, v)?;
                    }
                    expect(vm, shared, 30, 1)?;
                    let mut row = shared_word();
                    row[column] = value;
                    fields(vm, shared, &row)?;
                    assert!(command(vm, shared, 31).is_err(), "{what} was accepted");
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}
