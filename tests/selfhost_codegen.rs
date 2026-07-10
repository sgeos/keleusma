// The self-hosted codegen is a full-width host tool, gated to a 64-bit runtime
// like the other self-hosted tests.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
//! Stage 3 codegen (`compiler/kel/codegen.kel`). Increment 3 is real codegen over
//! a nested expression, driven by the explicit work-stack idiom. A throwaway
//! adapter flattens the Rust reference's expression tree into the shared-data node
//! arrays; the Keleusma stage walks it recursion-free and emits a post-order op
//! stream, checked for structural equality against the Rust compiler and run.

use keleusma::Arena;
use keleusma::ast::{BinOp, Expr, Literal, Param, Pattern};
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot bases: root at 0, then four 64-entry arrays.
const KINDS: usize = 1;
const ARGS: usize = 65;
const LHS: usize = 129;
const RHS: usize = 193;

struct Node {
    kind: i64,
    arg: i64,
    lhs: i64,
    rhs: i64,
}

/// The codegen input adapter (throwaway, Rust-side). Recursion here is fine; only
/// the Keleusma stage must be recursion-free. Flattens the reference's expression
/// tree into parallel node arrays, mapping literals to their constant-pool index
/// and parameters to their slot. Returns the root node index.
fn flatten(e: &Expr, params: &[Param], consts: &[ConstValue], out: &mut Vec<Node>) -> i64 {
    match e {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => {
            let idx = consts
                .iter()
                .position(|c| matches!(c, ConstValue::Int(v) if v == n))
                .expect("literal must be in the reference constant pool")
                as i64;
            out.push(Node {
                kind: 1,
                arg: idx,
                lhs: 0,
                rhs: 0,
            });
            (out.len() - 1) as i64
        }
        Expr::Ident { name, .. } => {
            let slot = params
                .iter()
                .position(|p| matches!(&p.pattern, Pattern::Variable(pn, _) if pn == name))
                .expect("identifier must be a parameter") as i64;
            out.push(Node {
                kind: 2,
                arg: slot,
                lhs: 0,
                rhs: 0,
            });
            (out.len() - 1) as i64
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            let l = flatten(left, params, consts, out);
            let r = flatten(right, params, consts, out);
            let opcode = match op {
                BinOp::Add => 1,
                BinOp::Mul => 2,
                other => panic!("increment 3 handles + and * only, got {other:?}"),
            };
            out.push(Node {
                kind: 3,
                arg: opcode,
                lhs: l,
                rhs: r,
            });
            (out.len() - 1) as i64
        }
        other => panic!("increment 3 handles literal/local/binop only, got {other:?}"),
    }
}

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

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn main_index(m: &Module) -> usize {
    m.chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("main")
}

/// Drive the Keleusma codegen with a flattened tree in shared data; return its ops.
fn run_codegen(nodes: &[Node], root: i64) -> Vec<Op> {
    let src = std::fs::read_to_string("compiler/kel/codegen.kel").expect("read codegen.kel");
    let m = compile_src(&src);
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify codegen.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(root))
        .expect("root");
    for (i, n) in nodes.iter().enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(n.kind))
            .expect("kind");
        vm.set_shared(&mut shared, ARGS + i, Value::Int(n.arg))
            .expect("arg");
        vm.set_shared(&mut shared, LHS + i, Value::Int(n.lhs))
            .expect("lhs");
        vm.set_shared(&mut shared, RHS + i, Value::Int(n.rhs))
            .expect("rhs");
    }

    let mut ops = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..4096 {
        match st {
            VmState::Yielded(Value::Int(w)) => {
                if w != 0 {
                    let op = decode_op(w);
                    let done = op == Op::Return;
                    ops.push(op);
                    if done {
                        break;
                    }
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected VM state {other:?}"),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    ops
}

#[test]
fn codegen_walks_nested_arithmetic_and_matches_reference() {
    // (source, call argument, expected result)
    let cases: &[(&str, i64, i64)] = &[
        ("fn main() -> Word { 1 + 2 }", 0, 3),
        ("fn main(input: Word) -> Word { input + 1 }", 41, 42),
        ("fn main(input: Word) -> Word { input * 2 + 1 }", 20, 41),
    ];
    for &(src, arg, expected) in cases {
        let program = parse(&tokenize(src).expect("lex")).expect("parse");
        let reference = compile_src(src);
        let idx = main_index(&reference);
        let reference_ops = reference.chunks[idx].ops.clone();

        let main_fn = program
            .functions
            .iter()
            .find(|f| f.name == "main")
            .expect("main fn");
        let body = main_fn
            .body
            .tail_expr
            .as_ref()
            .expect("main has a tail expression");
        let mut nodes = Vec::new();
        let root = flatten(
            body,
            &main_fn.params,
            &reference.chunks[idx].constants,
            &mut nodes,
        );

        let emitted = run_codegen(&nodes, root);
        assert_eq!(
            emitted, reference_ops,
            "emitted ops must match Rust for `{src}`"
        );

        // Build and run.
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        let call_args: Vec<Value> = if main_fn.params.is_empty() {
            Vec::new()
        } else {
            vec![Value::Int(arg)]
        };
        match vm.call(&call_args).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "wrong for `{src}`"),
            other => panic!("expected Int for `{src}`, got {other:?}"),
        }
    }
}
