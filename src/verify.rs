extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bytecode::{BlockType, Chunk, ConstValue, Module, Op};

/// An error produced by structural verification.
#[derive(Debug, Clone)]
pub struct VerifyError {
    /// The name of the chunk that failed verification.
    pub chunk_name: String,
    /// A description of the verification failure.
    pub message: String,
}

/// Block delimiter tracked during nesting validation.
#[derive(Debug, Clone, Copy)]
enum BlockKind {
    If,
    Loop,
}

/// Analyze yield coverage for a region of instructions `[start, end)`.
///
/// Returns `Some(true)` if all fall-through paths contain at least one Yield.
/// Returns `Some(false)` if some fall-through path lacks a Yield.
/// Returns `None` if all paths exit via Break (no fall-through to `end`).
///
/// Break and BreakIf states are accumulated in `break_states` for the
/// enclosing loop to collect.
fn analyze_yield_coverage(
    ops: &[Op],
    start: usize,
    end: usize,
    initial: bool,
    break_states: &mut Vec<bool>,
) -> Option<bool> {
    let mut has_yielded = initial;
    let mut ip = start;

    while ip < end {
        match &ops[ip] {
            Op::Yield => {
                has_yielded = true;
                ip += 1;
            }
            Op::Break(_) => {
                break_states.push(has_yielded);
                return None;
            }
            Op::BreakIf(_) => {
                break_states.push(has_yielded);
                ip += 1;
            }
            Op::If(target) => {
                let target = *target as usize;
                if target > 0 && matches!(&ops[target - 1], Op::Else(_)) {
                    // If-Else-EndIf pattern.
                    let endif_pos = if let Op::Else(e) = &ops[target - 1] {
                        *e as usize
                    } else {
                        unreachable!()
                    };
                    let then_result =
                        analyze_yield_coverage(ops, ip + 1, target - 1, has_yielded, break_states);
                    let else_result =
                        analyze_yield_coverage(ops, target, endif_pos, has_yielded, break_states);
                    match (then_result, else_result) {
                        (Some(a), Some(b)) => has_yielded = a && b,
                        (Some(a), None) => has_yielded = a,
                        (None, Some(b)) => has_yielded = b,
                        (None, None) => return None,
                    }
                    ip = endif_pos + 1;
                } else {
                    // If-EndIf without Else (pattern matching).
                    let then_result =
                        analyze_yield_coverage(ops, ip + 1, target, has_yielded, break_states);
                    match then_result {
                        Some(a) => has_yielded = a && has_yielded,
                        None => {
                            // Then-branch breaks out; false path falls through unchanged.
                        }
                    }
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                let loop_exit_target = *target as usize;
                let endloop_ip = loop_exit_target - 1;
                let mut loop_breaks: Vec<bool> = Vec::new();
                let _body_result =
                    analyze_yield_coverage(ops, ip + 1, endloop_ip, has_yielded, &mut loop_breaks);
                if loop_breaks.is_empty() {
                    return None;
                }
                has_yielded = loop_breaks.iter().all(|&b| b);
                ip = loop_exit_target;
            }
            // Else, EndIf, EndLoop are handled by the recursive calls above.
            // If encountered linearly, skip them.
            Op::Else(_) | Op::EndIf | Op::EndLoop(_) => {
                ip += 1;
            }
            _ => {
                ip += 1;
            }
        }
    }

    Some(has_yielded)
}

/// Compute the worst-case execution cost of a region of instructions `[start, end)`.
///
/// At control flow joins (If/Else/EndIf), takes the maximum cost branch.
/// For loops, multiplies the body cost by the iteration count when the
/// loop matches the canonical for-range pattern, otherwise assumes one
/// iteration (conservative default).
///
/// Returns `Some(cost)` for paths that fall through to `end`.
/// Returns `None` if all paths exit via Break.
///
/// Break costs are accumulated in `break_costs` for the enclosing loop.
/// True when every fall-through path of the op range `[start, end)`
/// passes through a `Yield` and the range contains no nested `Loop`.
///
/// Used by [`wcet_region`] under `clamp_productive_yield_loops` to
/// decide whether a loop body is provably productive, so the loop
/// contributes one iteration per coroutine resumption. The no-nested-
/// loop guard keeps the productivity decision within
/// [`analyze_yield_coverage`]'s `If`/`Break` domain rather than relying
/// on its handling of inner loops; a body with an inner loop is treated
/// as not-provably-productive and keeps its full iteration bound, which
/// is conservative (sound).
fn loop_body_all_paths_yield_no_inner_loop(ops: &[Op], start: usize, end: usize) -> bool {
    if ops[start..end].iter().any(|op| matches!(op, Op::Loop(_))) {
        return false;
    }
    let mut break_states: Vec<bool> = Vec::new();
    matches!(
        analyze_yield_coverage(ops, start, end, false, &mut break_states),
        Some(true)
    )
}

/// Flat plus length-dependent WCET cycles for the op at `ip` (#49). The flat
/// per-opcode cost from `cost_model` plus the per-op text-length term in
/// `wcet_extra` (computed once per chunk by
/// [`crate::text_size::chunk_text_wcet_cycles`]). A `u32::MAX` term marks a text
/// operation whose length cannot be statically bounded; the WCET is then
/// non-boundable and the chunk is rejected, the conservative-verification
/// stance applied to length-dependent string operations.
fn op_wcet_cycles(
    chunk: &Chunk,
    ip: usize,
    cost_model: &crate::bytecode::CostModel,
    wcet_extra: &[u32],
) -> Result<u32, VerifyError> {
    let extra = wcet_extra.get(ip).copied().unwrap_or(0);
    if extra == u32::MAX {
        return Err(VerifyError {
            chunk_name: chunk.name.clone(),
            message: alloc::format!(
                "text operation at instruction {} runs in time proportional to an \
                 unbounded-length string; WCET cannot be statically bounded",
                ip
            ),
        });
    }
    Ok(cost_model.cycles(&chunk.ops[ip]).saturating_add(extra))
}

fn wcet_region(
    chunk: &Chunk,
    start: usize,
    end: usize,
    break_costs: &mut Vec<u32>,
    cost_model: &crate::bytecode::CostModel,
    clamp_productive_yield_loops: bool,
    wcet_extra: &[u32],
) -> Result<Option<u32>, VerifyError> {
    let ops = &chunk.ops;
    let mut cost: u32 = 0;
    let mut ip = start;

    while ip < end {
        match &ops[ip] {
            Op::Break(_) => {
                cost = cost.saturating_add(op_wcet_cycles(chunk, ip, cost_model, wcet_extra)?);
                break_costs.push(cost);
                return Ok(None);
            }
            Op::Trap(_) => {
                // Trap halts execution. Treat as path-exit. Does not
                // push to break_costs because it does not transfer
                // control to the enclosing loop.
                cost = cost.saturating_add(op_wcet_cycles(chunk, ip, cost_model, wcet_extra)?);
                let _ = cost;
                return Ok(None);
            }
            Op::BreakIf(_) => {
                cost = cost.saturating_add(op_wcet_cycles(chunk, ip, cost_model, wcet_extra)?);
                break_costs.push(cost);
                ip += 1;
            }
            Op::If(target) => {
                cost = cost.saturating_add(op_wcet_cycles(chunk, ip, cost_model, wcet_extra)?);
                let target = *target as usize;
                if target > 0 && matches!(&ops[target - 1], Op::Else(_)) {
                    let endif_pos = if let Op::Else(e) = &ops[target - 1] {
                        *e as usize
                    } else {
                        unreachable!()
                    };
                    let then_cost = wcet_region(
                        chunk,
                        ip + 1,
                        target - 1,
                        break_costs,
                        cost_model,
                        clamp_productive_yield_loops,
                        wcet_extra,
                    )?;
                    let else_cost = wcet_region(
                        chunk,
                        target,
                        endif_pos,
                        break_costs,
                        cost_model,
                        clamp_productive_yield_loops,
                        wcet_extra,
                    )?;
                    let branch_cost = match (then_cost, else_cost) {
                        (Some(a), Some(b)) => Some(if a > b { a } else { b }),
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        (None, None) => return Ok(None),
                    };
                    cost += branch_cost.unwrap_or(0);
                    ip = endif_pos + 1;
                } else {
                    let then_cost = wcet_region(
                        chunk,
                        ip + 1,
                        target,
                        break_costs,
                        cost_model,
                        clamp_productive_yield_loops,
                        wcet_extra,
                    )?;
                    // False path has zero additional cost (skips to EndIf).
                    // Worst case is the then-body if it is more expensive.
                    match then_cost {
                        Some(c) => cost += c,
                        None => {
                            // Then-branch breaks. False path falls through with zero cost.
                        }
                    }
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                cost = cost.saturating_add(op_wcet_cycles(chunk, ip, cost_model, wcet_extra)?);
                let loop_exit_target = *target as usize;
                let endloop_ip = loop_exit_target - 1;
                let mut loop_break_costs: Vec<u32> = Vec::new();
                let body_cost = wcet_region(
                    chunk,
                    ip + 1,
                    endloop_ip,
                    &mut loop_break_costs,
                    cost_model,
                    clamp_productive_yield_loops,
                    wcet_extra,
                )?;
                if loop_break_costs.is_empty() && body_cost.is_none() {
                    return Ok(None);
                }
                // Strict mode iteration count. Under
                // `clamp_productive_yield_loops` (the per-resume WCET of a
                // Reentrant coroutine), a loop whose every body path
                // provably yields contributes at most one iteration per
                // resumption, because the resume suspends at the yield
                // before completing a second pass. The clamp is guarded
                // against nested loops so the productivity check stays in
                // `analyze_yield_coverage`'s well-tested If/Break domain;
                // a loop that is not provably productive keeps its full
                // iteration bound, so a conditional yield (which a resume
                // could skip across many iterations) is never under-counted.
                let iter_count = if body_cost.is_none()
                    || (clamp_productive_yield_loops
                        && loop_body_all_paths_yield_no_inner_loop(ops, ip + 1, endloop_ip))
                {
                    1
                } else {
                    match extract_loop_iteration_bound(chunk, ip) {
                        Some(n) => n,
                        Option::None => {
                            return Err(VerifyError {
                                chunk_name: chunk.name.clone(),
                                message: alloc::format!(
                                    "loop at instruction {} has no statically extractable \
                                     iteration bound; strict mode requires loops with \
                                     fall-through bodies to match the canonical for-range \
                                     pattern",
                                    ip
                                ),
                            });
                        }
                    }
                };
                let body_cost_total = body_cost.unwrap_or(0).saturating_mul(iter_count);
                let max_break = loop_break_costs.iter().copied().max().unwrap_or(0);
                cost += if max_break > body_cost_total {
                    max_break
                } else {
                    body_cost_total
                };
                ip = loop_exit_target;
            }
            Op::Else(_) | Op::EndIf | Op::EndLoop(_) => {
                ip += 1;
            }
            _ => {
                cost = cost.saturating_add(op_wcet_cycles(chunk, ip, cost_model, wcet_extra)?);
                ip += 1;
            }
        }
    }

    Ok(Some(cost))
}

/// Detect a bounded for-range loop pattern starting at `loop_ip` and
/// return the iteration count if extractable.
///
/// The Keleusma compiler emits for-range loops with the canonical shape
/// `Loop GetLocal(var) GetLocal(end) CmpGe BreakIf body... EndLoop`,
/// where `var` and `end` are local slots set by literal `Const`
/// instructions before the `Loop`. This helper recognizes that pattern
/// and extracts the iteration count from the difference of the literal
/// constants.
///
/// Returns `None` for loops whose bounds are not literal integers.
/// Callers fall back to the conservative one-iteration treatment in
/// that case, which is sound but typically loose.
fn extract_loop_iteration_bound(chunk: &Chunk, loop_ip: usize) -> Option<u32> {
    let ops = &chunk.ops;
    if loop_ip + 4 >= ops.len() {
        return None;
    }
    let var_slot = match &ops[loop_ip + 1] {
        Op::GetLocal(s) => *s,
        _ => return None,
    };
    let end_slot = match &ops[loop_ip + 2] {
        Op::GetLocal(s) => *s,
        _ => return None,
    };
    if !matches!(&ops[loop_ip + 3], Op::CmpGe) {
        return None;
    }
    if !matches!(&ops[loop_ip + 4], Op::BreakIf(_)) {
        return None;
    }

    // Trace back to find the most recent SetLocal(slot) and check if the
    // previous instruction is a Const that resolves to an integer.
    let end_val = trace_const_set_local(chunk, loop_ip, end_slot)?;
    let start_val = trace_const_set_local(chunk, loop_ip, var_slot)?;

    if end_val >= start_val {
        let count = (end_val - start_val) as u64;
        if count > u32::MAX as u64 {
            None
        } else {
            Some(count as u32)
        }
    } else {
        Some(0)
    }
}

/// Find the most recent `SetLocal(slot)` before `before_ip` and return
/// the statically known integer value assigned to the slot.
///
/// Two patterns are recognized.
///
/// 1. Direct constant. `Const(idx) SetLocal(slot)` where the constant
///    pool entry at `idx` is an integer.
/// 2. Length of a literal array. `GetLocal(arr_slot) Len SetLocal(slot)`,
///    where `arr_slot` was set from a literal `NewArray(n)`. This
///    matches the for-in over array iteration bound.
///
/// Returns `None` if the slot is not set by either pattern.
fn trace_const_set_local(chunk: &Chunk, before_ip: usize, slot: u16) -> Option<i64> {
    let ops = &chunk.ops;
    let mut ip = before_ip;
    while ip > 0 {
        ip -= 1;
        if let Op::SetLocal(s) = &ops[ip]
            && *s == slot
        {
            if ip == 0 {
                return None;
            }
            // Pattern 1: direct integer constant.
            if let Op::Const(idx) = &ops[ip - 1]
                && let Some(ConstValue::Int(n)) = chunk.constants.get(*idx as usize)
            {
                return Some(*n);
            }
            // Pattern 2: Length of a literal array. The compiler emits
            // GetLocal(arr_slot) Len SetLocal(end_slot) for for-in.
            if ip >= 2
                && matches!(&ops[ip - 1], Op::Len)
                && let Op::GetLocal(arr_slot) = &ops[ip - 2]
            {
                return trace_literal_array_length(chunk, ip - 2, *arr_slot);
            }
            return None;
        }
    }
    None
}

/// Find the most recent `SetLocal(arr_slot)` before `before_ip` and
/// return the literal array's length if the array was constructed via
/// `NewArray(n)`. Follows `GetLocal -> SetLocal` aliasing chains so that
/// for-in over a let-bound literal array is recognized. Returns `None`
/// if the chain terminates on a non-literal source.
fn trace_literal_array_length(chunk: &Chunk, before_ip: usize, arr_slot: u16) -> Option<i64> {
    let ops = &chunk.ops;
    let mut ip = before_ip;
    while ip > 0 {
        ip -= 1;
        if let Op::SetLocal(s) = &ops[ip]
            && *s == arr_slot
        {
            if ip == 0 {
                return None;
            }
            // Direct: NewComposite(Array, count) -> SetLocal(arr_slot).
            if let Op::NewComposite(o) = &ops[ip - 1]
                && o.kind() == crate::value_layout::CompositeKind::Array
            {
                return Some(o.count() as i64);
            }
            // Aliased: GetLocal(other) -> SetLocal(arr_slot). Chase the
            // alias backward until a NewArray or unsupported source is
            // reached. The recursion terminates because each step has a
            // strictly smaller `before_ip`.
            if let Op::GetLocal(other_slot) = &ops[ip - 1] {
                return trace_literal_array_length(chunk, ip - 1, *other_slot);
            }
            return None;
        }
    }
    None
}

/// Result of WCMU analysis over a region.
#[derive(Debug, Clone, Copy)]
struct McuResult {
    /// Maximum stack depth observed during the region, relative to the
    /// initial stack offset at the start of the region.
    peak_above_initial: u32,
    /// Stack offset at the end of the region, relative to the initial
    /// offset. May be negative conceptually if the region pops more than
    /// it pushes; we saturate at zero because the verifier guarantees the
    /// program is structurally valid.
    delta: i32,
    /// Total bytes allocated to the arena heap by the region, summed
    /// along the path that reaches `end`.
    heap_total: u32,
}

impl McuResult {
    fn empty() -> Self {
        Self {
            peak_above_initial: 0,
            delta: 0,
            heap_total: 0,
        }
    }
}

/// Lookup table for resolving the WCMU contribution of `Op::Call` and
/// `Op::CallNative` instructions. The empty resolver returns zero for
/// every lookup, which produces the local-only WCMU bound used by
/// `wcmu_stream_iteration`. The full resolver is populated by
/// `module_wcmu` for transitive call analysis.
struct CallResolver<'a> {
    /// Per-chunk WCMU as `(stack_bytes, heap_bytes)`. `None` for chunks
    /// not yet analyzed in the topological walk.
    chunk_wcmu: &'a [Option<(u32, u32)>],
    /// Per-native WCMU bytes from host attestation. Indexed by native
    /// function entry index.
    native_wcmu: &'a [u32],
    /// The module's shared-slot layout, indexed by shared slot. Used to size
    /// the arena copy-out a `GetData` on a flat composite shared slot performs
    /// (B28 item 2 / task #57). Empty on the local-only analysis path
    /// ([`CallResolver::empty`]), which therefore under-counts a composite
    /// shared read; only the module-level [`module_wcmu`] path, the
    /// soundness-critical bound, carries the layout.
    shared_layout: &'a [crate::bytecode::SharedSlotLayout],
}

impl<'a> CallResolver<'a> {
    /// A resolver that returns zero for every lookup. Used by the
    /// local-only analysis path.
    fn empty() -> Self {
        Self {
            chunk_wcmu: &[],
            native_wcmu: &[],
            shared_layout: &[],
        }
    }

    fn resolve_chunk(&self, idx: u16) -> (u32, u32) {
        self.chunk_wcmu
            .get(idx as usize)
            .and_then(|o| *o)
            .unwrap_or((0, 0))
    }

    fn resolve_native(&self, idx: u16) -> u32 {
        self.native_wcmu.get(idx as usize).copied().unwrap_or(0)
    }
}

/// Per-op arena heap allocation for the WCMU walk: the op's own construction
/// allocation ([`crate::bytecode::Op::heap_alloc`]) plus the shared-composite
/// copy-out a `GetData`/`GetDataIndexed` performs when it reads a flat composite
/// shared slot (B28 item 2 / task #57). Both accumulate in the arena top region
/// across a Stream iteration, so they sum, and the loop-multiplicity walk in
/// [`wcmu_region`] scales them by iteration count.
fn op_iteration_heap(op: &Op, chunk: &Chunk, resolver: &CallResolver) -> u32 {
    op.heap_alloc(chunk)
        .saturating_add(shared_composite_copyout_bytes(op, resolver.shared_layout))
}

/// Bytes the shared-composite copy-out allocates for `op` (task #57).
///
/// A `GetData` that reads a flat composite shared slot copies the body out of
/// the borrowed host buffer into a fresh arena body of the slot's `len` bytes
/// (`crate::vm::GenericVm::read_shared_from_buffer`); that per-read allocation is
/// a per-iteration arena cost the WCMU bound must include. A scalar shared slot,
/// a private slot (read in place from the arena persistent region), and every
/// non-data op allocate nothing here. Shared arrays-of-composites are rejected
/// at compile time, so a `GetDataIndexed` over a shared slot reads scalars only;
/// the element range is scanned defensively and contributes its largest
/// composite copy-out, which is zero for a well-formed module.
fn shared_composite_copyout_bytes(
    op: &Op,
    shared_layout: &[crate::bytecode::SharedSlotLayout],
) -> u32 {
    let slot_copyout = |slot: usize| -> u32 {
        shared_layout
            .get(slot)
            .filter(|e| e.kind & crate::bytecode::SHARED_SLOT_COMPOSITE_FLAG != 0)
            .map_or(0, |e| e.len as u32)
    };
    match op {
        Op::GetData(slot) => slot_copyout(*slot as usize),
        Op::GetDataIndexed(base, len) => (0..*len as usize)
            .map(|i| slot_copyout(*base as usize + i))
            .max()
            .unwrap_or(0),
        _ => 0,
    }
}

/// Compute the worst-case memory usage over a region of instructions
/// `[start, end)`. The analysis tracks operand-stack depth in slots and
/// arena heap bytes.
///
/// At control flow joins, the peak stack and heap total are taken as the
/// maximum across branches. The stack delta is taken from the branch that
/// reaches `end`, with the convention that the surface compiler ensures
/// branches end at the same depth.
///
/// Loops operate in strict mode. A loop whose body falls through to its
/// EndLoop must have its iteration count statically extractable through
/// the canonical for-range pattern. Loops whose body always exits via
/// Break or Trap are accepted with iteration count one. Other loops are
/// rejected with a `VerifyError`. The WCMU bound is therefore sound for
/// every program that passes verification.
///
/// Returns `Ok(Some(McuResult))` for paths that fall through to `end`.
/// Returns `Ok(None)` if all paths exit via Break or Trap. Returns
/// `Err(VerifyError)` for strict mode violations.
fn wcmu_region(
    chunk: &Chunk,
    start: usize,
    end: usize,
    break_results: &mut Vec<McuResult>,
    resolver: &CallResolver,
    value_slot_bytes: u32,
) -> Result<Option<McuResult>, VerifyError> {
    let ops = &chunk.ops;
    let mut current_offset: i32 = 0;
    let mut peak: u32 = 0;
    let mut heap: u32 = 0;
    let mut ip = start;

    while ip < end {
        let op = &ops[ip];
        match op {
            Op::Break(_) => {
                let shrink = op.stack_shrink() as i32;
                let growth = op.stack_growth() as i32;
                let during_peak = (current_offset + growth).max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(op_iteration_heap(op, chunk, resolver));
                current_offset += growth - shrink;
                break_results.push(McuResult {
                    peak_above_initial: peak,
                    delta: current_offset,
                    heap_total: heap,
                });
                return Ok(None);
            }
            Op::Trap(_) => {
                // Trap halts execution. Treat as path-exit so the analysis
                // does not walk past unreachable code. Trap does not push
                // to break_results because it does not transfer control to
                // the enclosing loop.
                let shrink = op.stack_shrink() as i32;
                let growth = op.stack_growth() as i32;
                let during_peak = (current_offset + growth).max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(op_iteration_heap(op, chunk, resolver));
                current_offset += growth - shrink;
                let _ = current_offset;
                let _ = peak;
                let _ = heap;
                return Ok(None);
            }
            Op::BreakIf(_) => {
                let shrink = op.stack_shrink() as i32;
                let growth = op.stack_growth() as i32;
                let during_peak = (current_offset + growth).max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(op_iteration_heap(op, chunk, resolver));
                current_offset += growth - shrink;
                break_results.push(McuResult {
                    peak_above_initial: peak,
                    delta: current_offset,
                    heap_total: heap,
                });
                ip += 1;
            }
            Op::If(target) => {
                // Account for the If instruction itself before recursing.
                let shrink = op.stack_shrink() as i32;
                let growth = op.stack_growth() as i32;
                let during_peak = (current_offset + growth).max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(op_iteration_heap(op, chunk, resolver));
                current_offset += growth - shrink;

                let target = *target as usize;
                if target > 0 && matches!(&ops[target - 1], Op::Else(_)) {
                    let endif_pos = if let Op::Else(e) = &ops[target - 1] {
                        *e as usize
                    } else {
                        unreachable!()
                    };
                    let then_branch = wcmu_subregion(
                        chunk,
                        ip + 1,
                        target - 1,
                        current_offset,
                        break_results,
                        resolver,
                        value_slot_bytes,
                    )?;
                    let else_branch = wcmu_subregion(
                        chunk,
                        target,
                        endif_pos,
                        current_offset,
                        break_results,
                        resolver,
                        value_slot_bytes,
                    )?;
                    match (then_branch, else_branch) {
                        (Some(a), Some(b)) => {
                            peak = peak.max(a.peak_above_initial).max(b.peak_above_initial);
                            heap = heap.saturating_add(a.heap_total.max(b.heap_total));
                            // Branches should end at the same offset, but if
                            // not, take the maximum to remain conservative.
                            current_offset = a.delta.max(b.delta);
                        }
                        (Some(a), None) => {
                            peak = peak.max(a.peak_above_initial);
                            heap = heap.saturating_add(a.heap_total);
                            current_offset = a.delta;
                        }
                        (None, Some(b)) => {
                            peak = peak.max(b.peak_above_initial);
                            heap = heap.saturating_add(b.heap_total);
                            current_offset = b.delta;
                        }
                        (None, None) => {
                            return Ok(None);
                        }
                    }
                    ip = endif_pos + 1;
                } else {
                    let then_branch = wcmu_subregion(
                        chunk,
                        ip + 1,
                        target,
                        current_offset,
                        break_results,
                        resolver,
                        value_slot_bytes,
                    )?;
                    if let Some(a) = then_branch {
                        peak = peak.max(a.peak_above_initial);
                        heap = heap.saturating_add(a.heap_total);
                        // The false path skips with zero contribution.
                        // Conservative final offset is the maximum.
                        current_offset = current_offset.max(a.delta);
                    }
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                let shrink = op.stack_shrink() as i32;
                let growth = op.stack_growth() as i32;
                let during_peak = (current_offset + growth).max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(op_iteration_heap(op, chunk, resolver));
                current_offset += growth - shrink;

                let loop_exit_target = *target as usize;
                let endloop_ip = loop_exit_target - 1;
                let mut loop_breaks: Vec<McuResult> = Vec::new();
                let body = wcmu_subregion(
                    chunk,
                    ip + 1,
                    endloop_ip,
                    current_offset,
                    &mut loop_breaks,
                    resolver,
                    value_slot_bytes,
                )?;
                let body_peak = body.as_ref().map_or(0, |r| r.peak_above_initial);
                let body_heap_one = body.as_ref().map_or(0, |r| r.heap_total);
                // Strict mode loop iteration determination.
                // - If body is None, all paths exit via Break or Trap.
                //   The loop iterates at most once. Sound.
                // - If body is Some, the body falls through. The
                //   iteration count must be extractable from the
                //   canonical for-range pattern. Otherwise the loop has
                //   no statically computable bound and the analysis
                //   rejects it.
                let iter_count = if body.is_none() {
                    1
                } else {
                    match extract_loop_iteration_bound(chunk, ip) {
                        Some(n) => n,
                        Option::None => {
                            return Err(VerifyError {
                                chunk_name: chunk.name.clone(),
                                message: alloc::format!(
                                    "loop at instruction {} has no statically extractable \
                                     iteration bound; strict mode requires loops with \
                                     fall-through bodies to match the canonical for-range \
                                     pattern",
                                    ip
                                ),
                            });
                        }
                    }
                };
                let body_heap = body_heap_one.saturating_mul(iter_count);
                let break_peak = loop_breaks
                    .iter()
                    .map(|r| r.peak_above_initial)
                    .max()
                    .unwrap_or(0);
                let break_heap = loop_breaks.iter().map(|r| r.heap_total).max().unwrap_or(0);
                peak = peak.max(body_peak).max(break_peak);
                heap = heap.saturating_add(body_heap.max(break_heap));
                if loop_breaks.is_empty() && body.is_none() {
                    return Ok(None);
                }
                ip = loop_exit_target;
            }
            Op::Call(callee_idx, n_args) => {
                // Transitive WCMU contribution of the called chunk.
                // The callee's stack WCMU includes its local frame plus
                // its body peak. During the call, the caller's depth
                // minus the n args being passed plus the callee's stack
                // is the peak observed.
                let (callee_stack_bytes, callee_heap_bytes) = resolver.resolve_chunk(*callee_idx);
                let callee_stack_slots = (callee_stack_bytes / value_slot_bytes) as i32;
                let n = *n_args as i32;
                let during_peak = (current_offset + callee_stack_slots - n)
                    .max(current_offset + 1)
                    .max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(callee_heap_bytes);
                // Net stack effect: pop n args, push 1 return value.
                current_offset += 1 - n;
                ip += 1;
            }
            Op::CallVerifiedNative(native_idx, n_args)
            | Op::CallExternalNative(native_idx, n_args) => {
                // Native function runs in host code. The operand-stack
                // effect is just the argument pop and return push. Heap
                // contribution comes from the host attestation. Both
                // classification opcodes contribute the same
                // structural effect at the operand-stack level; the
                // per-class WCET budget distinction is observed by the
                // pipelined-cycle pass.
                let native_heap = resolver.resolve_native(*native_idx);
                let n = *n_args as i32;
                let during_peak = (current_offset + 1).max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(native_heap);
                current_offset += 1 - n;
                ip += 1;
            }
            Op::Else(_) | Op::EndIf | Op::EndLoop(_) => {
                ip += 1;
            }
            _ => {
                let shrink = op.stack_shrink() as i32;
                let growth = op.stack_growth() as i32;
                let during_peak = (current_offset + growth).max(0) as u32;
                peak = peak.max(during_peak);
                heap = heap.saturating_add(op_iteration_heap(op, chunk, resolver));
                current_offset += growth - shrink;
                ip += 1;
            }
        }
    }

    Ok(Some(McuResult {
        peak_above_initial: peak,
        delta: current_offset,
        heap_total: heap,
    }))
}

/// Helper that recurses into a subregion with an explicit initial offset
/// and adjusts the result back to the caller's frame of reference. The
/// returned `peak_above_initial` is the peak above the caller's initial
/// position before this subregion.
fn wcmu_subregion(
    chunk: &Chunk,
    start: usize,
    end: usize,
    offset_at_start: i32,
    break_results: &mut Vec<McuResult>,
    resolver: &CallResolver,
    value_slot_bytes: u32,
) -> Result<Option<McuResult>, VerifyError> {
    let mut sub_breaks: Vec<McuResult> = Vec::new();
    let result = wcmu_region(
        chunk,
        start,
        end,
        &mut sub_breaks,
        resolver,
        value_slot_bytes,
    )?;
    // Lift breaks from the subregion into the caller's frame of reference.
    for b in sub_breaks {
        break_results.push(McuResult {
            peak_above_initial: (offset_at_start.max(0) as u32) + b.peak_above_initial,
            delta: offset_at_start + b.delta,
            heap_total: b.heap_total,
        });
    }
    Ok(result.map(|r| McuResult {
        peak_above_initial: (offset_at_start.max(0) as u32) + r.peak_above_initial,
        delta: offset_at_start + r.delta,
        heap_total: r.heap_total,
    }))
}

/// Compute the worst-case memory usage of one full Stream iteration.
///
/// Returns a tuple `(stack_wcmu_bytes, heap_wcmu_bytes)`. Stack WCMU
/// includes the chunk's local frame plus the peak operand-stack growth
/// during execution. Heap WCMU is the total bytes allocated to the arena
/// heap during one Stream-to-Reset cycle.
///
/// Both bounds are sound for programs that do not contain calls or
/// variable-iteration loops. Calls are treated locally, namely the call
/// instruction itself contributes its `stack_growth` and `stack_shrink`
/// but the transitive contribution of the called function is not
/// included. Loops are treated as one iteration. These limitations
/// mirror the existing WCET implementation and are tracked for future
/// work.
pub fn wcmu_stream_iteration(chunk: &Chunk) -> Result<(u32, u32), VerifyError> {
    wcmu_stream_iteration_with_value_slot_bytes(chunk, crate::bytecode::VALUE_SLOT_SIZE_BYTES)
}

/// Variant of [`wcmu_stream_iteration`] that uses a host-supplied
/// `value_slot_bytes` for the bytes-per-slot multiplier. Hosts
/// running narrow `GenericVm<W, A, F>` instances pass
/// `size_of::<GenericValue<W, F>>()` so the bound matches the
/// runtime's actual slot footprint rather than the conservative
/// 64-bit-runtime default. The default-parameter shape is
/// preserved through [`wcmu_stream_iteration`].
pub fn wcmu_stream_iteration_with_value_slot_bytes(
    chunk: &Chunk,
    value_slot_bytes: u32,
) -> Result<(u32, u32), VerifyError> {
    if chunk.block_type != BlockType::Stream {
        return Err(VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("wcmu_stream_iteration requires a Stream block"),
        });
    }

    let ops = &chunk.ops;
    let stream_pos = ops
        .iter()
        .position(|op| matches!(op, Op::Stream))
        .ok_or_else(|| VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("Stream block missing Stream instruction"),
        })?;
    let reset_pos = ops
        .iter()
        .position(|op| matches!(op, Op::Reset))
        .ok_or_else(|| VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("Stream block missing Reset instruction"),
        })?;

    let mut breaks: Vec<McuResult> = Vec::new();
    let resolver = CallResolver::empty();
    let body = wcmu_region(
        chunk,
        stream_pos + 1,
        reset_pos,
        &mut breaks,
        &resolver,
        value_slot_bytes,
    )?
    .unwrap_or(McuResult::empty());

    let body_peak = body.peak_above_initial;
    let body_heap = body.heap_total;

    let stack_slots = chunk.local_count as u32 + body_peak;
    let stack_bytes = stack_slots * value_slot_bytes;

    Ok((stack_bytes, body_heap))
}

/// Per-op WCET extra cycles for a chunk: the #49 text-length term plus, for
/// each verified-native call op, the host-attested per-call WCET body cost from
/// `native_bounds` (#50). Folding the verified-native body into the per-op
/// table lets [`wcet_region`] scale it by loop multiplicity exactly like the
/// script ops, symmetric with how the WCMU pass sums a verified native's
/// per-call bytes over its call sites. An empty `native_bounds` yields the
/// text-only table (the script-only WCET path). External natives are not folded
/// here; their per-iteration contribution is added once per chunk by
/// [`external_native_wcet`], because an external native's invocation count is
/// host-attested rather than derived from the loop structure.
fn chunk_wcet_extra(
    chunk: &Chunk,
    cost_model: &crate::bytecode::CostModel,
    native_bounds: &[NativeIterationBound],
) -> Vec<u32> {
    let mut extra = crate::text_size::chunk_text_wcet_cycles(chunk, cost_model.text_byte_cycles);
    if !native_bounds.is_empty() {
        for (ip, op) in chunk.ops.iter().enumerate() {
            if let Op::CallVerifiedNative(idx, _) = op
                && let Some(b) = native_bounds.get(*idx as usize)
            {
                extra[ip] = extra[ip].saturating_add(b.per_call_wcet_cycles);
            }
        }
    }
    extra
}

/// Once-per-chunk external-native WCET contribution (#50): for each unique
/// external native called in the chunk, `max_invocations * per_call_wcet`. This
/// mirrors the external-native WCMU contribution in [`module_wcmu_with_bounds`]:
/// an external native (`use external module::name`) carries a host-attested
/// per-iteration invocation count rather than a statically countable call-site
/// count, so its body cost is added once per chunk against that attestation, not
/// scaled by loop structure. Deduplication keeps the bound independent of the
/// static call-site count.
fn external_native_wcet(chunk: &Chunk, native_bounds: &[NativeIterationBound]) -> u32 {
    let mut seen: alloc::collections::BTreeSet<u16> = alloc::collections::BTreeSet::new();
    let mut total: u32 = 0;
    for op in &chunk.ops {
        if let Op::CallExternalNative(idx, _) = op
            && seen.insert(*idx)
            && let Some(b) = native_bounds.get(*idx as usize)
            && let Some(max_inv) = b.max_invocations
        {
            total = total.saturating_add(b.per_call_wcet_cycles.saturating_mul(max_inv));
        }
    }
    total
}

/// Compute the worst-case execution cost of one full Stream iteration
/// (from Stream to Reset), taking the maximum cost branch at each
/// control flow join.
///
/// Returns the worst-case cost as a unitless integer. Returns an error
/// if the chunk is not a Stream block type or lacks Stream/Reset.
///
/// This is the script-only bound: an `Op::Call` and a native call contribute
/// their dispatch cycles only, not the callee body. The host-attested native
/// body time is included by [`module_wcet_with_bounds`].
pub fn wcet_stream_iteration(chunk: &Chunk) -> Result<u32, VerifyError> {
    wcet_stream_iteration_with_cost_model(chunk, &crate::bytecode::NOMINAL_COST_MODEL, &[])
}

/// Variant of [`wcet_stream_iteration`] that uses a host-supplied
/// cost model for the per-op cycle table. Hosts targeting a
/// specific microarchitecture supply a measured `CostModel` so the
/// WCET bound reflects the target's pipelined-cycle costs rather
/// than the nominal table. The model's `value_slot_bytes` field is
/// not consulted by the WCET computation; the WCMU side uses it
/// through [`wcmu_stream_iteration_with_value_slot_bytes`].
pub fn wcet_stream_iteration_with_cost_model(
    chunk: &Chunk,
    cost_model: &crate::bytecode::CostModel,
    native_bounds: &[NativeIterationBound],
) -> Result<u32, VerifyError> {
    if chunk.block_type != BlockType::Stream {
        return Err(VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("wcet_stream_iteration requires a Stream block"),
        });
    }

    let ops = &chunk.ops;
    let stream_pos = ops
        .iter()
        .position(|op| matches!(op, Op::Stream))
        .ok_or_else(|| VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("Stream block missing Stream instruction"),
        })?;
    let reset_pos = ops
        .iter()
        .position(|op| matches!(op, Op::Reset))
        .ok_or_else(|| VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("Stream block missing Reset instruction"),
        })?;

    let mut break_costs: Vec<u32> = Vec::new();
    let wcet_extra = chunk_wcet_extra(chunk, cost_model, native_bounds);
    let body_cost = wcet_region(
        chunk,
        stream_pos + 1,
        reset_pos,
        &mut break_costs,
        cost_model,
        false,
        &wcet_extra,
    )?;

    // Include Stream and Reset instruction costs, plus the once-per-chunk
    // external-native body contribution (#50).
    let overhead = cost_model.cycles(&ops[stream_pos]) + cost_model.cycles(&ops[reset_pos]);
    let region_cost = body_cost.unwrap_or(0);

    Ok(overhead
        .saturating_add(region_cost)
        .saturating_add(external_native_wcet(chunk, native_bounds)))
}

/// Compute the worst-case execution cost of a non-Stream chunk's whole
/// op range, taking the maximum-cost branch at each control-flow join.
///
/// This is the [`wcet_stream_iteration`] computation applied to the
/// chunk's entire op range rather than a Stream-to-Reset body. It
/// accepts `Func` and `Reentrant` chunks and feeds their B29
/// `VerifierWitness` `resource-bounds` obligation; it is *not* folded
/// into the module's declared WCET header, which remains the
/// per-iteration maximum across `Stream` chunks.
///
/// Interpretation differs by block type. For a `Func` chunk the body is
/// one atomic call, so the result is the per-call WCET. For a
/// `Reentrant` chunk (a `yield` function) the result is a sound bound on
/// the worst-case cost of a single resumption, tightened in two stages:
///
/// * When every `Yield` is at the top level (not nested in an `If`/`Loop`
///   block), the body splits into inter-yield segments and the result is
///   the maximum segment cost — the exact per-resume WCET
///   (`reentrant_segmented_wcet`).
/// * Otherwise the result is the whole-body cost computed with each
///   provably-productive yield-loop clamped to one iteration (a single
///   resumption cannot complete more than one pass of a loop whose every
///   body path yields). This is a sound upper bound on any single
///   resume, and tighter than the plain cumulative cost whenever such a
///   loop has an iteration bound above one. A loop that is not provably
///   productive keeps its full iteration count, so a conditional yield
///   is never under-counted. Straight-line code summed across yields
///   keeps this bound loose but sound.
///
/// Like the Stream path, the cost is shallow with respect to calls: an
/// `Op::Call` contributes its dispatch cycle only, not the callee's
/// body (there is no transitive WCET resolver). Returns an error for a
/// `Stream` chunk (use [`wcet_stream_iteration`]) or if a loop lacks a
/// statically extractable iteration bound.
pub fn wcet_whole_chunk(chunk: &Chunk) -> Result<u32, VerifyError> {
    wcet_whole_chunk_with_cost_model(chunk, &crate::bytecode::NOMINAL_COST_MODEL, &[])
}

/// Variant of [`wcet_whole_chunk`] that uses a host-supplied cost model.
/// See [`wcet_stream_iteration_with_cost_model`] for the cost-model
/// contract.
pub fn wcet_whole_chunk_with_cost_model(
    chunk: &Chunk,
    cost_model: &crate::bytecode::CostModel,
    native_bounds: &[NativeIterationBound],
) -> Result<u32, VerifyError> {
    if chunk.block_type == BlockType::Stream {
        return Err(VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("wcet_whole_chunk requires a non-Stream block"),
        });
    }
    // The once-per-chunk external-native body contribution (#50) applies to
    // either path below.
    let external = external_native_wcet(chunk, native_bounds);
    // A Reentrant chunk's per-resume WCET is the exact maximum inter-yield
    // segment cost when the yields are top-level.
    if chunk.block_type == BlockType::Reentrant
        && let Some(segmented) = reentrant_segmented_wcet(chunk, cost_model, native_bounds)?
    {
        return Ok(segmented.saturating_add(external));
    }
    // Otherwise: the whole-body cost. For a Reentrant chunk, clamp
    // provably-productive yield-loops to one iteration (a sound per-resume
    // tightening); for a Func chunk the body is one atomic call, so no
    // clamp applies.
    let clamp = chunk.block_type == BlockType::Reentrant;
    let mut break_costs: Vec<u32> = Vec::new();
    let wcet_extra = chunk_wcet_extra(chunk, cost_model, native_bounds);
    let body_cost = wcet_region(
        chunk,
        0,
        chunk.ops.len(),
        &mut break_costs,
        cost_model,
        clamp,
        &wcet_extra,
    )?;
    Ok(body_cost.unwrap_or(0).saturating_add(external))
}

/// Per-chunk worst-case execution time including host-attested native body time
/// (#50). Returns a vector parallel to `module.chunks`: each entry is the
/// chunk's WCET in the cost model's unitless cycle space, with a `Stream`
/// chunk's per-iteration WCET and a `Func`/`Reentrant` chunk's per-call /
/// per-resume WCET each folding in the attested native body cost — a verified
/// native's per-call WCET summed over its call sites (scaled by loop
/// multiplicity), and an external native's `max_invocations * per_call_wcet`
/// once per chunk. This is the symmetric WCET counterpart of
/// [`module_wcmu_with_bounds`]; the host obtains `bounds` from its native
/// attestations (`Vm::set_native_bounds`).
///
/// Like the per-chunk WCET functions, the bound is shallow with respect to
/// script-to-script calls (an `Op::Call` contributes its dispatch cycle, not the
/// callee body); only direct native calls fold in attested body time. A chunk
/// whose WCET is not statically boundable (an unbounded-length text op, or a
/// loop without an extractable iteration bound) yields `Err`.
pub fn module_wcet_with_bounds(
    module: &Module,
    bounds: &[NativeIterationBound],
    cost_model: &crate::bytecode::CostModel,
) -> Result<Vec<u32>, VerifyError> {
    module
        .chunks
        .iter()
        .map(|chunk| match chunk.block_type {
            BlockType::Stream => wcet_stream_iteration_with_cost_model(chunk, cost_model, bounds),
            _ => wcet_whole_chunk_with_cost_model(chunk, cost_model, bounds),
        })
        .collect()
}

/// The worst-case execution cost of a single resumption of a
/// `Reentrant` chunk, computed by splitting the body into segments at
/// its `Yield` ops and taking the maximum segment cost.
///
/// Returns `Ok(Some(max))` when every `Yield` is at the top level (block
/// nesting depth 0), so that each resumption runs exactly one segment
/// (entry to the first yield, then between consecutive yields, then the
/// last yield to the end) and the maximum is the exact per-resume WCET.
/// Returns `Ok(None)` when any `Yield` is nested inside an `If`/`Loop`
/// block, since a resumption could then re-enter the middle of a
/// control-flow construct and the segment split is not structural; the
/// caller falls back to the whole-body cumulative bound. Propagates an
/// error if a loop within a segment lacks a statically extractable
/// iteration bound.
fn reentrant_segmented_wcet(
    chunk: &Chunk,
    cost_model: &crate::bytecode::CostModel,
    native_bounds: &[NativeIterationBound],
) -> Result<Option<u32>, VerifyError> {
    // Collect Yield positions, bailing to None if any is nested.
    let mut depth: i32 = 0;
    let mut yields: Vec<usize> = Vec::new();
    for (ip, op) in chunk.ops.iter().enumerate() {
        match op {
            Op::If(_) | Op::Loop(_) => depth += 1,
            Op::EndIf | Op::EndLoop(_) => depth -= 1,
            Op::Yield => {
                if depth != 0 {
                    return Ok(None);
                }
                yields.push(ip);
            }
            _ => {}
        }
    }
    if yields.is_empty() {
        // A Reentrant chunk must contain a Yield (pass 2), so this is
        // unreachable for a verified chunk; fall back conservatively.
        return Ok(None);
    }

    let len = chunk.ops.len();
    let wcet_extra = chunk_wcet_extra(chunk, cost_model, native_bounds);
    let mut max_cost: u32 = 0;
    let mut seg_start: usize = 0;
    // Each segment ends just past its terminating yield, so the yield
    // op's own cost is included in the segment it ends.
    // A segment contains no yields (the top-level yields delimit them),
    // so any loop within a segment runs fully in one resumption: no
    // productivity clamp applies (pass `false`).
    for &y in &yields {
        let mut break_costs: Vec<u32> = Vec::new();
        let c = wcet_region(
            chunk,
            seg_start,
            y + 1,
            &mut break_costs,
            cost_model,
            false,
            &wcet_extra,
        )?
        .unwrap_or(0);
        max_cost = max_cost.max(c);
        seg_start = y + 1;
    }
    // The final segment runs from after the last yield to the end.
    let mut break_costs: Vec<u32> = Vec::new();
    let c = wcet_region(
        chunk,
        seg_start,
        len,
        &mut break_costs,
        cost_model,
        false,
        &wcet_extra,
    )?
    .unwrap_or(0);
    max_cost = max_cost.max(c);
    Ok(Some(max_cost))
}

/// Compute the worst-case memory usage of a non-Stream chunk's whole op
/// range as `(stack_bytes, heap_bytes)`.
///
/// This is the [`wcmu_stream_iteration`] computation applied to the
/// chunk's entire op range. It accepts `Func` and `Reentrant` chunks.
/// For a `Reentrant` chunk the WCMU is genuinely the whole-body peak,
/// not a loose bound: a coroutine's call frame and operand stack
/// persist across `yield`, so the peak footprint is the maximum over
/// the whole body. Like the Stream path it uses an empty call resolver
/// (the per-site transitive contribution is composed by
/// [`module_wcmu`]); it feeds the B29 `VerifierWitness` `resource-bounds`
/// obligation and is not folded into the module WCMU header.
pub fn wcmu_whole_chunk(chunk: &Chunk) -> Result<(u32, u32), VerifyError> {
    wcmu_whole_chunk_with_value_slot_bytes(chunk, crate::bytecode::VALUE_SLOT_SIZE_BYTES)
}

/// Variant of [`wcmu_whole_chunk`] that uses a host-supplied
/// `value_slot_bytes`. See
/// [`wcmu_stream_iteration_with_value_slot_bytes`].
pub fn wcmu_whole_chunk_with_value_slot_bytes(
    chunk: &Chunk,
    value_slot_bytes: u32,
) -> Result<(u32, u32), VerifyError> {
    if chunk.block_type == BlockType::Stream {
        return Err(VerifyError {
            chunk_name: chunk.name.clone(),
            message: String::from("wcmu_whole_chunk requires a non-Stream block"),
        });
    }
    let mut breaks: Vec<McuResult> = Vec::new();
    let resolver = CallResolver::empty();
    let body = wcmu_region(
        chunk,
        0,
        chunk.ops.len(),
        &mut breaks,
        &resolver,
        value_slot_bytes,
    )?
    .unwrap_or(McuResult::empty());

    let stack_slots = chunk.local_count as u32 + body.peak_above_initial;
    let stack_bytes = stack_slots * value_slot_bytes;
    Ok((stack_bytes, body.heap_total))
}

/// Compute the per-chunk WCMU for an entire module.
///
/// Returns a vector indexed by chunk index. Each entry is `(stack_bytes,
/// heap_bytes)` and includes the chunk's local frame, body peak, and
/// transitive contributions of any chunks or natives the chunk calls.
///
/// `native_wcmu` supplies the host-attested heap usage per native
/// function, indexed by native function entry index. Natives whose
/// index falls outside the slice contribute zero. This matches the
/// default attestation when the host has not yet declared a native's
/// bounds.
///
/// The call graph is required to be acyclic (R4 forbids recursion).
/// Returns an error if a recursive call is detected.
pub fn module_wcmu(module: &Module, native_wcmu: &[u32]) -> Result<Vec<(u32, u32)>, VerifyError> {
    module_wcmu_with_value_slot_bytes(module, native_wcmu, crate::bytecode::VALUE_SLOT_SIZE_BYTES)
}

/// Variant of [`module_wcmu`] that uses a host-supplied
/// `value_slot_bytes` for the bytes-per-slot multiplier. Hosts
/// running narrow `GenericVm<W, A, F>` instances pass
/// `size_of::<GenericValue<W, F>>()` so the bound matches the
/// runtime's actual slot footprint rather than the conservative
/// 64-bit-runtime default.
pub fn module_wcmu_with_value_slot_bytes(
    module: &Module,
    native_wcmu: &[u32],
    value_slot_bytes: u32,
) -> Result<Vec<(u32, u32)>, VerifyError> {
    // The WCMU analysis is sound only for acyclic call graphs that
    // the static analysis can fully traverse. V0.2.0 Phase 4
    // retired the closure family (`Op::PushFunc`, `Op::MakeClosure`,
    // `Op::MakeRecursiveClosure`, `Op::CallIndirect`); first-class
    // function values and indirect-call dispatch are rejected at
    // the type-checker stage and cannot reach the verifier. The
    // previous pre-emptive rejection loop is therefore no longer
    // required. The call graph that this analysis traverses is
    // acyclic by construction because the type checker also
    // rejects mutually-recursive top-level functions; mutual
    // recursion is tracked under B14's CallIndirect flow analysis
    // for V0.3.
    //
    // `Vm::new_unchecked` exists for hosts that load precompiled
    // bytecode whose resource bounds were validated during the
    // build pipeline. It is not a path for admitting unbounded
    // programs at runtime; using it that way is intentional misuse
    // outside the language's WCET contract.
    let n = module.chunks.len();
    let mut chunk_wcmu: Vec<Option<(u32, u32)>> = alloc::vec![None; n];
    // Per-chunk text-returning flag. Populated in topological order
    // as each chunk is analysed, so when a caller is processed all
    // of its callees already have entries. Used by the text-size
    // pass to push `NotText` for `Op::Call` returns from non-text
    // callees, restoring the type-checker's "either operand NotText
    // implies result NotText" invariant under Op::Add.
    let mut chunk_returns_text: Vec<bool> = alloc::vec![false; n];
    let order = topological_call_order(module)?;
    for chunk_idx in order {
        let chunk = &module.chunks[chunk_idx];
        let resolver = CallResolver {
            chunk_wcmu: &chunk_wcmu,
            native_wcmu,
            shared_layout: module
                .data_layout
                .as_ref()
                .map_or(&[], |dl| &dl.shared_layout),
        };
        let (wcmu_result, returns_text) =
            compute_chunk_wcmu(chunk, &resolver, &chunk_returns_text, value_slot_bytes)?;
        chunk_wcmu[chunk_idx] = Some(wcmu_result);
        chunk_returns_text[chunk_idx] = returns_text;
    }
    Ok(chunk_wcmu
        .into_iter()
        .map(|o| o.unwrap_or((0, 0)))
        .collect())
}

/// Per-module runtime memory footprint used to pre-size the arena's
/// bottom-region working structures at VM construction (B28 P3 item 5,
/// priority 1: accurate worst-case memory usage).
///
/// All three figures are module-wide maxima taken over every chunk,
/// because the host may invoke any `Func` chunk directly through
/// [`crate::vm::GenericVm::call_function`] and the single operand-stack
/// and call-frame vectors must hold the worst case across every entry the
/// VM admits. Pre-sizing to these maxima realises the no-allocation-after-
/// initialisation contract (JPL Power-of-10 rule 3): a too-small arena
/// fails at construction, not mid-stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeFootprint {
    /// Peak number of operand-stack slots, transitively including the
    /// slots consumed by called chunks (`wcmu_region` folds the callee's
    /// stack into the caller at each `Op::Call`). Representation-
    /// independent: a slot count, not bytes. The byte size depends on the
    /// runtime's `GenericValue<W, F>` width, which the caller multiplies
    /// in.
    pub max_operand_slots: u32,
    /// Peak call-frame depth, equal to the longest root-to-leaf path in
    /// the (acyclic, recursion-rejected) static call graph. A chunk with
    /// no calls has depth one (the frame pushed when it is invoked).
    pub max_frame_depth: u32,
    /// Peak per-iteration arena-heap (top-region) bytes over every chunk.
    /// Real bytes, not slot-scaled.
    pub max_heap_bytes: u32,
}

/// Maximum call-frame depth of the module's static call graph.
///
/// The call graph is acyclic because the type checker rejects direct and
/// mutual recursion, so the depth is finite and computed in one pass over
/// the topological order (callees before callers, so a caller sees each
/// callee's resolved depth). A chunk with no calls has depth one. Returns
/// an error if a cycle is detected (which `topological_call_order` already
/// rejects).
pub fn module_call_depth(module: &Module) -> Result<u32, VerifyError> {
    let n = module.chunks.len();
    let order = topological_call_order(module)?;
    let mut depth = alloc::vec![1u32; n];
    for idx in order {
        let mut d = 1u32;
        for op in &module.chunks[idx].ops {
            if let Op::Call(callee, _) = op {
                let c = *callee as usize;
                if c < n {
                    d = d.max(depth[c].saturating_add(1));
                }
            }
        }
        depth[idx] = d;
    }
    Ok(depth.into_iter().max().unwrap_or(0))
}

/// Compute the module's [`RuntimeFootprint`] for arena pre-sizing.
///
/// Runs the per-chunk WCMU analysis with a unit slot size so the stack
/// component is denominated in slots rather than bytes; the slot count is
/// representation-independent, and the runtime multiplies by its actual
/// `size_of::<GenericValue<W, F>>()` when reserving. The Call-site folding
/// (`callee_stack_bytes / value_slot_bytes`) stays consistent at unit
/// scale, and the heap component (`Op::heap_alloc`) is independent of the
/// slot size, so it remains in real bytes. The call-frame component comes
/// from [`module_call_depth`].
pub fn module_runtime_footprint(
    module: &Module,
    native_wcmu: &[u32],
) -> Result<RuntimeFootprint, VerifyError> {
    let per_chunk = module_wcmu_with_value_slot_bytes(module, native_wcmu, 1)?;
    let mut max_operand_slots = 0u32;
    let mut max_heap_bytes = 0u32;
    for (stack_slots, heap_bytes) in per_chunk {
        max_operand_slots = max_operand_slots.max(stack_slots);
        max_heap_bytes = max_heap_bytes.max(heap_bytes);
    }
    let max_frame_depth = module_call_depth(module)?;
    Ok(RuntimeFootprint {
        max_operand_slots,
        max_frame_depth,
        max_heap_bytes,
    })
}

/// Per-native attestation passed to
/// [`module_wcmu_with_bounds`] and friends. Carries the host-
/// attested per-call WCMU and, for external natives, the
/// invocation-count attestation. The verifier sums per-call WCMU
/// over static call sites for verified natives and adds
/// `max_invocations_per_iteration * per_call_wcmu_bytes` once per
/// chunk for external natives, matching the
/// `use external module::name` source-level semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeIterationBound {
    /// Per-call WCMU bytes from the host attestation. Applies
    /// both to verified natives (summed over call sites) and
    /// external natives (multiplied by `max_invocations` once
    /// per chunk).
    pub per_call_wcmu_bytes: u32,
    /// Per-call worst-case execution time in the unitless cost space of
    /// `Op::cost()`, from the host attestation (`Vm::set_native_bounds`,
    /// default `DEFAULT_NATIVE_WCET`). The WCET pass adds it for the native's
    /// body, symmetric with `per_call_wcmu_bytes`: summed over static call sites
    /// for a verified native (scaled by loop multiplicity) and multiplied by
    /// `max_invocations` once per chunk for an external native (#50).
    pub per_call_wcet_cycles: u32,
    /// `None` for verified natives. `Some(n)` for external
    /// natives where `n` is the host-attested upper bound on
    /// per-iteration invocations.
    pub max_invocations: Option<u32>,
}

/// Variant of [`module_wcmu`] that consumes per-native attestations
/// with classification awareness. For each chunk in topological
/// call order, walks the chunk's ops once to compute the per-site
/// WCMU contribution (verified natives only, since external
/// natives' per-call bound is excluded from the per-site sum) and
/// then adds one chunk-level contribution per unique external
/// native referenced.
///
/// The chunk-level external contribution is
/// `max_invocations * per_call_wcmu` regardless of how many
/// static call sites reference the native. This matches the
/// `use external` contract: the host attests total invocations
/// per iteration, not per call site.
///
/// Hosts that need only per-call attestations call the simpler
/// [`module_wcmu`] entry point, which under the hood routes
/// through this function with all-verified bounds.
pub fn module_wcmu_with_bounds(
    module: &Module,
    bounds: &[NativeIterationBound],
    value_slot_bytes: u32,
) -> Result<Vec<(u32, u32)>, VerifyError> {
    let n = module.chunks.len();
    let mut chunk_wcmu: Vec<Option<(u32, u32)>> = alloc::vec![None; n];
    let mut chunk_returns_text: Vec<bool> = alloc::vec![false; n];
    // Per-site WCMU collected through `CallResolver` consumes
    // only the verified per-call values; external natives surface
    // as zero from the resolver and are added per chunk below.
    let per_site_native_wcmu: Vec<u32> = bounds
        .iter()
        .map(|b| match b.max_invocations {
            Some(_) => 0,
            None => b.per_call_wcmu_bytes,
        })
        .collect();
    let order = topological_call_order(module)?;
    for chunk_idx in order {
        let chunk = &module.chunks[chunk_idx];
        let resolver = CallResolver {
            chunk_wcmu: &chunk_wcmu,
            native_wcmu: &per_site_native_wcmu,
            shared_layout: module
                .data_layout
                .as_ref()
                .map_or(&[], |dl| &dl.shared_layout),
        };
        let (mut wcmu_result, returns_text) =
            compute_chunk_wcmu(chunk, &resolver, &chunk_returns_text, value_slot_bytes)?;
        // Chunk-level external-native contribution. Walk the
        // chunk's ops once to collect unique external native
        // indices, then add each native's
        // `max_invocations * per_call_wcmu` to the chunk's heap
        // total. Deduplication ensures the bound is independent
        // of the static call-site count.
        let mut seen_external: alloc::collections::BTreeSet<u16> =
            alloc::collections::BTreeSet::new();
        for op in &chunk.ops {
            if let Op::CallExternalNative(idx, _) = op
                && seen_external.insert(*idx)
                && let Some(bound) = bounds.get(*idx as usize)
                && let Some(max_inv) = bound.max_invocations
            {
                let contribution = bound.per_call_wcmu_bytes.saturating_mul(max_inv);
                wcmu_result.1 = wcmu_result.1.saturating_add(contribution);
            }
        }
        chunk_wcmu[chunk_idx] = Some(wcmu_result);
        chunk_returns_text[chunk_idx] = returns_text;
    }
    Ok(chunk_wcmu
        .into_iter()
        .map(|o| o.unwrap_or((0, 0)))
        .collect())
}

/// Per-chunk text-flow analysis for the whole module, computed in
/// topological call order so each caller sees its callees' resolved
/// text-returning bits. The returned vector is indexed by chunk index
/// and mirrors `module.chunks`.
///
/// The compiler uses this to refine the ephemerality decision: a
/// module that declares a `Text` return or yield type on its entry
/// point but whose entry chunk's `Op::Return`/`Op::Yield` peeks all
/// resolve to `TextSize::NotText` does not actually carry text across
/// the host-VM boundary at runtime, and is therefore admissible as
/// ephemeral.
///
/// Returns an error if the call graph contains a cycle. Direct
/// callers in the compiler should treat any error as "fall back to
/// the conservative signature-only ephemerality check" rather than
/// propagating, because recursion or cycles cannot occur in modules
/// that already passed type-check and the broader verifier.
pub fn module_chunk_text_analyses(
    module: &Module,
) -> Result<Vec<crate::text_size::ChunkTextAnalysis>, VerifyError> {
    let n = module.chunks.len();
    let mut chunk_returns_text: Vec<bool> = alloc::vec![false; n];
    let mut analyses: Vec<crate::text_size::ChunkTextAnalysis> = alloc::vec![
        crate::text_size::ChunkTextAnalysis {
            heap_alloc: 0,
            returns_text: false,
            yields_text: false,
        };
        n
    ];
    let order = topological_call_order(module)?;
    for chunk_idx in order {
        let chunk = &module.chunks[chunk_idx];
        let analysis = crate::text_size::analyze_chunk_text(chunk, &chunk_returns_text);
        chunk_returns_text[chunk_idx] = analysis.returns_text;
        analyses[chunk_idx] = analysis;
    }
    Ok(analyses)
}

/// Topological order of the call graph. Leaves come first, roots last.
fn topological_call_order(module: &Module) -> Result<Vec<usize>, VerifyError> {
    let n = module.chunks.len();
    let mut visited = alloc::vec![false; n];
    let mut on_stack = alloc::vec![false; n];
    let mut order = Vec::new();
    for i in 0..n {
        if !visited[i] {
            topo_visit(module, i, &mut visited, &mut on_stack, &mut order)?;
        }
    }
    Ok(order)
}

fn topo_visit(
    module: &Module,
    idx: usize,
    visited: &mut [bool],
    on_stack: &mut [bool],
    order: &mut Vec<usize>,
) -> Result<(), VerifyError> {
    if on_stack[idx] {
        return Err(VerifyError {
            chunk_name: module.chunks[idx].name.clone(),
            message: String::from("recursive call detected during WCMU topological sort"),
        });
    }
    if visited[idx] {
        return Ok(());
    }
    on_stack[idx] = true;
    for op in &module.chunks[idx].ops {
        if let Op::Call(callee, _) = op {
            let callee_idx = *callee as usize;
            if callee_idx < module.chunks.len() {
                topo_visit(module, callee_idx, visited, on_stack, order)?;
            }
        }
    }
    on_stack[idx] = false;
    visited[idx] = true;
    order.push(idx);
    Ok(())
}

/// Compute the WCMU of a single chunk given a resolver populated for
/// any chunks it calls. Also reports whether the chunk may return a
/// text-typed value, used by the module-level pass to populate
/// `chunk_returns_text` for subsequent callers in topological order.
fn compute_chunk_wcmu(
    chunk: &Chunk,
    resolver: &CallResolver,
    chunk_returns_text: &[bool],
    value_slot_bytes: u32,
) -> Result<((u32, u32), bool), VerifyError> {
    let (start, end) = match chunk.block_type {
        BlockType::Stream => {
            let stream_pos = chunk
                .ops
                .iter()
                .position(|op| matches!(op, Op::Stream))
                .ok_or_else(|| VerifyError {
                    chunk_name: chunk.name.clone(),
                    message: String::from("Stream block missing Stream instruction"),
                })?;
            let reset_pos = chunk
                .ops
                .iter()
                .position(|op| matches!(op, Op::Reset))
                .ok_or_else(|| VerifyError {
                    chunk_name: chunk.name.clone(),
                    message: String::from("Stream block missing Reset instruction"),
                })?;
            (stream_pos + 1, reset_pos)
        }
        BlockType::Func | BlockType::Reentrant => (0, chunk.ops.len()),
    };

    let mut breaks: Vec<McuResult> = Vec::new();
    let body = wcmu_region(chunk, start, end, &mut breaks, resolver, value_slot_bytes)?
        .unwrap_or(McuResult::empty());

    let stack_slots = chunk.local_count as u32 + body.peak_above_initial;
    let stack_bytes = stack_slots * value_slot_bytes;

    // Augment the heap bound with the chunk's text-allocation bound
    // computed by the text-size abstract interpretation pass. The
    // pass tracks per-callee text-ness through `chunk_returns_text`,
    // so calls to non-text-returning chunks contribute `NotText`
    // rather than `Unbounded` to the abstract operand stack. This
    // preserves the type-checker's "either operand NotText implies
    // result NotText" invariant under Op::Add and admits programs
    // whose helper-function chains the previous policy rejected
    // (see backlog item B12).
    let text_analysis = crate::text_size::analyze_chunk_text(chunk, chunk_returns_text);
    let heap_total = body.heap_total.saturating_add(text_analysis.heap_alloc);

    Ok(((stack_bytes, heap_total), text_analysis.returns_text))
}

/// Compute a memory budget for the given Stream chunk.
///
/// The budget bottom side carries the stack WCMU. The budget top side
/// carries the heap WCMU. This pairing matches the Keleusma runtime
/// convention in which the operand stack uses the arena's bottom end
/// and the dynamic-string heap uses the arena's top end.
///
/// Returns an error if the chunk is not a Stream block.
pub fn budget_for_stream(chunk: &Chunk) -> Result<keleusma_arena::Budget, VerifyError> {
    let (stack_bytes, heap_bytes) = wcmu_stream_iteration(chunk)?;
    Ok(keleusma_arena::Budget::new(
        stack_bytes as usize,
        heap_bytes as usize,
    ))
}

/// Verify that the module's worst-case memory usage fits within the
/// given arena capacity, using the local-only analysis.
///
/// Equivalent to [`verify_resource_bounds_with_natives`] with empty
/// native attestations. Suitable for programs without function calls
/// or natives, or as an initial sanity check before native attestation
/// has been declared.
pub fn verify_resource_bounds(module: &Module, arena_capacity: usize) -> Result<(), VerifyError> {
    verify_resource_bounds_with_natives(module, arena_capacity, &[])
}

/// Verify resource bounds against a host-supplied [`crate::bytecode::CostModel`].
///
/// **Unit contract.** WCMU is reported in **bytes** and compared
/// against the arena capacity. WCET is reported in **nominal cycles**
/// per the supplied cost model. The byte unit is target-independent
/// in principle; the actual byte count depends on the cost model's
/// `value_slot_bytes`. The cycle unit is target-dependent and the
/// numeric values reflect the cost model's `op_cycles` table.
///
/// Hosts that supply a custom cost model can use this entry point to
/// validate a module against measured per-target cycle and byte
/// tables. The cost model parameter currently affects the API
/// contract; full internal threading of the cost model through the
/// per-chunk WCMU computation remains future work tracked under
/// B10 cost-table follow-on. The present implementation delegates
/// to [`verify_resource_bounds_with_natives`], which uses the
/// bundled [`crate::bytecode::NOMINAL_COST_MODEL`]. A future refinement
/// will route the host-supplied model through the per-chunk
/// computation so that custom cycle and byte tables actually
/// determine the bound.
pub fn verify_resource_bounds_with_cost_model(
    module: &Module,
    arena_capacity: usize,
    cost_model: &crate::bytecode::CostModel,
    native_wcmu: &[u32],
) -> Result<(), VerifyError> {
    // The `value_slot_bytes` field of the cost model drives the WCMU
    // analysis's bytes-per-slot multiplier. Hosts running narrow
    // `GenericVm<W, A, F>` instances supply a cost model whose
    // `value_slot_bytes` equals `size_of::<GenericValue<W, F>>()` to
    // tighten the bound from the default 32-byte 64-bit-runtime
    // assumption. The cycle component of the cost model
    // (`op_cycles`) drives the WCET computation through
    // [`wcet_stream_iteration_with_cost_model`]; this entry point
    // currently routes only the WCMU side because the runtime
    // accepts the bytecode's declared WCET cycle field as a
    // host attestation rather than re-verifying it against the
    // arena.
    verify_resource_bounds_with_natives_and_value_slot_bytes(
        module,
        arena_capacity,
        native_wcmu,
        cost_model.value_slot_bytes,
    )
}

/// Verify that the module's worst-case memory usage fits within the
/// given arena capacity, with full call-graph integration and native
/// attestations.
///
/// Computes [`module_wcmu`] using `native_wcmu` for native functions
/// and the recursively computed per-chunk values for `Op::Call`. For
/// each Stream chunk, builds a [`keleusma_arena::Budget`] and checks
/// admissibility through [`keleusma_arena::Arena::fits_budget`].
/// Programs that exceed the bound are rejected with a `VerifyError`
/// describing which chunk failed.
///
/// Variable-iteration loops are still treated as one iteration. This
/// limitation is tracked separately and is unsound for programs that
/// rely on bounded iteration counts to stay within budget.
pub fn verify_resource_bounds_with_natives(
    module: &Module,
    arena_capacity: usize,
    native_wcmu: &[u32],
) -> Result<(), VerifyError> {
    verify_resource_bounds_with_natives_and_value_slot_bytes(
        module,
        arena_capacity,
        native_wcmu,
        crate::bytecode::VALUE_SLOT_SIZE_BYTES,
    )
}

/// Variant of [`verify_resource_bounds_with_natives`] that uses a
/// host-supplied `value_slot_bytes` for the bytes-per-slot
/// multiplier. The parametric `GenericVm<W, A, F>` runtime calls
/// this with `size_of::<GenericValue<W, F>>()` so the bound matches
/// the runtime's actual slot footprint.
pub fn verify_resource_bounds_with_natives_and_value_slot_bytes(
    module: &Module,
    arena_capacity: usize,
    native_wcmu: &[u32],
    value_slot_bytes: u32,
) -> Result<(), VerifyError> {
    let chunk_wcmu = module_wcmu_with_value_slot_bytes(module, native_wcmu, value_slot_bytes)?;
    enforce_arena_capacity(module, arena_capacity, &chunk_wcmu)
}

/// Variant of [`verify_resource_bounds_with_natives_and_value_slot_bytes`]
/// that consumes per-native attestations with classification
/// awareness. External natives' chunk-level contribution is
/// `max_invocations_per_iteration * per_call_wcmu_bytes` per
/// chunk, applied once regardless of the static call-site count.
pub fn verify_resource_bounds_with_bounds(
    module: &Module,
    arena_capacity: usize,
    bounds: &[NativeIterationBound],
    value_slot_bytes: u32,
) -> Result<(), VerifyError> {
    let chunk_wcmu = module_wcmu_with_bounds(module, bounds, value_slot_bytes)?;
    enforce_arena_capacity(module, arena_capacity, &chunk_wcmu)
}

/// Enforce that every Stream chunk's WCMU fits within the
/// supplied arena capacity. Shared by both
/// `verify_resource_bounds_with_natives_and_value_slot_bytes` and
/// `verify_resource_bounds_with_bounds`.
fn enforce_arena_capacity(
    module: &Module,
    arena_capacity: usize,
    chunk_wcmu: &[(u32, u32)],
) -> Result<(), VerifyError> {
    for (chunk_idx, chunk) in module.chunks.iter().enumerate() {
        if chunk.block_type != BlockType::Stream {
            continue;
        }
        let (stack_bytes, heap_bytes) = chunk_wcmu[chunk_idx];
        let budget = keleusma_arena::Budget::new(stack_bytes as usize, heap_bytes as usize);
        if budget.total() > arena_capacity {
            return Err(VerifyError {
                chunk_name: chunk.name.clone(),
                message: alloc::format!(
                    "WCMU budget {} bytes (bottom {} + top {}) exceeds arena capacity {} bytes",
                    budget.total(),
                    budget.bottom_bytes,
                    budget.top_bytes,
                    arena_capacity
                ),
            });
        }
    }
    Ok(())
}

/// The verification checks that admit `chunk`, as a structured
/// acceptance trace for the B29 `VerifierWitness` debug record. The
/// returned identifiers name the passes [`verify`] runs and that a
/// successful [`verify`] therefore establishes for the chunk: block
/// nesting and offset validation (pass 1) and block-type constraints
/// (pass 2) apply to every chunk, and productive divergence (pass 3,
/// every Stream-to-Reset path yields) additionally applies to Stream
/// chunks. The names are stable identifiers a auditor
/// can correlate to the verifier's passes; this is a per-chunk
/// admission summary. For a finer trace keyed to individual op
/// positions, see [`chunk_verification_obligations`].
pub fn chunk_verification_witness(chunk: &Chunk) -> alloc::vec::Vec<&'static str> {
    let mut checks = alloc::vec!["block-nesting-and-offsets", "block-type-constraints"];
    if chunk.block_type == BlockType::Stream {
        checks.push("productive-divergence");
    }
    checks
}

/// A single verification obligation discharged for a chunk: the
/// op-stream position it concerns, the [`verify`] pass that established
/// it, and a stable identifier for the property proven.
///
/// An obligation that pertains to the chunk as a whole rather than to a
/// particular construct carries `op_index == 0` (for example,
/// `all-blocks-closed`). Construct-level obligations carry the position
/// of the construct they describe, so a reader groups them with
/// [`DebugPool::records_at`](crate::debug_meta::DebugPool::records_at).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationObligation {
    /// The op the obligation concerns, or `0` for a chunk-level fact.
    pub op_index: u32,
    /// The [`verify`] pass that established the obligation.
    pub pass: &'static str,
    /// A stable identifier for the property the pass proved.
    pub property: &'static str,
}

/// The per-construct *structural* verification trace for `chunk`: one
/// [`VerificationObligation`] for each individual check the three
/// structural passes of [`verify`] discharge, keyed to the op position
/// it concerns.
///
/// Scope: this covers the three structural passes of [`verify`] only
/// (block nesting and offsets, block-type constraints, and productive
/// divergence). It does **not** cover the resource-bound analysis
/// (per-iteration WCET and WCMU), which is a distinct verification
/// activity the compile pipeline runs separately and whose obligations
/// it emits at that stage, nor the load-time arena-capacity admission
/// (`verify_resource_bounds`).
///
/// This is the finer counterpart to [`chunk_verification_witness`].
/// The obligations are produced by the same per-chunk routine that
/// renders the verdict ([`verify`] and this function both call
/// `verify_chunk`), so the trace cannot drift from the checks the
/// verifier actually performs: each obligation is recorded at the point
/// the corresponding check is discharged, one obligation per check.
/// Because the obligations are emitted only as checks *pass*, a chunk
/// that would fail [`verify`] yields a truncated trace ending at the
/// first failing check rather than fabricated facts; callers that want
/// a complete trace must verify first, as the compile pipeline does.
///
/// The properties remain a faithful record of the verifier's checks,
/// not a machine-checkable derivation.
pub fn chunk_verification_obligations(
    chunk: &Chunk,
    module: &Module,
) -> alloc::vec::Vec<VerificationObligation> {
    let mut obligations: alloc::vec::Vec<VerificationObligation> = alloc::vec::Vec::new();
    // The Result is ignored: on success the trace is complete; on
    // failure it is truncated at the first failing check, which is the
    // documented contract. Callers gate completeness on a prior verify.
    let _ = verify_chunk(chunk, module, Some(&mut obligations));
    obligations
}

/// Verify structural invariants of a compiled module.
///
/// Checks performed per chunk:
/// 1. Block nesting: Every If is matched by EndIf (with optional Else).
///    Every Loop is matched by EndLoop. No orphaned delimiters.
/// 2. Offset validation: If points to Else or EndIf. Else points to EndIf.
///    Loop points past EndLoop. EndLoop points after Loop. Break/BreakIf
///    point past an enclosing EndLoop.
/// 3. Block type constraints: Func chunks contain no Yield, Stream, or Reset.
///    Reentrant chunks contain at least one Yield and no Stream or Reset.
///    Stream chunks contain exactly one Stream, exactly one Reset, and at
///    least one Yield.
/// 4. Break containment: Every Break and BreakIf is inside a Loop/EndLoop.
/// 5. Productivity rule (Stream chunks only): All control flow paths from
///    Stream to Reset pass through at least one Yield.
pub fn verify(module: &Module) -> Result<(), VerifyError> {
    for chunk in &module.chunks {
        verify_chunk(chunk, module, None)?;
        verify_stack_depth(chunk)?;
    }
    // Typed operand-stack pass (A.2.1): reconstructs per-slot flat shapes and
    // validates baked flat offsets, branch/loop stack balance, and the
    // wire-carried layout tables. It runs in defer-on-`Top` mode — a value
    // whose shape it cannot reconstruct defers to the retained runtime guard —
    // so it only ever rejects a provable violation and never a valid program.
    let wb = (1usize << module.word_bits_log2) / 8;
    let fb = (1usize << module.float_bits_log2) / 8;
    crate::verify_typed::typed_check_module(module, wb, fb).map_err(|e| VerifyError {
        chunk_name: alloc::string::String::from("<typed operand-stack pass>"),
        message: alloc::format!("typed operand-stack verification failed: {e:?}"),
    })?;
    Ok(())
}

/// Operand-stack effect of `op` for the depth-verification pass (audit
/// finding 3): `(required, net)`, where `required` is the number of
/// operands that must be present on entry and `net` is the change to the
/// operand-stack depth (`produced - consumed`).
///
/// This is deliberately distinct from [`crate::bytecode::Op::stack_shrink`]
/// and [`crate::bytecode::Op::stack_growth`], which encode the worst-case-
/// memory net and do not capture actual operand consumption: `Add`
/// consumes two operands yet has `stack_shrink` 1, the checked ops consume
/// two yet have `stack_shrink` 0, and `Yield` is modelled there as net -1
/// though it pops the output and pushes the resume value (net 0). The
/// values here follow the VM handlers' actual pops and pushes. The
/// control-flow ops `If`, `Loop`, `Break`, `Trap`, and `Return` are
/// intercepted by [`verify_depth_region`]; their entries here are used
/// only as a defensive fall-through.
pub(crate) fn op_depth_effect(op: &Op, _chunk: &Chunk) -> (i32, i32) {
    match op {
        Op::Const(_) | Op::GetLocal(_) | Op::GetData(_) | Op::PushImmediate(_) => (0, 1),
        Op::Dup => (1, 1),
        Op::SetLocal(_) | Op::SetData(_) | Op::SetDataComposite(_, _) => (1, -1),
        Op::GetDataIndexed(_, _) => (1, 0),
        Op::SetDataIndexed(_, _) => (2, -2),
        Op::BoundsCheck(_) => (1, 0),
        Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Mod
        | Op::CmpEq
        | Op::CmpNe
        | Op::CmpLt
        | Op::CmpGt
        | Op::CmpLe
        | Op::CmpGe
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Shl
        | Op::Shr
        | Op::FixedMul(_)
        | Op::FixedDiv(_)
        | Op::GetIndex(_) => (2, -1),
        Op::Neg
        | Op::Not
        | Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToByte
        | Op::ByteToWord
        | Op::WordToFixed(_)
        | Op::FixedToWord(_)
        | Op::GetField(_)
        | Op::GetTupleField(_)
        | Op::GetEnumField(_)
        | Op::Len => (1, 0),
        // Yield pops the output and the resume pushes the input: net 0.
        Op::Yield => (1, 0),
        // IsEnum/IsStruct peek the value and push a bool, keeping the
        // value for a following field extraction: net +1.
        Op::IsEnum(_, _, _) | Op::IsStruct(_) => (1, 1),
        Op::Call(_, n) => (*n as i32, 1 - *n as i32),
        Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => {
            let m = (*n & 0x7F) as i32;
            let produced = if *n & 0x80 != 0 { 2 } else { 1 };
            (m, produced - m)
        }
        // NewComposite pops `count` values (an enum's leading discriminant
        // counts as one) and pushes one composite (B28 P4).
        Op::NewComposite(op) => {
            let c = op.count() as i32;
            (c, 1 - c)
        }
        Op::CheckedAdd
        | Op::CheckedSub
        | Op::CheckedMod
        | Op::CheckedMul(_)
        | Op::CheckedDiv(_) => (2, 1),
        Op::CheckedNeg => (1, 2),
        Op::PopN(n) => (*n as i32, -(*n as i32)),
        Op::If(_) | Op::BreakIf(_) => (1, -1),
        Op::Else(_)
        | Op::EndIf
        | Op::Loop(_)
        | Op::EndLoop(_)
        | Op::Break(_)
        | Op::Stream
        | Op::Reset
        | Op::Trap(_)
        | Op::Return => (0, 0),
    }
}

fn depth_underflow(chunk: &Chunk, ip: usize, op: &Op, need: i32, have: i32) -> VerifyError {
    VerifyError {
        chunk_name: chunk.name.clone(),
        message: alloc::format!(
            "{:?} at {} requires {} operand(s) but only {} are on the stack; the operand stack would underflow",
            op,
            ip,
            need,
            have
        ),
    }
}

/// Operand-stack-depth verification (audit finding 3,
/// `poc_newarray_underflow`).
///
/// A forward pass over the chunk that tracks the *absolute* operand-stack
/// depth and rejects any op that would consume more operands than are
/// present. Unlike [`wcmu_region`], which tracks a region-relative offset
/// for the worst-case-memory bound and clamps underflow, this pass passes
/// the entry depth into each branch and loop body, so the underflow check
/// is correct inside structured control flow. It mirrors that traversal's
/// control-flow shape: an `If` with or without an `Else`, a `Loop` body
/// treated as depth-neutral, and `Break`/`Trap`/`Return` as path exits.
///
/// This establishes the precondition the VM construct and call handlers
/// assume, so a safe `Vm::new` rejects an underflowing chunk instead of
/// relying on the runtime guards, which remain as defense in depth for
/// `Vm::new_unchecked`. Runs after `verify_chunk`, so branch and loop
/// targets are already validated in bounds.
fn verify_stack_depth(chunk: &Chunk) -> Result<(), VerifyError> {
    // The chunk body is not inside any loop, so its break collector stays
    // empty (Pass-1 already rejects a Break outside a loop).
    let mut breaks: Vec<i32> = Vec::new();
    verify_depth_region(chunk, 0, chunk.ops.len(), 0, &mut breaks).map(|_| ())
}

/// Walk ops `[start, end)` tracking absolute operand depth from `entry`.
/// Returns `Ok(Some(end_depth))` when the region falls through and
/// `Ok(None)` when every path exits via `Break`, `Trap`, or `Return`.
/// `breaks` collects the operand depth at each `Break`/`BreakIf` edge that
/// leaves the enclosing loop, so the loop can resume at the depth its
/// exits leave on the stack (a loop used as a labelled block can break
/// with a value). Returns `Err` on an operand-stack underflow.
fn verify_depth_region(
    chunk: &Chunk,
    start: usize,
    end: usize,
    entry: i32,
    breaks: &mut Vec<i32>,
) -> Result<Option<i32>, VerifyError> {
    let ops = &chunk.ops;
    let mut depth = entry;
    let mut ip = start;
    while ip < end {
        let op = &ops[ip];
        match op {
            Op::Trap(_) | Op::Return => return Ok(None),
            Op::Break(_) => {
                // Unconditional exit to after the enclosing loop, carrying
                // the current operand depth.
                breaks.push(depth);
                return Ok(None);
            }
            Op::BreakIf(_) => {
                // Pop the condition; the break edge and the fall-through
                // both continue at the post-pop depth.
                let (req, net) = op_depth_effect(op, chunk);
                if depth < req {
                    return Err(depth_underflow(chunk, ip, op, req, depth));
                }
                depth += net;
                breaks.push(depth);
                ip += 1;
            }
            Op::If(target) => {
                let (req, net) = op_depth_effect(op, chunk);
                if depth < req {
                    return Err(depth_underflow(chunk, ip, op, req, depth));
                }
                depth += net;
                let target = *target as usize;
                if target > 0 && matches!(&ops[target - 1], Op::Else(_)) {
                    let endif = match &ops[target - 1] {
                        Op::Else(e) => *e as usize,
                        _ => unreachable!(),
                    };
                    let then_end = verify_depth_region(chunk, ip + 1, target - 1, depth, breaks)?;
                    let else_end = verify_depth_region(chunk, target, endif, depth, breaks)?;
                    depth = match (then_end, else_end) {
                        (Some(a), Some(b)) => a.max(b),
                        (Some(a), None) => a,
                        (None, Some(b)) => b,
                        (None, None) => return Ok(None),
                    };
                    ip = endif + 1;
                } else {
                    let then_end = verify_depth_region(chunk, ip + 1, target, depth, breaks)?;
                    if let Some(a) = then_end {
                        depth = depth.max(a);
                    }
                    ip = target + 1;
                }
            }
            Op::Loop(target) => {
                let exit = *target as usize;
                // The body's own break edges determine the depth after the
                // loop. A loop exited only by falling through a neutral
                // body resumes at the loop-entry depth.
                let mut loop_breaks: Vec<i32> = Vec::new();
                let body_end =
                    verify_depth_region(chunk, ip + 1, exit - 1, depth, &mut loop_breaks)?;
                depth = loop_breaks
                    .iter()
                    .copied()
                    .max()
                    .or(body_end)
                    .unwrap_or(depth);
                ip = exit;
            }
            _ => {
                let (req, net) = op_depth_effect(op, chunk);
                if depth < req {
                    return Err(depth_underflow(chunk, ip, op, req, depth));
                }
                depth += net;
                ip += 1;
            }
        }
    }
    Ok(Some(depth))
}

/// Verify a single chunk, optionally recording one
/// [`VerificationObligation`] per check as it is discharged.
///
/// This is the single source of truth for structural verification:
/// [`verify`] calls it with `sink = None` for the verdict, and
/// [`chunk_verification_obligations`] calls it with a sink to collect
/// the trace. The verdict logic is identical in both modes; the only
/// difference is whether `record` is a no-op. Obligations are recorded
/// only on the path where a check *passes*, so an error return leaves a
/// trace truncated at the first failing check.
fn verify_chunk(
    chunk: &Chunk,
    module: &Module,
    mut sink: Option<&mut alloc::vec::Vec<VerificationObligation>>,
) -> Result<(), VerifyError> {
    const P1: &str = "block-nesting-and-offsets";
    const P2: &str = "block-type-constraints";
    const P3: &str = "productive-divergence";
    let mut record = |op_index: usize, pass: &'static str, property: &'static str| {
        if let Some(s) = sink.as_mut() {
            s.push(VerificationObligation {
                op_index: op_index as u32,
                pass,
                property,
            });
        }
    };

    {
        let name = &chunk.name;
        let ops = &chunk.ops;

        // -- Pass 1: Block nesting and offset validation --
        let mut block_stack: Vec<(BlockKind, usize)> = Vec::new();
        let mut loop_depth: usize = 0;

        for (ip, op) in ops.iter().enumerate() {
            match op {
                Op::If(target) => {
                    let t = *target as usize;
                    // Target must be within bounds. It may point to the
                    // else body start, EndIf, or any valid instruction
                    // depending on the compilation pattern.
                    if t > ops.len() {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "If at {} targets {} which is out of bounds (len={})",
                                ip,
                                t,
                                ops.len()
                            ),
                        });
                    }
                    block_stack.push((BlockKind::If, ip));
                    record(ip, P1, "if-branch-target-in-bounds");
                }
                Op::Else(target) => {
                    let t = *target as usize;
                    // Must be preceded by an If block on the stack.
                    match block_stack.last() {
                        Some((BlockKind::If, _)) => {}
                        _ => {
                            return Err(VerifyError {
                                chunk_name: name.clone(),
                                message: alloc::format!(
                                    "Else at {} without matching If on block stack",
                                    ip
                                ),
                            });
                        }
                    }
                    record(ip, P1, "else-preceded-by-matching-if");
                    // Target must point to EndIf within bounds.
                    if t >= ops.len() {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "Else at {} targets {} which is out of bounds (len={})",
                                ip,
                                t,
                                ops.len()
                            ),
                        });
                    }
                    record(ip, P1, "else-target-in-bounds");
                    if !matches!(&ops[t], Op::EndIf) {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "Else at {} targets {} which is {:?}, expected EndIf",
                                ip,
                                t,
                                &ops[t]
                            ),
                        });
                    }
                    record(ip, P1, "else-target-is-endif");
                }
                Op::EndIf => {
                    match block_stack.pop() {
                        Some((BlockKind::If, _)) => {}
                        Some((BlockKind::Loop, _)) => {
                            return Err(VerifyError {
                                chunk_name: name.clone(),
                                message: alloc::format!("EndIf at {} but expected EndLoop", ip),
                            });
                        }
                        None => {
                            return Err(VerifyError {
                                chunk_name: name.clone(),
                                message: alloc::format!("EndIf at {} with no matching If", ip),
                            });
                        }
                    }
                    record(ip, P1, "endif-closes-open-if");
                }
                Op::Loop(target) => {
                    let t = *target as usize;
                    // Target must be past the matching EndLoop.
                    // We allow target == ops.len() (points past end).
                    if t > ops.len() {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "Loop at {} targets {} which is out of bounds (len={})",
                                ip,
                                t,
                                ops.len()
                            ),
                        });
                    }
                    block_stack.push((BlockKind::Loop, ip));
                    loop_depth += 1;
                    record(ip, P1, "loop-exit-target-in-bounds");
                }
                Op::EndLoop(target) => {
                    let t = *target as usize;
                    match block_stack.pop() {
                        Some((BlockKind::Loop, loop_ip)) => {
                            record(ip, P1, "endloop-closes-open-loop");
                            // EndLoop back-edge must point to instruction after Loop.
                            if t != loop_ip + 1 {
                                return Err(VerifyError {
                                    chunk_name: name.clone(),
                                    message: alloc::format!(
                                        "EndLoop at {} back-edge targets {} but Loop is at {} (expected {})",
                                        ip,
                                        t,
                                        loop_ip,
                                        loop_ip + 1
                                    ),
                                });
                            }
                            record(ip, P1, "endloop-back-edge-targets-loop-entry");
                        }
                        Some((BlockKind::If, _)) => {
                            return Err(VerifyError {
                                chunk_name: name.clone(),
                                message: alloc::format!("EndLoop at {} but expected EndIf", ip),
                            });
                        }
                        None => {
                            return Err(VerifyError {
                                chunk_name: name.clone(),
                                message: alloc::format!("EndLoop at {} with no matching Loop", ip),
                            });
                        }
                    }
                    loop_depth -= 1;
                }
                Op::Break(target) => {
                    if loop_depth == 0 {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!("Break at {} outside any Loop block", ip),
                        });
                    }
                    record(ip, P1, "break-within-loop");
                    let t = *target as usize;
                    if t > ops.len() {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "Break at {} targets {} which is out of bounds (len={})",
                                ip,
                                t,
                                ops.len()
                            ),
                        });
                    }
                    record(ip, P1, "break-target-in-bounds");
                }
                Op::BreakIf(target) => {
                    if loop_depth == 0 {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!("BreakIf at {} outside any Loop block", ip),
                        });
                    }
                    record(ip, P1, "break-if-within-loop");
                    let t = *target as usize;
                    if t > ops.len() {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "BreakIf at {} targets {} which is out of bounds (len={})",
                                ip,
                                t,
                                ops.len()
                            ),
                        });
                    }
                    record(ip, P1, "break-if-target-in-bounds");
                }
                Op::GetData(slot) | Op::SetData(slot) => {
                    let idx = *slot as usize;
                    let data_len = module.data_layout.as_ref().map_or(0, |dl| dl.slots.len());
                    if data_len == 0 {
                        let op_name = if matches!(op, Op::GetData(_)) {
                            "GetData"
                        } else {
                            "SetData"
                        };
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{} at {} but module has no data layout declared",
                                op_name,
                                ip
                            ),
                        });
                    }
                    record(ip, P1, "data-slot-layout-declared");
                    if idx >= data_len {
                        let op_name = if matches!(op, Op::GetData(_)) {
                            "GetData"
                        } else {
                            "SetData"
                        };
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{} at {} references slot {} but data layout has {} slot(s)",
                                op_name,
                                ip,
                                idx,
                                data_len
                            ),
                        });
                    }
                    record(ip, P1, "data-slot-index-in-range");
                }
                Op::SetDataComposite(slot, _offset) => {
                    // Same slot-bounds obligation as `SetData`; the second
                    // immediate is a compiler-assigned persistent body offset,
                    // bounded by construction, and carries no extra obligation
                    // here (B28 P3 item 5, item 3a).
                    let idx = *slot as usize;
                    let data_len = module.data_layout.as_ref().map_or(0, |dl| dl.slots.len());
                    if data_len == 0 {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "SetDataComposite at {} but module has no data layout declared",
                                ip
                            ),
                        });
                    }
                    record(ip, P1, "data-slot-layout-declared");
                    if idx >= data_len {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "SetDataComposite at {} references slot {} but data layout has {} slot(s)",
                                ip,
                                idx,
                                data_len
                            ),
                        });
                    }
                    record(ip, P1, "data-slot-index-in-range");
                }
                Op::GetDataIndexed(base, len) | Op::SetDataIndexed(base, len) => {
                    let data_len = module.data_layout.as_ref().map_or(0, |dl| dl.slots.len());
                    let op_name = if matches!(op, Op::GetDataIndexed(_, _)) {
                        "GetDataIndexed"
                    } else {
                        "SetDataIndexed"
                    };
                    if data_len == 0 {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{} at {} but module has no data layout declared",
                                op_name,
                                ip
                            ),
                        });
                    }
                    record(ip, P1, "data-range-layout-declared");
                    let base_usize = *base as usize;
                    let len_usize = *len as usize;
                    let end = base_usize.saturating_add(len_usize);
                    if end > data_len {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{} at {} references slot range [{}, {}) but data layout has {} slot(s)",
                                op_name,
                                ip,
                                base_usize,
                                end,
                                data_len
                            ),
                        });
                    }
                    record(ip, P1, "data-range-in-bounds");
                }
                // Constant-pool index validation (audit finding 1,
                // poc_const_oob). The VM dereferences these operands
                // directly, so an out-of-range index must be rejected at
                // load. `Const` carries a value index; `IsStruct` a name
                // index; `IsEnum` and `NewEnum` an enum and a variant name
                // index. `GetField`'s boxed form carries a name index; its
                // flat form carries a byte offset (validated by the flat
                // body, like `GetTupleField`), so only the boxed form is
                // checked here.
                Op::Const(idx)
                | Op::IsStruct(idx)
                | Op::GetField(crate::bytecode::StructField::Boxed { name_const: idx }) => {
                    let len = chunk.constants.len();
                    if *idx as usize >= len {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{:?} at {} references constant {} but the pool has {} entr(ies)",
                                op,
                                ip,
                                idx,
                                len
                            ),
                        });
                    }
                    record(ip, P1, "constant-index-in-range");
                }
                Op::IsEnum(e, v, d) => {
                    // Enum-name, variant-name, and discriminant-value
                    // constant indices are all dereferenced by the VM.
                    let len = chunk.constants.len();
                    if *e as usize >= len || *v as usize >= len || *d as usize >= len {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{:?} at {} references constants ({}, {}, {}) but the pool has {} entr(ies)",
                                op,
                                ip,
                                e,
                                v,
                                d,
                                len
                            ),
                        });
                    }
                    record(ip, P1, "constant-index-in-range");
                }
                // Call target and argument-count validation (audit
                // finding 4, poc/zz_call). The callee index must name a
                // chunk, and the argument count must not exceed the
                // callee's local-slot count (parameters are a prefix of
                // locals), which would underflow the dispatch frame setup.
                Op::Call(callee, arg_count) => {
                    let nchunks = module.chunks.len();
                    if *callee as usize >= nchunks {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "Call at {} targets chunk {} but the module has {} chunk(s)",
                                ip,
                                callee,
                                nchunks
                            ),
                        });
                    }
                    let callee_locals = module.chunks[*callee as usize].local_count;
                    if *arg_count as u16 > callee_locals {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "Call at {} passes {} arguments but callee chunk {} declares only {} local slot(s)",
                                ip,
                                arg_count,
                                callee,
                                callee_locals
                            ),
                        });
                    }
                    record(ip, P1, "call-target-and-arity-in-range");
                }
                // A Q-format fraction-bit count must be less than the
                // declared word width: the fraction cannot meet or exceed
                // the word, and a count at or beyond the wide width would
                // overflow the shift in the VM. Rejecting it here keeps a
                // safe load from reaching a panicking shift (audit
                // poc_wordtofixed_overshift). The runtime additionally
                // saturates as defense in depth for new_unchecked loads.
                Op::WordToFixed(fb)
                | Op::FixedToWord(fb)
                | Op::FixedMul(fb)
                | Op::FixedDiv(fb)
                | Op::CheckedMul(fb)
                | Op::CheckedDiv(fb) => {
                    let word_bits = 1usize << module.word_bits_log2;
                    if (*fb as usize) >= word_bits {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{:?} at {} declares {} fraction bits but the word is {} bits; a Q-format fraction count must be less than the word width",
                                op,
                                ip,
                                fb,
                                word_bits
                            ),
                        });
                    }
                    record(ip, P1, "fixed-frac-bits-in-range");
                }
                // Local-slot indices are dereferenced directly by the VM
                // against the frame's local region. An out-of-range slot reads
                // or writes past the declared locals, and in the `SetLocal`
                // case can corrupt another call frame on the shared stack
                // without panicking (audit finding 2). The locals are the
                // first `local_count` slots of the frame, mirroring the
                // `GetData`/`SetData` bound already enforced above.
                Op::GetLocal(slot) | Op::SetLocal(slot) => {
                    let nlocals = chunk.local_count as usize;
                    if *slot as usize >= nlocals {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "{:?} at {} references local slot {} but the chunk declares {} local(s)",
                                op,
                                ip,
                                slot,
                                nlocals
                            ),
                        });
                    }
                    record(ip, P1, "local-slot-in-range");
                }
                // A boxed struct or enum construction reads the chunk's
                // struct-template table for its type and field or variant
                // names; an out-of-range `meta` index panics the VM (audit
                // finding 13). Boxed tuples and arrays carry no metadata, and a
                // flat operand is validated by its baked byte size.
                Op::NewComposite(crate::bytecode::NewCompositeOperand::Boxed {
                    kind:
                        crate::value_layout::CompositeKind::Struct
                        | crate::value_layout::CompositeKind::Enum,
                    meta,
                    ..
                }) => {
                    let len = chunk.struct_templates.len();
                    if *meta as usize >= len {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!(
                                "NewComposite at {} references struct/enum template {} but the chunk has {} template(s)",
                                ip,
                                meta,
                                len
                            ),
                        });
                    }
                    record(ip, P1, "struct-template-index-in-range");
                }
                _ => {}
            }
        }

        if !block_stack.is_empty() {
            let (kind, ip) = block_stack.last().unwrap();
            let kind_str = match kind {
                BlockKind::If => "If",
                BlockKind::Loop => "Loop",
            };
            return Err(VerifyError {
                chunk_name: name.clone(),
                message: alloc::format!("unclosed {} block opened at {}", kind_str, ip),
            });
        }
        record(0, P1, "all-blocks-closed");

        // -- Pass 2: Block type constraints --
        let mut yield_count = 0usize;
        let mut stream_count = 0usize;
        let mut reset_count = 0usize;

        for op in ops {
            match op {
                Op::Yield => yield_count += 1,
                Op::Stream => stream_count += 1,
                Op::Reset => reset_count += 1,
                _ => {}
            }
        }

        // Positions of the marker ops, for keying the obligations to
        // the construct each block-type constraint concerns.
        let first_yield = ops.iter().position(|op| matches!(op, Op::Yield));
        let first_stream = ops.iter().position(|op| matches!(op, Op::Stream));
        let first_reset = ops.iter().position(|op| matches!(op, Op::Reset));

        match chunk.block_type {
            BlockType::Func => {
                if yield_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Func block contains {} Yield instruction(s)",
                            yield_count
                        ),
                    });
                }
                record(0, P2, "func-has-no-yield");
                if stream_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Func block contains {} Stream instruction(s)",
                            stream_count
                        ),
                    });
                }
                record(0, P2, "func-has-no-stream");
                if reset_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Func block contains {} Reset instruction(s)",
                            reset_count
                        ),
                    });
                }
                record(0, P2, "func-has-no-reset");
            }
            BlockType::Reentrant => {
                if yield_count == 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: String::from("Reentrant block must contain at least one Yield"),
                    });
                }
                record(first_yield.unwrap_or(0), P2, "reentrant-has-yield");
                if stream_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Reentrant block contains {} Stream instruction(s)",
                            stream_count
                        ),
                    });
                }
                record(0, P2, "reentrant-has-no-stream");
                if reset_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Reentrant block contains {} Reset instruction(s)",
                            reset_count
                        ),
                    });
                }
                record(0, P2, "reentrant-has-no-reset");
            }
            BlockType::Stream => {
                if stream_count != 1 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Stream block must contain exactly one Stream, found {}",
                            stream_count
                        ),
                    });
                }
                record(
                    first_stream.unwrap_or(0),
                    P2,
                    "stream-has-exactly-one-stream",
                );
                if reset_count != 1 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Stream block must contain exactly one Reset, found {}",
                            reset_count
                        ),
                    });
                }
                record(first_reset.unwrap_or(0), P2, "stream-has-exactly-one-reset");
                if yield_count == 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: String::from("Stream block must contain at least one Yield"),
                    });
                }
                record(first_yield.unwrap_or(0), P2, "stream-has-yield");
            }
        }

        // -- Pass 3: Productivity verification (Stream chunks only) --
        if chunk.block_type == BlockType::Stream {
            let stream_pos = ops.iter().position(|op| matches!(op, Op::Stream));
            let reset_pos = ops.iter().position(|op| matches!(op, Op::Reset));
            if let (Some(s), Some(r)) = (stream_pos, reset_pos) {
                let mut break_states: Vec<bool> = Vec::new();
                let result = analyze_yield_coverage(ops, s + 1, r, false, &mut break_states);
                if let Some(false) = result {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: String::from(
                            "productivity violation: some path from Stream to Reset \
                             does not pass through any Yield",
                        ),
                    });
                }
                record(s, P3, "every-stream-to-reset-path-yields");
            }
        }
    }

    Ok(())
}

#[cfg(all(test, feature = "compile"))]
mod tests {
    use super::*;
    use crate::bytecode::{BlockType, Chunk, ConstValue, Module, Op};
    use alloc::vec;

    fn make_module(chunks: Vec<Chunk>) -> Module {
        Module {
            schema_hash: 0,
            enum_layouts: alloc::vec::Vec::new(),
            signatures: alloc::vec::Vec::new(),
            native_return_shapes: alloc::vec::Vec::new(),
            chunks,
            native_names: Vec::new(),
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            aux_arena_bytes: 0,
            persistent_composite_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
        }
    }

    fn make_chunk(name: &str, ops: Vec<Op>, block_type: BlockType) -> Chunk {
        // Derive `local_count` from the ops so the local-slot operand-index
        // check (audit finding 2) is satisfied: a fixture that reads or writes
        // local slot N declares at least N+1 locals, as a real compiled chunk
        // would (the entry parameter occupies slot 0). Fixtures with no local
        // ops keep `local_count` 0 and an unchanged WCMU bound.
        let local_count = ops
            .iter()
            .filter_map(|op| match op {
                Op::GetLocal(s) | Op::SetLocal(s) => Some(*s + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        Chunk {
            name: String::from(name),
            ops,
            // One backing constant so the `Const(0)` filler these
            // structural fixtures use satisfies the operand-index check
            // (audit finding 1). Constants do not affect the WCMU bound.
            constants: alloc::vec![crate::bytecode::ConstValue::Int(0)],
            struct_templates: Vec::new(),
            local_count,
            param_count: 0,
            block_type,
            param_types: Vec::new(),
            debug_pool: None,
        }
    }

    #[test]
    fn valid_func_chunk() {
        let chunk = make_chunk("main", vec![Op::Const(0), Op::Return], BlockType::Func);
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    // Audit remediation (SECURITY_AUDIT_V0_2_1, poc_const_oob). The
    // verifier now rejects an out-of-range constant-pool index (finding 1)
    // rather than admitting it for the VM to dereference and panic on.
    #[test]
    fn const_oob_index_rejected_by_verifier() {
        let mut chunk = make_chunk("main", vec![Op::Const(5), Op::Return], BlockType::Func);
        chunk.constants = vec![ConstValue::Int(0)]; // len 1, index 5 is OOB
        let module = make_module(vec![chunk]);
        assert!(
            verify(&module).is_err(),
            "expected the verifier to reject the out-of-range Const index"
        );
    }

    // Audit remediation (finding 2). The verifier rejects a local-slot index
    // beyond the chunk's declared `local_count`. A `SetLocal` past the locals
    // could otherwise corrupt another call frame on the shared stack without
    // panicking; this is silent intra-arena corruption from verified bytecode.
    #[test]
    fn local_slot_oob_index_rejected_by_verifier() {
        let mut chunk = make_chunk("main", vec![Op::GetLocal(5), Op::Return], BlockType::Func);
        chunk.local_count = 1; // slot 5 is out of range
        let module = make_module(vec![chunk]);
        assert!(
            verify(&module).is_err(),
            "expected the verifier to reject the out-of-range local slot"
        );
    }

    // Audit remediation (finding 13). The verifier rejects an out-of-range
    // struct/enum template index in a boxed `NewComposite`, which the VM would
    // otherwise dereference (`struct_templates[meta]`) and panic on.
    #[test]
    fn struct_template_oob_index_rejected_by_verifier() {
        use crate::bytecode::NewCompositeOperand;
        use crate::value_layout::CompositeKind;
        // `struct_templates` is empty (len 0), so meta 0 is out of range.
        let chunk = make_chunk(
            "main",
            vec![
                Op::NewComposite(NewCompositeOperand::Boxed {
                    kind: CompositeKind::Struct,
                    count: 0,
                    meta: 0,
                }),
                Op::Return,
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        assert!(
            verify(&module).is_err(),
            "expected the verifier to reject the out-of-range struct template index"
        );
    }

    #[test]
    fn valid_if_else() {
        // If targets the else body (instruction after Else), Else targets EndIf.
        // Both arms leave one value so the operand stack is balanced at the
        // merge (the typed pass's exact-height join requires it).
        let chunk = make_chunk(
            "main",
            vec![
                Op::PushImmediate(1), // 0
                Op::If(4),            // 1 -> else body at 4
                Op::Const(0),         // 2 (then body)
                Op::Else(5),          // 3 -> EndIf at 5
                Op::Const(0),         // 4 (else body)
                Op::EndIf,            // 5
                Op::Return,           // 6
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn obligations_key_if_else_to_their_positions() {
        // The same If/Else/EndIf chunk that verify accepts yields
        // pass-1 obligations keyed to the construct op positions.
        let chunk = make_chunk(
            "main",
            vec![
                Op::PushImmediate(1), // 0
                Op::If(4),            // 1
                Op::Const(0),         // 2
                Op::Else(5),          // 3
                Op::Const(0),         // 4
                Op::EndIf,            // 5
                Op::Return,           // 6
            ],
            BlockType::Func,
        );
        // Precondition for faithfulness: the chunk verifies.
        let module = make_module(vec![chunk.clone()]);
        assert!(verify(&module).is_ok());
        let obs = chunk_verification_obligations(&chunk, &module);
        let has = |op: u32, property: &str| {
            obs.iter()
                .any(|o| o.op_index == op && o.property == property)
        };
        assert!(has(1, "if-branch-target-in-bounds"));
        // The Else construct discharges three distinct checks, each its
        // own obligation, all keyed to the Else op.
        assert!(has(3, "else-preceded-by-matching-if"));
        assert!(has(3, "else-target-in-bounds"));
        assert!(has(3, "else-target-is-endif"));
        assert!(has(5, "endif-closes-open-if"));
        // Chunk-level pass-1 and pass-2 facts at op 0.
        assert!(has(0, "all-blocks-closed"));
        assert!(has(0, "func-has-no-yield"));
        assert!(has(0, "func-has-no-stream"));
        assert!(has(0, "func-has-no-reset"));
        // A Func chunk records no productive-divergence obligation.
        assert!(obs.iter().all(|o| o.pass != "productive-divergence"));
    }

    #[test]
    fn obligations_record_stream_productive_divergence() {
        // Stream(0) ... Yield ... Reset
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::PushImmediate(1), // 1
                Op::Yield,            // 2
                Op::Reset,            // 3
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk.clone()]);
        assert!(verify(&module).is_ok());
        let obs = chunk_verification_obligations(&chunk, &module);
        // Productive divergence keyed to the Stream op.
        assert!(obs.iter().any(|o| o.op_index == 0
            && o.pass == "productive-divergence"
            && o.property == "every-stream-to-reset-path-yields"));
        // Block-type obligations keyed to their marker ops.
        assert!(
            obs.iter()
                .any(|o| o.op_index == 0 && o.property == "stream-has-exactly-one-stream")
        );
        assert!(
            obs.iter()
                .any(|o| o.op_index == 3 && o.property == "stream-has-exactly-one-reset")
        );
        assert!(
            obs.iter()
                .any(|o| o.op_index == 2 && o.property == "stream-has-yield")
        );
    }

    #[test]
    fn obligations_truncate_at_first_failing_check() {
        // A chunk that fails verify yields a trace ending at the first
        // failing check: the obligations for the constructs admitted
        // before it are present, the failing construct's is absent, and
        // no later or chunk-level facts are fabricated. This is what
        // makes the trace a faithful record of `verify` rather than an
        // independent re-derivation.
        let chunk = make_chunk(
            "main",
            vec![
                Op::If(3),      // 0 valid: target 3 <= len 4
                Op::EndIf,      // 1 closes the If
                Op::Loop(99),   // 2 INVALID: target 99 > len 4
                Op::EndLoop(3), // 3
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk.clone()]);
        // The chunk does not verify.
        assert!(verify(&module).is_err());
        let obs = chunk_verification_obligations(&chunk, &module);
        let has = |property: &str| obs.iter().any(|o| o.property == property);
        // Admitted before the failure:
        assert!(has("if-branch-target-in-bounds"));
        assert!(has("endif-closes-open-if"));
        // The failing construct's obligation is absent:
        assert!(!has("loop-exit-target-in-bounds"));
        // No pass-1 completion or pass-2 facts are fabricated:
        assert!(!has("all-blocks-closed"));
        assert!(obs.iter().all(|o| o.pass != "block-type-constraints"));
    }

    #[test]
    fn whole_chunk_resource_bounds_accept_func_and_reentrant_reject_stream() {
        // A Func chunk yields finite whole-call WCET/WCMU bounds.
        let func = make_chunk(
            "main",
            vec![Op::PushImmediate(1), Op::Return],
            BlockType::Func,
        );
        assert!(wcet_whole_chunk(&func).is_ok());
        assert!(wcmu_whole_chunk(&func).is_ok());

        // A Reentrant chunk (a yield function) is accepted: its
        // whole-body WCET is the cumulative-across-resumptions bound and
        // its WCMU is the persistent peak.
        let reentrant = make_chunk(
            "gen",
            vec![Op::PushImmediate(1), Op::Yield, Op::Return],
            BlockType::Reentrant,
        );
        assert!(wcet_whole_chunk(&reentrant).is_ok());
        assert!(wcmu_whole_chunk(&reentrant).is_ok());

        // A Stream chunk is rejected; it has its own iteration entry
        // points.
        let stream = make_chunk(
            "tick",
            vec![Op::Stream, Op::PushImmediate(1), Op::Yield, Op::Reset],
            BlockType::Stream,
        );
        assert!(wcet_whole_chunk(&stream).is_err());
        assert!(wcmu_whole_chunk(&stream).is_err());
    }

    #[test]
    fn reentrant_wcet_is_max_segment_for_top_level_yields() {
        // A Reentrant chunk with two top-level yields and an uneven
        // second segment. The per-resume WCET is the maximum segment
        // cost, which is strictly less than the whole-body cumulative
        // cost (the sum of the segments).
        let cm = &crate::bytecode::NOMINAL_COST_MODEL;
        // Segments: [0,3) = {Push,Push,Yield}; [3,6) = {Push,Push,Yield};
        // [6,8) = {Push,Return}. The cheap final segment plus identical
        // first two means max == one full segment < cumulative.
        let reentrant = make_chunk(
            "gen",
            vec![
                Op::PushImmediate(1), // 0
                Op::PushImmediate(1), // 1
                Op::Yield,            // 2
                Op::PushImmediate(1), // 3
                Op::PushImmediate(1), // 4
                Op::Yield,            // 5
                Op::PushImmediate(1), // 6
                Op::Return,           // 7
            ],
            BlockType::Reentrant,
        );
        let segmented = reentrant_segmented_wcet(&reentrant, cm, &[])
            .expect("no loop bound error")
            .expect("top-level yields segment");
        // Whole-body cumulative cost for comparison.
        let mut breaks = Vec::new();
        let extra = crate::text_size::chunk_text_wcet_cycles(&reentrant, cm.text_byte_cycles);
        let cumulative = wcet_region(
            &reentrant,
            0,
            reentrant.ops.len(),
            &mut breaks,
            cm,
            false,
            &extra,
        )
        .unwrap()
        .unwrap_or(0);
        assert!(
            segmented < cumulative,
            "per-segment max {segmented} should be tighter than cumulative {cumulative}"
        );
        // wcet_whole_chunk returns the tighter per-segment value.
        assert_eq!(wcet_whole_chunk(&reentrant).unwrap(), segmented);
    }

    #[test]
    fn productive_loop_predicate_guards() {
        // Unconditional yield, no inner loop: provably productive.
        let body = [Op::PushImmediate(1), Op::Yield, Op::BreakIf(0)];
        assert!(loop_body_all_paths_yield_no_inner_loop(
            &body,
            0,
            body.len()
        ));
        // An inner loop is rejected (kept out of analyze_yield_coverage's
        // domain) regardless of the yield.
        let inner = [Op::Loop(2), Op::Yield, Op::EndLoop(1)];
        assert!(!loop_body_all_paths_yield_no_inner_loop(
            &inner,
            0,
            inner.len()
        ));
        // No yield at all: rejected.
        let none = [Op::PushImmediate(1)];
        assert!(!loop_body_all_paths_yield_no_inner_loop(
            &none,
            0,
            none.len()
        ));
    }

    #[test]
    fn reentrant_nested_productive_yield_loop_clamps_to_one_iteration() {
        // A yield nested in a loop whose every body path yields: the
        // per-resume WCET clamps the loop to one iteration. The clamp is
        // observable here because this loop has no for-range bound, so
        // the unclamped whole-body cost errors ("no extractable iteration
        // bound") while the clamped per-resume cost succeeds.
        let cm = &crate::bytecode::NOMINAL_COST_MODEL;
        let chunk = make_chunk(
            "gen",
            vec![
                Op::Loop(5),          // 0 -> exit target after EndLoop
                Op::PushImmediate(1), // 1 (body)
                Op::Yield,            // 2 (every path yields)
                Op::BreakIf(5),       // 3
                Op::EndLoop(1),       // 4 back-edge to 1
                Op::Return,           // 5
            ],
            BlockType::Reentrant,
        );
        // The yield is nested, so the top-level segment split declines.
        assert_eq!(reentrant_segmented_wcet(&chunk, cm, &[]).unwrap(), None);
        // The clamped per-resume cost succeeds (loop counted once).
        let clamped = wcet_whole_chunk_with_cost_model(&chunk, cm, &[])
            .expect("clamp succeeds for productive loop");
        assert!(clamped > 0);
        // Without the clamp, the unbounded loop has no extractable
        // iteration count, so the plain whole-body cost errors. This is
        // the unclamped path a Func chunk (or the old behaviour) would
        // take.
        let mut breaks = Vec::new();
        let extra = crate::text_size::chunk_text_wcet_cycles(&chunk, cm.text_byte_cycles);
        assert!(
            wcet_region(&chunk, 0, chunk.ops.len(), &mut breaks, cm, false, &extra).is_err(),
            "unclamped whole-body cost requires a for-range bound"
        );
    }

    #[test]
    fn reentrant_wcet_falls_back_to_cumulative_for_nested_yield() {
        // A yield nested inside a loop body cannot be segmented
        // structurally, so the per-segment analysis declines (None) and
        // wcet_whole_chunk falls back to the whole-body cumulative cost.
        let cm = &crate::bytecode::NOMINAL_COST_MODEL;
        // Loop(3) Yield EndLoop(1) Return — the canonical for-range
        // pattern is required by strict mode, so use a body that exits
        // via the loop's natural fall-through. A bare infinite loop has
        // no extractable bound; instead test the segmentation decline
        // directly with a nested yield and assert None.
        let nested = make_chunk(
            "gen",
            vec![
                Op::Loop(4),    // 0
                Op::Yield,      // 1 (depth 1: nested)
                Op::Break(4),   // 2
                Op::EndLoop(1), // 3
                Op::Return,     // 4
            ],
            BlockType::Reentrant,
        );
        assert_eq!(
            reentrant_segmented_wcet(&nested, cm, &[]).expect("no bound error"),
            None,
            "a nested yield declines per-segment analysis"
        );
    }

    #[test]
    fn external_native_wcet_dedups_and_multiplies_by_invocations() {
        // An external native's per-iteration WCET contribution is
        // `max_invocations * per_call_wcet`, counted once per chunk regardless
        // of the static call-site count (#50). A verified native (no
        // `max_invocations`) contributes nothing through this once-per-chunk
        // path; it is folded per call site by `chunk_wcet_extra` instead.
        let chunk = make_chunk(
            "main",
            alloc::vec![
                Op::CallExternalNative(0, 0),
                Op::PopN(1),
                Op::CallExternalNative(0, 0),
                Op::PopN(1),
                Op::CallVerifiedNative(1, 0),
                Op::PopN(1),
                Op::Return,
            ],
            BlockType::Func,
        );
        let bounds = alloc::vec![
            NativeIterationBound {
                per_call_wcmu_bytes: 0,
                per_call_wcet_cycles: 50,
                max_invocations: Some(4),
            },
            NativeIterationBound {
                per_call_wcmu_bytes: 0,
                per_call_wcet_cycles: 100,
                max_invocations: None,
            },
        ];
        // 50 * 4 = 200, counted once despite two call sites; the verified
        // native contributes 0 here.
        assert_eq!(external_native_wcet(&chunk, &bounds), 200);
    }

    #[test]
    fn chunk_wcet_extra_folds_verified_native_per_call_at_each_site() {
        // A verified native's per-call WCET is added to the per-op extra table
        // at each of its call sites, so `wcet_region` scales it by loop
        // multiplicity (#50).
        let chunk = make_chunk(
            "main",
            alloc::vec![
                Op::CallVerifiedNative(0, 0),
                Op::PopN(1),
                Op::CallVerifiedNative(0, 0),
                Op::PopN(1),
                Op::Return,
            ],
            BlockType::Func,
        );
        let bounds = alloc::vec![NativeIterationBound {
            per_call_wcmu_bytes: 0,
            per_call_wcet_cycles: 100,
            max_invocations: None,
        }];
        let extra = chunk_wcet_extra(&chunk, &crate::bytecode::NOMINAL_COST_MODEL, &bounds);
        assert_eq!(extra[0], 100, "first verified-native call site");
        assert_eq!(extra[2], 100, "second verified-native call site");
        assert_eq!(extra[1], 0, "non-call op contributes no native cost");
        // With no bounds, the verified-native cost is not folded (script-only).
        let none = chunk_wcet_extra(&chunk, &crate::bytecode::NOMINAL_COST_MODEL, &[]);
        assert_eq!(none[0], 0);
    }

    #[test]
    fn valid_loop() {
        // Loop(4) BreakIf(4) EndLoop(1) PushUnit
        let chunk = make_chunk(
            "main",
            vec![
                Op::Loop(4),          // 0 -> past EndLoop
                Op::PushImmediate(1), // 1
                Op::BreakIf(4),       // 2 -> past EndLoop
                Op::EndLoop(1),       // 3 -> after Loop (ip 1)
                Op::PushImmediate(0), // 4
                Op::Return,           // 5
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn valid_stream_chunk() {
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,      // 0
                Op::GetLocal(0), // 1
                Op::Yield,       // 2
                Op::PopN(1),     // 3
                Op::Reset,       // 4
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn valid_reentrant_chunk() {
        let chunk = make_chunk(
            "gen",
            vec![
                Op::GetLocal(0), // 0
                Op::Yield,       // 1
                Op::PopN(1),     // 2
                Op::Return,      // 3
            ],
            BlockType::Reentrant,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn func_with_yield_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::PushImmediate(0), Op::Yield, Op::Return],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Yield"));
    }

    #[test]
    fn func_with_stream_fails() {
        let chunk = make_chunk("bad", vec![Op::Stream, Op::Return], BlockType::Func);
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Stream"));
    }

    #[test]
    fn func_with_reset_fails() {
        let chunk = make_chunk("bad", vec![Op::Reset], BlockType::Func);
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Reset"));
    }

    #[test]
    fn reentrant_without_yield_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::PushImmediate(0), Op::Return],
            BlockType::Reentrant,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Yield"));
    }

    #[test]
    fn reentrant_with_stream_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::Stream, Op::PushImmediate(0), Op::Yield, Op::Return],
            BlockType::Reentrant,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Stream"));
    }

    #[test]
    fn stream_without_yield_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::Stream, Op::PushImmediate(0), Op::Reset],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Yield"));
    }

    #[test]
    fn stream_missing_reset_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::Stream, Op::PushImmediate(0), Op::Yield, Op::PopN(1)],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Reset"));
    }

    #[test]
    fn stream_missing_stream_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::PushImmediate(0), Op::Yield, Op::PopN(1), Op::Reset],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Stream"));
    }

    #[test]
    fn unclosed_if_fails() {
        let chunk = make_chunk(
            "bad",
            vec![
                Op::PushImmediate(1),
                Op::If(3), // targets EndIf-like position
                Op::PushImmediate(0),
                Op::Return, // but no EndIf
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("If") || err.message.contains("expected"));
    }

    #[test]
    fn break_outside_loop_fails() {
        let chunk = make_chunk("bad", vec![Op::Break(1), Op::Return], BlockType::Func);
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("outside"));
    }

    #[test]
    fn breakif_outside_loop_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::PushImmediate(1), Op::BreakIf(2), Op::Return],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("outside"));
    }

    #[test]
    fn endloop_bad_backedge_fails() {
        let chunk = make_chunk(
            "bad",
            vec![
                Op::Loop(4),          // 0
                Op::PushImmediate(1), // 1
                Op::BreakIf(4),       // 2
                Op::EndLoop(0),       // 3 -> should be 1, not 0
                Op::PushImmediate(0), // 4
                Op::Return,           // 5
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("back-edge"));
    }

    #[test]
    fn else_targets_wrong_op_fails() {
        let chunk = make_chunk(
            "bad",
            vec![
                Op::PushImmediate(1), // 0
                Op::If(3),            // 1 -> Else at 3
                Op::PushImmediate(0), // 2
                Op::Else(5),          // 3 -> targets PushUnit, not EndIf
                Op::PushImmediate(0), // 4
                Op::PushImmediate(0), // 5 (not EndIf)
                Op::Return,           // 6
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("expected EndIf"));
    }

    #[test]
    fn mismatched_if_endloop_fails() {
        let chunk = make_chunk(
            "bad",
            vec![
                Op::PushImmediate(1), // 0
                Op::If(3),            // 1 -> targets EndLoop
                Op::PushImmediate(0), // 2
                Op::EndLoop(0),       // 3 (EndLoop instead of EndIf)
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_err());
    }

    #[test]
    fn verify_compiled_programs() {
        // Integration test: compile real programs and verify them.
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let programs = [
            "fn main() -> Word { 42 }",
            "fn main() -> Word { if true { 1 } else { 2 } }",
            "fn main() -> Word { let sum = 0; for i in 0..5 { let x = sum + i; } sum }",
            "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(21) }",
            "fn main() -> Text { let x = 1; match x { 1 => \"one\", _ => \"other\" } }",
            "loop tick(x: Word) -> Word { let x = yield x * 2; x }",
        ];

        for src in &programs {
            let tokens = tokenize(src).expect("lex error");
            let program = parse(&tokens).expect("parse error");
            let module = compile(&program).expect("compile error");
            if let Err(e) = verify(&module) {
                panic!(
                    "verification failed for {:?}: {}: {}",
                    src, e.chunk_name, e.message
                );
            }
        }
    }

    // -- Productivity rule tests --

    #[test]
    fn productivity_linear_yield() {
        // Stream -> Yield -> Reset: all paths yield. Should pass.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,      // 0
                Op::GetLocal(0), // 1
                Op::Yield,       // 2
                Op::PopN(1),     // 3
                Op::Reset,       // 4
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn productivity_yield_both_branches() {
        // Stream -> If { Yield } Else { Yield } -> Reset: both branches yield.
        // Each arm discards its resume value so both leave the stack balanced.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::PushImmediate(1), // 1
                Op::If(7),            // 2 -> else body at 7
                Op::GetLocal(0),      // 3 (then)
                Op::Yield,            // 4 (then)
                Op::PopN(1),          // 5 (then)
                Op::Else(10),         // 6 -> EndIf at 10
                Op::GetLocal(0),      // 7 (else)
                Op::Yield,            // 8 (else)
                Op::PopN(1),          // 9 (else)
                Op::EndIf,            // 10
                Op::Reset,            // 11
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn productivity_yield_before_if() {
        // Stream -> Yield -> If/Else -> Reset: yield dominates both branches.
        // Both arms push then pop, leaving the stack balanced at the merge.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::GetLocal(0),      // 1
                Op::Yield,            // 2
                Op::PopN(1),          // 3
                Op::PushImmediate(1), // 4
                Op::If(9),            // 5 -> else body at 9
                Op::PushImmediate(0), // 6 (then)
                Op::PopN(1),          // 7 (then)
                Op::Else(11),         // 8 -> EndIf at 11
                Op::PushImmediate(0), // 9 (else)
                Op::PopN(1),          // 10 (else)
                Op::EndIf,            // 11
                Op::Reset,            // 12
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn productivity_yield_only_in_then_fails() {
        // Stream -> If { Yield } Else { no yield } -> Reset: else branch missing yield.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::PushImmediate(1), // 1
                Op::If(6),            // 2 -> else body at 6
                Op::GetLocal(0),      // 3 (then)
                Op::Yield,            // 4 (then)
                Op::Else(9),          // 5 -> EndIf at 9
                Op::PushImmediate(0), // 6 (else, no yield)
                Op::PopN(1),          // 7 (else)
                Op::PushImmediate(0), // 8 (else)
                Op::EndIf,            // 9
                Op::PopN(1),          // 10
                Op::Reset,            // 11
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("productivity violation"));
    }

    #[test]
    fn productivity_no_yield_path_fails() {
        // Stream -> If(no-else) { Yield } -> Reset: false path has no yield.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::PushImmediate(1), // 1
                Op::If(6),            // 2 -> EndIf at 6 (no Else)
                Op::GetLocal(0),      // 3 (then)
                Op::Yield,            // 4 (then)
                Op::PopN(1),          // 5 (then)
                Op::EndIf,            // 6
                Op::Reset,            // 7
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("productivity violation"));
    }

    #[test]
    fn productivity_yield_in_loop_fails() {
        // Stream -> Loop { BreakIf; Yield } -> Reset.
        // The BreakIf can exit before the Yield, so some path has no yield.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::Loop(8),          // 1 -> past EndLoop
                Op::PushImmediate(1), // 2
                Op::BreakIf(8),       // 3 -> past EndLoop
                Op::GetLocal(0),      // 4
                Op::Yield,            // 5
                Op::PopN(1),          // 6
                Op::EndLoop(2),       // 7 -> back to 2
                Op::Reset,            // 8
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("productivity violation"));
    }

    #[test]
    fn productivity_yield_before_loop() {
        // Stream -> Yield -> Loop { BreakIf } -> Reset.
        // Yield dominates the loop, so all paths have yielded. The loop body is
        // stack-neutral (it pushes and pops the condition only), as the typed
        // pass's back-edge neutrality requires.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::GetLocal(0),      // 1
                Op::Yield,            // 2
                Op::PopN(1),          // 3
                Op::Loop(8),          // 4 -> past EndLoop
                Op::PushImmediate(1), // 5
                Op::BreakIf(8),       // 6 -> past EndLoop
                Op::EndLoop(5),       // 7 -> back to 5
                Op::Reset,            // 8
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn productivity_compiled_stream() {
        // Integration test: compile a real loop function and verify productivity.
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "loop tick(x: Word) -> Word { let x = yield x * 2; x }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        assert!(verify(&module).is_ok());
    }

    // -- WCET cost table tests --

    #[test]
    fn cost_basic_ops() {
        // Verify representative Op::cost() values.
        assert_eq!(Op::Const(0).cost(), 1);
        assert_eq!(Op::PushImmediate(0).cost(), 1);
        assert_eq!(Op::GetLocal(0).cost(), 1);
        assert_eq!(Op::SetLocal(0).cost(), 1);
        assert_eq!(Op::PopN(1).cost(), 1);
        assert_eq!(Op::Not.cost(), 1);

        assert_eq!(Op::Add.cost(), 2);
        assert_eq!(Op::Sub.cost(), 2);
        assert_eq!(Op::Mul.cost(), 2);
        assert_eq!(Op::CmpEq.cost(), 2);
        assert_eq!(Op::Return.cost(), 2);

        assert_eq!(Op::Div.cost(), 3);
        assert_eq!(Op::Mod.cost(), 3);
        assert_eq!(
            Op::GetField(crate::bytecode::StructField::Boxed { name_const: 0 }).cost(),
            3
        );

        assert_eq!(
            Op::NewComposite(crate::bytecode::NewCompositeOperand::Flat {
                kind: crate::value_layout::CompositeKind::Array,
                count: 0,
                byte_size: 0,
            })
            .cost(),
            5
        );

        assert_eq!(Op::Call(0, 0).cost(), 10);
        assert_eq!(Op::CallVerifiedNative(0, 0).cost(), 10);
        assert_eq!(Op::CallExternalNative(0, 0).cost(), 10);
    }

    #[test]
    fn wcet_linear_stream() {
        // Stream -> GetLocal -> Add -> Yield -> Pop -> Reset.
        // Body cost: 1 + 2 + 1 + 1 = 5, overhead: 1 + 1 = 2, total = 7.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,      // 0: cost 1 (overhead)
                Op::GetLocal(0), // 1: cost 1
                Op::Add,         // 2: cost 2
                Op::Yield,       // 3: cost 1
                Op::PopN(1),     // 4: cost 1
                Op::Reset,       // 5: cost 1 (overhead)
            ],
            BlockType::Stream,
        );
        let cost = wcet_stream_iteration(&chunk).unwrap();
        assert_eq!(cost, 7);
    }

    #[test]
    fn wcet_branching_takes_max() {
        // Stream -> PushTrue -> If { Add(2) } Else { Div(3) + Mul(2) } ->
        //   Yield -> Pop -> Reset.
        // Then body [3,4): Add = 2. Else body [5,7): Div(3) + Mul(2) = 5.
        // Max branch = 5.
        // Body: PushTrue(1) + If(1) + 5 + Yield(1) + Pop(1) = 9.
        // Overhead: Stream(1) + Reset(1) = 2. Total = 11.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::PushImmediate(1), // 1
                Op::If(5),            // 2 -> else body at 5
                Op::Add,              // 3 (then body)
                Op::Else(7),          // 4 -> EndIf at 7
                Op::Div,              // 5 (else body)
                Op::Mul,              // 6 (else body)
                Op::EndIf,            // 7
                Op::Yield,            // 8
                Op::PopN(1),          // 9
                Op::Reset,            // 10
            ],
            BlockType::Stream,
        );
        let cost = wcet_stream_iteration(&chunk).unwrap();
        assert_eq!(cost, 11);
    }

    #[test]
    fn wcet_non_stream_errors() {
        let chunk = make_chunk(
            "main",
            vec![Op::PushImmediate(0), Op::Return],
            BlockType::Func,
        );
        let err = wcet_stream_iteration(&chunk).unwrap_err();
        assert!(err.message.contains("Stream"));
    }

    #[test]
    fn wcet_compiled_stream() {
        // Integration test: compile a real loop function and compute WCET.
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "loop tick(x: Word) -> Word { let x = yield x * 2; x }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");

        // Find the stream chunk.
        let stream_chunk = module
            .chunks
            .iter()
            .find(|c| c.block_type == BlockType::Stream)
            .expect("no stream chunk found");

        let cost = wcet_stream_iteration(stream_chunk).unwrap();
        // Cost must be positive and finite.
        assert!(cost > 0, "WCET should be positive, got {}", cost);
    }

    // -- Data segment verification --

    #[test]
    fn data_slot_out_of_bounds_fails() {
        // GetData with index beyond data layout should fail verification.
        use crate::bytecode::{DataLayout, DataSlot};
        let chunk = make_chunk("main", vec![Op::GetData(5), Op::Return], BlockType::Func);
        let module = Module {
            schema_hash: 0,
            enum_layouts: alloc::vec::Vec::new(),
            signatures: alloc::vec::Vec::new(),
            native_return_shapes: alloc::vec::Vec::new(),
            chunks: vec![chunk],
            native_names: Vec::new(),
            entry_point: Some(0),
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            aux_arena_bytes: 0,
            persistent_composite_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            data_layout: Some(DataLayout {
                slots: vec![DataSlot {
                    name: String::from("ctx.x"),
                    visibility: crate::bytecode::SlotVisibility::Shared,
                }],
                shared_layout: Vec::new(),
                private_composite_layout: Vec::new(),
            }),
        };
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("slot"));
    }

    #[test]
    fn data_no_layout_fails() {
        // GetData without any data layout should fail verification.
        let chunk = make_chunk("main", vec![Op::GetData(0), Op::Return], BlockType::Func);
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("no data layout"));
    }

    #[test]
    fn data_valid_slot_passes() {
        // GetData/SetData with valid indices should pass.
        use crate::bytecode::{DataLayout, DataSlot};
        let chunk = make_chunk(
            "main",
            vec![
                Op::GetData(0),
                Op::SetData(1),
                Op::PushImmediate(0),
                Op::Return,
            ],
            BlockType::Func,
        );
        let module = Module {
            schema_hash: 0,
            enum_layouts: alloc::vec::Vec::new(),
            signatures: alloc::vec::Vec::new(),
            native_return_shapes: alloc::vec::Vec::new(),
            chunks: vec![chunk],
            native_names: Vec::new(),
            entry_point: Some(0),
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            aux_arena_bytes: 0,
            persistent_composite_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            data_layout: Some(DataLayout {
                slots: vec![
                    DataSlot {
                        name: String::from("ctx.a"),
                        visibility: crate::bytecode::SlotVisibility::Shared,
                    },
                    DataSlot {
                        name: String::from("ctx.b"),
                        visibility: crate::bytecode::SlotVisibility::Shared,
                    },
                ],
                shared_layout: Vec::new(),
                private_composite_layout: Vec::new(),
            }),
        };
        assert!(verify(&module).is_ok());
    }

    // -- WCMU analysis tests --

    #[test]
    fn wcmu_stream_simple() {
        // Stream Yield Reset. The body is just one Yield, which pops the
        // yielded value. Stack peak is 1 slot for the value plus
        // local_count. Heap is zero.
        use crate::bytecode::VALUE_SLOT_SIZE_BYTES;
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,      // 0
                Op::GetLocal(0), // 1
                Op::Yield,       // 2
                Op::PopN(1),     // 3 — never reached after yield
                Op::Reset,       // 4
            ],
            BlockType::Stream,
        );
        let mut chunk = chunk;
        chunk.local_count = 1;
        let (stack, heap) = wcmu_stream_iteration(&chunk).unwrap();
        // local_count=1 + peak above local=1 = 2 slots.
        assert_eq!(stack, 2 * VALUE_SLOT_SIZE_BYTES);
        assert_eq!(heap, 0);
    }

    #[test]
    fn wcmu_branching_takes_max() {
        // If/Else where one branch pushes more than the other.
        use crate::bytecode::VALUE_SLOT_SIZE_BYTES;
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::PushImmediate(1), // 1
                Op::If(7),            // 2 -> else body at 7
                Op::Const(0),         // 3 (then push)
                Op::Const(0),         // 4 (then push)
                Op::Const(0),         // 5 (then push, total 3 deep)
                Op::Else(9),          // 6 -> EndIf at 9
                Op::Const(0),         // 7 (else, push 1)
                Op::PopN(1),          // 8 (else, pop)
                Op::EndIf,            // 9
                Op::PopN(1),          // 10 (consume one if any)
                Op::PopN(1),          // 11
                Op::PopN(1),          // 12
                Op::GetLocal(0),      // 13
                Op::Yield,            // 14
                Op::PopN(1),          // 15
                Op::Reset,            // 16
            ],
            BlockType::Stream,
        );
        chunk.local_count = 1;
        chunk.constants = vec![ConstValue::Int(0)];
        let (stack, _heap) = wcmu_stream_iteration(&chunk).unwrap();
        // Then branch peaks at 3 above the IfBoolPop. Plus local frame.
        // The actual peak should be at least 3 slots above the local frame.
        assert!(stack >= 3 * VALUE_SLOT_SIZE_BYTES);
    }

    #[test]
    fn wcmu_new_struct_heap() {
        // B28 P4: NewComposite carries its exact flat byte size in the
        // operand. A two-word struct allocates 16 bytes precisely.
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,   // 0
                Op::Const(0), // 1
                Op::Const(0), // 2
                Op::NewComposite(crate::bytecode::NewCompositeOperand::Flat {
                    kind: crate::value_layout::CompositeKind::Struct,
                    count: 2,
                    byte_size: 16,
                }), // 3
                Op::Yield,    // 4
                Op::Reset,    // 5
            ],
            BlockType::Stream,
        );
        chunk.local_count = 0;
        chunk.constants = vec![ConstValue::Int(0)];
        let (_stack, heap) = wcmu_stream_iteration(&chunk).unwrap();
        assert_eq!(heap, 16);
    }

    #[test]
    fn wcmu_new_array_heap() {
        // B28 P4: a three-word flat array allocates 24 bytes precisely,
        // read verbatim from the NewComposite operand.
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::Const(0),
                Op::Const(0),
                Op::Const(0),
                Op::NewComposite(crate::bytecode::NewCompositeOperand::Flat {
                    kind: crate::value_layout::CompositeKind::Array,
                    count: 3,
                    byte_size: 24,
                }),
                Op::Yield,
                Op::Reset,
            ],
            BlockType::Stream,
        );
        chunk.constants = vec![ConstValue::Int(0)];
        let (_stack, heap) = wcmu_stream_iteration(&chunk).unwrap();
        assert_eq!(heap, 24);
    }

    #[test]
    fn wcmu_non_stream_errors() {
        let chunk = make_chunk(
            "main",
            vec![Op::PushImmediate(0), Op::Return],
            BlockType::Func,
        );
        let err = wcmu_stream_iteration(&chunk).unwrap_err();
        assert!(err.message.contains("Stream"));
    }

    #[test]
    fn verify_resource_bounds_passes() {
        // Small program fits in default arena.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let result = verify_resource_bounds(&module, 1024 * 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_resource_bounds_rejects_oversized() {
        // Tiny arena rejects any nontrivial stream.
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::Const(0),
                Op::Const(0),
                Op::NewComposite(crate::bytecode::NewCompositeOperand::Flat {
                    kind: crate::value_layout::CompositeKind::Array,
                    count: 2,
                    byte_size: 64,
                }),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        chunk.local_count = 4;
        chunk.constants = vec![ConstValue::Int(0)];
        let module = make_module(vec![chunk]);
        // Arena of 16 bytes is much smaller than the stream's WCMU.
        let err = verify_resource_bounds(&module, 16).unwrap_err();
        assert!(err.message.contains("WCMU"));
        assert!(err.message.contains("exceeds arena capacity"));
    }

    // V0.2.0 Phase 4 retired the closure family
    // (`Op::PushFunc`, `Op::MakeClosure`, `Op::MakeRecursiveClosure`,
    // `Op::CallIndirect`). The previous verifier tests that
    // exercised the pre-emptive rejection path are no longer
    // applicable; the opcodes do not exist and closure-shaped
    // source expressions are rejected at the type checker. The
    // type-checker-stage rejection is covered by
    // `closures_rejected_at_typecheck` and
    // `first_class_function_rejected_at_compile` in `typecheck.rs`.

    #[test]
    fn verify_resource_bounds_skips_non_stream() {
        // A module with only Func chunks has no WCMU bound to verify.
        let chunk = make_chunk(
            "util",
            vec![Op::PushImmediate(0), Op::Return],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        let result = verify_resource_bounds(&module, 16);
        assert!(result.is_ok());
    }

    // -- Module-level WCMU and call-graph integration --

    #[test]
    fn module_wcmu_returns_per_chunk_results() {
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        let results = module_wcmu(&module, &[]).unwrap();
        assert_eq!(results.len(), 1);
        let (stack_bytes, heap_bytes) = results[0];
        assert!(stack_bytes > 0);
        assert_eq!(heap_bytes, 0);
    }

    #[test]
    fn module_wcmu_includes_transitive_call_heap() {
        // chunk 0: callee that allocates an array.
        // chunk 1: stream that calls chunk 0.
        let mut callee = make_chunk(
            "alloc_array",
            vec![
                Op::Const(0),
                Op::Const(0),
                Op::Const(0),
                Op::NewComposite(crate::bytecode::NewCompositeOperand::Flat {
                    kind: crate::value_layout::CompositeKind::Array,
                    count: 3,
                    byte_size: 24,
                }),
                Op::PopN(1),
                Op::PushImmediate(0),
                Op::Return,
            ],
            BlockType::Func,
        );
        callee.constants = vec![ConstValue::Int(0)];

        let stream_chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::Call(0, 0),       // 1 — calls alloc_array
                Op::PopN(1),          // 2
                Op::PushImmediate(0), // 3
                Op::Yield,            // 4
                Op::PopN(1),          // 5
                Op::Reset,            // 6
            ],
            BlockType::Stream,
        );

        let module = make_module(vec![callee, stream_chunk]);
        let results = module_wcmu(&module, &[]).unwrap();
        // Stream chunk's heap should include callee's array allocation.
        let (_stream_stack, stream_heap) = results[1];
        let (_callee_stack, callee_heap) = results[0];
        assert!(callee_heap > 0, "callee heap should be > 0");
        assert!(
            stream_heap >= callee_heap,
            "stream heap should include callee heap"
        );
    }

    #[test]
    fn module_wcmu_uses_native_attestation() {
        let stream_chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,                   // 0
                Op::CallVerifiedNative(0, 0), // 1 — calls native 0
                Op::PopN(1),                  // 2
                Op::PushImmediate(0),         // 3
                Op::Yield,                    // 4
                Op::PopN(1),                  // 5
                Op::Reset,                    // 6
            ],
            BlockType::Stream,
        );

        let mut module = make_module(vec![stream_chunk]);
        module.native_names = vec![String::from("host::alloc")];

        // No attestation: heap should be zero.
        let results = module_wcmu(&module, &[]).unwrap();
        let (_, heap_no_attest) = results[0];
        assert_eq!(heap_no_attest, 0);

        // With attestation of 256 bytes: heap should reflect.
        let results = module_wcmu(&module, &[256]).unwrap();
        let (_, heap_with_attest) = results[0];
        assert_eq!(heap_with_attest, 256);
    }

    #[test]
    fn verify_resource_bounds_with_natives_rejects_attested_overflow() {
        let stream_chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::CallVerifiedNative(0, 0),
                Op::PopN(1),
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        let mut module = make_module(vec![stream_chunk]);
        module.native_names = vec![String::from("host::alloc")];

        // Attestation of 1024 bytes; arena of 16 bytes is too small.
        let err = verify_resource_bounds_with_natives(&module, 16, &[1024]).unwrap_err();
        assert!(err.message.contains("exceeds arena capacity"));
    }

    #[test]
    fn module_wcmu_with_bounds_verified_matches_per_site_sum() {
        // Verified natives accumulate per static call site. Two
        // call sites in one chunk yield twice the per-call WCMU.
        let stream_chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::CallVerifiedNative(0, 0),
                Op::PopN(1),
                Op::CallVerifiedNative(0, 0),
                Op::PopN(1),
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        let mut module = make_module(vec![stream_chunk]);
        module.native_names = vec![String::from("host::alloc")];

        let bounds = alloc::vec![NativeIterationBound {
            per_call_wcmu_bytes: 100,
            per_call_wcet_cycles: 0,
            max_invocations: None,
        }];
        let results =
            module_wcmu_with_bounds(&module, &bounds, crate::bytecode::VALUE_SLOT_SIZE_BYTES)
                .unwrap();
        let (_, heap) = results[0];
        assert_eq!(heap, 200, "two verified call sites at 100 each");
    }

    #[test]
    fn module_wcmu_with_bounds_external_uses_max_invocations() {
        // External natives apply max_invocations * per_call_wcmu
        // once per chunk regardless of the static call-site
        // count. Two call sites still yield a single
        // chunk-level contribution.
        let stream_chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::CallExternalNative(0, 0),
                Op::PopN(1),
                Op::CallExternalNative(0, 0),
                Op::PopN(1),
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        let mut module = make_module(vec![stream_chunk]);
        module.native_names = vec![String::from("host::log_event")];

        let bounds = alloc::vec![NativeIterationBound {
            per_call_wcmu_bytes: 100,
            per_call_wcet_cycles: 0,
            max_invocations: Some(50),
        }];
        let results =
            module_wcmu_with_bounds(&module, &bounds, crate::bytecode::VALUE_SLOT_SIZE_BYTES)
                .unwrap();
        let (_, heap) = results[0];
        // External contribution: 100 * 50 = 5000. Independent of
        // the two static call sites.
        assert_eq!(heap, 5000);
    }

    #[test]
    fn module_wcmu_with_bounds_mixed_classifications() {
        // A chunk that calls both a verified and an external
        // native sums the verified per-site contribution and the
        // external chunk-level contribution.
        let stream_chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::CallVerifiedNative(0, 0),
                Op::PopN(1),
                Op::CallExternalNative(1, 0),
                Op::PopN(1),
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        let mut module = make_module(vec![stream_chunk]);
        module.native_names = vec![String::from("host::alloc"), String::from("host::log_event")];

        let bounds = alloc::vec![
            NativeIterationBound {
                per_call_wcmu_bytes: 256,
                per_call_wcet_cycles: 0,
                max_invocations: None,
            },
            NativeIterationBound {
                per_call_wcmu_bytes: 64,
                per_call_wcet_cycles: 0,
                max_invocations: Some(10),
            },
        ];
        let results =
            module_wcmu_with_bounds(&module, &bounds, crate::bytecode::VALUE_SLOT_SIZE_BYTES)
                .unwrap();
        let (_, heap) = results[0];
        // Verified: 256 (one call site). External: 64 * 10 = 640.
        // Total: 896.
        assert_eq!(heap, 896);
    }

    #[test]
    fn module_wcmu_topological_handles_chain() {
        // Three-chunk chain: stream calls helper, helper calls leaf.
        let leaf = make_chunk(
            "leaf",
            vec![Op::PushImmediate(0), Op::Return],
            BlockType::Func,
        );
        let helper = make_chunk("helper", vec![Op::Call(0, 0), Op::Return], BlockType::Func);
        let stream = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::Call(1, 0),
                Op::PopN(1),
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![leaf, helper, stream]);
        let results = module_wcmu(&module, &[]).unwrap();
        assert_eq!(results.len(), 3);
        // All chunks should have a non-zero stack bound (their local frame
        // contributes at least one slot for the chunk).
    }

    // -- Bounded-iteration loop analysis --

    #[test]
    fn for_range_loop_multiplies_heap() {
        // Compile a real for-range loop with array allocation in body.
        // Verify the heap WCMU reflects the iteration count.
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "loop main(input: Word) -> Word { \
            for i in 0..5 { \
                let _arr = [1, 2, 3, 4]; \
            } \
            let _ignored = yield input; \
            input \
        }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let stream_chunk = module
            .chunks
            .iter()
            .find(|c| c.block_type == BlockType::Stream)
            .expect("no stream chunk");
        let (_stack_bytes, heap_bytes) = wcmu_stream_iteration(stream_chunk).unwrap();
        // Each iteration allocates a four-Word array, now sized at its exact
        // flat layout (`4 * word_bytes` bytes) from the `NewComposite`
        // allocation operand rather than the legacy `4 * VALUE_SLOT_SIZE_BYTES`
        // estimate (B28 P4). With the bundled eight-byte word and five
        // iterations, heap = 5 * (4 * 8) = 160 bytes.
        let word_bytes = (1usize << module.word_bits_log2) / 8;
        let expected = 5 * 4 * word_bytes as u32;
        assert_eq!(heap_bytes, expected);
    }

    #[test]
    fn for_range_loop_multiplies_wcet() {
        // Compile a real for-range loop. Verify WCET reflects the
        // iteration count.
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "loop main(input: Word) -> Word { \
            for i in 0..3 { \
                let _x = i + 1; \
            } \
            let _ignored = yield input; \
            input \
        }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let stream_chunk = module
            .chunks
            .iter()
            .find(|c| c.block_type == BlockType::Stream)
            .expect("no stream chunk");
        let cost_with_loop = wcet_stream_iteration(stream_chunk).unwrap();

        // A simpler version without the loop should cost less.
        let src_no_loop = "loop main(input: Word) -> Word { \
            let _x = input + 1; \
            let _ignored = yield input; \
            input \
        }";
        let tokens = tokenize(src_no_loop).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module2 = compile(&program).expect("compile error");
        let stream_chunk2 = module2
            .chunks
            .iter()
            .find(|c| c.block_type == BlockType::Stream)
            .expect("no stream chunk");
        let cost_without_loop = wcet_stream_iteration(stream_chunk2).unwrap();

        // The loop version should cost more than the non-loop version,
        // and should reflect at least three iterations of the body cost.
        assert!(
            cost_with_loop > cost_without_loop,
            "loop cost {} should exceed non-loop cost {}",
            cost_with_loop,
            cost_without_loop
        );
    }

    #[test]
    fn extract_loop_iteration_bound_matches_canonical() {
        // Synthetic chunk in the canonical for-range shape.
        let mut chunk = make_chunk(
            "test",
            vec![
                Op::Const(0),    // 0: push start (0)
                Op::SetLocal(0), // 1: var = 0
                Op::Const(1),    // 2: push end (10)
                Op::SetLocal(1), // 3: end = 10
                Op::Loop(11),    // 4
                Op::GetLocal(0), // 5: get var
                Op::GetLocal(1), // 6: get end
                Op::CmpGe,       // 7
                Op::BreakIf(11), // 8
                Op::EndLoop(5),  // 9
                Op::Return,      // 10
            ],
            BlockType::Func,
        );
        chunk.constants = vec![ConstValue::Int(0), ConstValue::Int(10)];

        let count = extract_loop_iteration_bound(&chunk, 4);
        assert_eq!(count, Some(10));
    }

    #[test]
    fn extract_loop_iteration_bound_returns_none_for_non_canonical() {
        // A loop without the canonical pattern. Should return None.
        let chunk = make_chunk(
            "test",
            vec![
                Op::Loop(4),
                Op::PushImmediate(1),
                Op::BreakIf(4),
                Op::EndLoop(1),
            ],
            BlockType::Func,
        );
        let count = extract_loop_iteration_bound(&chunk, 0);
        assert_eq!(count, None);
    }

    #[test]
    fn strict_mode_rejects_non_extractable_loop() {
        // A Stream chunk with a Loop that has fall-through but no
        // canonical for-range pattern. Strict mode rejects.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::Loop(7),          // 1 — non-canonical: no GetLocal/Const/CmpGe/BreakIf pattern.
                Op::PushImmediate(1), // 2
                Op::BreakIf(7),       // 3
                Op::PushImmediate(0), // 4
                Op::PopN(1),          // 5
                Op::EndLoop(2),       // 6 — body falls through.
                Op::Yield,            // 7 — past loop.
                Op::PopN(1),          // 8
                Op::Reset,            // 9
            ],
            BlockType::Stream,
        );
        let err = wcmu_stream_iteration(&chunk).unwrap_err();
        assert!(
            err.message.contains("strict mode") || err.message.contains("iteration bound"),
            "expected strict mode error, got: {}",
            err.message
        );
    }

    #[test]
    fn strict_mode_accepts_match_via_trap_exit() {
        // A Loop whose body always exits via Trap (modeling the match
        // expression's no-arm-matched fallback). The body returns None,
        // so strict mode treats the loop as iterating at most once.
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,           // 0
                Op::Loop(5),          // 1
                Op::Trap(0),          // 2 — body exits via Trap.
                Op::EndLoop(2),       // 3 — unreachable but required.
                Op::PushImmediate(0), // 4
                Op::Yield,            // 5 — wait this index is 5, after EndLoop.
                Op::PopN(1),          // 6
                Op::Reset,            // 7
            ],
            BlockType::Stream,
        );
        chunk.constants = vec![ConstValue::StaticStr(String::from("trap"))];
        // Hmm the Loop target needs to point past EndLoop. Let me fix.
        // Loop(5) means jump to ip 5, which would be Yield. Plausible.
        // EndLoop(2) means back-edge to ip 2 (Trap). Plausible.
        // So body region [2, 3) is just Trap. That returns None.
        let result = wcmu_stream_iteration(&chunk);
        assert!(result.is_ok(), "expected acceptance, got: {:?}", result);
    }

    #[test]
    fn for_range_zero_iterations_yields_zero_heap() {
        // An empty range produces zero iterations.
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::Const(0),    // start = 5
                Op::SetLocal(0), // var = 5
                Op::Const(1),    // end = 5
                Op::SetLocal(1), // end_slot = 5
                Op::Loop(15),
                Op::GetLocal(0),
                Op::GetLocal(1),
                Op::CmpGe,
                Op::BreakIf(15),
                Op::Const(2),
                Op::Const(2),
                Op::NewComposite(crate::bytecode::NewCompositeOperand::Flat {
                    kind: crate::value_layout::CompositeKind::Array,
                    count: 2,
                    byte_size: 16,
                }), // body: allocate 2-element array
                Op::PopN(1),
                Op::EndLoop(6),
                Op::PushImmediate(0),
                Op::Yield,
                Op::PopN(1),
                Op::Reset,
            ],
            BlockType::Stream,
        );
        chunk.constants = vec![ConstValue::Int(5), ConstValue::Int(5), ConstValue::Int(0)];
        chunk.local_count = 2;
        let (_stack, heap) = wcmu_stream_iteration(&chunk).unwrap();
        // 0 iterations means the body's heap allocation does not count.
        assert_eq!(heap, 0);
    }
}
