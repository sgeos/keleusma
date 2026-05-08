extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bytecode::{BlockType, Chunk, Module, Op};

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
/// For loops, assumes one iteration (conservative default).
///
/// Returns `Some(cost)` for paths that fall through to `end`.
/// Returns `None` if all paths exit via Break.
///
/// Break costs are accumulated in `break_costs` for the enclosing loop.
fn wcet_region(ops: &[Op], start: usize, end: usize, break_costs: &mut Vec<u32>) -> Option<u32> {
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
                    let then_cost = wcet_region(ops, ip + 1, target - 1, break_costs);
                    let else_cost = wcet_region(ops, target, endif_pos, break_costs);
                    let branch_cost = match (then_cost, else_cost) {
                        (Some(a), Some(b)) => Some(if a > b { a } else { b }),
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        (None, None) => return None,
                    };
                    cost += branch_cost.unwrap_or(0);
                    ip = endif_pos + 1;
                } else {
                    let then_cost = wcet_region(ops, ip + 1, target, break_costs);
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
                let body_cost = wcet_region(ops, ip + 1, endloop_ip, &mut loop_break_costs);
                if loop_break_costs.is_empty() && body_cost.is_none() {
                    return None;
                }
                let max_break = loop_break_costs.iter().copied().max().unwrap_or(0);
                let max_body = body_cost.unwrap_or(0);
                cost += if max_break > max_body {
                    max_break
                } else {
                    max_body
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
    let body_cost = wcet_region(ops, stream_pos + 1, reset_pos, &mut break_costs);

    // Include Stream and Reset instruction costs.
    let overhead = ops[stream_pos].cost() + ops[reset_pos].cost();
    let region_cost = body_cost.unwrap_or(0);

    Ok(overhead + region_cost)
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
    use crate::bytecode::{BlockType, Chunk, Module, Op};
    use alloc::vec;

    fn make_module(chunks: Vec<Chunk>) -> Module {
        Module {
            chunks,
            native_names: Vec::new(),
            entry_point: Some(0),
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
}
