//! The shipping side. Compile with the Rust compiler, load through the safe `Vm::new`, take
//! the verifier's worst-case execution time under a unit cost model, and measure concrete
//! runs instruction by instruction.
//!
//! # How a run is measured
//!
//! Every operation of every chunk is armed as a breakpoint, so the virtual machine stops
//! before each operation it dispatches and the harness counts it. An operation is counted
//! once, at its breakpoint, whatever it does. The implicit return at the end of a chunk
//! dispatches no operation and is not counted. Breakpoints are runtime state and do not
//! change the bytecode or its verification.

use keleusma::bytecode::{CostModel, Module, Op, Value};
use keleusma::vm::{Vm, VmState};

/// The arena given to every module, far above what the generated modules need.
const ARENA_BYTES: usize = 1 << 20;

/// The most operations one run may dispatch before the harness calls it runaway.
const STEP_CAP: usize = 2_000_000;

fn unit_cycles(_: &Op) -> u32 {
    1
}

/// The negative control's cost model, which undercharges the loop machinery.
fn broken_cycles(op: &Op) -> u32 {
    match op {
        Op::Loop(_) | Op::EndLoop(_) | Op::BreakIf(_) => 0,
        _ => 1,
    }
}

/// The unit cost model, one cycle per operation, which matches what a run measures.
pub fn unit_cost_model(broken: bool) -> CostModel {
    CostModel {
        value_slot_bytes: keleusma::bytecode::VALUE_SLOT_SIZE_BYTES,
        op_cycles: if broken { broken_cycles } else { unit_cycles },
        text_byte_cycles: 0,
    }
}

/// Where the shipping pipeline stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Lex,
    Parse,
    Compile,
    Load,
    Accepted,
}

/// The shipping pipeline's verdict on one source.
pub struct Verdict {
    pub stage: Stage,
    pub message: String,
    pub module: Option<Module>,
}

/// Lex, parse, compile, which runs the verifier, then load through `Vm::new`, which runs
/// it again with the resource bounds.
pub fn judge(src: &str) -> Verdict {
    let fail = |stage, message| Verdict {
        stage,
        message,
        module: None,
    };
    let tokens = match keleusma::lexer::tokenize(src) {
        Ok(t) => t,
        Err(e) => return fail(Stage::Lex, format!("{e:?}")),
    };
    let program = match keleusma::parser::parse(&tokens) {
        Ok(p) => p,
        Err(e) => return fail(Stage::Parse, format!("{e:?}")),
    };
    let module = match keleusma::compiler::compile(&program) {
        Ok(m) => m,
        Err(e) => return fail(Stage::Compile, e.message),
    };
    let arena = keleusma::Arena::with_capacity(ARENA_BYTES);
    match Vm::new(module.clone(), &arena) {
        Ok(_) => Verdict {
            stage: Stage::Accepted,
            message: String::new(),
            module: Some(module),
        },
        Err(e) => Verdict {
            stage: Stage::Load,
            message: format!("{e:?}"),
            module: Some(module),
        },
    }
}

/// The index of the module's single Stream chunk.
pub fn stream_chunk(m: &Module) -> Option<usize> {
    m.chunks
        .iter()
        .position(|c| c.block_type == keleusma::bytecode::BlockType::Stream)
}

/// The verifier's per-iteration worst-case execution time of the Stream chunk.
pub fn wcet(m: &Module, broken: bool) -> Result<u32, String> {
    let i = stream_chunk(m).ok_or("no Stream chunk")?;
    keleusma::verify::wcet_stream_iteration_with_cost_model(
        &m.chunks[i],
        &unit_cost_model(broken),
        &[],
    )
    .map_err(|e| e.message)
}

/// One measured run.
#[derive(Debug, Default)]
pub struct Run {
    /// The yielded outputs, in order.
    pub outs: Vec<u64>,
    /// The operations dispatched between consecutive yields, the junction through each
    /// reset included, and last the operations after the final yield through the final
    /// reset.
    pub segs: Vec<u64>,
    /// Per completed iteration, the operations dispatched in the Stream chunk only.
    pub iter_shallow: Vec<u64>,
    /// Per completed iteration, the operations dispatched in every chunk.
    pub iter_deep: Vec<u64>,
    /// Per completed iteration, the number of yields.
    pub iter_yields: Vec<u64>,
}

/// Run `m` for `cycles` iterations, answering the yield of index `k` with character `k`
/// of `replies`, and resuming after each reset with the last reply, which is what the
/// model's locals hold.
pub fn measure(m: &Module, cycles: usize, replies: &[u8]) -> Result<Run, String> {
    let main = stream_chunk(m).ok_or("no Stream chunk")?;
    let arena = keleusma::Arena::with_capacity(ARENA_BYTES);
    let mut vm = Vm::new(m.clone(), &arena).map_err(|e| format!("load {e:?}"))?;
    for (ci, c) in m.chunks.iter().enumerate() {
        for oi in 0..c.ops.len() {
            vm.set_breakpoint(ci, oi);
        }
    }
    let mut run = Run::default();
    let (mut seg, mut shallow, mut deep, mut ys) = (0u64, 0u64, 0u64, 0u64);
    let mut last_reply: i64 = 0;
    let mut steps = 0usize;
    let mut state = vm
        .call(&[Value::Int(0)])
        .map_err(|e| format!("call {e:?}"))?;
    loop {
        state = match state {
            VmState::BreakpointHit { chunk, .. } => {
                steps += 1;
                if steps > STEP_CAP {
                    return Err(String::from("runaway, step cap reached"));
                }
                seg += 1;
                deep += 1;
                if chunk == main {
                    shallow += 1;
                }
                vm.resume_from_breakpoint()
                    .map_err(|e| format!("step {e:?}"))?
            }
            VmState::Yielded(v) => {
                let Value::Int(o) = v else {
                    return Err(format!("non-integer output {v:?}"));
                };
                let k = run.outs.len();
                let reply = match replies.get(k) {
                    Some(b'1') => 1,
                    Some(_) => 0,
                    None => return Err(String::from("replies exhausted")),
                };
                run.outs.push(o as u64);
                run.segs.push(seg);
                seg = 0;
                ys += 1;
                last_reply = reply;
                vm.resume(Value::Int(reply))
                    .map_err(|e| format!("resume {e:?}"))?
            }
            VmState::Reset => {
                run.iter_shallow.push(shallow);
                run.iter_deep.push(deep);
                run.iter_yields.push(ys);
                (shallow, deep, ys) = (0, 0, 0);
                if run.iter_deep.len() == cycles {
                    run.segs.push(seg);
                    return Ok(run);
                }
                vm.resume(Value::Int(last_reply))
                    .map_err(|e| format!("resume after reset {e:?}"))?
            }
            other => return Err(format!("unexpected state {other:?}")),
        };
    }
}
