//! Module resource summaries and admission decisions are held by analyze.kel.
use super::analysis_machine::{AnalysisMachine, error};
use super::*;

const COORD: [&str; 13] = [
    "wa.tf_command",
    "wa.cg_count",
    "wa.arena_capacity",
    "wa.rs_index",
    "wa.rs_kind",
    "wa.rs_text",
    "wa.rs_known",
    "wa.out_stack_bytes",
    "wa.out_heap",
    "wa.out_reject",
    "wa.tb_heap_hi",
    "wa.tb_heap_lo",
    "wa.out_cause",
];
const CHUNK: [&str; 28] = [
    "wa.tf_command",
    "wa.op_count",
    "wa.stream_pos",
    "wa.reset_pos",
    "wa.local_count",
    "wa.value_slot_bytes",
    "wa.arena_capacity",
    "wa.region_start",
    "wa.region_end",
    "wa.cost",
    "wa.class",
    "wa.arg",
    "wa.growth",
    "wa.shrink",
    "wa.heap",
    "wa.opk",
    "wa.slot",
    "wa.cval",
    "wa.cint",
    "wa.callee_slots",
    "wa.callee_heap",
    "wa.out_stack_bytes",
    "wa.out_heap",
    "wa.out_reject",
    "wa.out_cause",
    "wa.native_kind",
    "wa.native_heap",
    "wa.native_limit",
];
fn arena(stage: &Module) -> Result<Arena, VerificationError> {
    let persistent = required_persistent_capacity_for(stage);
    let mut arena = Arena::try_with_capacity(
        persistent
            .checked_add(DEFAULT_ARENA_CAPACITY)
            .ok_or_else(|| error("arena size overflow"))?,
    )
    .map_err(error)?;
    arena.resize_persistent(persistent).map_err(error)?;
    Ok(arena)
}
fn summary(
    coordinator: &mut AnalysisMachine<'_, 13>,
    index: usize,
) -> Result<Option<(i64, i64, bool)>, VerificationError> {
    coordinator.set(3, i64::try_from(index).map_err(error)?)?;
    if coordinator.command(32)? != 0 {
        return Err(error("summary request refused"));
    }
    match coordinator.get(6)? {
        0 => Ok(None),
        1 => {
            let stack = coordinator.get(7)?;
            let heap = coordinator.get(8)?;
            let text = coordinator.get(5)?;
            if stack < 0 || heap < 0 || !(0..=1).contains(&text) {
                return Err(error("invalid summary"));
            }
            Ok(Some((stack, heap, text == 1)))
        }
        _ => Err(error("invalid summary presence")),
    }
}
fn chunk_bounds(
    worker: &mut AnalysisMachine<'_, 28>,
    coordinator: &mut AnalysisMachine<'_, 13>,
    module: &Module,
    index: usize,
    bounds: &[crate::verify::NativeIterationBound],
    value_slot_bytes: u32,
) -> Result<(i64, i64, bool), VerificationError> {
    let chunk = &module.chunks[index];
    if chunk.ops.len() > crate::selfhost_host::VERIFY_OP_CAP {
        return Err(VerificationError::Capacity {
            stage: "resources",
            resource: "opcodes",
            limit: crate::selfhost_host::VERIFY_OP_CAP,
            actual: chunk.ops.len(),
        });
    }
    let (start, end, sp, rp) = if chunk.block_type == crate::bytecode::BlockType::Stream {
        let sp = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Stream))
            .ok_or_else(|| error("missing Stream"))?;
        let rp = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Reset))
            .ok_or_else(|| error("missing Reset"))?;
        (sp + 1, rp, sp, rp)
    } else {
        (0, chunk.ops.len(), 0, 0)
    };
    for (field, value) in [
        (1, chunk.ops.len() as i64),
        (2, sp as i64),
        (3, rp as i64),
        (4, i64::from(chunk.local_count)),
        (5, i64::from(value_slot_bytes)),
        (6, i64::MAX),
        (7, start as i64),
        (8, end as i64),
    ] {
        worker.set(field, value)?;
    }
    let layout = module
        .data_layout
        .as_ref()
        .map_or(&[][..], |d| &d.shared_layout);
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        let (opk, slot, cval, cint) = analyze_opk(op, chunk);
        let (growth, shrink) = analyze_stack_effect(op);
        for (field, value) in [
            (9, i64::from(op.cost())),
            (10, class),
            (11, arg),
            (12, growth),
            (13, shrink),
            (14, analyze_op_heap(op, chunk, layout)),
            (15, opk),
            (16, slot),
            (17, cval),
            (18, cint),
        ] {
            worker.set_element(field, i, value)?;
        }
        let native = match op {
            Op::CallVerifiedNative(id, _) => Some((1, usize::from(*id))),
            Op::CallExternalNative(id, _) => Some((2, usize::from(*id))),
            _ => None,
        };
        worker.set_element(25, i, native.map_or(0, |(kind, _)| kind))?;
        if let Some((_, id)) = native {
            let bound = bounds.get(id).copied().unwrap_or_default();
            worker.set_element(16, i, id as i64)?;
            worker.set_element(26, i, i64::from(bound.per_call_wcmu_bytes))?;
            worker.set_element(27, i, bound.max_invocations.map_or(-1, i64::from))?;
        }
        let (stack, heap) = if let Op::Call(callee, _) = op {
            let (stack, heap, _) = summary(coordinator, usize::from(*callee))?
                .ok_or_else(|| error("callee resource summary missing"))?;
            (stack, heap)
        } else {
            (0, 0)
        };
        worker.set_element(19, i, stack / i64::from(value_slot_bytes))?;
        worker.set_element(20, i, heap)?;
    }
    worker.command(0)?;
    let cause = worker.get(24)?;
    match cause {
        0 => {}
        1 | 2 => {
            return Err(VerificationError::Incomplete {
                stage: "resources",
                cause,
            });
        }
        _ => return Err(error("unknown resource completion cause")),
    }
    let stack = worker.get(21)?;
    let heap = worker.get(22)?;
    let rejected = worker.get(23)?;
    if stack < 0 || heap < 0 || !(0..=1).contains(&rejected) {
        return Err(error("invalid resource result"));
    }
    Ok((stack, heap, rejected == 1))
}

pub(super) fn check(
    module: &Module,
    capacity: usize,
) -> Result<VerificationVerdict, VerificationError> {
    check_with_bounds(
        module,
        capacity,
        &[],
        crate::bytecode::VALUE_SLOT_SIZE_BYTES,
    )
}
pub(super) fn check_with_bounds(
    module: &Module,
    capacity: usize,
    bounds: &[crate::verify::NativeIterationBound],
    value_slot_bytes: u32,
) -> Result<VerificationVerdict, VerificationError> {
    check_with_stage(
        module,
        capacity,
        analyze_kel_module(),
        bounds,
        value_slot_bytes,
    )
}
fn check_with_stage(
    module: &Module,
    capacity: usize,
    stage: Module,
    bounds: &[crate::verify::NativeIterationBound],
    value_slot_bytes: u32,
) -> Result<VerificationVerdict, VerificationError> {
    if value_slot_bytes == 0 {
        return Err(error("value-slot width must be positive"));
    }
    if module.chunks.len() > 1024 {
        return Err(VerificationError::Capacity {
            stage: "resources",
            resource: "chunks",
            limit: 1024,
            actual: module.chunks.len(),
        });
    }
    let Some(order) = call_order_via_kel(module).map_err(error)? else {
        return Ok(VerificationVerdict::Rejected {
            stage: "resource cycle",
            chunk: None,
        });
    };
    let coord_arena = arena(&stage)?;
    let text_arena = arena(&stage)?;
    let chunk_arena = arena(&stage)?;
    let mut coordinator = AnalysisMachine::new(stage.clone(), &coord_arena, &COORD)?;
    let mut text = AnalysisMachine::new(stage.clone(), &text_arena, &text_bounds::FIELDS)?;
    let mut worker = AnalysisMachine::new(stage, &chunk_arena, &CHUNK)?;
    coordinator.set(1, i64::try_from(module.chunks.len()).map_err(error)?)?;
    // All reference resource terms are u32, so larger capacities are equivalent.
    coordinator.set(
        2,
        capacity.min((u32::MAX as usize).saturating_mul(2)) as i64,
    )?;
    if coordinator.command(30)? != 0 {
        return Err(error("resource reset refused"));
    }
    for index in order {
        let text_bound = text_bounds::chunk_bounds(&mut text, &module.chunks[index], |callee| {
            let (_, _, text) = summary(&mut coordinator, callee)?
                .ok_or_else(|| error("callee text summary missing"))?;
            Ok(Some(text))
        })?;
        let (stack, heap, rejected) = chunk_bounds(
            &mut worker,
            &mut coordinator,
            module,
            index,
            bounds,
            value_slot_bytes,
        )?;
        for (field, value) in [
            (3, index as i64),
            (4, block_type_tag(&module.chunks[index])),
            (5, i64::from(text_bound.returns_text)),
            (7, stack),
            (8, heap),
            (9, i64::from(rejected)),
            (10, i64::from(text_bound.heap_bytes >> 16)),
            (11, i64::from(text_bound.heap_bytes & 65535)),
        ] {
            coordinator.set(field, value)?;
        }
        match coordinator.command(31)? {
            0 => {}
            1 => {
                return Ok(VerificationVerdict::Rejected {
                    stage: "resources",
                    chunk: Some(index),
                });
            }
            other => return Err(error(other)),
        }
    }
    match coordinator.command(33)? {
        0 => Ok(VerificationVerdict::Accepted),
        1 => Ok(VerificationVerdict::Rejected {
            stage: "resources",
            chunk: None,
        }),
        other => Err(error(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_resource_analysis_cannot_become_a_verdict() {
        let module = compile_src("fn f() -> Word { 7 } loop main(n: Word) -> Word { yield f() }");
        let source = read_stage("kel/analyze.kel");
        let cases = [
            (
                "for step in 0..16384 limit 16384",
                "for step in 0..0 limit 16384",
                0,
            ),
            ("wa.out_cause = an.over;", "wa.out_cause = 77;", 1),
            (
                "rs.known[wa.rs_index] = 1;",
                "rs.known[wa.rs_index] = 0;",
                1,
            ),
        ];
        for (marker, replacement, kind) in cases {
            assert_eq!(source.matches(marker).count(), 1);
            let changed = compile_src(&source.replace(marker, replacement));
            let result = check_with_stage(
                &module,
                65536,
                changed,
                &[],
                crate::bytecode::VALUE_SLOT_SIZE_BYTES,
            );
            if kind == 0 {
                assert!(
                    matches!(
                        result,
                        Err(VerificationError::Incomplete {
                            stage: "resources",
                            cause: 2
                        })
                    ),
                    "{result:?}"
                );
            } else {
                assert!(
                    matches!(result, Err(VerificationError::Protocol { .. })),
                    "{result:?}"
                );
            }
        }
        assert_eq!(
            check(&module, 65536).unwrap(),
            VerificationVerdict::Accepted
        );
    }
    #[test]
    fn admission_and_summary_use_depend_on_stage_decisions() {
        let module = compile_src(
            "fn f() -> Word { let a = [1,2,3]; a[0] } loop main(n: Word) -> Word { yield f() }",
        );
        let bounds = crate::verify::module_wcmu(&module, &[]).unwrap();
        let i = module
            .chunks
            .iter()
            .position(|c| c.block_type == crate::bytecode::BlockType::Stream)
            .unwrap();
        let capacity = (bounds[i].0 as usize) + (bounds[i].1 as usize) - 1;
        assert!(matches!(
            check(&module, capacity).unwrap(),
            VerificationVerdict::Rejected { .. }
        ));
        let source = read_stage("kel/analyze.kel");
        let marker = "if wa.rs_kind == 2 andalso wa.out_stack_bytes + heap > wa.arena_capacity { rs.valid = 0; }";
        assert_eq!(source.matches(marker).count(), 1);
        let without_admission=compile_src(&source.replace(marker,"if wa.rs_kind == 2 andalso wa.out_stack_bytes + heap > wa.arena_capacity { rs.valid = 1; }"));
        assert_eq!(
            check_with_stage(
                &module,
                capacity,
                without_admission,
                &[],
                crate::bytecode::VALUE_SLOT_SIZE_BYTES
            )
            .unwrap(),
            VerificationVerdict::Accepted
        );
        let marker = "rs.heaps[wa.rs_index] = heap;";
        assert_eq!(source.matches(marker).count(), 1);
        let without_memo = compile_src(&source.replace(marker, "rs.heaps[wa.rs_index] = 0;"));
        assert_eq!(
            check_with_stage(
                &module,
                capacity,
                without_memo,
                &[],
                crate::bytecode::VALUE_SLOT_SIZE_BYTES
            )
            .unwrap(),
            VerificationVerdict::Accepted
        );
        assert_eq!(
            check(&module, capacity + 1).unwrap(),
            VerificationVerdict::Accepted
        );
    }
}
