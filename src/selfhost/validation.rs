//! Fallible orchestration of the Keleusma structural and typed verifier stages.
use super::*;
use alloc::collections::BTreeSet;
use core::fmt;

const OPS: usize = 1536;
const LOCALS: usize = 256;

/// A completed result for the passes configured by the called entry point.
/// `verify_module_via_kel` checks structure and types. `admit_module_via_kel`
/// additionally checks resource admission. Typed control-flow widening applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationVerdict {
    /// Every configured pass completed and accepted.
    Accepted,
    /// A stage proved a violation.
    Rejected {
        /// The pass that established the violation.
        stage: &'static str,
        /// Subject chunk index, absent for a module layout failure.
        chunk: Option<usize>,
    },
}

/// A failed analysis is not a proven invalid module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    /// An input exceeds a declared transport or stage-storage limit.
    Capacity {
        /// Stage whose storage cannot hold the input.
        stage: &'static str,
        /// Bounded storage dimension.
        resource: &'static str,
        /// Maximum admitted count.
        limit: usize,
        /// Requested count.
        actual: usize,
    },
    /// A stage reported that it could not finish its bounded walk.
    Incomplete {
        /// Stage whose walk did not complete.
        stage: &'static str,
        /// Stage-specific capacity or work-budget cause.
        cause: i64,
    },
    /// Stage construction or execution failed.
    Execution {
        /// Failed stage.
        stage: &'static str,
        /// Underlying execution diagnostic.
        detail: String,
    },
    /// A transport response or module width cannot be represented by this driver.
    Protocol {
        /// Stage or module boundary with an invalid transport value.
        stage: &'static str,
        /// Failed protocol condition.
        detail: String,
    },
}
impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity {
                stage,
                resource,
                limit,
                actual,
            } => write!(f, "{stage} {resource} capacity {limit}, requested {actual}"),
            Self::Incomplete { stage, cause } => {
                write!(f, "{stage} did not complete, cause {cause}")
            }
            Self::Execution { stage, detail } | Self::Protocol { stage, detail } => {
                write!(f, "{stage} unavailable: {detail}")
            }
        }
    }
}
impl std::error::Error for VerificationError {}

type Result<T> = core::result::Result<T, VerificationError>;
fn execution(stage: &'static str, error: impl fmt::Debug) -> VerificationError {
    VerificationError::Execution {
        stage,
        detail: format!("{error:?}"),
    }
}
fn protocol(stage: &'static str, detail: impl Into<String>) -> VerificationError {
    VerificationError::Protocol {
        stage,
        detail: detail.into(),
    }
}
fn capacity(
    stage: &'static str,
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<()> {
    if actual > limit {
        Err(VerificationError::Capacity {
            stage,
            resource,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}
fn word(vm: &Vm<'_, '_>, shared: &[u8], slot: usize, stage: &'static str) -> Result<i64> {
    match vm
        .get_shared(shared, slot)
        .map_err(|e| execution(stage, e))?
    {
        Value::Int(value) => Ok(value),
        _ => Err(protocol(stage, "non-word response")),
    }
}
fn set(
    vm: &Vm<'_, '_>,
    shared: &mut [u8],
    slot: usize,
    value: i64,
    stage: &'static str,
) -> Result<()> {
    vm.set_shared(shared, slot, Value::Int(value))
        .map_err(|e| execution(stage, e))
}
fn size(value: usize, stage: &'static str) -> Result<i64> {
    i64::try_from(value).map_err(|_| protocol(stage, "count exceeds transport word"))
}
fn flag(value: i64, stage: &'static str) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(protocol(stage, "non-Boolean verdict")),
    }
}
fn completed(vm: &Vm<'_, '_>, shared: &[u8], slot: usize, stage: &'static str) -> Result<()> {
    match word(vm, shared, slot, stage)? {
        0 => Ok(()),
        cause if (1..=if stage == "types" { 3 } else { 2 }).contains(&cause) => {
            Err(VerificationError::Incomplete { stage, cause })
        }
        _ => Err(protocol(stage, "unknown completion cause")),
    }
}
fn verdict(
    vm: &Vm<'_, '_>,
    shared: &[u8],
    slot: usize,
    yielded: i64,
    stage: &'static str,
) -> Result<bool> {
    let stored = word(vm, shared, slot, stage)?;
    if stored != yielded {
        return Err(protocol(stage, "yielded and stored verdicts disagree"));
    }
    flag(stored, stage)
}
fn arena(module: &Module, stage: &'static str) -> Result<Arena> {
    let persistent = required_persistent_capacity_for(module);
    let capacity = persistent
        .checked_add(DEFAULT_ARENA_CAPACITY)
        .ok_or_else(|| protocol(stage, "arena size overflow"))?;
    let mut arena = Arena::try_with_capacity(capacity).map_err(|e| execution(stage, e))?;
    arena
        .resize_persistent(persistent)
        .map_err(|e| execution(stage, e))?;
    Ok(arena)
}
fn command(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    started: &mut bool,
    stage: &'static str,
) -> Result<i64> {
    let mut state = if *started {
        vm.resume_with_shared(shared, Value::Int(0))
    } else {
        *started = true;
        vm.call_with_shared(shared, &[Value::Int(0)])
    }
    .map_err(|e| execution(stage, e))?;
    for _ in 0..3 {
        match state {
            VmState::Yielded(Value::Int(value)) => return Ok(value),
            VmState::Reset => {
                state = vm
                    .resume_with_shared(shared, Value::Int(0))
                    .map_err(|e| execution(stage, e))?
            }
            other => return Err(execution(stage, other)),
        }
    }
    Err(protocol(stage, "coroutine did not yield a result"))
}
fn run<T>(
    stage: &'static str,
    module: Module,
    seed: impl FnOnce(&Vm<'_, '_>) -> Result<Vec<u8>>,
    result: impl FnOnce(&Vm<'_, '_>, &[u8], i64) -> Result<T>,
) -> Result<T> {
    let arena = arena(&module, stage)?;
    let mut vm = Vm::new(module, &arena).map_err(|e| execution(stage, e))?;
    let mut shared = seed(&vm)?;
    let yielded = command(&mut vm, &mut shared, &mut false, stage)?;
    result(&vm, &shared, yielded)
}
fn widths(module: &Module) -> Result<(usize, usize, usize)> {
    if !(3..=6).contains(&module.word_bits_log2)
        || !(3..=6).contains(&module.addr_bits_log2)
        || module.float_bits_log2 > 6
    {
        return Err(protocol("module", "unsupported scalar-width metadata"));
    }
    Ok((
        1usize << (module.word_bits_log2 - 3),
        (1usize << module.float_bits_log2) / 8,
        1usize << (module.addr_bits_log2 - 3),
    ))
}
fn structure(
    module: &Module,
    chunk: &crate::bytecode::Chunk,
    always: &BTreeSet<usize>,
    early: bool,
) -> Result<bool> {
    run(
        "structure",
        verify_structural_kel_module(),
        |vm| {
            let mut shared = seed_verify_structural_shared(vm, module, chunk, always);
            set(
                vm,
                &mut shared,
                SV_OUT_REJECT + 1,
                i64::from(early),
                "structure",
            )?;
            Ok(shared)
        },
        |vm, shared, yielded| verdict(vm, shared, SV_OUT_REJECT, yielded, "structure"),
    )
}
fn depth(chunk: &crate::bytecode::Chunk) -> Result<bool> {
    depth_with_stage(chunk, verify_depth_kel_module())
}
fn depth_with_stage(chunk: &crate::bytecode::Chunk, stage: Module) -> Result<bool> {
    run(
        "depth",
        stage,
        |vm| Ok(seed_verify_depth_shared(vm, chunk)),
        |vm, shared, yielded| {
            completed(vm, shared, DV_OUT_CAUSE, "depth")?;
            verdict(vm, shared, DV_OUT_REJECT, yielded, "depth")
        },
    )
}
#[cfg(test)]
fn yield_region(
    chunk: &crate::bytecode::Chunk,
    start: usize,
    end: usize,
    always: &BTreeSet<usize>,
) -> Result<(bool, bool)> {
    yield_with_stage(chunk, start, end, always, verify_yield_kel_module())
}
#[cfg(test)]
fn yield_with_stage(
    chunk: &crate::bytecode::Chunk,
    start: usize,
    end: usize,
    always: &BTreeSet<usize>,
    stage: Module,
) -> Result<(bool, bool)> {
    yield_using_lookup(chunk, start, end, |i| Ok(always.contains(&i)), stage)
}
pub(super) fn yield_using_lookup(
    chunk: &crate::bytecode::Chunk,
    start: usize,
    end: usize,
    lookup: impl FnMut(usize) -> Result<bool>,
    stage: Module,
) -> Result<(bool, bool)> {
    yield_summary(chunk, start, end, lookup, stage).map(|(fell, yielded, _)| (fell, yielded))
}
fn yield_summary(
    chunk: &crate::bytecode::Chunk,
    start: usize,
    end: usize,
    mut lookup: impl FnMut(usize) -> Result<bool>,
    stage: Module,
) -> Result<(bool, bool, bool)> {
    run(
        "productivity",
        stage,
        |vm| {
            let mut shared = vec![0; vm.shared_data_bytes()];
            for (slot, value) in [
                (YV_OP_COUNT, chunk.ops.len()),
                (YV_REGION_START, start),
                (YV_REGION_END, end),
            ] {
                set(
                    vm,
                    &mut shared,
                    slot,
                    size(value, "productivity")?,
                    "productivity",
                )?;
            }
            for (i, op) in chunk.ops.iter().enumerate() {
                let (class, arg) = analyze_class(op);
                for (slot, value) in [
                    (YV_CLASS + i, class),
                    (YV_ARG + i, arg),
                    (YV_MARK + i, structural_marker(op)),
                    (
                        YV_CAY + i,
                        i64::from(match op {
                            Op::Call(c, _) => lookup(usize::from(*c))?,
                            _ => false,
                        }),
                    ),
                ] {
                    set(vm, &mut shared, slot, value, "productivity")?;
                }
            }
            Ok(shared)
        },
        |vm, shared, yielded| {
            completed(vm, shared, YV_OUT_HY + 1, "productivity")?;
            Ok((
                verdict(vm, shared, YV_OUT_FELL, yielded, "productivity")?,
                flag(word(vm, shared, YV_OUT_HY, "productivity")?, "productivity")?,
                flag(
                    word(vm, shared, YV_OUT_REJECT, "productivity")?,
                    "productivity",
                )?,
            ))
        },
    )
}
fn typed(module: &Module, index: usize, widths: (usize, usize, usize)) -> Result<bool> {
    typed_with_stage(module, index, widths, verify_typed_kel_module())
}
fn typed_with_stage(
    module: &Module,
    index: usize,
    widths: (usize, usize, usize),
    stage: Module,
) -> Result<bool> {
    let chunk = &module.chunks[index];
    capacity("types", "locals", usize::from(chunk.local_count), LOCALS)?;
    if let Some(sig) = module.signatures.get(index) {
        capacity("types", "parameter seeds", sig.params.len(), LOCALS)?;
    }
    capacity("types", "enum layouts", module.enum_layouts.len(), 256)?;
    let count = chunk
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::Call(callee, n) => module
                .signatures
                .get(usize::from(*callee))
                .map(|s| s.params.len().min(usize::from(*n))),
            _ => None,
        })
        .try_fold(0usize, |sum, n| {
            sum.checked_add(n)
                .ok_or_else(|| protocol("types", "shape pool count overflow"))
        })?;
    capacity("types", "call shapes", count, TV_CP_CAPACITY)?;
    run(
        "types",
        stage,
        |vm| {
            Ok(seed_typed_shared(
                vm,
                module,
                chunk,
                TypedSeeding::Module(module.signatures.get(index)),
                widths.0,
                widths.1,
                widths.2,
            ))
        },
        |vm, shared, yielded| {
            completed(vm, shared, TV_CP_COUNT + OPS, "types")?;
            verdict(vm, shared, TV_OUT_REJECT, yielded, "types")
        },
    )
}
fn layout(module: &Module, widths: (usize, usize, usize)) -> Result<bool> {
    use crate::bytecode::{SHARED_SLOT_COMPOSITE_FLAG, SlotVisibility};
    let Some(layout) = module.data_layout.as_ref() else {
        return Ok(false);
    };
    let m = verify_datalayout_kel_module();
    let arena = arena(&m, "data layout")?;
    let mut vm = Vm::new(m, &arena).map_err(|e| execution("data layout", e))?;
    let mut shared = vec![0; vm.shared_data_bytes()];
    let mut started = false;
    for (slot, value) in [
        (DL_N_SLOTS, layout.slots.len()),
        (DL_BUFFER, module.shared_data_bytes as usize),
        (DL_POOL, module.persistent_composite_bytes as usize),
        (DL_A2 + DL_BATCH, layout.shared_layout.len()),
    ] {
        set(
            &vm,
            &mut shared,
            slot,
            size(value, "data layout")?,
            "data layout",
        )?;
    }
    command(&mut vm, &mut shared, &mut started, "data layout")?;
    for phase in 1..=3 {
        let len = match phase {
            1 => layout.slots.len(),
            2 => layout.shared_layout.len(),
            _ => layout.private_composite_layout.len(),
        };
        for start in (0..len).step_by(DL_BATCH) {
            let count = (len - start).min(DL_BATCH);
            set(&vm, &mut shared, DL_PHASE, phase, "data layout")?;
            set(
                &vm,
                &mut shared,
                DL_COUNT,
                size(count, "data layout")?,
                "data layout",
            )?;
            for j in 0..count {
                let (a, b, c) = match phase {
                    1 => (
                        i64::from(matches!(
                            layout.slots[start + j].visibility,
                            SlotVisibility::Shared
                        )),
                        0,
                        0,
                    ),
                    2 => {
                        let row = &layout.shared_layout[start + j];
                        let extent = if row.kind & SHARED_SLOT_COMPOSITE_FLAG != 0 {
                            Some(usize::from(row.len))
                        } else {
                            ScalarKind::from_tag(row.kind)
                                .map(|k| k.size_in_bytes(widths.0, widths.1, widths.2))
                        };
                        (
                            i64::from(row.offset),
                            size(extent.unwrap_or(0), "data layout")?,
                            i64::from(extent.is_none()),
                        )
                    }
                    _ => {
                        let row = &layout.private_composite_layout[start + j];
                        (i64::from(row.slot), i64::from(row.offset), 0)
                    }
                };
                for (slot, value) in [(DL_A0 + j, a), (DL_A1 + j, b), (DL_A2 + j, c)] {
                    set(&vm, &mut shared, slot, value, "data layout")?;
                }
            }
            command(&mut vm, &mut shared, &mut started, "data layout")?;
        }
    }
    set(&vm, &mut shared, DL_PHASE, 4, "data layout")?;
    flag(
        command(&mut vm, &mut shared, &mut started, "data layout")?,
        "data layout",
    )
}

/// Run the bounded Keleusma counterpart of `verify`, without executing subject code.
/// This checks structure, depth, productivity, typed operands and data layout.
/// Resource admission is provided by `admit_module_via_kel`. Capacity or incomplete
/// analysis returns an error rather than an invalid-module verdict.
/// Typed control-flow widening remains as documented by the stage.
pub fn verify_module_via_kel(module: &Module) -> Result<VerificationVerdict> {
    let widths = widths(module)?;
    let empty = BTreeSet::new();
    for (index, chunk) in module.chunks.iter().enumerate() {
        capacity("structure", "opcodes", chunk.ops.len(), OPS)?;
        if structure(module, chunk, &empty, true)? {
            return Ok(VerificationVerdict::Rejected {
                stage: "structure",
                chunk: Some(index),
            });
        }
    }
    for (index, chunk) in module.chunks.iter().enumerate() {
        if depth(chunk)? {
            return Ok(VerificationVerdict::Rejected {
                stage: "depth",
                chunk: Some(index),
            });
        }
    }
    let always = super::yield_fixpoint::always(module)?;
    for (index, chunk) in module.chunks.iter().enumerate() {
        if structure(module, chunk, &always, false)? {
            return Ok(VerificationVerdict::Rejected {
                stage: "structure",
                chunk: Some(index),
            });
        }
        if chunk.block_type == crate::bytecode::BlockType::Stream {
            let start = chunk
                .ops
                .iter()
                .position(|op| matches!(op, Op::Stream))
                .ok_or_else(|| protocol("productivity", "validated stream missing marker"))?
                + 1;
            let end = chunk
                .ops
                .iter()
                .position(|op| matches!(op, Op::Reset))
                .ok_or_else(|| protocol("productivity", "validated stream missing reset"))?;
            let (_, _, rejected) = yield_summary(
                chunk,
                start,
                end,
                |i| Ok(always.contains(&i)),
                verify_yield_kel_module(),
            )?;
            if rejected {
                return Ok(VerificationVerdict::Rejected {
                    stage: "productivity",
                    chunk: Some(index),
                });
            }
        }
    }
    if layout(module, widths)? {
        return Ok(VerificationVerdict::Rejected {
            stage: "data layout",
            chunk: None,
        });
    }
    for index in 0..module.chunks.len() {
        if typed(module, index, widths)? {
            return Ok(VerificationVerdict::Rejected {
                stage: "types",
                chunk: Some(index),
            });
        }
    }
    Ok(VerificationVerdict::Accepted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn productivity_verdict_is_decided_by_the_stage() {
        let module = compile_src("loop main(n: Word) -> Word { yield n }");
        let chunk = &module.chunks[0];
        let start = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Stream))
            .unwrap()
            + 1;
        let end = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Reset))
            .unwrap();
        let stage = verify_yield_kel_module();
        assert!(
            !yield_summary(chunk, start, end, |_| Ok(false), stage.clone())
                .unwrap()
                .2
        );
        let mut bad = chunk.clone();
        for op in &mut bad.ops {
            if matches!(op, Op::Yield) {
                *op = Op::PopN(0);
            }
        }
        assert!(
            yield_summary(&bad, start, end, |_| Ok(false), stage)
                .unwrap()
                .2
        );
        let source = read_stage("kel/verify_yield.kel");
        let marker =
            "yv.out_reject = if yf.child_fell == 1 andalso yf.child_hy == 0 { 1 } else { 0 };";
        assert_eq!(source.matches(marker).count(), 1);
        let changed = compile_src(&source.replace(marker, "yv.out_reject = 0;"));
        assert!(
            !yield_summary(&bad, start, end, |_| Ok(false), changed)
                .unwrap()
                .2
        );
    }

    #[test]
    fn unfinished_walks_are_errors_for_identical_subjects() {
        let module = compile_src("fn main() -> Word { 7 }");
        let shorten = |name, before| {
            let source = read_stage(name);
            assert_eq!(source.matches(before).count(), 1);
            compile_src(&source.replace(before, "in 0..0 limit 8192"))
        };
        assert!(!depth(&module.chunks[0]).unwrap());
        assert!(matches!(
            depth_with_stage(
                &module.chunks[0],
                shorten("kel/verify_depth.kel", "in 0..8192 limit 8192")
            ),
            Err(VerificationError::Incomplete {
                stage: "depth",
                cause: 2
            })
        ));
        let always = BTreeSet::new();
        assert!(yield_region(&module.chunks[0], 0, module.chunks[0].ops.len(), &always).is_ok());
        assert!(matches!(
            yield_with_stage(
                &module.chunks[0],
                0,
                module.chunks[0].ops.len(),
                &always,
                shorten("kel/verify_yield.kel", "in 0..8192 limit 8192")
            ),
            Err(VerificationError::Incomplete {
                stage: "productivity",
                cause: 2
            })
        ));
        let widths = widths(&module).unwrap();
        assert!(!typed(&module, 0, widths).unwrap());
        assert!(matches!(
            typed_with_stage(
                &module,
                0,
                widths,
                shorten("kel/verify_typed.kel", "in 0..8192 limit 8192")
            ),
            Err(VerificationError::Incomplete {
                stage: "types",
                cause: 3
            })
        ));
    }

    #[test]
    fn failed_stage_execution_is_not_a_rejection() {
        let stage = compile_src("loop main(n: Word) -> Word { yield 1 / n }");
        let result = run(
            "fault",
            stage,
            |vm| Ok(vec![0; vm.shared_data_bytes()]),
            |_, _, _| Ok(false),
        );
        assert!(matches!(
            result,
            Err(VerificationError::Execution { stage: "fault", .. })
        ));
    }
    #[test]
    fn malformed_stage_responses_are_protocol_errors() {
        let module = compile_src("fn main() -> Word { 7 }");
        let source = read_stage("kel/verify_typed.kel");
        let unknown = source.replace("tv.out_cause = tf.over;", "tv.out_cause = 99;");
        assert_ne!(source, unknown);
        assert!(matches!(
            typed_with_stage(&module, 0, widths(&module).unwrap(), compile_src(&unknown)),
            Err(VerificationError::Protocol { stage: "types", .. })
        ));
        let wrong = source.replace("    tv.out_reject\n}", "    1 - tv.out_reject\n}");
        assert_ne!(source, wrong);
        assert!(matches!(
            typed_with_stage(&module, 0, widths(&module).unwrap(), compile_src(&wrong)),
            Err(VerificationError::Protocol { stage: "types", .. })
        ));
    }
}
