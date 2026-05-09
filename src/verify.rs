extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bytecode::{BlockType, Chunk, Module, Op, Value};

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
fn wcet_region(chunk: &Chunk, start: usize, end: usize, break_costs: &mut Vec<u32>) -> Option<u32> {
    let ops = &chunk.ops;
    let mut cost: u32 = 0;
    let mut ip = start;

    while ip < end {
        match &ops[ip] {
            Op::Break(_) => {
                cost += ops[ip].cost();
                break_costs.push(cost);
                return None;
            }
            Op::BreakIf(_) => {
                cost += ops[ip].cost();
                break_costs.push(cost);
                ip += 1;
            }
            Op::If(target) => {
                cost += ops[ip].cost();
                let target = *target as usize;
                if target > 0 && matches!(&ops[target - 1], Op::Else(_)) {
                    let endif_pos = if let Op::Else(e) = &ops[target - 1] {
                        *e as usize
                    } else {
                        unreachable!()
                    };
                    let then_cost = wcet_region(chunk, ip + 1, target - 1, break_costs);
                    let else_cost = wcet_region(chunk, target, endif_pos, break_costs);
                    let branch_cost = match (then_cost, else_cost) {
                        (Some(a), Some(b)) => Some(if a > b { a } else { b }),
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        (None, None) => return None,
                    };
                    cost += branch_cost.unwrap_or(0);
                    ip = endif_pos + 1;
                } else {
                    let then_cost = wcet_region(chunk, ip + 1, target, break_costs);
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
                cost += ops[ip].cost();
                let loop_exit_target = *target as usize;
                let endloop_ip = loop_exit_target - 1;
                let mut loop_break_costs: Vec<u32> = Vec::new();
                let body_cost = wcet_region(chunk, ip + 1, endloop_ip, &mut loop_break_costs);
                if loop_break_costs.is_empty() && body_cost.is_none() {
                    return None;
                }
                // Multiply body cost by iteration count when the loop
                // matches the canonical for-range pattern. Otherwise use
                // the conservative one-iteration body cost.
                let iter_count = extract_loop_iteration_bound(chunk, ip).unwrap_or(1);
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
                cost += ops[ip].cost();
                ip += 1;
            }
        }
    }

    Some(cost)
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
/// the integer constant pushed immediately before it. Returns `None` if
/// the slot is not set by a literal constant.
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
            if let Op::Const(idx) = &ops[ip - 1]
                && let Some(Value::Int(n)) = chunk.constants.get(*idx as usize)
            {
                return Some(*n);
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
}

impl<'a> CallResolver<'a> {
    /// A resolver that returns zero for every lookup. Used by the
    /// local-only analysis path.
    fn empty() -> Self {
        Self {
            chunk_wcmu: &[],
            native_wcmu: &[],
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

/// Compute the worst-case memory usage over a region of instructions
/// `[start, end)`. The analysis tracks operand-stack depth in slots and
/// arena heap bytes.
///
/// At control flow joins, the peak stack and heap total are taken as the
/// maximum across branches. The stack delta is taken from the branch that
/// reaches `end`, with the convention that the surface compiler ensures
/// branches end at the same depth.
///
/// For loops, the body is treated as one iteration. This mirrors the
/// existing WCET limitation and is unsound for variable-iteration loops.
/// Programs that compile from bounded for-range loops produce sound
/// bounds at their static iteration count, but the analysis here
/// underestimates by the iteration factor. A future pass will lift this
/// limitation.
///
/// Returns `Some(McuResult)` for paths that fall through to `end`.
/// Returns `None` if all paths exit via Break.
fn wcmu_region(
    chunk: &Chunk,
    start: usize,
    end: usize,
    break_results: &mut Vec<McuResult>,
    resolver: &CallResolver,
) -> Option<McuResult> {
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
                heap += op.heap_alloc(chunk);
                current_offset += growth - shrink;
                break_results.push(McuResult {
                    peak_above_initial: peak,
                    delta: current_offset,
                    heap_total: heap,
                });
                return None;
            }
            Op::BreakIf(_) => {
                let shrink = op.stack_shrink() as i32;
                let growth = op.stack_growth() as i32;
                let during_peak = (current_offset + growth).max(0) as u32;
                peak = peak.max(during_peak);
                heap += op.heap_alloc(chunk);
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
                heap += op.heap_alloc(chunk);
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
                    );
                    let else_branch = wcmu_subregion(
                        chunk,
                        target,
                        endif_pos,
                        current_offset,
                        break_results,
                        resolver,
                    );
                    match (then_branch, else_branch) {
                        (Some(a), Some(b)) => {
                            peak = peak.max(a.peak_above_initial).max(b.peak_above_initial);
                            heap += a.heap_total.max(b.heap_total);
                            // Branches should end at the same offset, but if
                            // not, take the maximum to remain conservative.
                            current_offset = a.delta.max(b.delta);
                        }
                        (Some(a), None) => {
                            peak = peak.max(a.peak_above_initial);
                            heap += a.heap_total;
                            current_offset = a.delta;
                        }
                        (None, Some(b)) => {
                            peak = peak.max(b.peak_above_initial);
                            heap += b.heap_total;
                            current_offset = b.delta;
                        }
                        (None, None) => {
                            return None;
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
                    );
                    if let Some(a) = then_branch {
                        peak = peak.max(a.peak_above_initial);
                        heap += a.heap_total;
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
                heap += op.heap_alloc(chunk);
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
                );
                let body_peak = body.as_ref().map_or(0, |r| r.peak_above_initial);
                let body_heap_one = body.as_ref().map_or(0, |r| r.heap_total);
                // Multiply heap by iteration count if we can extract a
                // bound from the canonical for-range pattern. Stack peak
                // is a maximum, not a sum, so it does not multiply.
                let iter_count = extract_loop_iteration_bound(chunk, ip).unwrap_or(1);
                let body_heap = body_heap_one.saturating_mul(iter_count);
                let break_peak = loop_breaks
                    .iter()
                    .map(|r| r.peak_above_initial)
                    .max()
                    .unwrap_or(0);
                let break_heap = loop_breaks.iter().map(|r| r.heap_total).max().unwrap_or(0);
                peak = peak.max(body_peak).max(break_peak);
                heap += body_heap.max(break_heap);
                if loop_breaks.is_empty() && body.is_none() {
                    return None;
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
                let callee_stack_slots =
                    (callee_stack_bytes / crate::bytecode::VALUE_SLOT_SIZE_BYTES) as i32;
                let n = *n_args as i32;
                let during_peak = (current_offset + callee_stack_slots - n)
                    .max(current_offset + 1)
                    .max(0) as u32;
                peak = peak.max(during_peak);
                heap += callee_heap_bytes;
                // Net stack effect: pop n args, push 1 return value.
                current_offset += 1 - n;
                ip += 1;
            }
            Op::CallNative(native_idx, n_args) => {
                // Native function runs in host code. The operand-stack
                // effect is just the argument pop and return push. Heap
                // contribution comes from the host attestation.
                let native_heap = resolver.resolve_native(*native_idx);
                let n = *n_args as i32;
                let during_peak = (current_offset + 1).max(0) as u32;
                peak = peak.max(during_peak);
                heap += native_heap;
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
                heap += op.heap_alloc(chunk);
                current_offset += growth - shrink;
                ip += 1;
            }
        }
    }

    Some(McuResult {
        peak_above_initial: peak,
        delta: current_offset,
        heap_total: heap,
    })
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
) -> Option<McuResult> {
    let mut sub_breaks: Vec<McuResult> = Vec::new();
    let result = wcmu_region(chunk, start, end, &mut sub_breaks, resolver);
    // Lift breaks from the subregion into the caller's frame of reference.
    for b in sub_breaks {
        break_results.push(McuResult {
            peak_above_initial: (offset_at_start.max(0) as u32) + b.peak_above_initial,
            delta: offset_at_start + b.delta,
            heap_total: b.heap_total,
        });
    }
    result.map(|r| McuResult {
        peak_above_initial: (offset_at_start.max(0) as u32) + r.peak_above_initial,
        delta: offset_at_start + r.delta,
        heap_total: r.heap_total,
    })
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
    let body = wcmu_region(chunk, stream_pos + 1, reset_pos, &mut breaks, &resolver)
        .unwrap_or(McuResult::empty());

    let body_peak = body.peak_above_initial;
    let body_heap = body.heap_total;

    let stack_slots = chunk.local_count as u32 + body_peak;
    let stack_bytes = stack_slots * crate::bytecode::VALUE_SLOT_SIZE_BYTES;

    Ok((stack_bytes, body_heap))
}

/// Compute the worst-case execution cost of one full Stream iteration
/// (from Stream to Reset), taking the maximum cost branch at each
/// control flow join.
///
/// Returns the worst-case cost as a unitless integer. Returns an error
/// if the chunk is not a Stream block type or lacks Stream/Reset.
pub fn wcet_stream_iteration(chunk: &Chunk) -> Result<u32, VerifyError> {
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
    let body_cost = wcet_region(chunk, stream_pos + 1, reset_pos, &mut break_costs);

    // Include Stream and Reset instruction costs.
    let overhead = ops[stream_pos].cost() + ops[reset_pos].cost();
    let region_cost = body_cost.unwrap_or(0);

    Ok(overhead + region_cost)
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
    let n = module.chunks.len();
    let mut chunk_wcmu: Vec<Option<(u32, u32)>> = alloc::vec![None; n];
    let order = topological_call_order(module)?;
    for chunk_idx in order {
        let chunk = &module.chunks[chunk_idx];
        let resolver = CallResolver {
            chunk_wcmu: &chunk_wcmu,
            native_wcmu,
        };
        let result = compute_chunk_wcmu(chunk, &resolver)?;
        chunk_wcmu[chunk_idx] = Some(result);
    }
    Ok(chunk_wcmu
        .into_iter()
        .map(|o| o.unwrap_or((0, 0)))
        .collect())
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
/// any chunks it calls.
fn compute_chunk_wcmu(chunk: &Chunk, resolver: &CallResolver) -> Result<(u32, u32), VerifyError> {
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
    let body = wcmu_region(chunk, start, end, &mut breaks, resolver).unwrap_or(McuResult::empty());

    let stack_slots = chunk.local_count as u32 + body.peak_above_initial;
    let stack_bytes = stack_slots * crate::bytecode::VALUE_SLOT_SIZE_BYTES;

    Ok((stack_bytes, body.heap_total))
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
    let chunk_wcmu = module_wcmu(module, native_wcmu)?;
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
                }
                Op::EndIf => match block_stack.pop() {
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
                },
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
                }
                Op::EndLoop(target) => {
                    let t = *target as usize;
                    match block_stack.pop() {
                        Some((BlockKind::Loop, loop_ip)) => {
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
                }
                Op::BreakIf(target) => {
                    if loop_depth == 0 {
                        return Err(VerifyError {
                            chunk_name: name.clone(),
                            message: alloc::format!("BreakIf at {} outside any Loop block", ip),
                        });
                    }
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
                if stream_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Func block contains {} Stream instruction(s)",
                            stream_count
                        ),
                    });
                }
                if reset_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Func block contains {} Reset instruction(s)",
                            reset_count
                        ),
                    });
                }
            }
            BlockType::Reentrant => {
                if yield_count == 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: String::from("Reentrant block must contain at least one Yield"),
                    });
                }
                if stream_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Reentrant block contains {} Stream instruction(s)",
                            stream_count
                        ),
                    });
                }
                if reset_count > 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Reentrant block contains {} Reset instruction(s)",
                            reset_count
                        ),
                    });
                }
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
                if reset_count != 1 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: alloc::format!(
                            "Stream block must contain exactly one Reset, found {}",
                            reset_count
                        ),
                    });
                }
                if yield_count == 0 {
                    return Err(VerifyError {
                        chunk_name: name.clone(),
                        message: String::from("Stream block must contain at least one Yield"),
                    });
                }
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
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{BlockType, Chunk, Module, Op, Value};
    use alloc::vec;

    fn make_module(chunks: Vec<Chunk>) -> Module {
        Module {
            chunks,
            native_names: Vec::new(),
            entry_point: Some(0),
            data_layout: None,
        }
    }

    fn make_chunk(name: &str, ops: Vec<Op>, block_type: BlockType) -> Chunk {
        Chunk {
            name: String::from(name),
            ops,
            constants: Vec::new(),
            struct_templates: Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type,
        }
    }

    #[test]
    fn valid_func_chunk() {
        let chunk = make_chunk("main", vec![Op::Const(0), Op::Return], BlockType::Func);
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn valid_if_else() {
        // If targets the else body (instruction after Else), Else targets EndIf.
        let chunk = make_chunk(
            "main",
            vec![
                Op::PushTrue, // 0
                Op::If(5),    // 1 -> else body at 5
                Op::Const(0), // 2 (then body)
                Op::Const(0), // 3 (then body continued)
                Op::Else(6),  // 4 -> EndIf at 6
                Op::Const(0), // 5 (else body)
                Op::EndIf,    // 6
                Op::Return,   // 7
            ],
            BlockType::Func,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn valid_loop() {
        // Loop(4) BreakIf(4) EndLoop(1) PushUnit
        let chunk = make_chunk(
            "main",
            vec![
                Op::Loop(4),    // 0 -> past EndLoop
                Op::PushTrue,   // 1
                Op::BreakIf(4), // 2 -> past EndLoop
                Op::EndLoop(1), // 3 -> after Loop (ip 1)
                Op::PushUnit,   // 4
                Op::Return,     // 5
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
                Op::Pop,         // 3
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
                Op::Pop,         // 2
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
            vec![Op::PushUnit, Op::Yield, Op::Return],
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
        let chunk = make_chunk("bad", vec![Op::PushUnit, Op::Return], BlockType::Reentrant);
        let module = make_module(vec![chunk]);
        let err = verify(&module).unwrap_err();
        assert!(err.message.contains("Yield"));
    }

    #[test]
    fn reentrant_with_stream_fails() {
        let chunk = make_chunk(
            "bad",
            vec![Op::Stream, Op::PushUnit, Op::Yield, Op::Return],
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
            vec![Op::Stream, Op::PushUnit, Op::Reset],
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
            vec![Op::Stream, Op::PushUnit, Op::Yield, Op::Pop],
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
            vec![Op::PushUnit, Op::Yield, Op::Pop, Op::Reset],
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
                Op::PushTrue,
                Op::If(3), // targets EndIf-like position
                Op::PushUnit,
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
            vec![Op::PushTrue, Op::BreakIf(2), Op::Return],
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
                Op::Loop(4),    // 0
                Op::PushTrue,   // 1
                Op::BreakIf(4), // 2
                Op::EndLoop(0), // 3 -> should be 1, not 0
                Op::PushUnit,   // 4
                Op::Return,     // 5
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
                Op::PushTrue, // 0
                Op::If(3),    // 1 -> Else at 3
                Op::PushUnit, // 2
                Op::Else(5),  // 3 -> targets PushUnit, not EndIf
                Op::PushUnit, // 4
                Op::PushUnit, // 5 (not EndIf)
                Op::Return,   // 6
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
                Op::PushTrue,   // 0
                Op::If(3),      // 1 -> targets EndLoop
                Op::PushUnit,   // 2
                Op::EndLoop(0), // 3 (EndLoop instead of EndIf)
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
            "fn main() -> i64 { 42 }",
            "fn main() -> i64 { if true { 1 } else { 2 } }",
            "fn main() -> i64 { let sum = 0; for i in 0..5 { let x = sum + i; } sum }",
            "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(21) }",
            "fn main() -> String { let x = 1; match x { 1 => \"one\", _ => \"other\" } }",
            "loop tick(x: i64) -> i64 { let x = yield x * 2; x }",
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
                Op::Pop,         // 3
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
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,      // 0
                Op::PushTrue,    // 1
                Op::If(6),       // 2 -> else body at 6
                Op::GetLocal(0), // 3 (then)
                Op::Yield,       // 4 (then)
                Op::Else(9),     // 5 -> EndIf at 9
                Op::GetLocal(0), // 6 (else)
                Op::Yield,       // 7 (else)
                Op::Pop,         // 8 (else)
                Op::EndIf,       // 9
                Op::Pop,         // 10
                Op::Reset,       // 11
            ],
            BlockType::Stream,
        );
        let module = make_module(vec![chunk]);
        assert!(verify(&module).is_ok());
    }

    #[test]
    fn productivity_yield_before_if() {
        // Stream -> Yield -> If/Else -> Reset: yield dominates both branches.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,      // 0
                Op::GetLocal(0), // 1
                Op::Yield,       // 2
                Op::Pop,         // 3
                Op::PushTrue,    // 4
                Op::If(8),       // 5 -> else body at 8
                Op::PushUnit,    // 6 (then)
                Op::Else(10),    // 7 -> EndIf at 10
                Op::PushUnit,    // 8 (else)
                Op::Pop,         // 9 (else)
                Op::EndIf,       // 10
                Op::Reset,       // 11
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
                Op::Stream,      // 0
                Op::PushTrue,    // 1
                Op::If(6),       // 2 -> else body at 6
                Op::GetLocal(0), // 3 (then)
                Op::Yield,       // 4 (then)
                Op::Else(9),     // 5 -> EndIf at 9
                Op::PushUnit,    // 6 (else, no yield)
                Op::Pop,         // 7 (else)
                Op::PushUnit,    // 8 (else)
                Op::EndIf,       // 9
                Op::Pop,         // 10
                Op::Reset,       // 11
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
                Op::Stream,      // 0
                Op::PushTrue,    // 1
                Op::If(6),       // 2 -> EndIf at 6 (no Else)
                Op::GetLocal(0), // 3 (then)
                Op::Yield,       // 4 (then)
                Op::Pop,         // 5 (then)
                Op::EndIf,       // 6
                Op::Reset,       // 7
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
                Op::Stream,      // 0
                Op::Loop(8),     // 1 -> past EndLoop
                Op::PushTrue,    // 2
                Op::BreakIf(8),  // 3 -> past EndLoop
                Op::GetLocal(0), // 4
                Op::Yield,       // 5
                Op::Pop,         // 6
                Op::EndLoop(2),  // 7 -> back to 2
                Op::Reset,       // 8
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
        // Yield dominates the loop, so all paths have yielded.
        let chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,      // 0
                Op::GetLocal(0), // 1
                Op::Yield,       // 2
                Op::Pop,         // 3
                Op::Loop(9),     // 4 -> past EndLoop
                Op::PushTrue,    // 5
                Op::BreakIf(9),  // 6 -> past EndLoop
                Op::PushUnit,    // 7
                Op::EndLoop(5),  // 8 -> back to 5
                Op::Reset,       // 9
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

        let src = "loop tick(x: i64) -> i64 { let x = yield x * 2; x }";
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
        assert_eq!(Op::PushUnit.cost(), 1);
        assert_eq!(Op::GetLocal(0).cost(), 1);
        assert_eq!(Op::SetLocal(0).cost(), 1);
        assert_eq!(Op::Pop.cost(), 1);
        assert_eq!(Op::Not.cost(), 1);

        assert_eq!(Op::Add.cost(), 2);
        assert_eq!(Op::Sub.cost(), 2);
        assert_eq!(Op::Mul.cost(), 2);
        assert_eq!(Op::CmpEq.cost(), 2);
        assert_eq!(Op::Return.cost(), 2);

        assert_eq!(Op::Div.cost(), 3);
        assert_eq!(Op::Mod.cost(), 3);
        assert_eq!(Op::GetField(0).cost(), 3);

        assert_eq!(Op::NewStruct(0).cost(), 5);
        assert_eq!(Op::NewArray(0).cost(), 5);
        assert_eq!(Op::NewTuple(0).cost(), 5);

        assert_eq!(Op::Call(0, 0).cost(), 10);
        assert_eq!(Op::CallNative(0, 0).cost(), 10);
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
                Op::Pop,         // 4: cost 1
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
                Op::Stream,   // 0
                Op::PushTrue, // 1
                Op::If(5),    // 2 -> else body at 5
                Op::Add,      // 3 (then body)
                Op::Else(7),  // 4 -> EndIf at 7
                Op::Div,      // 5 (else body)
                Op::Mul,      // 6 (else body)
                Op::EndIf,    // 7
                Op::Yield,    // 8
                Op::Pop,      // 9
                Op::Reset,    // 10
            ],
            BlockType::Stream,
        );
        let cost = wcet_stream_iteration(&chunk).unwrap();
        assert_eq!(cost, 11);
    }

    #[test]
    fn wcet_non_stream_errors() {
        let chunk = make_chunk("main", vec![Op::PushUnit, Op::Return], BlockType::Func);
        let err = wcet_stream_iteration(&chunk).unwrap_err();
        assert!(err.message.contains("Stream"));
    }

    #[test]
    fn wcet_compiled_stream() {
        // Integration test: compile a real loop function and compute WCET.
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "loop tick(x: i64) -> i64 { let x = yield x * 2; x }";
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
            chunks: vec![chunk],
            native_names: Vec::new(),
            entry_point: Some(0),
            data_layout: Some(DataLayout {
                slots: vec![DataSlot {
                    name: String::from("ctx.x"),
                }],
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
            vec![Op::GetData(0), Op::SetData(1), Op::PushUnit, Op::Return],
            BlockType::Func,
        );
        let module = Module {
            chunks: vec![chunk],
            native_names: Vec::new(),
            entry_point: Some(0),
            data_layout: Some(DataLayout {
                slots: vec![
                    DataSlot {
                        name: String::from("ctx.a"),
                    },
                    DataSlot {
                        name: String::from("ctx.b"),
                    },
                ],
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
                Op::Pop,         // 3 — never reached after yield
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
                Op::Stream,      // 0
                Op::PushTrue,    // 1
                Op::If(7),       // 2 -> else body at 7
                Op::Const(0),    // 3 (then push)
                Op::Const(0),    // 4 (then push)
                Op::Const(0),    // 5 (then push, total 3 deep)
                Op::Else(9),     // 6 -> EndIf at 9
                Op::Const(0),    // 7 (else, push 1)
                Op::Pop,         // 8 (else, pop)
                Op::EndIf,       // 9
                Op::Pop,         // 10 (consume one if any)
                Op::Pop,         // 11
                Op::Pop,         // 12
                Op::GetLocal(0), // 13
                Op::Yield,       // 14
                Op::Pop,         // 15
                Op::Reset,       // 16
            ],
            BlockType::Stream,
        );
        chunk.local_count = 1;
        chunk.constants = vec![Value::Int(0)];
        let (stack, _heap) = wcmu_stream_iteration(&chunk).unwrap();
        // Then branch peaks at 3 above the IfBoolPop. Plus local frame.
        // The actual peak should be at least 3 slots above the local frame.
        assert!(stack >= 3 * VALUE_SLOT_SIZE_BYTES);
    }

    #[test]
    fn wcmu_new_struct_heap() {
        // NewStruct with two fields allocates 2 * VALUE_SLOT_SIZE_BYTES.
        use crate::bytecode::{StructTemplate, VALUE_SLOT_SIZE_BYTES};
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,       // 0
                Op::Const(0),     // 1
                Op::Const(0),     // 2
                Op::NewStruct(0), // 3
                Op::Yield,        // 4
                Op::Reset,        // 5
            ],
            BlockType::Stream,
        );
        chunk.local_count = 0;
        chunk.constants = vec![Value::Int(0)];
        chunk.struct_templates = vec![StructTemplate {
            type_name: String::from("Point"),
            field_names: vec![String::from("x"), String::from("y")],
        }];
        let (_stack, heap) = wcmu_stream_iteration(&chunk).unwrap();
        assert_eq!(heap, 2 * VALUE_SLOT_SIZE_BYTES);
    }

    #[test]
    fn wcmu_new_array_heap() {
        // NewArray with three elements allocates 3 * VALUE_SLOT_SIZE_BYTES.
        use crate::bytecode::VALUE_SLOT_SIZE_BYTES;
        let mut chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::Const(0),
                Op::Const(0),
                Op::Const(0),
                Op::NewArray(3),
                Op::Yield,
                Op::Reset,
            ],
            BlockType::Stream,
        );
        chunk.constants = vec![Value::Int(0)];
        let (_stack, heap) = wcmu_stream_iteration(&chunk).unwrap();
        assert_eq!(heap, 3 * VALUE_SLOT_SIZE_BYTES);
    }

    #[test]
    fn wcmu_non_stream_errors() {
        let chunk = make_chunk("main", vec![Op::PushUnit, Op::Return], BlockType::Func);
        let err = wcmu_stream_iteration(&chunk).unwrap_err();
        assert!(err.message.contains("Stream"));
    }

    #[test]
    fn verify_resource_bounds_passes() {
        // Small program fits in default arena.
        let chunk = make_chunk(
            "tick",
            vec![Op::Stream, Op::PushUnit, Op::Yield, Op::Pop, Op::Reset],
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
                Op::NewArray(2),
                Op::Yield,
                Op::Pop,
                Op::Reset,
            ],
            BlockType::Stream,
        );
        chunk.local_count = 4;
        chunk.constants = vec![Value::Int(0)];
        let module = make_module(vec![chunk]);
        // Arena of 16 bytes is much smaller than the stream's WCMU.
        let err = verify_resource_bounds(&module, 16).unwrap_err();
        assert!(err.message.contains("WCMU"));
        assert!(err.message.contains("exceeds arena capacity"));
    }

    #[test]
    fn verify_resource_bounds_skips_non_stream() {
        // A module with only Func chunks has no WCMU bound to verify.
        let chunk = make_chunk("util", vec![Op::PushUnit, Op::Return], BlockType::Func);
        let module = make_module(vec![chunk]);
        let result = verify_resource_bounds(&module, 16);
        assert!(result.is_ok());
    }

    // -- Module-level WCMU and call-graph integration --

    #[test]
    fn module_wcmu_returns_per_chunk_results() {
        let chunk = make_chunk(
            "tick",
            vec![Op::Stream, Op::PushUnit, Op::Yield, Op::Pop, Op::Reset],
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
                Op::NewArray(3),
                Op::Pop,
                Op::PushUnit,
                Op::Return,
            ],
            BlockType::Func,
        );
        callee.constants = vec![Value::Int(0)];

        let stream_chunk = make_chunk(
            "tick",
            vec![
                Op::Stream,     // 0
                Op::Call(0, 0), // 1 — calls alloc_array
                Op::Pop,        // 2
                Op::PushUnit,   // 3
                Op::Yield,      // 4
                Op::Pop,        // 5
                Op::Reset,      // 6
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
                Op::Stream,           // 0
                Op::CallNative(0, 0), // 1 — calls native 0
                Op::Pop,              // 2
                Op::PushUnit,         // 3
                Op::Yield,            // 4
                Op::Pop,              // 5
                Op::Reset,            // 6
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
                Op::CallNative(0, 0),
                Op::Pop,
                Op::PushUnit,
                Op::Yield,
                Op::Pop,
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
    fn module_wcmu_topological_handles_chain() {
        // Three-chunk chain: stream calls helper, helper calls leaf.
        let leaf = make_chunk("leaf", vec![Op::PushUnit, Op::Return], BlockType::Func);
        let helper = make_chunk("helper", vec![Op::Call(0, 0), Op::Return], BlockType::Func);
        let stream = make_chunk(
            "tick",
            vec![
                Op::Stream,
                Op::Call(1, 0),
                Op::Pop,
                Op::PushUnit,
                Op::Yield,
                Op::Pop,
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

        let src = "loop main(input: i64) -> i64 { \
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
        // Each iteration allocates a 4-element array. With 5 iterations,
        // heap = 5 * 4 * VALUE_SLOT_SIZE_BYTES = 5 * 128 = 640 bytes.
        let expected = 5 * 4 * crate::bytecode::VALUE_SLOT_SIZE_BYTES;
        assert_eq!(heap_bytes, expected);
    }

    #[test]
    fn for_range_loop_multiplies_wcet() {
        // Compile a real for-range loop. Verify WCET reflects the
        // iteration count.
        use crate::compiler::compile;
        use crate::lexer::tokenize;
        use crate::parser::parse;

        let src = "loop main(input: i64) -> i64 { \
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
        let src_no_loop = "loop main(input: i64) -> i64 { \
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
        chunk.constants = vec![Value::Int(0), Value::Int(10)];

        let count = extract_loop_iteration_bound(&chunk, 4);
        assert_eq!(count, Some(10));
    }

    #[test]
    fn extract_loop_iteration_bound_returns_none_for_non_canonical() {
        // A loop without the canonical pattern. Should return None.
        let chunk = make_chunk(
            "test",
            vec![Op::Loop(4), Op::PushTrue, Op::BreakIf(4), Op::EndLoop(1)],
            BlockType::Func,
        );
        let count = extract_loop_iteration_bound(&chunk, 0);
        assert_eq!(count, None);
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
                Op::NewArray(2), // body: allocate 2-element array
                Op::Pop,
                Op::EndLoop(6),
                Op::PushUnit,
                Op::Yield,
                Op::Pop,
                Op::Reset,
            ],
            BlockType::Stream,
        );
        chunk.constants = vec![Value::Int(5), Value::Int(5), Value::Int(0)];
        chunk.local_count = 2;
        let (_stack, heap) = wcmu_stream_iteration(&chunk).unwrap();
        // 0 iterations means the body's heap allocation does not count.
        assert_eq!(heap, 0);
    }
}
