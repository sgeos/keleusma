// The self-hosted codegen is a full-width host tool, gated to a 64-bit runtime.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
//! Stage 3 codegen (`compiler/kel/codegen.kel`). A throwaway adapter flattens the
//! reference's block into the shared-data node arrays and statement metadata, the
//! Keleusma stage walks it recursion-free, interns each literal into its own
//! deduplicating constant pool, and emits the ops followed by the pool. The host
//! builds the module from the stage's ops and pool, checks structural equality
//! against the Rust compiler, and runs it. Increment 10 has the stage resolve its
//! own `If`/`Else` targets: it buffers the op stream and backpatches the markers in
//! place, so no host `resolve_targets` step is needed and the emitted stream
//! already carries the reference's absolute targets.

use keleusma::Arena;
use keleusma::ast::{BinOp, Block, Expr, Literal, Param, Pattern, Stmt};
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `ast` block's field order in codegen.kel
// (one slot per scalar, arrays contiguous). root=0, then the four length-64 node
// arrays, then the statement metadata.
const KINDS: usize = 1;
const ARGS: usize = 65;
const LHS: usize = 129;
const RHS: usize = 193;
const STMT_COUNT: usize = 257;
const STMT_EXPR: usize = 258;
const STMT_SLOT: usize = 290;

struct Node {
    kind: i64,
    arg: i64,
    lhs: i64,
    rhs: i64,
}

/// A flattened block: the expression forest, the `let` statements as
/// (value-expression root, target slot) pairs in source order, and the tail
/// expression's node index.
struct Body {
    nodes: Vec<Node>,
    stmts: Vec<(i64, i64)>,
    tail: i64,
}

fn param_name(p: &Param) -> &str {
    match &p.pattern {
        Pattern::Variable(n, _) => n,
        other => panic!("increment handles simple parameter patterns only, got {other:?}"),
    }
}

/// The codegen input adapter (throwaway, Rust-side). Flattens one expression
/// tree; a literal carries its value (the stage builds the pool), an identifier
/// its slot resolved through `scope` (name -> slot, honouring shadowing by taking
/// the last binding). Returns the root node index.
fn flatten(e: &Expr, scope: &[(String, i64)], out: &mut Vec<Node>) -> i64 {
    match e {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => {
            out.push(Node {
                kind: 1,
                arg: *n,
                lhs: 0,
                rhs: 0,
            });
            (out.len() - 1) as i64
        }
        Expr::Ident { name, .. } => {
            let slot = scope
                .iter()
                .rev()
                .find(|(nm, _)| nm == name)
                .map(|(_, s)| *s)
                .expect("identifier must be a parameter or a let binding in scope");
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
            let l = flatten(left, scope, out);
            let r = flatten(right, scope, out);
            let opcode = match op {
                BinOp::Add => 1,
                BinOp::Mul => 2,
                BinOp::Sub => 3,
                BinOp::Div => 4,
                BinOp::Mod => 5,
                BinOp::Eq => 6,
                BinOp::NotEq => 7,
                BinOp::Lt => 8,
                BinOp::Gt => 9,
                BinOp::LtEq => 10,
                BinOp::GtEq => 11,
                other => panic!("increment handles + - * / % and comparisons, got {other:?}"),
            };
            out.push(Node {
                kind: 3,
                arg: opcode,
                lhs: l,
                rhs: r,
            });
            (out.len() - 1) as i64
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            let else_block = else_block
                .as_ref()
                .expect("increment requires an else branch");
            assert!(
                then_block.stmts.is_empty() && else_block.stmts.is_empty(),
                "increment restricts if-branches to a single expression"
            );
            let then_e = then_block
                .tail_expr
                .as_ref()
                .expect("then branch needs a tail expression");
            let else_e = else_block
                .tail_expr
                .as_ref()
                .expect("else branch needs a tail expression");
            // Node layout for an If (kind 4): arg = cond, lhs = then, rhs = else.
            let cond = flatten(condition, scope, out);
            let t = flatten(then_e, scope, out);
            let el = flatten(else_e, scope, out);
            out.push(Node {
                kind: 4,
                arg: cond,
                lhs: t,
                rhs: el,
            });
            (out.len() - 1) as i64
        }
        other => panic!("increment handles literal/local/binop/if only, got {other:?}"),
    }
}

/// Flatten a whole block: assign each `let` a slot after the parameters in
/// declaration order, flatten its value expression under the scope visible at
/// that point, then flatten the tail expression.
fn build_body(block: &Block, params: &[Param]) -> Body {
    let mut nodes = Vec::new();
    let mut scope: Vec<(String, i64)> = params
        .iter()
        .enumerate()
        .map(|(i, p)| (param_name(p).to_string(), i as i64))
        .collect();
    let mut stmts = Vec::new();
    let mut next_slot = params.len() as i64;
    for st in &block.stmts {
        match st {
            Stmt::Let(l) => {
                let root = flatten(&l.value, &scope, &mut nodes);
                let name = match &l.pattern {
                    Pattern::Variable(n, _) => n.clone(),
                    other => panic!("increment handles simple let patterns only, got {other:?}"),
                };
                let slot = next_slot;
                next_slot += 1;
                scope.push((name, slot));
                stmts.push((root, slot));
            }
            other => panic!("increment handles let statements only, got {other:?}"),
        }
    }
    let tail = flatten(
        block
            .tail_expr
            .as_ref()
            .expect("block has a tail expression"),
        &scope,
        &mut nodes,
    );
    Body { nodes, stmts, tail }
}

fn decode_op(w: i64) -> Op {
    let (tag, operand) = (w % 32, w / 32);
    match tag {
        1 => Op::Const(operand as u16),
        2 => Op::Return,
        3 => Op::GetLocal(operand as u16),
        4 => Op::CheckedMul(operand as u8),
        5 => Op::CheckedAdd,
        6 => Op::PopN(operand as u8),
        7 => Op::SetLocal(operand as u16),
        8 => Op::CheckedSub,
        9 => Op::Div,
        10 => Op::Mod,
        11 => Op::CmpEq,
        12 => Op::CmpNe,
        13 => Op::CmpLt,
        14 => Op::CmpGt,
        15 => Op::CmpLe,
        16 => Op::CmpGe,
        17 => Op::If(operand as u16),
        18 => Op::Else(operand as u16),
        19 => Op::EndIf,
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

/// Resume until the next yielded word, skipping the loop RESET. Used for the raw
/// pool metadata, where a yielded 0 is a real value, not a PENDING marker.
fn next_word(vm: &mut Vm<'_, '_>, shared: &mut [u8]) -> i64 {
    loop {
        match vm
            .resume_with_shared(shared, Value::Int(0))
            .expect("resume")
        {
            VmState::Yielded(Value::Int(w)) => return w,
            VmState::Reset => continue,
            other => panic!("unexpected {other:?}"),
        }
    }
}

/// Drive the codegen; return its emitted ops and the constant pool it built.
fn run_codegen(body: &Body) -> (Vec<Op>, Vec<i64>) {
    let src = std::fs::read_to_string("compiler/kel/codegen.kel").expect("read codegen.kel");
    let m = compile_src(&src);
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify codegen.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(body.tail))
        .expect("root");
    for (i, n) in body.nodes.iter().enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(n.kind))
            .expect("kind");
        vm.set_shared(&mut shared, ARGS + i, Value::Int(n.arg))
            .expect("arg");
        vm.set_shared(&mut shared, LHS + i, Value::Int(n.lhs))
            .expect("lhs");
        vm.set_shared(&mut shared, RHS + i, Value::Int(n.rhs))
            .expect("rhs");
    }
    vm.set_shared(&mut shared, STMT_COUNT, Value::Int(body.stmts.len() as i64))
        .expect("stmt_count");
    for (k, &(expr_root, slot)) in body.stmts.iter().enumerate() {
        vm.set_shared(&mut shared, STMT_EXPR + k, Value::Int(expr_root))
            .expect("stmt_expr");
        vm.set_shared(&mut shared, STMT_SLOT + k, Value::Int(slot))
            .expect("stmt_slot");
    }

    // Phase 1: ops until Return.
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

    // Phase 2: the pool the stage built. Size, then that many raw values.
    let count = next_word(&mut vm, &mut shared);
    let pool = (0..count)
        .map(|_| next_word(&mut vm, &mut shared))
        .collect();
    (ops, pool)
}

/// Expected result of running a built module. Arithmetic bodies return a `Word`;
/// comparison bodies return a `bool`.
#[derive(Clone, Copy, Debug)]
enum Expect {
    Int(i64),
    Bool(bool),
}

#[test]
fn codegen_owns_its_constant_pool_and_matches_reference() {
    use Expect::{Bool, Int};
    let cases: &[(&str, i64, Expect)] = &[
        ("fn main() -> Word { 1 + 2 }", 0, Int(3)),
        ("fn main(input: Word) -> Word { input + 1 }", 41, Int(42)),
        (
            "fn main(input: Word) -> Word { input * 2 + 1 }",
            20,
            Int(41),
        ),
        // Repeated literal: the pool must deduplicate to a single entry, matching
        // the reference compiler's constant interning (both `2`s use Const(0)).
        (
            "fn main(input: Word) -> Word { input * 2 + 2 }",
            20,
            Int(42),
        ),
        // `let` bindings: each lowers to its value's ops then SetLocal(slot), with
        // slots assigned after the parameters; the tail reads a let-bound slot.
        (
            "fn main(input: Word) -> Word { let x = input + 1; x * 2 }",
            20,
            Int(42),
        ),
        // Chained lets, later reading an earlier binding twice (slot reuse).
        (
            "fn main(input: Word) -> Word { let x = input + 1; let y = x + x; y }",
            20,
            Int(42),
        ),
        // Subtraction: CheckedSub, same two-word-then-PopN shape as add.
        ("fn main(a: Word) -> Word { a - 1 }", 43, Int(42)),
        // Division and modulo: single-word Div/Mod, no PopN fixup.
        ("fn main(a: Word) -> Word { a / 2 }", 84, Int(42)),
        ("fn main(a: Word) -> Word { a % 5 }", 47, Int(2)),
        // Mixed: subtraction under division, exercising both shapes and nesting.
        ("fn main(a: Word) -> Word { (a - 2) / 2 }", 86, Int(42)),
        // The six comparison operators: each a single Cmp* op, no PopN, bool result.
        ("fn main(a: Word) -> bool { a < 5 }", 3, Bool(true)),
        ("fn main(a: Word) -> bool { a > 5 }", 3, Bool(false)),
        ("fn main(a: Word) -> bool { a <= 5 }", 5, Bool(true)),
        ("fn main(a: Word) -> bool { a >= 5 }", 4, Bool(false)),
        ("fn main(a: Word) -> bool { a == 5 }", 5, Bool(true)),
        ("fn main(a: Word) -> bool { a != 5 }", 5, Bool(false)),
        // if/else expression: condition ops, If, then, Else, else, EndIf. The
        // stage emits placeholder targets; resolve_targets fills the absolute
        // indices by bracket-matching, matching the reference's baked targets.
        (
            "fn main(a: Word) -> Word { if a < 5 { 2 } else { 3 } }",
            3,
            Int(2),
        ),
        (
            "fn main(a: Word) -> Word { if a < 5 { 2 } else { 3 } }",
            10,
            Int(3),
        ),
        // Arithmetic inside the branches.
        (
            "fn main(a: Word) -> Word { if a < 5 { a + 1 } else { a - 1 } }",
            10,
            Int(9),
        ),
        // Nested if in the else branch: exercises resolve_targets on nesting.
        (
            "fn main(a: Word) -> Word { if a < 5 { 1 } else { if a < 10 { 2 } else { 3 } } }",
            7,
            Int(2),
        ),
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
        let body = build_body(&main_fn.body, &main_fn.params);

        // The stage now resolves its own If/Else targets (backpatched in its op
        // buffer), so the emitted stream is a complete logical module as-is.
        let (emitted, pool) = run_codegen(&body);
        assert_eq!(
            emitted, reference_ops,
            "emitted ops must match Rust for `{src}`"
        );

        // Build the module from the stage's own ops and constant pool.
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        let call_args: Vec<Value> = if main_fn.params.is_empty() {
            Vec::new()
        } else {
            vec![Value::Int(arg)]
        };
        match (expected, vm.call(&call_args).expect("call built")) {
            (Expect::Int(e), VmState::Finished(Value::Int(n))) => {
                assert_eq!(n, e, "wrong for `{src}`")
            }
            (Expect::Bool(e), VmState::Finished(Value::Bool(b))) => {
                assert_eq!(b, e, "wrong for `{src}`")
            }
            (exp, got) => panic!("result mismatch for `{src}`: expected {exp:?}, got {got:?}"),
        }
    }
}
