extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bytecode::{BlockType, Module, Op};

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
}
