//! Data-layout expansion and initializer selection directed by Keleusma.

use super::wire_directory::decode_directory;
use super::wire_framing::{get_word, set_word, with_compiled_wire};
use super::wire_records::decode_records;
use super::wire_semantics::{ARG, FIN, NameTables, byte_count, command, fields, name_lookup};
use super::*;
use crate::bytecode::{
    DataLayout, DataSlot, PrivateCompositeSlot, SharedSlotLayout, SlotVisibility,
};
use crate::wire_schema::{DataSlotRecord, kind};

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire data layout: {detail}"),
    }
}
fn copy_name(name: &str) -> Result<String, SelfHostError> {
    let mut result = String::new();
    result
        .try_reserve_exact(name.len())
        .map_err(|_| error("slot name allocation failed"))?;
    result.push_str(name);
    Ok(result)
}
fn required(bytes: Option<&[u8]>) -> Result<&[u8], SelfHostError> {
    bytes.ok_or_else(|| error("required data region absent after validation"))
}
fn send_slot(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    record: &WireSchemaRecord,
) -> Result<(), SelfHostError> {
    let WireSchemaRecord::DataSlotRecord(DataSlotRecord {
        name,
        visibility,
        run,
        ..
    }) = record
    else {
        return Err(error("unexpected slot record"));
    };
    fields(
        vm,
        shared,
        &[i64::from(*name), i64::from(*visibility), i64::from(*run)],
    )
}
fn expect(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    cmd: i64,
    expected: usize,
) -> Result<(), SelfHostError> {
    if command(vm, shared, cmd)? != expected {
        return Err(error("unexpected data protocol response"));
    }
    Ok(())
}
fn word(vm: &Vm<'_, '_>, shared: &[u8], slot: usize) -> Result<u32, SelfHostError> {
    u32::try_from(get_word(vm, shared, slot)?).map_err(|_| error("word response exceeds u32"))
}

/// Decode an optional data layout through Keleusma semantic operations.
///
/// The stage distinguishes absent from empty layouts, validates slot names,
/// visibility and run lengths, expands shared offsets, and selects stored or
/// elided initializers. Stored initializers use a separate constant-pool scope.
/// Raw kind bytes, private placements and unused shared first-slot fields retain
/// reference codec behavior. Later verifier checks are separate.
///
/// # Errors
/// Refuses malformed required tables, bad names, invalid runs or visibility,
/// shared-offset overflow, invalid initializer pools and stage/allocation failures.
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
            let slots = region(kind::DATA_SLOTS);
            let names = NameTables {
                names: region(kind::NAMES),
                pool: region(kind::STRING_POOL),
            };
            let shared_rows = region(kind::SHARED_LAYOUT);
            let placements = region(kind::PRIVATE_COMPOSITE);
            let init = region(kind::DATA_INIT);
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    for (i, value) in [
                        byte_count(slots)?,
                        byte_count(names.names)?,
                        byte_count(names.pool)?,
                        byte_count(shared_rows)?,
                        byte_count(placements)?,
                        byte_count(init)?,
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
                    let slots =
                        decode_records(codec, codec_shared, kind::DATA_SLOTS, required(slots)?)?;
                    let shared_rows = decode_records(
                        codec,
                        codec_shared,
                        kind::SHARED_LAYOUT,
                        required(shared_rows)?,
                    )?;
                    let placements = decode_records(
                        codec,
                        codec_shared,
                        kind::PRIVATE_COMPOSITE,
                        required(placements)?,
                    )?;
                    let init =
                        decode_records(codec, codec_shared, kind::DATA_INIT, required(init)?)?;
                    for record in &slots {
                        send_slot(vm, shared, record)?;
                        expect(vm, shared, 31, 0)?;
                    }
                    expect(vm, shared, 32, 0)?;
                    let total = get_word(vm, shared, FIN)?;
                    let mut result = DataLayout {
                        slots: Vec::new(),
                        shared_layout: Vec::new(),
                        private_composite_layout: Vec::new(),
                        private_init: Vec::new(),
                    };
                    result
                        .slots
                        .try_reserve_exact(total)
                        .map_err(|_| error("slot allocation failed"))?;
                    for record in &slots {
                        send_slot(vm, shared, record)?;
                        expect(vm, shared, 33, 1)?;
                        let index = word(vm, shared, FIN)?;
                        let name = name_lookup(codec, codec_shared, vm, shared, names, index)?;
                        expect(vm, shared, 34, 0)?;
                        let count = get_word(vm, shared, FIN)?;
                        let visibility = match get_word(vm, shared, FIN + 1)? {
                            0 => SlotVisibility::Shared,
                            1 => SlotVisibility::Private,
                            _ => return Err(error("invalid semantic visibility")),
                        };
                        for _ in 0..count {
                            result.slots.push(DataSlot {
                                name: copy_name(name)?,
                                visibility,
                            });
                        }
                        set_word(vm, shared, ARG, count as i64)?;
                        expect(vm, shared, 35, 0)?;
                    }
                    if result.slots.len() != total {
                        return Err(error("logical slot count mismatch"));
                    }
                    for record in &shared_rows {
                        let WireSchemaRecord::SharedSlotRecord(r) = record else {
                            return Err(error("unexpected shared record"));
                        };
                        fields(
                            vm,
                            shared,
                            &[
                                i64::from(r.offset),
                                i64::from(r.kind),
                                i64::from(r.len),
                                i64::from(r.first_slot),
                                i64::from(r.run),
                                i64::from(r.stride),
                            ],
                        )?;
                        expect(vm, shared, 36, 0)?;
                        let count = get_word(vm, shared, FIN)?;
                        result
                            .shared_layout
                            .try_reserve_exact(count)
                            .map_err(|_| error("shared layout allocation failed"))?;
                        let mut received = 0;
                        let mut completed = false;
                        for _ in 0..count / 5 + 2 {
                            let n = command(vm, shared, 37)?;
                            if n == 0 {
                                completed = true;
                                break;
                            }
                            if n > 5 || n > count - received {
                                return Err(error("shared batch count invalid"));
                            }
                            for i in 0..n {
                                let base = FIN + i * 3;
                                result.shared_layout.push(SharedSlotLayout {
                                    offset: word(vm, shared, base)?,
                                    kind: u8::try_from(get_word(vm, shared, base + 1)?)
                                        .map_err(|_| error("shared kind exceeds byte"))?,
                                    len: u16::try_from(get_word(vm, shared, base + 2)?)
                                        .map_err(|_| error("shared length exceeds u16"))?,
                                });
                            }
                            received += n;
                        }
                        if !completed || received != count {
                            return Err(error("shared run incomplete"));
                        }
                    }
                    result
                        .private_composite_layout
                        .try_reserve_exact(placements.len())
                        .map_err(|_| error("placement allocation failed"))?;
                    for record in &placements {
                        let WireSchemaRecord::PrivateCompositeRecord(r) = record else {
                            return Err(error("unexpected placement record"));
                        };
                        fields(vm, shared, &[i64::from(r.slot), i64::from(r.offset)])?;
                        expect(vm, shared, 38, 0)?;
                        result.private_composite_layout.push(PrivateCompositeSlot {
                            slot: u16::try_from(get_word(vm, shared, FIN)?)
                                .map_err(|_| error("slot response exceeds u16"))?,
                            offset: word(vm, shared, FIN + 1)?,
                        });
                    }
                    expect(vm, shared, 39, 3)?;
                    let index = get_word(vm, shared, FIN)?;
                    let Some(WireSchemaRecord::DataInitRecord(r)) = init.get(index) else {
                        return Err(error("initializer record absent"));
                    };
                    fields(vm, shared, &[i64::from(r.first), i64::from(r.count)])?;
                    match command(vm, shared, 39)? {
                        1 => {
                            let count = get_word(vm, shared, FIN)?;
                            let value = i64::try_from(get_word(vm, shared, FIN + 1)?)
                                .map_err(|_| error("default initializer exceeds word"))?;
                            result
                                .private_init
                                .try_reserve_exact(count)
                                .map_err(|_| error("initializer allocation failed"))?;
                            result.private_init.resize(count, ConstValue::Int(value));
                        }
                        2 => {
                            let range = (word(vm, shared, FIN)?, word(vm, shared, FIN + 1)?);
                            let mut pools = wire_decode_constant_pools_via_kel(bytes, &[range])?;
                            if pools.len() != 1 {
                                return Err(error("initializer scope count mismatch"));
                            }
                            result.private_init = pools
                                .pop()
                                .ok_or_else(|| error("initializer pool absent"))?;
                        }
                        _ => return Err(error("unknown initializer plan")),
                    }
                    set_word(
                        vm,
                        shared,
                        ARG,
                        i64::try_from(result.private_init.len())
                            .map_err(|_| error("initializer count exceeds word"))?,
                    )?;
                    expect(vm, shared, 40, 0)?;
                    Ok(Some(result))
                },
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_protocol_preserves_refused_counts_and_derives_offsets() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                for (i, value) in [8, 8, 1, 16, 8, 8].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                expect(vm, shared, 30, 1)?;
                for cmd in 0..30 {
                    assert!(command(vm, shared, cmd).is_err(), "command {cmd}");
                }
                assert!(command(vm, shared, 32).is_err());
                fields(vm, shared, &[0, 2, 2])?;
                expect(vm, shared, 31, 0)?;
                expect(vm, shared, 32, 0)?;
                assert_eq!(get_word(vm, shared, FIN)?, 2);
                fields(vm, shared, &[0, 2, 2])?;
                expect(vm, shared, 33, 1)?;
                for (i, value) in [8, 1, 0].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                expect(vm, shared, 8, 1)?;
                fields(vm, shared, &[0, 1])?;
                expect(vm, shared, 9, 1)?;
                vm.set_shared(shared, 24, Value::Byte(b'x')).unwrap();
                set_word(vm, shared, ARG, 1)?;
                expect(vm, shared, 10, 0)?;
                expect(vm, shared, 11, 0)?;
                expect(vm, shared, 34, 0)?;
                assert_eq!(get_word(vm, shared, FIN)?, 2);
                assert_eq!(get_word(vm, shared, FIN + 1)?, 1);
                assert!(command(vm, shared, 35).is_err());
                set_word(vm, shared, ARG, 2)?;
                expect(vm, shared, 35, 0)?;
                fields(
                    vm,
                    shared,
                    &[
                        i64::from(u32::MAX) - 8,
                        255,
                        65535,
                        i64::from(u32::MAX),
                        2,
                        8,
                    ],
                )?;
                expect(vm, shared, 36, 0)?;
                assert!(command(vm, shared, 36).is_err());
                assert!(command(vm, shared, 40).is_err());
                expect(vm, shared, 37, 2)?;
                assert_eq!(word(vm, shared, FIN)?, u32::MAX - 8);
                assert_eq!(word(vm, shared, FIN + 3)?, u32::MAX);
                expect(vm, shared, 37, 0)?;
                fields(vm, shared, &[65535, i64::from(u32::MAX)])?;
                expect(vm, shared, 38, 0)?;
                expect(vm, shared, 39, 3)?;
                assert_eq!(get_word(vm, shared, FIN)?, 0);
                fields(vm, shared, &[i64::from(u32::MAX), 3])?;
                expect(vm, shared, 39, 1)?;
                assert_eq!(get_word(vm, shared, FIN)?, 3);
                assert_eq!(get_word(vm, shared, FIN + 1)?, 0);
                set_word(vm, shared, ARG, 2)?;
                assert!(command(vm, shared, 40).is_err());
                set_word(vm, shared, ARG, 3)?;
                expect(vm, shared, 40, 0)?;
                Ok(())
            },
        )
        .unwrap();
    }
}
