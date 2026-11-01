//! Debug ordering and encoding are executed by the Keleusma debug stage.

use super::wire_framing::{command, read_window, set_word, with_compiled_wire};
use super::*;
use crate::debug_meta::{DebugPool, DebugRecord};

const ARG: usize = 4097;
const FIN: usize = ARG + 1;
const BIN: usize = FIN + 1024;
const BYTE_WINDOW: usize = 4096;
const WORD_WINDOW: usize = 1024;

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire debug: {detail}"),
    }
}

fn word(vm: &dyn StageExecutor, shared: &[u8], slot: usize) -> Result<usize, SelfHostError> {
    match vm
        .get_shared(shared, slot)
        .map_err(|_| error("response read failed"))?
    {
        Value::Int(n) => usize::try_from(n).map_err(|_| error("negative response")),
        _ => Err(error("non-integer response")),
    }
}

fn fields(
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    values: &[i64],
) -> Result<(), SelfHostError> {
    if values.len() > WORD_WINDOW {
        return Err(error("field window overflow"));
    }
    for (i, &value) in values.iter().enumerate() {
        set_word(vm, shared, FIN + i, value)?;
    }
    Ok(())
}

fn header(record: &DebugRecord) -> [i64; 3] {
    [
        i64::from(record.op_index),
        i64::from(record.kind.as_u8()),
        record.operands.len() as i64,
    ]
}

fn append(
    vm: &dyn StageExecutor,
    shared: &[u8],
    size: usize,
    output: &mut Vec<u8>,
) -> Result<(), SelfHostError> {
    if size > BYTE_WINDOW {
        return Err(error("output window overflow"));
    }
    let end = output
        .len()
        .checked_add(size)
        .filter(|&n| u32::try_from(n).is_ok())
        .ok_or_else(|| error("debug pool exceeds u32"))?;
    output
        .try_reserve(size)
        .map_err(|_| error("output allocation failed"))?;
    let start = output.len();
    output.resize(end, 0);
    read_window(vm, shared, &mut output[start..])
}

// The stage supplies every merge cursor, comparison and copy decision. The host
// retains the two index arrays and transports the selected semantic records.
fn sorted_indices(
    vm: &mut dyn StageExecutor,
    shared: &mut [u8],
    records: &[DebugRecord],
) -> Result<Vec<usize>, SelfHostError> {
    let n = records.len();
    let mut order = [Vec::new(), Vec::new()];
    for buffer in &mut order {
        buffer
            .try_reserve_exact(n)
            .map_err(|_| error("sort allocation failed"))?;
        buffer.extend(0..n);
    }
    set_word(vm, shared, ARG, n as i64)?;
    if command(vm, shared, 0)? != 0 {
        return Err(error("unexpected sort start"));
    }
    let rounds = if n < 2 {
        0
    } else {
        usize::BITS - (n - 1).leading_zeros()
    } as usize;
    let budget = n
        .checked_mul(rounds)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| error("sort request budget overflow"))?;
    for _ in 0..budget {
        let action = command(vm, shared, 1)?;
        let generation = word(vm, shared, FIN + 2)?;
        if generation > 1 {
            return Err(error("invalid sort generation"));
        }
        if action == 0 {
            return Ok(core::mem::take(&mut order[generation]));
        }
        if action == 2 {
            let left = word(vm, shared, FIN)?;
            let right = word(vm, shared, FIN + 1)?;
            let a = records
                .get(
                    *order[generation]
                        .get(left)
                        .ok_or_else(|| error("invalid left cursor"))?,
                )
                .ok_or_else(|| error("invalid left record"))?;
            let b = records
                .get(
                    *order[generation]
                        .get(right)
                        .ok_or_else(|| error("invalid right cursor"))?,
                )
                .ok_or_else(|| error("invalid right record"))?;
            let [a0, a1, a2] = header(a);
            let [b0, b1, b2] = header(b);
            fields(vm, shared, &[a0, a1, a2, b0, b1, b2])?;
            let needed = command(vm, shared, 2)?;
            if needed > a.operands.len().min(b.operands.len()) {
                return Err(error("invalid comparison length"));
            }
            for start in (0..needed).step_by(WORD_WINDOW / 2) {
                let count = (needed - start).min(WORD_WINDOW / 2);
                for i in 0..count {
                    set_word(vm, shared, FIN + i * 2, i64::from(a.operands[start + i]))?;
                    set_word(
                        vm,
                        shared,
                        FIN + i * 2 + 1,
                        i64::from(b.operands[start + i]),
                    )?;
                }
                set_word(vm, shared, ARG, count as i64)?;
                if command(vm, shared, 3)? != 0 {
                    return Err(error("unexpected comparison response"));
                }
            }
            if command(vm, shared, 4)? != 1 {
                return Err(error("missing comparison copy"));
            }
        } else if action != 1 {
            return Err(error("unknown sort request"));
        }
        let source = word(vm, shared, FIN)?;
        let destination = word(vm, shared, FIN + 1)?;
        if word(vm, shared, FIN + 2)? != generation {
            return Err(error("sort generation changed during comparison"));
        }
        let value = *order[generation]
            .get(source)
            .ok_or_else(|| error("invalid sort source"))?;
        *order[1 - generation]
            .get_mut(destination)
            .ok_or_else(|| error("invalid sort destination"))? = value;
    }
    Err(error("sort exceeded its request budget"))
}

fn checked_count(n: usize) -> Result<i64, SelfHostError> {
    u32::try_from(n)
        .map(i64::from)
        .map_err(|_| error("sub-pool count or byte length exceeds u32"))
}

/// Encode a structured debug pool through Keleusma, including canonical ordering.
///
/// The stage schedules stable merge sorting and the complete output sequence.
/// Strings and opaque type blobs are copied through bounded windows. Span and
/// record fields are supplied as integers, and Keleusma packs their bytes.
/// The host never calls `DebugPool::encode` or sorts debug records itself.
///
/// # Errors
/// Rejects unrepresentable counts, oversized operand lists, allocation failures,
/// malformed stage requests and execution failures.
pub fn wire_debug_pool_via_kel(pool: &DebugPool) -> Result<Vec<u8>, SelfHostError> {
    let counts = [
        checked_count(pool.string_pool.len())?,
        checked_count(pool.span_pool.len())?,
        checked_count(pool.type_pool.len())?,
        checked_count(pool.records.len())?,
    ];
    let mut budget = 4usize;
    for bytes in pool
        .string_pool
        .iter()
        .map(|s| s.as_bytes())
        .chain(pool.type_pool.iter().map(Vec::as_slice))
    {
        checked_count(bytes.len())?;
        budget = budget
            .checked_add(1 + bytes.len().div_ceil(BYTE_WINDOW))
            .ok_or_else(|| error("request budget overflow"))?;
    }
    budget = budget
        .checked_add(pool.span_pool.len())
        .ok_or_else(|| error("request budget overflow"))?;
    for record in &pool.records {
        if record.operands.len() > u16::MAX as usize {
            return Err(error("record operand count exceeds u16"));
        }
        budget = budget
            .checked_add(1 + record.operands.len().div_ceil(WORD_WINDOW))
            .ok_or_else(|| error("request budget overflow"))?;
    }
    with_compiled_wire(
        compile_src(&read_stage("kel/wire_debug.kel")),
        |vm, shared| {
            let order = sorted_indices(vm, shared, &pool.records)?;
            fields(vm, shared, &counts)?;
            if command(vm, shared, 5)? != 4 {
                return Err(error("unexpected pool start"));
            }
            let mut output = Vec::new();
            append(vm, shared, 4, &mut output)?;
            for _ in 0..budget {
                let request = command(vm, shared, 6)?;
                if request == 0 {
                    return Ok(output);
                }
                if request == 9 {
                    append(vm, shared, 4, &mut output)?;
                    continue;
                }
                let index = word(vm, shared, FIN)?;
                let wrote = match request {
                    1 | 3 => {
                        let size = if request == 1 {
                            pool.string_pool
                                .get(index)
                                .ok_or_else(|| error("invalid string index"))?
                                .len()
                        } else {
                            pool.type_pool
                                .get(index)
                                .ok_or_else(|| error("invalid type index"))?
                                .len()
                        };
                        fields(vm, shared, &[checked_count(size)?])?;
                        let wrote = command(vm, shared, 7)?;
                        if wrote != 4 {
                            return Err(error("invalid byte-length prefix"));
                        }
                        wrote
                    }
                    2 => {
                        let &(file, offset, length) = pool
                            .span_pool
                            .get(index)
                            .ok_or_else(|| error("invalid span index"))?;
                        fields(
                            vm,
                            shared,
                            &[i64::from(file), i64::from(offset), i64::from(length)],
                        )?;
                        let wrote = command(vm, shared, 7)?;
                        if wrote != 10 {
                            return Err(error("invalid span length"));
                        }
                        wrote
                    }
                    4 => {
                        let record = pool
                            .records
                            .get(
                                *order
                                    .get(index)
                                    .ok_or_else(|| error("invalid sorted index"))?,
                            )
                            .ok_or_else(|| error("invalid record index"))?;
                        fields(vm, shared, &header(record))?;
                        let wrote = command(vm, shared, 7)?;
                        if wrote != 7 {
                            return Err(error("invalid record header length"));
                        }
                        wrote
                    }
                    5 | 7 => {
                        let offset = word(vm, shared, FIN + 1)?;
                        let bytes = if request == 5 {
                            pool.string_pool
                                .get(index)
                                .ok_or_else(|| error("invalid string index"))?
                                .as_bytes()
                        } else {
                            pool.type_pool
                                .get(index)
                                .ok_or_else(|| error("invalid type index"))?
                                .as_slice()
                        };
                        let tail = bytes
                            .get(offset..)
                            .ok_or_else(|| error("invalid byte cursor"))?;
                        let count = tail.len().min(BYTE_WINDOW);
                        if count == 0 {
                            return Err(error("byte request made no progress"));
                        }
                        for (i, &byte) in tail[..count].iter().enumerate() {
                            vm.set_shared(shared, BIN + i, Value::Byte(byte))
                                .map_err(|_| error("byte transport failed"))?;
                        }
                        set_word(vm, shared, ARG, count as i64)?;
                        let wrote = command(vm, shared, 8)?;
                        if wrote != count {
                            return Err(error("invalid byte continuation length"));
                        }
                        wrote
                    }
                    8 => {
                        let offset = word(vm, shared, FIN + 1)?;
                        let record = pool
                            .records
                            .get(
                                *order
                                    .get(index)
                                    .ok_or_else(|| error("invalid sorted index"))?,
                            )
                            .ok_or_else(|| error("invalid record index"))?;
                        let tail = record
                            .operands
                            .get(offset..)
                            .ok_or_else(|| error("invalid operand cursor"))?;
                        let count = tail.len().min(WORD_WINDOW);
                        if count == 0 {
                            return Err(error("operand request made no progress"));
                        }
                        for (i, &value) in tail[..count].iter().enumerate() {
                            set_word(vm, shared, FIN + i, i64::from(value))?;
                        }
                        set_word(vm, shared, ARG, count as i64)?;
                        let wrote = command(vm, shared, 8)?;
                        if wrote != count * 2 {
                            return Err(error("invalid operand continuation length"));
                        }
                        wrote
                    }
                    _ => return Err(error("unknown pool request")),
                };
                append(vm, shared, wrote, &mut output)?;
            }
            Err(error("pool exceeded its request budget"))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug_meta::DebugRecordKind;

    #[test]
    fn stage_merge_schedule_is_stable_and_matches_the_reference_order() {
        let records: Vec<_> = (0..1025)
            .map(|i| DebugRecord {
                op_index: ((i * 37) % 31) as u32,
                kind: DebugRecordKind::from_u8((i % 12) as u8).unwrap(),
                operands: vec![(i % 7) as u16, (i % 3) as u16],
            })
            .chain((0..5).map(|_| DebugRecord {
                op_index: 0,
                kind: DebugRecordKind::CallSite,
                operands: vec![],
            }))
            .collect();
        let mut expected: Vec<_> = (0..records.len()).collect();
        expected.sort_by(|&a, &b| {
            let a = &records[a];
            let b = &records[b];
            a.op_index
                .cmp(&b.op_index)
                .then(a.kind.cmp(&b.kind))
                .then(a.operands.cmp(&b.operands))
        });
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_debug.kel")),
            |vm, shared| {
                assert_eq!(sorted_indices(vm, shared, &records)?, expected);
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn comparison_refusals_preserve_the_pending_merge() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_debug.kel")),
            |vm, shared| {
                assert!(command(vm, shared, 1).is_err());
                for n in [-1, 0x1_0000_0000] {
                    set_word(vm, shared, ARG, n)?;
                    assert!(command(vm, shared, 0).is_err());
                }
                set_word(vm, shared, ARG, 2)?;
                command(vm, shared, 0)?;
                assert_eq!(command(vm, shared, 1)?, 2);
                assert!(command(vm, shared, 1).is_err());
                assert!(command(vm, shared, 4).is_err());
                for values in [
                    [-1, 0, 2, 0, 0, 2],
                    [0, 12, 2, 0, 0, 2],
                    [0, 0, 65536, 0, 0, 2],
                ] {
                    fields(vm, shared, &values)?;
                    assert!(command(vm, shared, 2).is_err());
                }
                fields(vm, shared, &[0, 0, 2, 0, 0, 2])?;
                assert_eq!(command(vm, shared, 2)?, 2);
                for n in [0, -1, 3, 513] {
                    set_word(vm, shared, ARG, n)?;
                    assert!(command(vm, shared, 3).is_err());
                }
                set_word(vm, shared, ARG, 1)?;
                fields(vm, shared, &[1, 65536])?;
                assert!(command(vm, shared, 3).is_err());
                fields(vm, shared, &[1, 2])?;
                command(vm, shared, 3)?;
                assert!(command(vm, shared, 4).is_err());
                fields(vm, shared, &[65535, 0])?;
                command(vm, shared, 3)?;
                assert_eq!(command(vm, shared, 4)?, 1);
                assert_eq!((word(vm, shared, FIN)?, word(vm, shared, FIN + 1)?), (0, 0));
                assert_eq!(command(vm, shared, 1)?, 1);
                assert_eq!((word(vm, shared, FIN)?, word(vm, shared, FIN + 1)?), (1, 1));
                assert_eq!(command(vm, shared, 1)?, 0);
                assert_eq!(word(vm, shared, FIN + 2)?, 1);
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn pool_protocol_refuses_incomplete_items_and_overwide_fields() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_debug.kel")),
            |vm, shared| {
                for cmd in [5, 6, 7, 8, 9] {
                    assert!(command(vm, shared, cmd).is_err());
                }
                set_word(vm, shared, ARG, 1)?;
                command(vm, shared, 0)?;
                command(vm, shared, 1)?;
                fields(vm, shared, &[1, 1, 0, 2])?;
                assert!(command(vm, shared, 5).is_err());
                fields(vm, shared, &[1, 1, 0, 1])?;
                assert_eq!(command(vm, shared, 5)?, 4);
                assert_eq!(command(vm, shared, 6)?, 1);
                fields(vm, shared, &[-1])?;
                assert!(command(vm, shared, 7).is_err());
                fields(vm, shared, &[2])?;
                assert_eq!(command(vm, shared, 7)?, 4);
                assert!(command(vm, shared, 7).is_err());
                for n in [-1, 0, 3, 4097] {
                    set_word(vm, shared, ARG, n)?;
                    assert!(command(vm, shared, 8).is_err());
                }
                vm.set_shared(shared, BIN, Value::Byte(b'a')).unwrap();
                vm.set_shared(shared, BIN + 1, Value::Byte(b'b')).unwrap();
                set_word(vm, shared, ARG, 2)?;
                assert_eq!(command(vm, shared, 8)?, 2);
                assert_eq!(command(vm, shared, 6)?, 9);
                assert_eq!(command(vm, shared, 6)?, 2);
                fields(vm, shared, &[65536, 0, 0])?;
                assert!(command(vm, shared, 7).is_err());
                fields(vm, shared, &[65535, 0xFFFF_FFFF, 0])?;
                assert_eq!(command(vm, shared, 7)?, 10);
                assert_eq!(command(vm, shared, 6)?, 9);
                assert_eq!(command(vm, shared, 6)?, 9);
                assert_eq!(command(vm, shared, 6)?, 4);
                fields(vm, shared, &[0, 12, 1])?;
                assert!(command(vm, shared, 7).is_err());
                fields(vm, shared, &[0, 11, 1])?;
                assert_eq!(command(vm, shared, 7)?, 7);
                fields(vm, shared, &[-1])?;
                set_word(vm, shared, ARG, 1)?;
                assert!(command(vm, shared, 8).is_err());
                fields(vm, shared, &[65535])?;
                assert_eq!(command(vm, shared, 8)?, 2);
                assert_eq!(command(vm, shared, 6)?, 0);
                assert!(command(vm, shared, 6).is_err());
                Ok(())
            },
        )
        .unwrap();
    }
}
