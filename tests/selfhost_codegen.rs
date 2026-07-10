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
//! against the Rust compiler, and runs it. Increment 15 adds short-circuit
//! `andalso`/`orelse`, which reuse the structured control flow (`Dup` + `If`/`Else`/
//! `EndIf`) and whose targets the existing backpatcher resolves.

use keleusma::Arena;
use keleusma::ast::{BinOp, Block, Expr, Literal, Param, Pattern, Stmt, UnaryOp};
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `ast` block's field order in codegen.kel
// (one slot per scalar, arrays contiguous). root=0, then the four length-64 node
// arrays, then param_count.
const KINDS: usize = 1;
const ARGS: usize = 65;
const LHS: usize = 129;
const RHS: usize = 193;
const CALL_ARGS: usize = 257;
const PARAM_COUNT: usize = 321;

struct Node {
    kind: i64,
    arg: i64,
    lhs: i64,
    rhs: i64,
}

/// Flattening context: the node forest, the packed call-argument node indices, and
/// the callee chunk names (for resolving a call target to a chunk index).
struct Ctx {
    nodes: Vec<Node>,
    call_args: Vec<i64>,
    chunk_names: Vec<String>,
}

/// A flattened function body: the flattening context and the body's block root.
struct Body {
    nodes: Vec<Node>,
    call_args: Vec<i64>,
    root: i64,
}

fn param_name(p: &Param) -> &str {
    match &p.pattern {
        Pattern::Variable(n, _) => n,
        other => panic!("increment handles simple parameter patterns only, got {other:?}"),
    }
}

/// The codegen input adapter (throwaway, Rust-side). Flattens one expression into
/// the node forest, threading a lexical `scope` (name -> slot, latest binding wins)
/// and a monotonic `next_slot` counter. A literal carries its value, an identifier
/// its resolved slot; an `if` flattens each branch as a block; a call resolves its
/// callee name to a chunk index and packs its argument node indices. Returns the
/// root node index.
fn flatten(e: &Expr, scope: &mut Vec<(String, i64)>, next_slot: &mut i64, ctx: &mut Ctx) -> i64 {
    match e {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => {
            ctx.nodes.push(Node {
                kind: 1,
                arg: *n,
                lhs: 0,
                rhs: 0,
            });
            (ctx.nodes.len() - 1) as i64
        }
        Expr::Ident { name, .. } => {
            let slot = scope
                .iter()
                .rev()
                .find(|(nm, _)| nm == name)
                .map(|(_, s)| *s)
                .expect("identifier must be a parameter or a let binding in scope");
            ctx.nodes.push(Node {
                kind: 2,
                arg: slot,
                lhs: 0,
                rhs: 0,
            });
            (ctx.nodes.len() - 1) as i64
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            let l = flatten(left, scope, next_slot, ctx);
            let r = flatten(right, scope, next_slot, ctx);
            // Most binary operators are kind 3 with an operator code; the
            // short-circuit booleans are their own node kinds (8, 9).
            let (kind, arg) = match op {
                BinOp::Add => (3, 1),
                BinOp::Mul => (3, 2),
                BinOp::Sub => (3, 3),
                BinOp::Div => (3, 4),
                BinOp::Mod => (3, 5),
                BinOp::Eq => (3, 6),
                BinOp::NotEq => (3, 7),
                BinOp::Lt => (3, 8),
                BinOp::Gt => (3, 9),
                BinOp::LtEq => (3, 10),
                BinOp::GtEq => (3, 11),
                BinOp::Andalso => (8, 0),
                BinOp::Orelse => (9, 0),
                other => panic!(
                    "increment handles arithmetic, comparisons, andalso/orelse, got {other:?}"
                ),
            };
            ctx.nodes.push(Node {
                kind,
                arg,
                lhs: l,
                rhs: r,
            });
            (ctx.nodes.len() - 1) as i64
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
            // Node layout for an If (kind 4): arg = cond, lhs = then, rhs = else.
            // Each branch is a full block (it may bind its own locals).
            let cond = flatten(condition, scope, next_slot, ctx);
            let t = flatten_block(then_block, scope, next_slot, ctx);
            let el = flatten_block(else_block, scope, next_slot, ctx);
            ctx.nodes.push(Node {
                kind: 4,
                arg: cond,
                lhs: t,
                rhs: el,
            });
            (ctx.nodes.len() - 1) as i64
        }
        Expr::UnaryOp {
            op: UnaryOp::Not,
            operand,
            ..
        } => {
            // UnaryNot (kind 6): operand in lhs.
            let operand = flatten(operand, scope, next_slot, ctx);
            ctx.nodes.push(Node {
                kind: 6,
                arg: 0,
                lhs: operand,
                rhs: 0,
            });
            (ctx.nodes.len() - 1) as i64
        }
        Expr::Call { name, args, .. } => {
            let chunk = ctx
                .chunk_names
                .iter()
                .position(|n| n == name)
                .expect("callee must be a known chunk") as i64;
            // Flatten the argument expressions first (a nested call appends its own
            // call_args), then reserve a contiguous slice for this call's args.
            let arg_nodes: Vec<i64> = args
                .iter()
                .map(|a| flatten(a, scope, next_slot, ctx))
                .collect();
            let start = ctx.call_args.len() as i64;
            let count = arg_nodes.len() as i64;
            ctx.call_args.extend(arg_nodes);
            // Call (kind 7): arg = chunk index, lhs = args_start, rhs = arg count.
            ctx.nodes.push(Node {
                kind: 7,
                arg: chunk,
                lhs: start,
                rhs: count,
            });
            (ctx.nodes.len() - 1) as i64
        }
        other => panic!("increment handles literal/local/binop/if/not/call only, got {other:?}"),
    }
}

/// Flatten a block into a `LetIn` cons-list ending in the tail expression, and
/// return its root node index. Each `let` is flattened under the scope visible at
/// that point and assigned the next monotonic slot; block-local bindings leave the
/// scope on exit, but slots are never reused (matching the reference).
fn flatten_block(
    block: &Block,
    scope: &mut Vec<(String, i64)>,
    next_slot: &mut i64,
    ctx: &mut Ctx,
) -> i64 {
    let mark = scope.len();
    let mut lets: Vec<(i64, i64)> = Vec::new();
    for st in &block.stmts {
        match st {
            Stmt::Let(l) => {
                let value = flatten(&l.value, scope, next_slot, ctx);
                let name = match &l.pattern {
                    Pattern::Variable(n, _) => n.clone(),
                    other => panic!("increment handles simple let patterns only, got {other:?}"),
                };
                let slot = *next_slot;
                *next_slot += 1;
                scope.push((name, slot));
                lets.push((slot, value));
            }
            other => panic!("increment handles let statements only, got {other:?}"),
        }
    }
    let tail = flatten(
        block
            .tail_expr
            .as_ref()
            .expect("block has a tail expression"),
        scope,
        next_slot,
        ctx,
    );
    // Fold the lets into a LetIn cons-list from the innermost (tail) outward.
    let mut cont = tail;
    for &(slot, value) in lets.iter().rev() {
        ctx.nodes.push(Node {
            kind: 5,
            arg: slot,
            lhs: value,
            rhs: cont,
        });
        cont = (ctx.nodes.len() - 1) as i64;
    }
    scope.truncate(mark);
    cont
}

/// Flatten a function body, returning the node forest, the packed call arguments,
/// and the body's block root. Parameters occupy the first slots. `chunk_names` maps
/// a call target to its chunk index by position.
fn build_body(block: &Block, params: &[Param], chunk_names: Vec<String>) -> Body {
    let mut ctx = Ctx {
        nodes: Vec::new(),
        call_args: Vec::new(),
        chunk_names,
    };
    let mut scope: Vec<(String, i64)> = params
        .iter()
        .enumerate()
        .map(|(i, p)| (param_name(p).to_string(), i as i64))
        .collect();
    let mut next_slot = params.len() as i64;
    let root = flatten_block(block, &mut scope, &mut next_slot, &mut ctx);
    Body {
        nodes: ctx.nodes,
        call_args: ctx.call_args,
        root,
    }
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
        20 => Op::Not,
        21 => Op::Call((operand % 65536) as u16, (operand / 65536) as u8),
        22 => Op::Dup,
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

/// Drive the codegen; return its emitted ops, the constant pool it built, and the
/// local-frame size (`local_count`) it computed.
fn run_codegen(body: &Body, param_count: usize) -> (Vec<Op>, Vec<i64>, i64) {
    let src = std::fs::read_to_string("compiler/kel/codegen.kel").expect("read codegen.kel");
    let m = compile_src(&src);
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify codegen.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(body.root))
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
    for (k, &node) in body.call_args.iter().enumerate() {
        vm.set_shared(&mut shared, CALL_ARGS + k, Value::Int(node))
            .expect("call_arg");
    }
    vm.set_shared(&mut shared, PARAM_COUNT, Value::Int(param_count as i64))
        .expect("param_count");

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
    // Phase 3: the local-frame size the stage computed.
    let local_count = next_word(&mut vm, &mut shared);
    (ops, pool, local_count)
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
        // Nested if in the else branch.
        (
            "fn main(a: Word) -> Word { if a < 5 { 1 } else { if a < 10 { 2 } else { 3 } } }",
            7,
            Int(2),
        ),
        // let inside each if-branch: slots assigned monotonically (y=1, z=2),
        // local_count=3. The block-in-branch case the LetIn unification unlocks.
        (
            "fn main(a: Word) -> Word { if a < 5 { let y = a + 1; y } else { let z = a - 1; z } }",
            3,
            Int(4),
        ),
        // let outside, then an if whose branches each bind their own local.
        (
            "fn main(a: Word) -> Word { let x = a + 1; if x < 5 { let y = x + 1; y } else { let z = x - 1; z } }",
            10,
            Int(10),
        ),
        // Unary not: operand ops then a single Not.
        ("fn main(a: Word) -> bool { not (a < 5) }", 3, Bool(false)),
        ("fn main(a: Word) -> bool { not (a < 5) }", 10, Bool(true)),
        // Function calls: args pushed left-to-right, then Call(chunk, count).
        (
            "fn inc(x: Word) -> Word { x + 1 } fn main(a: Word) -> Word { inc(a) }",
            41,
            Int(42),
        ),
        (
            "fn add(x: Word, y: Word) -> Word { x + y } fn main(a: Word) -> Word { add(a, 2) }",
            40,
            Int(42),
        ),
        // Nested call: inner result is the outer argument.
        (
            "fn inc(x: Word) -> Word { x + 1 } fn main(a: Word) -> Word { inc(inc(a)) }",
            40,
            Int(42),
        ),
        // Call bound in a let, then used.
        (
            "fn inc(x: Word) -> Word { x + 1 } fn main(a: Word) -> Word { let b = inc(a); b + 1 }",
            40,
            Int(42),
        ),
        // Short-circuit andalso: left, Dup, If, PopN(1), right, Else, EndIf.
        (
            "fn main(a: Word) -> bool { a < 5 andalso a > 1 }",
            3,
            Bool(true),
        ),
        (
            "fn main(a: Word) -> bool { a < 5 andalso a > 1 }",
            10,
            Bool(false),
        ),
        (
            "fn main(a: Word) -> bool { a < 5 andalso a > 1 }",
            0,
            Bool(false),
        ),
        // Short-circuit orelse: adds a Not after the Dup.
        (
            "fn main(a: Word) -> bool { a < 5 orelse a > 100 }",
            3,
            Bool(true),
        ),
        (
            "fn main(a: Word) -> bool { a < 5 orelse a > 100 }",
            50,
            Bool(false),
        ),
        (
            "fn main(a: Word) -> bool { a < 5 orelse a > 100 }",
            200,
            Bool(true),
        ),
    ];
    for &(src, arg, expected) in cases {
        let program = parse(&tokenize(src).expect("lex")).expect("parse");
        let reference = compile_src(src);
        let idx = main_index(&reference);
        let reference_ops = reference.chunks[idx].ops.clone();
        let chunk_names: Vec<String> = reference.chunks.iter().map(|c| c.name.clone()).collect();

        let main_fn = program
            .functions
            .iter()
            .find(|f| f.name == "main")
            .expect("main fn");
        let body = build_body(&main_fn.body, &main_fn.params, chunk_names);

        // The stage resolves its own If/Else targets and emits its own local_count,
        // so the emitted stream plus local_count is a complete logical chunk body.
        let (emitted, pool, local_count) = run_codegen(&body, main_fn.params.len());
        assert_eq!(
            emitted, reference_ops,
            "emitted ops must match Rust for `{src}`"
        );
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "emitted local_count must match Rust for `{src}`"
        );

        // Build the module from the stage's own ops, constant pool, and local_count.
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
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
