//! Constant construction follows schema-stage requests and ownership decisions.

use super::wire_directory::decode_directory;
use super::wire_framing::{get_word, set_word, with_compiled_wire};
use super::wire_records::decode_records;
use super::wire_semantics::{ARG, FIN, NameTables, byte_count, command, fields, name_lookup};
use super::*;
use crate::wire_schema::{ConstRecord, kind};

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire constants: {detail}"),
    }
}
fn required(bytes: Option<&[u8]>) -> Result<&[u8], SelfHostError> {
    bytes.ok_or_else(|| error("required region absent after stage validation"))
}
fn signed(vm: &dyn StageExecutor, shared: &[u8], slot: usize) -> Result<i64, SelfHostError> {
    match vm
        .get_shared(shared, slot)
        .map_err(|_| error("field read failed"))?
    {
        Value::Int(n) => Ok(n),
        _ => Err(error("non-integer semantic field")),
    }
}
fn record(value: &WireSchemaRecord) -> Result<&ConstRecord, SelfHostError> {
    match value {
        WireSchemaRecord::ConstRecord(value) => Ok(value),
        _ => Err(error("unexpected constant record")),
    }
}
fn send_record(
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    record: &ConstRecord,
) -> Result<(), SelfHostError> {
    fields(
        vm,
        shared,
        &[
            i64::from(record.tag),
            i64::from(record.flags),
            i64::from(record.aux),
            record.payload as i64,
        ],
    )
}
fn expect(
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    cmd: i64,
    expected: usize,
) -> Result<(), SelfHostError> {
    if command(vm, shared, cmd)? != expected {
        return Err(error("unexpected protocol response"));
    }
    Ok(())
}
fn owned(text: &str) -> Result<String, SelfHostError> {
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| error("name allocation failed"))?;
    result.push_str(text);
    Ok(result)
}

#[derive(Default)]
struct Parts {
    text: Option<String>,
    type_name: Option<String>,
    variant: Option<String>,
    field_names: Vec<String>,
    children: Vec<ConstValue>,
}

fn materialize(
    form: usize,
    payload: i64,
    has_disc: usize,
    disc: i64,
    parts: Parts,
) -> Result<ConstValue, SelfHostError> {
    Ok(match form {
        0 => ConstValue::Unit,
        1 => ConstValue::None,
        2 => ConstValue::Bool(match payload {
            0 => false,
            1 => true,
            _ => return Err(error("invalid Boolean value")),
        }),
        3 => ConstValue::Int(payload),
        4 => ConstValue::Byte(u8::try_from(payload).map_err(|_| error("invalid byte value"))?),
        5 => ConstValue::Fixed(payload),
        #[cfg(feature = "floats")]
        6 => ConstValue::Float(f64::from_bits(payload as u64)),
        7 => ConstValue::StaticStr(parts.text.ok_or_else(|| error("missing string"))?),
        8 => ConstValue::Tuple(parts.children),
        9 => ConstValue::Array(parts.children),
        10 => {
            if parts.field_names.len() != parts.children.len() {
                return Err(error("field names and values disagree"));
            }
            let mut values = Vec::new();
            values
                .try_reserve_exact(parts.children.len())
                .map_err(|_| error("field allocation failed"))?;
            values.extend(parts.field_names.into_iter().zip(parts.children));
            ConstValue::Struct {
                type_name: parts
                    .type_name
                    .ok_or_else(|| error("missing struct type"))?,
                fields: values,
            }
        }
        11 => ConstValue::Enum {
            type_name: parts.type_name.ok_or_else(|| error("missing enum type"))?,
            variant: parts.variant.ok_or_else(|| error("missing enum variant"))?,
            discriminant: match has_disc {
                0 => None,
                1 => Some(disc),
                _ => return Err(error("invalid discriminant presence")),
            },
            fields: parts.children,
        },
        _ => return Err(error("unknown semantic constant form")),
    })
}

struct Tables<'a> {
    nodes: Vec<WireSchemaRecord>,
    structs: Vec<WireSchemaRecord>,
    enums: Vec<WireSchemaRecord>,
    names: NameTables<'a>,
}

fn build_node(
    codec: &mut dyn StageExecutor,
    codec_shared: &mut [u8],
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    tables: &Tables<'_>,
    built: &mut [Option<ConstValue>],
) -> Result<(), SelfHostError> {
    expect(vm, shared, 16, 1)?;
    let index = get_word(vm, shared, FIN)?;
    let node = record(
        tables
            .nodes
            .get(index)
            .ok_or_else(|| error("node request out of bounds"))?,
    )?;
    send_record(vm, shared, node)?;
    expect(vm, shared, 17, 0)?;
    let count = get_word(vm, shared, FIN)?;
    let form = get_word(vm, shared, FIN + 1)?;
    let limit = count
        .checked_mul(2)
        .and_then(|n| n.checked_add(5))
        .ok_or_else(|| error("node request budget overflow"))?;
    let mut parts = Parts::default();
    parts
        .children
        .try_reserve_exact(count)
        .map_err(|_| error("child allocation failed"))?;
    if form == 10 {
        parts
            .field_names
            .try_reserve_exact(count)
            .map_err(|_| error("field-name allocation failed"))?;
    }
    for _ in 0..limit {
        match command(vm, shared, 16)? {
            2 => {
                let index = get_word(vm, shared, FIN)?;
                let Some(WireSchemaRecord::StructAux(row)) = tables.structs.get(index) else {
                    return Err(error("struct auxiliary request out of bounds"));
                };
                fields(
                    vm,
                    shared,
                    &[i64::from(row.type_name), i64::from(row.field_names_first)],
                )?;
                expect(vm, shared, 18, 0)?;
            }
            3 => {
                let index = get_word(vm, shared, FIN)?;
                let Some(WireSchemaRecord::EnumAux(row)) = tables.enums.get(index) else {
                    return Err(error("enum auxiliary request out of bounds"));
                };
                fields(
                    vm,
                    shared,
                    &[
                        i64::from(row.type_name),
                        i64::from(row.variant),
                        row.discriminant,
                    ],
                )?;
                expect(vm, shared, 18, 0)?;
            }
            4 => {
                let name = u32::try_from(get_word(vm, shared, FIN)?)
                    .map_err(|_| error("name index exceeds u32"))?;
                let role = get_word(vm, shared, FIN + 1)?;
                let value = owned(name_lookup(
                    codec,
                    codec_shared,
                    vm,
                    shared,
                    tables.names,
                    name,
                )?)?;
                match role {
                    0 if parts.text.is_none() => parts.text = Some(value),
                    1 if parts.type_name.is_none() => parts.type_name = Some(value),
                    2 if parts.field_names.len() < count => parts.field_names.push(value),
                    3 if parts.variant.is_none() => parts.variant = Some(value),
                    _ => return Err(error("unexpected name role")),
                }
                expect(vm, shared, 20, 0)?;
            }
            5 => {
                let index = get_word(vm, shared, FIN)?;
                let slot = built
                    .get_mut(index)
                    .ok_or_else(|| error("child request out of bounds"))?;
                let value = slot.take();
                set_word(vm, shared, ARG, i64::from(value.is_some()))?;
                expect(vm, shared, 20, 0)?;
                if parts.children.len() >= count {
                    return Err(error("child count exceeded"));
                }
                parts
                    .children
                    .push(value.ok_or_else(|| error("stage accepted an unavailable child"))?);
            }
            6 => {
                if parts.children.len() != count {
                    return Err(error("incomplete child sequence"));
                }
                let response_form = get_word(vm, shared, FIN)?;
                if response_form != form {
                    return Err(error("construction form changed"));
                }
                let value = materialize(
                    form,
                    signed(vm, shared, FIN + 1)?,
                    get_word(vm, shared, FIN + 2)?,
                    signed(vm, shared, FIN + 3)?,
                    parts,
                )?;
                let slot = built
                    .get_mut(index)
                    .ok_or_else(|| error("store request out of bounds"))?;
                if slot.is_some() {
                    return Err(error("constant slot overwritten"));
                }
                *slot = Some(value);
                expect(vm, shared, 20, 0)?;
                return Ok(());
            }
            _ => return Err(error("unexpected node construction request")),
        }
    }
    Err(error(
        "node construction did not terminate within its request budget",
    ))
}

/// Decode owned constant pools through Keleusma-directed validation and traversal.
///
/// Keleusma owns tag interpretation, forward-range checks, auxiliary and name
/// requests, child ordering, and root extraction. The host holds typed storage,
/// reports whether a requested value is still available, and constructs values.
/// Ranges in one call share ownership, matching the reference owned decoder.
/// Call separately for ownership scopes the reference decodes separately.
///
/// # Errors
/// Refuses malformed required tables, invalid tags or references, non-forward
/// child ranges, duplicate ownership, malformed text and unavailable float tags.
/// Allocation and stage failures propagate. No fixed node or name-length cap is
/// imposed by the stage's transport windows.
pub fn wire_decode_constant_pools_via_kel(
    bytes: &[u8],
    ranges: &[(u32, u32)],
) -> Result<Vec<Vec<ConstValue>>, SelfHostError> {
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
            let raw_nodes = region(kind::CONSTS);
            let names = NameTables {
                names: region(kind::NAMES),
                pool: region(kind::STRING_POOL),
            };
            let raw_structs = region(kind::STRUCT_AUX);
            let raw_enums = region(kind::ENUM_AUX);
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    let args = [
                        byte_count(raw_nodes)?,
                        byte_count(names.names)?,
                        byte_count(names.pool)?,
                        byte_count(raw_structs)?,
                        byte_count(raw_enums)?,
                        i64::try_from(ranges.len())
                            .map_err(|_| error("range count exceeds word"))?,
                        i64::from(cfg!(feature = "floats")),
                    ];
                    for (i, value) in args.into_iter().enumerate() {
                        set_word(vm, shared, ARG + i, value)?;
                    }
                    expect(vm, shared, 12, 0)?;
                    let count = get_word(vm, shared, FIN)?;

                    let tables = Tables {
                        nodes: decode_records(
                            codec,
                            codec_shared,
                            kind::CONSTS,
                            required(raw_nodes)?,
                        )?,
                        structs: decode_records(
                            codec,
                            codec_shared,
                            kind::STRUCT_AUX,
                            required(raw_structs)?,
                        )?,
                        enums: decode_records(
                            codec,
                            codec_shared,
                            kind::ENUM_AUX,
                            required(raw_enums)?,
                        )?,
                        names,
                    };
                    if tables.nodes.len() != count {
                        return Err(error("node count disagreement"));
                    }
                    for node in &tables.nodes {
                        send_record(vm, shared, record(node)?)?;
                        expect(vm, shared, 13, 0)?;
                    }
                    for &(first, count) in ranges {
                        fields(vm, shared, &[i64::from(first), i64::from(count)])?;
                        expect(vm, shared, 14, 0)?;
                    }
                    expect(vm, shared, 15, 0)?;
                    let mut built = Vec::new();
                    built
                        .try_reserve_exact(count)
                        .map_err(|_| error("constant storage allocation failed"))?;
                    built.resize_with(count, || None);
                    for _ in 0..count {
                        build_node(codec, codec_shared, vm, shared, &tables, &mut built)?;
                    }
                    expect(vm, shared, 16, 0)?;
                    let mut result = Vec::new();
                    result
                        .try_reserve_exact(ranges.len())
                        .map_err(|_| error("pool allocation failed"))?;
                    for &(first, count) in ranges {
                        fields(vm, shared, &[i64::from(first), i64::from(count)])?;
                        expect(vm, shared, 19, 0)?;
                        let count = get_word(vm, shared, FIN)?;
                        let mut pool = Vec::new();
                        pool.try_reserve_exact(count)
                            .map_err(|_| error("root allocation failed"))?;
                        for _ in 0..count {
                            expect(vm, shared, 16, 7)?;
                            let index = get_word(vm, shared, FIN)?;
                            let value = built
                                .get_mut(index)
                                .ok_or_else(|| error("root request out of bounds"))?
                                .take();
                            set_word(vm, shared, ARG, i64::from(value.is_some()))?;
                            expect(vm, shared, 20, 0)?;
                            pool.push(
                                value.ok_or_else(|| error("stage accepted unavailable root"))?,
                            );
                        }
                        expect(vm, shared, 16, 0)?;
                        result.push(pool);
                    }
                    expect(vm, shared, 21, 0)?;
                    Ok(result)
                },
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_protocol_validates_before_building_and_rejects_unavailable_roots() {
        with_compiled_wire(compile_src(&read_stage("kel/wire_schema.kel")), |vm, shared| {
            for (i, value) in [32,0,0,0,0,1,0].into_iter().enumerate() { set_word(vm, shared, ARG+i, value)?; }
            expect(vm, shared, 12, 0)?;
            assert!(command(vm, shared, 15).is_err());
            for _ in 0..2 { fields(vm, shared, &[3,0,0,0])?; expect(vm, shared, 13, 0)?; }
            assert!(command(vm, shared, 15).is_err());
            fields(vm, shared, &[0,2])?;
            expect(vm, shared, 14, 0)?;
            expect(vm, shared, 15, 0)?;
            for cmd in 0..12 { assert!(command(vm, shared, cmd).is_err(), "command {cmd}"); }
            for (index, tag, payload, form, normalized) in [(1,3,i64::MIN,3,i64::MIN),(0,2,-1,2,1)] {
                expect(vm, shared, 16, 1)?;
                assert_eq!(get_word(vm, shared, FIN)?, index);
                assert!(command(vm, shared, 18).is_err());
                assert!(command(vm, shared, 20).is_err());
                fields(vm, shared, &[-1,0,0,0])?;
                assert!(command(vm, shared, 17).is_err());
                fields(vm, shared, &[tag,0,0,payload])?;
                expect(vm, shared, 17, 0)?;
                expect(vm, shared, 16, 6)?;
                assert_eq!(get_word(vm, shared, FIN)?, form);
                assert_eq!(signed(vm, shared, FIN+1)?, normalized);
                assert!(command(vm, shared, 16).is_err());
                expect(vm, shared, 20, 0)?;
            }
            expect(vm, shared, 16, 0)?;
            fields(vm, shared, &[0,2])?;
            expect(vm, shared, 19, 0)?;
            expect(vm, shared, 16, 7)?;
            assert_eq!(get_word(vm, shared, FIN)?, 0);
            set_word(vm, shared, ARG, 0)?;
            let failure = command(vm, shared, 20).unwrap_err();
            assert!(matches!(failure, SelfHostError::Unsupported { detail } if detail.contains("refused with -302")));
            assert!(command(vm, shared, 21).is_err());
            Ok(())
        }).unwrap();
    }
}
