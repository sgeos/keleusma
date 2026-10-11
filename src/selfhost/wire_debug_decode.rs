//! Parameter tags and debug pools decoded through schema-stage requests.

use super::wire_framing::{get_word, set_word, with_compiled_wire};
use super::wire_semantics::{
    ARG, BYTES, FIN, NAME_WINDOW, byte_count, command, requested_bytes, validate_name_bytes,
};
use super::*;
use crate::bytecode::TypeTag;
use crate::debug_meta::{DebugPool, DebugRecord, DebugRecordKind};

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire debug decode: {detail}"),
    }
}
pub(super) fn copy_window(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    bytes: &[u8],
) -> Result<(), SelfHostError> {
    if bytes.len() > NAME_WINDOW {
        return Err(error("requested field exceeds byte window"));
    }
    for (i, &value) in bytes.iter().enumerate() {
        vm.set_shared(shared, BYTES + i, Value::Byte(value))
            .map_err(|_| error("byte transport failed"))?;
    }
    set_word(vm, shared, ARG, bytes.len() as i64)
}
fn word(vm: &Vm<'_, '_>, shared: &[u8], slot: usize) -> Result<u32, SelfHostError> {
    u32::try_from(get_word(vm, shared, slot)?).map_err(|_| error("word response exceeds u32"))
}
fn short(vm: &Vm<'_, '_>, shared: &[u8], slot: usize) -> Result<u16, SelfHostError> {
    u16::try_from(get_word(vm, shared, slot)?).map_err(|_| error("short response exceeds u16"))
}
fn record_kind(form: usize) -> Result<DebugRecordKind, SelfHostError> {
    Ok(match form {
        1 => DebugRecordKind::CallSite,
        2 => DebugRecordKind::SourceSpan,
        3 => DebugRecordKind::LineNumber,
        4 => DebugRecordKind::VariableName,
        5 => DebugRecordKind::TypeAnnotation,
        6 => DebugRecordKind::AssertionContext,
        7 => DebugRecordKind::BreakpointCandidate,
        8 => DebugRecordKind::GenericInstantiation,
        9 => DebugRecordKind::IfcLabelAnnotation,
        10 => DebugRecordKind::WcetMarker,
        11 => DebugRecordKind::OptimisationMarker,
        12 => DebugRecordKind::VerifierWitness,
        _ => return Err(error("unknown semantic debug kind")),
    })
}

/// Decode a parameter-type range, using the auxiliary decoder's absent-pool policy.
///
/// The stage validates the range and each wire tag. Float remains a recognized
/// type tag with or without runtime floating-point support, matching the codec.
///
/// # Errors
/// Refuses invalid ranges or tags and propagates stage/transport/allocation failures.
pub fn wire_decode_parameter_tags_via_kel(
    pool: Option<&[u8]>,
    range: (u32, u32),
) -> Result<Vec<TypeTag>, SelfHostError> {
    with_compiled_wire(
        compile_src(&read_stage("kel/wire_schema.kel")),
        |vm, shared| decode_parameters(vm, shared, pool, range),
    )
}

pub(super) fn decode_parameters(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    pool: Option<&[u8]>,
    range: (u32, u32),
) -> Result<Vec<TypeTag>, SelfHostError> {
    for (i, value) in [byte_count(pool)?, i64::from(range.0), i64::from(range.1)]
        .into_iter()
        .enumerate()
    {
        set_word(vm, shared, ARG + i, value)?;
    }
    match command(vm, shared, 41)? {
        0 => return Ok(Vec::new()),
        1 => {}
        _ => return Err(error("unknown parameter presence response")),
    }
    let raw = requested_bytes(
        vm,
        shared,
        pool.ok_or_else(|| error("parameter pool absent after validation"))?,
    )?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(raw.len())
        .map_err(|_| error("parameter allocation failed"))?;
    for chunk in raw.chunks(16) {
        copy_window(vm, shared, chunk)?;
        let n = command(vm, shared, 42)?;
        if n != chunk.len() {
            return Err(error("parameter count disagreement"));
        }
        for i in 0..n {
            result.push(match get_word(vm, shared, FIN + i)? {
                0 => TypeTag::Composite,
                1 => TypeTag::Byte,
                2 => TypeTag::Word,
                3 => TypeTag::Fixed,
                4 => TypeTag::Float,
                5 => TypeTag::Bool,
                6 => TypeTag::Unit,
                7 => TypeTag::Text,
                _ => return Err(error("unknown semantic parameter type")),
            });
        }
    }
    if command(vm, shared, 43)? != 0 {
        return Err(error("parameter stream incomplete"));
    }
    Ok(result)
}

pub(super) fn decode_debug(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    bytes: &[u8],
) -> Result<DebugPool, SelfHostError> {
    set_word(
        vm,
        shared,
        ARG,
        i64::try_from(bytes.len()).map_err(|_| error("debug input exceeds word range"))?,
    )?;
    if command(vm, shared, 44)? != 0 {
        return Err(error("debug begin response invalid"));
    }
    let limit = bytes
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(16))
        .ok_or_else(|| error("debug request budget overflow"))?;
    let mut result = DebugPool::default();
    let mut expected = [None; 4];
    let mut record: Option<DebugRecord> = None;
    let mut operand_count = 0;
    for _ in 0..limit {
        let mut event = command(vm, shared, 45)?;
        if event == 0 {
            if record.is_some()
                || expected
                    != [
                        Some(result.string_pool.len()),
                        Some(result.span_pool.len()),
                        Some(result.type_pool.len()),
                        Some(result.records.len()),
                    ]
            {
                return Err(error("debug pool counts disagree"));
            }
            return Ok(result);
        }
        if event == 1 {
            let field = requested_bytes(vm, shared, bytes)?;
            copy_window(vm, shared, field)?;
            event = command(vm, shared, 46)?;
        }
        match event {
            2 => {
                let count = get_word(vm, shared, FIN)?;
                let pool = get_word(vm, shared, FIN + 1)?;
                let slot = expected
                    .get_mut(pool)
                    .ok_or_else(|| error("unknown debug sub-pool"))?;
                if slot.is_some() {
                    return Err(error("debug count repeated"));
                }
                *slot = Some(count);
                let allocation = match pool {
                    0 => result.string_pool.try_reserve_exact(count),
                    1 => result.span_pool.try_reserve_exact(count),
                    2 => result.type_pool.try_reserve_exact(count),
                    3 => result.records.try_reserve_exact(count),
                    _ => return Err(error("unknown debug sub-pool")),
                };
                allocation.map_err(|_| error("debug pool allocation failed"))?;
            }
            3 => {
                let raw = requested_bytes(vm, shared, bytes)?;
                validate_name_bytes(vm, shared, raw)?;
                let text = core::str::from_utf8(raw)
                    .map_err(|_| error("stage accepted invalid debug text"))?;
                let mut value = String::new();
                value
                    .try_reserve_exact(text.len())
                    .map_err(|_| error("debug string allocation failed"))?;
                value.push_str(text);
                result.string_pool.push(value);
                if command(vm, shared, 47)? != 0 {
                    return Err(error("debug string acknowledgement failed"));
                }
            }
            4 => result.span_pool.push((
                short(vm, shared, FIN)?,
                word(vm, shared, FIN + 1)?,
                word(vm, shared, FIN + 2)?,
            )),
            5 => {
                let raw = requested_bytes(vm, shared, bytes)?;
                let mut value = Vec::new();
                value
                    .try_reserve_exact(raw.len())
                    .map_err(|_| error("debug type allocation failed"))?;
                value.extend_from_slice(raw);
                result.type_pool.push(value);
                if command(vm, shared, 47)? != 0 {
                    return Err(error("debug type acknowledgement failed"));
                }
            }
            6 => {
                if record.is_some() {
                    return Err(error("record began before previous completion"));
                }
                let op_index = word(vm, shared, FIN)?;
                let kind = record_kind(get_word(vm, shared, FIN + 1)?)?;
                operand_count = get_word(vm, shared, FIN + 2)?;
                let mut operands = Vec::new();
                operands
                    .try_reserve_exact(operand_count)
                    .map_err(|_| error("debug operand allocation failed"))?;
                record = Some(DebugRecord {
                    op_index,
                    kind,
                    operands,
                });
            }
            7 => {
                let value = record
                    .take()
                    .ok_or_else(|| error("record completed before beginning"))?;
                if value.operands.len() != operand_count {
                    return Err(error("operand count disagreement"));
                }
                result.records.push(value);
                if command(vm, shared, 47)? != 0 {
                    return Err(error("record acknowledgement failed"));
                }
            }
            8 => {
                let n = get_word(vm, shared, FIN)?;
                let current = record
                    .as_mut()
                    .ok_or_else(|| error("operands arrived outside record"))?;
                if n == 0 || n > 15 || n > operand_count - current.operands.len() {
                    return Err(error("invalid operand batch"));
                }
                for i in 0..n {
                    current.operands.push(short(vm, shared, FIN + i + 1)?);
                }
            }
            _ => return Err(error("unknown debug request")),
        }
    }
    Err(error("debug request budget exhausted"))
}

/// Decode debug metadata through Keleusma-controlled cursor and tag interpretation.
///
/// Input record order, trailing bytes and opaque type blobs retain reference
/// behavior. Pool indices and record arity are interpreted by later consumers.
///
/// # Errors
/// Refuses truncated fields, unknown record kinds and malformed string text,
/// and propagates stage, transport and allocation failures.
pub fn wire_decode_debug_pool_via_kel(bytes: &[u8]) -> Result<DebugPool, SelfHostError> {
    with_compiled_wire(
        compile_src(&read_stage("kel/wire_schema.kel")),
        |vm, shared| decode_debug(vm, shared, bytes),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_requests_refuse_wrong_lengths_and_foreign_transactions() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                set_word(vm, shared, ARG, 16)?;
                assert_eq!(command(vm, shared, 44)?, 0);
                assert_eq!(command(vm, shared, 45)?, 1);
                assert_eq!(enter_wire(vm, shared, 0)?, -311);
                copy_window(vm, shared, &[0; 3])?;
                assert_eq!(enter_wire(vm, shared, 46)?, -311);
                copy_window(vm, shared, &[0; 4])?;
                assert_eq!(command(vm, shared, 46)?, 2);
                assert_eq!(get_word(vm, shared, FIN)?, 0);
                for pool in 1..4 {
                    assert_eq!(command(vm, shared, 45)?, 1);
                    copy_window(vm, shared, &[0; 4])?;
                    assert_eq!(command(vm, shared, 46)?, 2);
                    assert_eq!(get_word(vm, shared, FIN + 1)?, pool);
                }
                assert_eq!(command(vm, shared, 45)?, 0);
                assert_eq!(enter_wire(vm, shared, 47)?, -311);
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn parameter_protocol_refuses_overfeed_and_premature_completion() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                for (i, n) in [2, 0, 2].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, n)?;
                }
                assert_eq!(command(vm, shared, 41)?, 1);
                assert_eq!(enter_wire(vm, shared, 43)?, -310);
                assert_eq!(enter_wire(vm, shared, 44)?, -310);
                copy_window(vm, shared, &[3, 5, 7])?;
                assert_eq!(enter_wire(vm, shared, 42)?, -310);
                copy_window(vm, shared, &[3, 5])?;
                assert_eq!(command(vm, shared, 42)?, 2);
                assert_eq!(get_word(vm, shared, FIN)?, 2);
                assert_eq!(get_word(vm, shared, FIN + 1)?, 4);
                assert_eq!(command(vm, shared, 43)?, 0);
                assert_eq!(enter_wire(vm, shared, 42)?, -310);
                Ok(())
            },
        )
        .unwrap();
    }
}
