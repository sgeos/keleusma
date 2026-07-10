#![cfg(all(feature = "compile", feature = "verify"))]
//! Regression test for the self-hosted compiler's Stage 3 codegen, the emit spike
//! (`compiler/kel/codegen.kel`). It runs the Keleusma emitter, lowers its yielded
//! op stream to real opcodes, checks structural equality against the Rust
//! compiler's output for the same program (the primary migration gate), and
//! builds a runnable module from the emitted ops and runs it. Increment 1 targets
//! `fn main(input: Word) -> Word { input * 2 + 1 }`, exercising a parameter, a
//! two-entry constant pool, and the checked-arithmetic-then-PopN lowering.

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

const REFERENCE_SOURCE: &str = "fn main(input: Word) -> Word { input * 2 + 1 }";

/// The host side of the emit boundary: lower a yielded logical op-encoding
/// (`tag + operand*16`) to a real `Op`.
fn decode_op(w: i64) -> Op {
    let (tag, operand) = (w % 16, w / 16);
    match tag {
        1 => Op::Const(operand as u16),
        2 => Op::Return,
        3 => Op::GetLocal(operand as u16),
        4 => Op::CheckedMul(operand as u8),
        5 => Op::CheckedAdd,
        6 => Op::PopN(operand as u8),
        other => panic!("unknown op tag {other} (word {w})"),
    }
}

fn run_emitter() -> Vec<Op> {
    let src = std::fs::read_to_string("compiler/kel/codegen.kel").expect("read codegen.kel");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify codegen.kel");
    let mut ops = Vec::new();
    let mut st = vm.call(&[]).expect("call");
    for _ in 0..32 {
        match st {
            VmState::Yielded(Value::Int(w)) => ops.push(decode_op(w)),
            VmState::Finished(_) => break,
            other => panic!("unexpected VM state {:?}", other),
        }
        st = vm.resume(Value::Int(0)).expect("resume");
    }
    ops
}

fn compile_reference() -> Module {
    compile(&parse(&tokenize(REFERENCE_SOURCE).expect("lex")).expect("parse")).expect("compile")
}

fn main_index(m: &Module) -> usize {
    m.chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("reference has a main chunk")
}

#[test]
fn emit_spike_matches_reference_and_runs() {
    // The Rust compiler's output for the same program is the equivalence oracle.
    let reference = compile_reference();
    let idx = main_index(&reference);
    let reference_ops = reference.chunks[idx].ops.clone();

    let emitted = run_emitter();

    // Primary gate: logical-artifact equivalence of the op stream.
    assert_eq!(
        emitted, reference_ops,
        "emitted ops must match the Rust compiler's for `{REFERENCE_SOURCE}`"
    );

    // Runnable artifact: the host supplies the module frame and constant pool, the
    // Keleusma emitter supplies the ops. Build it and run it over several inputs.
    let mut built = compile_reference();
    built.chunks[idx].ops = emitted;
    let need = required_persistent_capacity_for(&built);
    for input in [0i64, 1, 3, 7, 100] {
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built.clone(), &arena).expect("verify built module");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => {
                assert_eq!(n, input * 2 + 1, "built module wrong for input {input}")
            }
            other => panic!("expected Int for input {input}, got {:?}", other),
        }
    }
}
