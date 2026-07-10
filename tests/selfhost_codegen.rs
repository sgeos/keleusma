#![cfg(all(feature = "compile", feature = "verify"))]
//! Regression test for the self-hosted compiler's Stage 3 codegen, increment 0
//! (`compiler/kel/codegen.kel`), the emit-to-host spike. It runs the Keleusma
//! emitter, maps its yielded op stream to real opcodes, checks structural
//! equality against the Rust compiler's output for the same program (the
//! primary migration gate), and builds a runnable module from the emitted ops
//! and runs it. This is the first backward-migration increment and the proof
//! that Keleusma can drive the emit boundary to a runnable artifact.

use keleusma::Arena;
use keleusma::bytecode::{Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

const REFERENCE_SOURCE: &str = "fn main() -> Word { 1 }";

/// Map a yielded logical op-encoding (`tag + operand*16`) to a real `Op`. This is
/// the host side of the emit boundary: Keleusma produces logical ops, the host
/// lowers them to the wire opcodes.
fn decode_op(w: i64) -> Op {
    let (tag, operand) = (w % 16, w / 16);
    match tag {
        1 => Op::Const(operand as u16),
        2 => Op::Return,
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
    for _ in 0..16 {
        match st {
            VmState::Yielded(Value::Int(w)) => ops.push(decode_op(w)),
            VmState::Finished(_) => break,
            other => panic!("unexpected VM state {:?}", other),
        }
        st = vm.resume(Value::Int(0)).expect("resume");
    }
    ops
}

fn compile_reference() -> keleusma::bytecode::Module {
    compile(&parse(&tokenize(REFERENCE_SOURCE).expect("lex")).expect("parse")).expect("compile")
}

#[test]
fn emit_spike_matches_reference_and_runs() {
    // The Rust compiler's output for the same program is the equivalence oracle.
    let reference = compile_reference();
    let main_idx = reference
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("reference has a main chunk");
    let reference_ops = reference.chunks[main_idx].ops.clone();

    // Keleusma-emitted ops.
    let emitted = run_emitter();

    // Primary gate: structural equality of the op stream (logical-artifact
    // equivalence), Keleusma against Rust.
    assert_eq!(
        emitted, reference_ops,
        "emitted ops must match the Rust compiler's for `{REFERENCE_SOURCE}`"
    );

    // Runnable artifact: the host supplies the module frame and the Keleusma
    // emitter supplies the ops. Build and run it.
    let mut built = compile_reference();
    built.chunks[main_idx].ops = emitted;
    let need = required_persistent_capacity_for(&built);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(built, &arena).expect("verify built module");
    match vm.call(&[]).expect("call built") {
        VmState::Finished(Value::Int(n)) => assert_eq!(n, 1, "built module must return 1"),
        other => panic!("expected Int(1), got {:?}", other),
    }
}
