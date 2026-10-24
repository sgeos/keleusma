//! Complete module reconstruction with framing and section decisions in Keleusma.

use super::wire_debug_decode::{copy_window, decode_debug};
use super::wire_decode::{decode_one, transport};
use super::wire_framing::{command as codec_command, get_word, set_word, with_compiled_wire};
use super::wire_semantics::{ARG, FIN, NAME_WINDOW, command, fields, requested_bytes};
use super::*;
use crate::bytecode::{
    Chunk, RUNTIME_ADDRESS_BITS_LOG2, RUNTIME_FLOAT_BITS_LOG2, RUNTIME_WORD_BITS_LOG2,
};

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire module decode: {detail}"),
    }
}
fn size(n: usize) -> Result<i64, SelfHostError> {
    i64::try_from(n).map_err(|_| error("transport size exceeds word range"))
}
fn word(n: usize) -> Result<u32, SelfHostError> {
    u32::try_from(n).map_err(|_| error("metadata exceeds u32"))
}
fn span(bytes: &[u8], start: usize, length: usize) -> Result<&[u8], SelfHostError> {
    let end = start
        .checked_add(length)
        .ok_or_else(|| error("response span overflow"))?;
    bytes
        .get(start..end)
        .ok_or_else(|| error("stage selected an invalid input span"))
}
fn accept(vm: &mut Vm<'_, '_>, shared: &mut [u8], cmd: i64) -> Result<(), SelfHostError> {
    if command(vm, shared, cmd)? != 0 {
        return Err(error("unexpected framing response"));
    }
    Ok(())
}

struct Frame<'a> {
    bytes: &'a [u8],
    opcodes: &'a [u8],
    operands: &'a [u8],
    auxiliary: &'a [u8],
    arena: u32,
    persistent: u32,
}
fn frame<'a>(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    bytes: &'a [u8],
) -> Result<Frame<'a>, SelfHostError> {
    for (i, n) in [
        size(bytes.len())?,
        RUNTIME_WORD_BITS_LOG2.into(),
        RUNTIME_ADDRESS_BITS_LOG2.into(),
        RUNTIME_FLOAT_BITS_LOG2.into(),
    ]
    .into_iter()
    .enumerate()
    {
        set_word(vm, shared, ARG + i, n)?;
    }
    let mut event = command(vm, shared, 57)?;
    // Prefix windows plus prefix, base-header, extension and final response.
    let budget = bytes.len() / NAME_WINDOW + 5;
    for _ in 0..budget {
        match event {
            1 => {
                let raw = requested_bytes(vm, shared, bytes)?;
                copy_window(vm, shared, raw)?;
                event = command(vm, shared, 58)?;
            }
            2 => {
                let mut p = [0; 10];
                for (i, n) in p.iter_mut().enumerate() {
                    *n = get_word(vm, shared, FIN + i)?;
                }
                return Ok(Frame {
                    bytes: span(bytes, p[0], p[1])?,
                    opcodes: span(bytes, p[2], p[3])?,
                    operands: span(bytes, p[4], p[5])?,
                    auxiliary: span(bytes, p[6], p[7])?,
                    arena: word(p[8])?,
                    persistent: word(p[9])?,
                });
            }
            _ => return Err(error("unknown framing request")),
        }
    }
    Err(error("framing request budget exhausted"))
}

fn checksum(
    codec: &mut Vm<'_, '_>,
    shared: &mut [u8],
    bytes: &[u8],
) -> Result<usize, SelfHostError> {
    set_word(codec, shared, wire_slots::WARG, size(bytes.len())?)?;
    if codec_command(codec, shared, 220)? != 1 {
        return Err(error("checksum start refused"));
    }
    let mut seen = 0;
    for window in bytes.chunks(65536) {
        transport(codec, shared, 0, window)?;
        set_word(codec, shared, wire_slots::WARG, size(window.len())?)?;
        seen += window.len();
        if codec_command(codec, shared, 189)? != seen {
            return Err(error("checksum transport count disagrees"));
        }
    }
    codec_command(codec, shared, 192)
}
fn pool_parity(
    codec: &mut Vm<'_, '_>,
    shared: &mut [u8],
    bytes: &[u8],
) -> Result<(), SelfHostError> {
    let (records, remainder) = bytes.as_chunks::<8>();
    if !remainder.is_empty() {
        return Err(error("stage accepted a partial operand record"));
    }
    for record in records {
        transport(codec, shared, 0, record)?;
        set_word(codec, shared, wire_slots::WARG, 0)?;
        if codec_command(codec, shared, 95)? != 1 {
            return Err(error("operand pool parity mismatch"));
        }
    }
    Ok(())
}

/// Decode a complete module with Keleusma framing, schema and opcode readers.
///
/// Matches the decoding scope of `wire_format::module_from_wire_bytes`, including
/// shebang prefixes, trailing bytes, noncanonical section placement and selected
/// opcode spans. All operand-pool entries are parity-checked, including unused ones.
/// Header extensions are checked for structural consistency. This low-level codec
/// does not authenticate signatures, decrypt payloads or run the virtual-machine
/// verifier. Those remain the caller's separate admission steps.
///
/// # Errors
/// Refuses invalid framing, checksums, section contents, mirrored metadata or
/// selected debug pools, and propagates stage, transport and allocation failures.
/// Acceptance and values are compared with the reference. Error variants and
/// precedence for multiple defects are not required to be identical.
pub fn wire_decode_module_via_kel(bytes: &[u8]) -> Result<Module, SelfHostError> {
    with_compiled_wire(
        compile_src(&read_stage("kel/wire.kel")),
        |codec, codec_shared| {
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    let f = frame(vm, shared, bytes)?;
                    let valid = checksum(codec, codec_shared, f.bytes)?;
                    set_word(vm, shared, ARG, size(valid)?)?;
                    accept(vm, shared, 59)?;
                    pool_parity(codec, codec_shared, f.operands)?;
                    let aux = wire_decode_aux_body_via_kel(f.auxiliary)?;
                    fields(
                        vm,
                        shared,
                        &[
                            aux.word_bits_log2.into(),
                            aux.addr_bits_log2.into(),
                            aux.float_bits_log2.into(),
                            aux.flags.into(),
                            aux.wcet_cycles.into(),
                            aux.wcmu_bytes.into(),
                            aux.shared_data_bytes.into(),
                            aux.private_data_bytes.into(),
                        ],
                    )?;
                    set_word(vm, shared, ARG, size(aux.chunks.len())?)?;
                    accept(vm, shared, 60)?;
                    let mut chunks = Vec::new();
                    chunks
                        .try_reserve_exact(aux.chunks.len())
                        .map_err(|_| error("chunk allocation failed"))?;
                    let pools = word(f.operands.len() / 8)?;
                    with_compiled_wire(
                        compile_src(&read_stage("kel/wire_schema.kel")),
                        |debug_vm, debug_shared| {
                            for chunk in aux.chunks {
                                set_word(vm, shared, ARG, chunk.op_byte_offset.into())?;
                                set_word(vm, shared, ARG + 1, chunk.op_record_count.into())?;
                                accept(vm, shared, 61)?;
                                let raw = requested_bytes(vm, shared, f.opcodes)?;
                                let (records, remainder) = raw.as_chunks::<4>();
                                if !remainder.is_empty() {
                                    return Err(error("stage selected partial opcode record"));
                                }
                                let mut ops = Vec::new();
                                ops.try_reserve_exact(records.len())
                                    .map_err(|_| error("instruction allocation failed"))?;
                                for record in records {
                                    ops.push(decode_one(
                                        codec,
                                        codec_shared,
                                        record,
                                        f.operands,
                                        pools,
                                    )?);
                                }
                                let debug_pool = chunk
                                    .debug_pool_bytes
                                    .as_deref()
                                    .map(|b| decode_debug(debug_vm, debug_shared, b))
                                    .transpose()?;
                                chunks.push(Chunk {
                                    name: chunk.name,
                                    ops,
                                    constants: chunk.constants,
                                    struct_templates: chunk.struct_templates,
                                    local_count: chunk.local_count,
                                    param_count: chunk.param_count,
                                    block_type: chunk.block_type,
                                    param_types: chunk.param_types,
                                    debug_pool,
                                });
                                accept(vm, shared, 62)?;
                            }
                            Ok(())
                        },
                    )?;
                    accept(vm, shared, 63)?;
                    Ok(Module {
                        chunks,
                        enum_layouts: aux.enum_layouts,
                        signatures: aux.signatures,
                        native_return_shapes: aux.native_return_shapes,
                        native_names: aux.native_names,
                        entry_point: aux.entry_point,
                        data_layout: aux.data_layout,
                        word_bits_log2: aux.word_bits_log2,
                        addr_bits_log2: aux.addr_bits_log2,
                        float_bits_log2: aux.float_bits_log2,
                        wcet_cycles: aux.wcet_cycles,
                        wcmu_bytes: aux.wcmu_bytes,
                        aux_arena_bytes: f.arena,
                        persistent_composite_bytes: f.persistent,
                        flags: aux.flags,
                        shared_data_bytes: aux.shared_data_bytes,
                        private_data_bytes: aux.private_data_bytes,
                        schema_hash: aux.schema_hash,
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
    fn independent_checksum_stream_crosses_windows_and_checks_the_residue() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            let mut raw = vec![37; 70001];
            let crc = keleusma_wire::crc32(&raw);
            raw.extend_from_slice(&crc.to_le_bytes());
            assert_eq!(checksum(vm, shared, &raw)?, 1);
            raw[65536] ^= 1;
            assert_eq!(checksum(vm, shared, &raw)?, 0);
            set_word(vm, shared, wire_slots::WARG, -1)?;
            assert_eq!(enter_wire(vm, shared, 220)?, -261);
            assert_eq!(enter_wire(vm, shared, 192)?, -260);
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn framing_protocol_excludes_foreign_commands_and_requires_chunk_acknowledgement() {
        let module = compile_src("fn main() -> Word { 7 }");
        let bytes = module.to_bytes().unwrap();
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                let f = frame(vm, shared, &bytes)?;
                assert_eq!(enter_wire(vm, shared, 44)?, -316);
                assert_eq!(enter_wire(vm, shared, 60)?, -316);
                set_word(vm, shared, ARG, 1)?;
                accept(vm, shared, 59)?;
                fields(
                    vm,
                    shared,
                    &[
                        module.word_bits_log2.into(),
                        module.addr_bits_log2.into(),
                        module.float_bits_log2.into(),
                        module.flags.into(),
                        module.wcet_cycles.into(),
                        module.wcmu_bytes.into(),
                        module.shared_data_bytes.into(),
                        module.private_data_bytes.into(),
                    ],
                )?;
                set_word(vm, shared, ARG, 1)?;
                accept(vm, shared, 60)?;
                assert_eq!(enter_wire(vm, shared, 63)?, -316);
                set_word(vm, shared, ARG, 0)?;
                set_word(vm, shared, ARG + 1, size(f.opcodes.len() / 4)?)?;
                accept(vm, shared, 61)?;
                assert_eq!(requested_bytes(vm, shared, f.opcodes)?, f.opcodes);
                assert_eq!(enter_wire(vm, shared, 61)?, -316);
                assert_eq!(enter_wire(vm, shared, 63)?, -316);
                accept(vm, shared, 62)?;
                accept(vm, shared, 63)?;
                assert_eq!(enter_wire(vm, shared, 62)?, -316);
                Ok(())
            },
        )
        .unwrap();
    }
}
