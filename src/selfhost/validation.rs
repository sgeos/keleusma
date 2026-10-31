//! Fallible orchestration of the Keleusma structural and typed verifier stages.
use super::*;
use alloc::collections::BTreeSet;
use core::fmt;

const OPS: usize = crate::selfhost_host::VERIFY_OP_CAP;
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
fn word(vm: &dyn StageExecutor, shared: &[u8], slot: usize, stage: &'static str) -> Result<i64> {
    match vm
        .get_shared(shared, slot)
        .map_err(|e| execution(stage, e))?
    {
        Value::Int(value) => Ok(value),
        _ => Err(protocol(stage, "non-word response")),
    }
}
fn set(
    vm: &dyn StageExecutor,
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
fn completed(
    vm: &dyn StageExecutor,
    shared: &[u8],
    slot: usize,
    stage: &'static str,
) -> Result<()> {
    match word(vm, shared, slot, stage)? {
        0 => Ok(()),
        cause if (1..=if stage == "types" { 3 } else { 2 }).contains(&cause) => {
            Err(VerificationError::Incomplete { stage, cause })
        }
        _ => Err(protocol(stage, "unknown completion cause")),
    }
}
fn verdict(
    vm: &dyn StageExecutor,
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
    vm: &mut dyn StageExecutor,
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
    seed: impl FnOnce(&dyn StageExecutor) -> Result<Vec<u8>>,
    result: impl FnOnce(&dyn StageExecutor, &[u8], i64) -> Result<T>,
) -> Result<T> {
    let arena = arena(&module, stage)?;
    let mut vm = stage_vm(module, &arena).map_err(|e| execution(stage, e))?;
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
#[cfg(test)]
fn depth(chunk: &crate::bytecode::Chunk) -> Result<bool> {
    depth_with_stage(chunk, verify_depth_kel_module())
}
#[cfg(test)]
fn depth_with_stage(chunk: &crate::bytecode::Chunk, stage: Module) -> Result<bool> {
    depth_and_initialization(chunk, stage).map(|(depth, _)| depth)
}
/// The definite-initialization verdict the depth stage publishes beside its own.
/// `Some` holds the lowest refused instruction. A nonzero cause is unavailable
/// analysis, never a verdict.
fn initialization(
    vm: &dyn StageExecutor,
    shared: &[u8],
    chunk: &crate::bytecode::Chunk,
) -> Result<Option<usize>> {
    const STAGE: &str = "initialization";
    match word(vm, shared, DV_OUT_INIT_CAUSE, STAGE)? {
        0 => {
            if !flag(word(vm, shared, DV_OUT_INIT_REJECT, STAGE)?, STAGE)? {
                return Ok(None);
            }
            match usize::try_from(word(vm, shared, DV_OUT_INIT_IP, STAGE)?) {
                Ok(ip) if ip < chunk.ops.len() => Ok(Some(ip)),
                _ => Err(protocol(STAGE, "refused instruction outside the chunk")),
            }
        }
        1 => Err(VerificationError::Capacity {
            stage: STAGE,
            resource: "locals",
            limit: DV_INIT_LOCALS,
            actual: usize::from(chunk.local_count),
        }),
        2 => Err(VerificationError::Incomplete {
            stage: STAGE,
            cause: 2,
        }),
        _ => Err(protocol(STAGE, "unknown completion cause")),
    }
}
/// One run of the depth stage, which decides operand-stack depth and definite
/// initialization together. A failed depth analysis fails the call. The
/// initialization result is returned unresolved, so that the caller can order it
/// as the reference does.
fn depth_and_initialization(
    chunk: &crate::bytecode::Chunk,
    stage: Module,
) -> Result<(bool, Result<Option<usize>>)> {
    run(
        "depth",
        stage,
        |vm| Ok(seed_verify_depth_shared(vm, chunk)),
        |vm, shared, yielded| {
            completed(vm, shared, DV_OUT_CAUSE, "depth")?;
            let depth = verdict(vm, shared, DV_OUT_REJECT, yielded, "depth")?;
            Ok((depth, initialization(vm, shared, chunk)))
        },
    )
}
/// The chunk and instruction of the first read the self-hosted
/// definite-initialization rule refuses, in the order
/// `crate::verify_init::verify_definite_initialization` reports, without the
/// other passes. Capacity or unfinished analysis is an error.
pub fn initialization_rejects_via_kel(module: &Module) -> Result<Option<(usize, usize)>> {
    for (index, chunk) in module.chunks.iter().enumerate() {
        capacity("depth", "opcodes", chunk.ops.len(), OPS)?;
        let (_, init) = depth_and_initialization(chunk, verify_depth_kel_module())?;
        if let Some(ip) = init? {
            return Ok(Some((index, ip)));
        }
    }
    private_data(module, verify_depth_kel_module())
}

/// The private data slots whose load-time initializer is `Unit`, or absent, ascending.
///
/// This reads the initializers, which is what rule 2 of
/// `crate::verify_init` tracks. It is transport, the same selection the reference makes, and
/// the stage receives only the dense numbering it induces.
fn unit_private_slots(module: &Module) -> Vec<u32> {
    use crate::bytecode::{ConstValue, SlotVisibility};
    let Some(layout) = module.data_layout.as_ref() else {
        return Vec::new();
    };
    let private = layout
        .slots
        .iter()
        .enumerate()
        .filter(|(_, s)| s.visibility == SlotVisibility::Private);
    private
        .enumerate()
        .filter(|(k, _)| matches!(layout.private_init.get(*k), None | Some(ConstValue::Unit)))
        .filter_map(|(_, (slot, _))| u32::try_from(slot).ok())
        .collect()
}

/// The dense numbers, into `unit`, of the slots in `[base, base + len)`.
fn unit_span(unit: &[u32], base: u32, len: u32) -> (usize, usize) {
    let end = base.saturating_add(len);
    (
        unit.partition_point(|&s| s < base),
        unit.partition_point(|&s| s < end),
    )
}

/// The chunks in an order where every callee precedes its callers, and whether each was
/// ordered. A chunk on a call cycle, or one that reaches a cycle, is not ordered and keeps the
/// empty summary, as in the reference, which the surface language makes unreachable.
fn callees_first(module: &Module) -> (Vec<usize>, Vec<bool>) {
    let n = module.chunks.len();
    let mut callers: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pending = vec![0usize; n];
    for (c, chunk) in module.chunks.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for op in &chunk.ops {
            if let Op::Call(callee, _) = op {
                let callee = usize::from(*callee);
                if callee < n && seen.insert(callee) {
                    callers[callee].push(c);
                    pending[c] += 1;
                }
            }
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut ordered = vec![false; n];
    let mut ready: alloc::collections::VecDeque<usize> =
        (0..n).filter(|&c| pending[c] == 0).collect();
    // Each chunk enters `ready` once, when its last callee is ordered, so this ends.
    while let Some(c) = ready.pop_front() {
        order.push(c);
        ordered[c] = true;
        for &caller in &callers[c] {
            pending[caller] -= 1;
            if pending[caller] == 0 {
                ready.push_back(caller);
            }
        }
    }
    (order, ordered)
}

/// One stage run deciding rule 2 for chunk `index`, given the callee summaries so far.
/// Returns the lowest refused instruction and the chunk's own summary.
fn private_data_chunk(
    module: &Module,
    index: usize,
    unit: &[u32],
    summaries: &[[i64; 8]],
    stage: Module,
) -> Result<(Option<usize>, [i64; 8])> {
    const STAGE: &str = "initialization";
    let chunk = &module.chunks[index];
    capacity("depth", "opcodes", chunk.ops.len(), OPS)?;
    let n = module.chunks.len();
    let mut rows: Vec<usize> = Vec::new();
    for op in &chunk.ops {
        if let Op::Call(callee, _) = op {
            let callee = usize::from(*callee);
            if callee < n && !rows.contains(&callee) {
                rows.push(callee);
            }
        }
    }
    capacity(STAGE, "callees", rows.len(), DV_DATA_CALLEES)?;
    run(
        STAGE,
        stage,
        |vm| {
            let mut shared = seed_verify_depth_shared(vm, chunk);
            set(vm, &mut shared, DV_D_WIDTH, size(unit.len(), STAGE)?, STAGE)?;
            for (i, op) in chunk.ops.iter().enumerate() {
                let (put, span, call) = match op {
                    Op::SetData(s) => (unit.binary_search(s).map_or(0, |k| k + 1) as i64, None, 0),
                    Op::Return => (-1, None, 0),
                    Op::GetData(s) => (0, Some(unit_span(unit, *s, 1)), 0),
                    Op::GetDataIndexed(base, len) => (0, Some(unit_span(unit, *base, *len)), 0),
                    Op::Call(callee, _) => (
                        0,
                        None,
                        rows.iter()
                            .position(|&r| r == usize::from(*callee))
                            .map_or(0, |r| r + 1),
                    ),
                    _ => (0, None, 0),
                };
                if put != 0 {
                    set(vm, &mut shared, DV_D_SET + i, put, STAGE)?;
                }
                if let Some((lo, hi)) = span.filter(|(lo, hi)| lo < hi) {
                    set(vm, &mut shared, DV_D_LO + i, size(lo, STAGE)?, STAGE)?;
                    set(vm, &mut shared, DV_D_HI + i, size(hi, STAGE)?, STAGE)?;
                }
                if call != 0 {
                    set(vm, &mut shared, DV_D_CALL + i, size(call, STAGE)?, STAGE)?;
                }
            }
            for (row, &callee) in rows.iter().enumerate() {
                for (w, &bits) in summaries[callee].iter().enumerate() {
                    set(vm, &mut shared, DV_D_SUM + row * 8 + w, bits, STAGE)?;
                }
            }
            Ok(shared)
        },
        |vm, shared, _| {
            let mut summary = [0i64; 8];
            for (w, bits) in summary.iter_mut().enumerate() {
                *bits = word(vm, shared, DV_OUT_DATA_SUM + w, STAGE)?;
            }
            match word(vm, shared, DV_OUT_DATA_CAUSE, STAGE)? {
                0 => {}
                1 => {
                    return Err(VerificationError::Capacity {
                        stage: STAGE,
                        resource: "private slots",
                        limit: DV_INIT_LOCALS,
                        actual: unit.len(),
                    });
                }
                2 => {
                    return Err(VerificationError::Incomplete {
                        stage: STAGE,
                        cause: 2,
                    });
                }
                _ => return Err(protocol(STAGE, "unknown completion cause")),
            }
            if !flag(word(vm, shared, DV_OUT_DATA_REJECT, STAGE)?, STAGE)? {
                return Ok((None, summary));
            }
            match usize::try_from(word(vm, shared, DV_OUT_DATA_IP, STAGE)?) {
                Ok(ip) if ip < chunk.ops.len() => Ok((Some(ip), summary)),
                _ => Err(protocol(STAGE, "refused instruction outside the chunk")),
            }
        },
    )
}

/// Rule 2 of `crate::verify_init`, private data, decided by `verify_depth.kel`. The first
/// refused read in module order, as the chunk and instruction.
///
/// The driver supplies what the reference computes outside its dataflow, which slots are
/// tracked and the callees-first order of the chunks, and carries each chunk's summary from
/// its run to its callers' runs. The stage computes every set, every summary and every
/// verdict. A module with no tracked slot costs no run, as in the reference.
fn private_data(module: &Module, stage: Module) -> Result<Option<(usize, usize)>> {
    let unit = unit_private_slots(module);
    if unit.is_empty() {
        return Ok(None);
    }
    capacity(
        "initialization",
        "private slots",
        unit.len(),
        DV_INIT_LOCALS,
    )?;
    let n = module.chunks.len();
    let (order, ordered) = callees_first(module);
    let mut summaries = vec![[0i64; 8]; n];
    let mut verdicts: Vec<Option<usize>> = vec![None; n];
    for c in order {
        let (verdict, summary) = private_data_chunk(module, c, &unit, &summaries, stage.clone())?;
        summaries[c] = summary;
        verdicts[c] = verdict;
    }
    for c in (0..n).filter(|&c| !ordered[c]) {
        verdicts[c] = private_data_chunk(module, c, &unit, &summaries, stage.clone())?.0;
    }
    Ok(verdicts
        .iter()
        .enumerate()
        .find_map(|(c, v)| v.map(|ip| (c, ip))))
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
    let mut vm = stage_vm(m, &arena).map_err(|e| execution("data layout", e))?;
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
/// This checks structure, depth, productivity, definite initialization, typed
/// operands and data layout.
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
    // The reference decides definite initialization after every chunk's
    // structural, depth and productivity checks, so the first refusal or
    // unavailable analysis is held until those have run.
    let mut initialization: Option<Result<usize>> = None;
    for (index, chunk) in module.chunks.iter().enumerate() {
        let (rejected, init) = depth_and_initialization(chunk, verify_depth_kel_module())?;
        if rejected {
            return Ok(VerificationVerdict::Rejected {
                stage: "depth",
                chunk: Some(index),
            });
        }
        if initialization.is_none() {
            initialization = match init {
                Ok(None) => None,
                Ok(Some(_)) => Some(Ok(index)),
                Err(error) => Some(Err(error)),
            };
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
    // Rule 2, private data, is decided after rule 1 has admitted every chunk, as the reference
    // orders them, and only once the structural checks have validated every slot index.
    let initialization = match initialization {
        Some(refusal) => Some(refusal),
        None => private_data(module, verify_depth_kel_module())?.map(|(chunk, _)| Ok(chunk)),
    };
    if let Some(refusal) = initialization {
        return Ok(VerificationVerdict::Rejected {
            stage: "initialization",
            chunk: Some(refusal?),
        });
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

    /// The initialization verdict is the stage's, its capacity and budget
    /// exhaustion are errors, and neither becomes a verdict.
    #[test]
    fn initialization_verdict_is_decided_by_the_stage() {
        let module = compile_src("fn main(n: Word) -> Word { let a = n + 1; a }");
        let mut bad = module.chunks[0].clone();
        let write = bad
            .ops
            .iter()
            .position(|op| *op == Op::SetLocal(1))
            .unwrap();
        bad.ops[write] = Op::PopN(1);
        let stage = verify_depth_kel_module();
        let (_, good) = depth_and_initialization(&module.chunks[0], stage.clone()).unwrap();
        assert_eq!(good, Ok(None));
        let (depth, refused) = depth_and_initialization(&bad, stage.clone()).unwrap();
        assert!(!depth, "the depth verdict is unchanged");
        assert!(matches!(refused, Ok(Some(_))));

        // The same input through a stage whose refusal is disabled is admitted.
        let source = read_stage("kel/verify_depth.kel");
        let marker = "                dv.out_init_reject = 1;";
        assert_eq!(source.matches(marker).count(), 1);
        let silent =
            compile_src(&source.replace(marker, "                dv.out_init_reject = 0;"));
        assert_eq!(depth_and_initialization(&bad, silent).unwrap().1, Ok(None));

        // A budget of one sweep cannot confirm a fixpoint, which is unfinished
        // analysis and not a verdict.
        let budget = "fn init_sweeps() -> Word { 72 }";
        assert_eq!(source.matches(budget).count(), 1);
        let short = compile_src(&source.replace(budget, "fn init_sweeps() -> Word { 1 }"));
        assert_eq!(
            depth_and_initialization(&bad, short).unwrap().1,
            Err(VerificationError::Incomplete {
                stage: "initialization",
                cause: 2
            })
        );

        // A chunk wider than the stage's sets is a capacity error.
        let mut wide = module.clone();
        wide.chunks[0].local_count = 257;
        assert!(matches!(
            verify_module_via_kel(&wide),
            Err(VerificationError::Capacity {
                stage: "initialization",
                limit: 256,
                actual: 257,
                ..
            })
        ));
        wide.chunks[0].local_count = 256;
        assert_eq!(
            initialization_rejects_via_kel(&wide).unwrap(),
            None,
            "the declared width is admitted"
        );
    }

    /// Rule 2's verdict and each chunk's summary are the stage's. A stage whose
    /// refusal is disabled admits the refused module, and a stage that never
    /// records a reached `Return` gives every callee the empty summary, so a
    /// read that only a callee's write protects is then refused.
    #[test]
    fn private_data_verdict_and_summary_are_decided_by_the_stage() {
        let source = read_stage("kel/verify_depth.kel");
        let stage = verify_depth_kel_module();

        let mut unread = compile_src(
            "private data d { u: (), n: Word }\nfn main() -> Word { d.u = (); d.n = 1; let v = d.u; d.n }",
        );
        assert_eq!(private_data(&unread, stage.clone()).unwrap(), None);
        let write = unread.chunks[0]
            .ops
            .iter()
            .position(|op| matches!(op, Op::SetData(0)))
            .unwrap();
        unread.chunks[0].ops[write] = Op::PopN(1);
        assert!(private_data(&unread, stage.clone()).unwrap().is_some());
        let marker = "                    dv.out_data_reject = 1;";
        assert_eq!(source.matches(marker).count(), 1);
        let silent =
            compile_src(&source.replace(marker, "                    dv.out_data_reject = 0;"));
        assert_eq!(private_data(&unread, silent).unwrap(), None);

        let mut callee = compile_src(
            "private data d { u: (), n: Word }\nfn init() -> Word { d.u = (); 0 }\n\
             fn main() -> Word { let z = init(); let v = d.u; d.n + z }",
        );
        callee.data_layout.as_mut().unwrap().private_init[1] = crate::bytecode::ConstValue::Int(0);
        assert_eq!(private_data(&callee, stage).unwrap(), None);
        let found = "            di.ret = 1;";
        assert_eq!(source.matches(found).count(), 1);
        let blind = compile_src(&source.replace(found, "            di.ret = 0;"));
        assert!(private_data(&callee, blind).unwrap().is_some());
    }

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
            compile_src(&source.replace(before, "in 0..0 limit 32768"))
        };
        assert!(!depth(&module.chunks[0]).unwrap());
        assert!(matches!(
            depth_with_stage(
                &module.chunks[0],
                shorten("kel/verify_depth.kel", "in 0..32768 limit 32768")
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
                shorten("kel/verify_yield.kel", "in 0..32768 limit 32768")
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
                shorten("kel/verify_typed.kel", "in 0..32768 limit 32768")
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
