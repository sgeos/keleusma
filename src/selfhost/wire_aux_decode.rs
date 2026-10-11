//! Complete auxiliary-body assembly from Keleusma metadata decisions.

use super::wire_debug_decode::decode_parameters;
use super::wire_directory::decode_directory;
use super::wire_framing::{get_word, set_word, with_compiled_wire};
use super::wire_records::decode_records;
use super::wire_semantics::{
    ARG, FIN, NameTables, byte_count, command, fields, name_lookup, requested_bytes,
};
use super::*;
use crate::bytecode::BlockType;
use crate::wire_format::{WireAuxBody, WireChunk};
use crate::wire_schema::{ChunkRecord, kind};

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire auxiliary decode: {detail}"),
    }
}
fn accept(vm: &mut Vm<'_, '_>, shared: &mut [u8], cmd: i64) -> Result<(), SelfHostError> {
    if command(vm, shared, cmd)? != 0 {
        return Err(error("unexpected metadata response"));
    }
    Ok(())
}
fn outputs<const N: usize>(vm: &Vm<'_, '_>, shared: &[u8]) -> Result<[usize; N], SelfHostError> {
    let mut values = [0; N];
    for (i, value) in values.iter_mut().enumerate() {
        *value = get_word(vm, shared, FIN + i)?;
    }
    Ok(values)
}
fn u32_value(n: usize) -> Result<u32, SelfHostError> {
    u32::try_from(n).map_err(|_| error("metadata value exceeds u32"))
}
fn chunk(record: &WireSchemaRecord) -> Result<&ChunkRecord, SelfHostError> {
    match record {
        WireSchemaRecord::ChunkRecord(value) => Ok(value),
        _ => Err(error("unexpected chunk record")),
    }
}
fn chunk_fields(value: &ChunkRecord) -> [i64; 14] {
    [
        value.name.into(),
        value.consts_first.into(),
        value.consts_count.into(),
        value.templates_first.into(),
        value.templates_count.into(),
        value.param_types_first.into(),
        value.param_types_count.into(),
        value.debug_first.into(),
        value.debug_len.into(),
        value.op_byte_offset.into(),
        value.op_record_count.into(),
        value.local_count.into(),
        value.param_count.into(),
        value.block_type.into(),
    ]
}
fn native_name(record: &WireSchemaRecord) -> Result<u32, SelfHostError> {
    match record {
        WireSchemaRecord::NativeRecord(value) => Ok(value.name),
        _ => Err(error("unexpected native record")),
    }
}
fn copy<T: Clone>(values: &[T]) -> Result<Vec<T>, SelfHostError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(values.len())
        .map_err(|_| error("payload allocation failed"))?;
    result.extend_from_slice(values);
    Ok(result)
}
fn owned_name(value: &str) -> Result<String, SelfHostError> {
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| error("name allocation failed"))?;
    result.push_str(value);
    Ok(result)
}

/// Decode a complete auxiliary body through Keleusma readers.
///
/// The stage validates chunk/native metadata, selects header and chunk ranges,
/// interprets block and parameter tags, and decides optional debug presence.
/// Debug payloads remain opaque here, matching the auxiliary-body contract.
/// Optional layouts and signatures retain their reference defaulting policies.
/// This does not validate outer module framing or decode opcode sections.
///
/// # Errors
/// Refuses malformed required metadata, invalid selected ranges and names, and
/// propagates component, stage, transport and allocation failures.
pub fn wire_decode_aux_body_via_kel(bytes: &[u8]) -> Result<WireAuxBody, SelfHostError> {
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
            let names = NameTables {
                names: region(kind::NAMES),
                pool: region(kind::STRING_POOL),
            };
            let raw_chunks = region(kind::CHUNKS);
            let raw_natives = region(kind::NATIVES);
            let raw_header = region(kind::HEADER);
            let raw_params = region(kind::PARAM_TYPES);
            let raw_debug = region(kind::DEBUG_POOL);
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    for (i, value) in [
                        names.names,
                        names.pool,
                        raw_chunks,
                        raw_natives,
                        raw_header,
                        raw_params,
                        raw_debug,
                        region(kind::NATIVE_RETURNS),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        set_word(vm, shared, ARG + i, byte_count(value)?)?;
                    }
                    accept(vm, shared, 48)?;
                    let [chunk_count, native_count] = outputs(vm, shared)?;
                    let records = decode_records(
                        codec,
                        codec_shared,
                        kind::CHUNKS,
                        raw_chunks.unwrap_or(&[]),
                    )?;
                    let natives = decode_records(
                        codec,
                        codec_shared,
                        kind::NATIVES,
                        raw_natives.unwrap_or(&[]),
                    )?;
                    if records.len() != chunk_count || natives.len() != native_count {
                        return Err(error("metadata counts disagree"));
                    }
                    let mut ranges = Vec::new();
                    ranges
                        .try_reserve_exact(chunk_count)
                        .map_err(|_| error("range allocation failed"))?;
                    for record in &records {
                        fields(vm, shared, &chunk_fields(chunk(record)?))?;
                        accept(vm, shared, 49)?;
                        let [first, count] = outputs(vm, shared)?;
                        ranges.push((u32_value(first)?, u32_value(count)?));
                    }
                    for record in &natives {
                        fields(vm, shared, &[native_name(record)?.into()])?;
                        accept(vm, shared, 50)?;
                    }
                    accept(vm, shared, 51)?;
                    let selected = requested_bytes(
                        vm,
                        shared,
                        raw_header.ok_or_else(|| error("header absent after validation"))?,
                    )?;
                    let mut header = decode_records(codec, codec_shared, kind::HEADER, selected)?;
                    let Some(WireSchemaRecord::HeaderRecord(h)) = header.pop() else {
                        return Err(error("unexpected header record"));
                    };
                    if !header.is_empty() {
                        return Err(error("multiple requested headers"));
                    }
                    fields(
                        vm,
                        shared,
                        &[
                            h.entry_point.into(),
                            h.word_bits_log2.into(),
                            h.addr_bits_log2.into(),
                            h.float_bits_log2.into(),
                            h.flags.into(),
                            h.wcet_cycles.into(),
                            h.wcmu_bytes.into(),
                            h.shared_data_bytes.into(),
                            h.private_data_bytes.into(),
                            h.schema_hash.into(),
                            h.reserved.into(),
                        ],
                    )?;
                    accept(vm, shared, 52)?;
                    let h: [usize; 11] = outputs(vm, shared)?;
                    let layouts = wire_decode_layout_metadata_via_kel(bytes)?;
                    let templates = layouts.templates.unwrap_or_default();
                    // A zero-chunk module does not inspect unused constant tables.
                    let pools = if ranges.is_empty() {
                        Vec::new()
                    } else {
                        wire_decode_constant_pools_via_kel(bytes, &ranges)?
                    };
                    if pools.len() != chunk_count {
                        return Err(error("constant pool count disagreement"));
                    }
                    let mut chunks = Vec::new();
                    chunks
                        .try_reserve_exact(chunk_count)
                        .map_err(|_| error("chunk allocation failed"))?;
                    for (record, constants) in records.iter().zip(pools) {
                        fields(vm, shared, &chunk_fields(chunk(record)?))?;
                        set_word(
                            vm,
                            shared,
                            ARG,
                            i64::try_from(templates.len())
                                .map_err(|_| error("template count exceeds word range"))?,
                        )?;
                        accept(vm, shared, 53)?;
                        let p: [usize; 13] = outputs(vm, shared)?;
                        let block_type = match p[1] {
                            0 => BlockType::Func,
                            1 => BlockType::Reentrant,
                            2 => BlockType::Stream,
                            _ => return Err(error("unknown semantic block type")),
                        };
                        let end = p[2]
                            .checked_add(p[3])
                            .ok_or_else(|| error("template range overflow"))?;
                        let struct_templates = copy(
                            templates
                                .get(p[2]..end)
                                .ok_or_else(|| error("stage selected invalid templates"))?,
                        )?;
                        let param_types = decode_parameters(
                            vm,
                            shared,
                            raw_params,
                            (u32_value(p[4])?, u32_value(p[5])?),
                        )?;
                        let debug_pool_bytes = match p[6] {
                            0 => None,
                            1 => {
                                let end = p[7]
                                    .checked_add(p[8])
                                    .ok_or_else(|| error("debug span overflow"))?;
                                Some(copy(
                                    raw_debug.and_then(|raw| raw.get(p[7]..end)).ok_or_else(
                                        || error("stage selected invalid debug span"),
                                    )?,
                                )?)
                            }
                            _ => return Err(error("unknown debug presence response")),
                        };
                        let name = owned_name(name_lookup(
                            codec,
                            codec_shared,
                            vm,
                            shared,
                            names,
                            u32_value(p[0])?,
                        )?)?;
                        accept(vm, shared, 55)?;
                        chunks.push(WireChunk {
                            name,
                            constants,
                            struct_templates,
                            param_types,
                            block_type,
                            local_count: u16::try_from(p[11])
                                .map_err(|_| error("local count exceeds u16"))?,
                            param_count: u8::try_from(p[12])
                                .map_err(|_| error("parameter count exceeds u8"))?,
                            op_byte_offset: u32_value(p[9])?,
                            op_record_count: u32_value(p[10])?,
                            debug_pool_bytes,
                        });
                    }
                    let signatures = wire_decode_signature_metadata_via_kel(bytes)?;
                    let mut native_names = Vec::new();
                    native_names
                        .try_reserve_exact(native_count)
                        .map_err(|_| error("native allocation failed"))?;
                    for record in &natives {
                        fields(vm, shared, &[native_name(record)?.into()])?;
                        accept(vm, shared, 54)?;
                        let index = u32_value(get_word(vm, shared, FIN)?)?;
                        native_names.push(owned_name(name_lookup(
                            codec,
                            codec_shared,
                            vm,
                            shared,
                            names,
                            index,
                        )?)?);
                        accept(vm, shared, 55)?;
                    }
                    accept(vm, shared, 56)?;
                    Ok(WireAuxBody {
                        chunks,
                        native_names,
                        entry_point: match h[0] {
                            0 => None,
                            1 => Some(h[1]),
                            _ => return Err(error("unknown entry presence")),
                        },
                        data_layout: wire_decode_data_layout_via_kel(bytes)?,
                        word_bits_log2: u8::try_from(h[2])
                            .map_err(|_| error("word width exceeds u8"))?,
                        addr_bits_log2: u8::try_from(h[3])
                            .map_err(|_| error("address width exceeds u8"))?,
                        float_bits_log2: u8::try_from(h[4])
                            .map_err(|_| error("float width exceeds u8"))?,
                        flags: u8::try_from(h[5]).map_err(|_| error("flags exceed u8"))?,
                        wcet_cycles: u32_value(h[6])?,
                        wcmu_bytes: u32_value(h[7])?,
                        shared_data_bytes: u32_value(h[8])?,
                        private_data_bytes: u32_value(h[9])?,
                        schema_hash: u32_value(h[10])?,
                        enum_layouts: layouts.enum_layouts.unwrap_or_default(),
                        signatures: signatures.signatures,
                        native_return_shapes: signatures.native_return_shapes,
                    })
                },
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn module_transaction_requires_validation_and_excludes_other_families() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                for (i, n) in [8, 1, 48, 0, 32, -1, -1, 0].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, n)?;
                }
                accept(vm, shared, 48)?;
                assert_eq!(enter_wire(vm, shared, 51)?, -314);
                assert_eq!(enter_wire(vm, shared, 56)?, -314);
                assert_eq!(enter_wire(vm, shared, 41)?, -314);
                fields(vm, shared, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])?;
                accept(vm, shared, 49)?;
                accept(vm, shared, 51)?;
                fields(vm, shared, &[u32::MAX.into(), 6, 6, 6, 0, 0, 0, 0, 0, 0, 0])?;
                accept(vm, shared, 52)?;
                assert_eq!(outputs::<2>(vm, shared)?, [0, 0]);
                assert_eq!(enter_wire(vm, shared, 56)?, -314);
                fields(vm, shared, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])?;
                set_word(vm, shared, ARG, 0)?;
                accept(vm, shared, 53)?;
                assert!(decode_parameters(vm, shared, None, (u32::MAX, u32::MAX))?.is_empty());
                accept(vm, shared, 55)?;
                accept(vm, shared, 56)?;
                assert_eq!(enter_wire(vm, shared, 55)?, -314);
                Ok(())
            },
        )
        .unwrap();
    }
}
