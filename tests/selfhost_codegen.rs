#![cfg(all(feature = "compile", feature = "verify"))]
//! Regression test for the self-hosted compiler's Stage 3 codegen
//! (`compiler/kel/codegen.kel`). Increment 2 is the first real codegen driven by
//! input: a throwaway adapter derives the declaration's single-node body from the
//! Rust reference, hands it to the Keleusma stage through shared data, and the
//! stage computes the ops. The test checks structural equality against the Rust
//! compiler (the primary migration gate) and runs the built module.

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

/// Host side of the emit boundary: lower a yielded `tag + operand*16` to a real `Op`.
fn decode_op(w: i64) -> Op {
    let (tag, operand) = (w % 16, w / 16);
    match tag {
        1 => Op::Const(operand as u16),
        2 => Op::Return,
        3 => Op::GetLocal(operand as u16),
        other => panic!("unknown op tag {other} (word {w})"),
    }
}

fn main_index(m: &Module) -> usize {
    m.chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("main chunk")
}

/// The codegen input adapter (throwaway). It derives the declaration's single-node
/// body from the reference's compiled main chunk and returns `(body_kind, body_arg)`
/// for the shared-data channel. Increment 2 handles only literal and local bodies.
fn adapt_body(reference: &Module) -> (i64, i64) {
    let ops = &reference.chunks[main_index(reference)].ops;
    assert_eq!(
        ops.last(),
        Some(&Op::Return),
        "expected a `<body>; Return` shape"
    );
    match ops[0] {
        Op::Const(i) => (1, i as i64),
        Op::GetLocal(s) => (2, s as i64),
        ref other => panic!("increment 2 handles only literal/local bodies, got {other:?}"),
    }
}

/// Run the Keleusma codegen with a declaration in shared data, returning its ops.
fn run_codegen(body_kind: i64, body_arg: i64) -> Vec<Op> {
    let src = std::fs::read_to_string("compiler/kel/codegen.kel").expect("read codegen.kel");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify codegen.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(body_kind))
        .expect("kind");
    vm.set_shared(&mut shared, 1, Value::Int(body_arg))
        .expect("arg");

    let mut ops = Vec::new();
    let mut st = vm.call_with_shared(&mut shared, &[]).expect("call");
    for _ in 0..32 {
        match st {
            VmState::Yielded(Value::Int(w)) => ops.push(decode_op(w)),
            VmState::Finished(_) => break,
            other => panic!("unexpected VM state {:?}", other),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    ops
}

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

#[test]
fn codegen_from_input_matches_reference_and_runs() {
    // (source, call argument, expected result)
    let cases: &[(&str, i64, i64)] = &[
        ("fn main() -> Word { 1 }", 0, 1),
        ("fn main(input: Word) -> Word { input }", 42, 42),
    ];
    for &(src, arg, expected) in cases {
        let reference = compile_src(src);
        let idx = main_index(&reference);
        let reference_ops = reference.chunks[idx].ops.clone();

        // Adapter -> shared data -> Keleusma codegen -> ops.
        let (body_kind, body_arg) = adapt_body(&reference);
        let emitted = run_codegen(body_kind, body_arg);

        // Primary gate: logical-artifact equivalence.
        assert_eq!(
            emitted, reference_ops,
            "emitted ops must match Rust for `{src}`"
        );

        // Runnable artifact.
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        // Call with the argument only when `main` takes a parameter.
        let call_args: Vec<Value> = if reference.chunks[idx].param_count == 0 {
            Vec::new()
        } else {
            vec![Value::Int(arg)]
        };
        match vm.call(&call_args).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "wrong for `{src}`"),
            other => panic!("expected Int for `{src}`, got {:?}", other),
        }
    }
}
