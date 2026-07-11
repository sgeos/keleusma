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
//! against the Rust compiler, and runs it. Increment 27 adds statement-form
//! expressions: a bare call or a statement-form `if` (both `Stmt::Expr`) lowers to
//! the expression ops then `PopN(1)`; an `if` without `else` synthesizes a Unit
//! else so both branches produce a unit value.

use keleusma::Arena;
use keleusma::ast::{
    BinOp, Block, ConstInitializer, DataVisibility, Expr, FunctionCategory, Iterable, Literal,
    Param, Pattern, Stmt, UnaryOp,
};
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
const FOR_PARTS: usize = 321;
const PARAM_COUNT: usize = 385;

struct Node {
    kind: i64,
    arg: i64,
    lhs: i64,
    rhs: i64,
}

/// Flattening context: the node forest, the packed call-argument node indices, the
/// callee chunk names (for resolving a call target to a chunk index), and the data
/// slot names (for resolving a `d.field` access to its data-layout slot index).
struct Ctx {
    nodes: Vec<Node>,
    call_args: Vec<i64>,
    for_parts: Vec<i64>,
    chunk_names: Vec<String>,
    data_slots: Vec<String>,
    /// `const data` scalar fields by `name.field` -> value. A read of one of these
    /// inlines to a constant, exactly as the reference compiler does, and never
    /// occupies a runtime data slot.
    const_data: Vec<(String, i64)>,
}

/// If `data_name.field` is a scalar `const data` field, return its value.
fn const_data_value(ctx: &Ctx, data_name: &str, field: &str) -> Option<i64> {
    let key = format!("{data_name}.{field}");
    ctx.const_data
        .iter()
        .find(|(k, _)| k == &key)
        .map(|(_, v)| *v)
}

/// Append a node to the forest and return its index.
fn node(ctx: &mut Ctx, kind: i64, arg: i64, lhs: i64, rhs: i64) -> i64 {
    ctx.nodes.push(Node {
        kind,
        arg,
        lhs,
        rhs,
    });
    (ctx.nodes.len() - 1) as i64
}

/// Resolve a `data.field` reference to its slot index in the module data layout.
fn data_slot(ctx: &Ctx, data_name: &str, field: &str) -> i64 {
    let slot_name = format!("{data_name}.{field}");
    ctx.data_slots
        .iter()
        .position(|n| n == &slot_name)
        .unwrap_or_else(|| panic!("no data slot named `{slot_name}`")) as i64
}

/// Resolve a `data.field[..]` array reference to its element-0 slot (base) and its
/// length, from the per-element slot names `data.field[0]`, `data.field[1]`, ...
fn array_base_len(ctx: &Ctx, data_name: &str, field: &str) -> (i64, i64) {
    let prefix = format!("{data_name}.{field}[");
    let base =
        ctx.data_slots
            .iter()
            .position(|n| n.starts_with(&prefix))
            .unwrap_or_else(|| panic!("no array data slot with prefix `{prefix}`")) as i64;
    let len = ctx
        .data_slots
        .iter()
        .filter(|n| n.starts_with(&prefix))
        .count() as i64;
    (base, len)
}

/// A flattened function body: the flattening context and the body's block root.
struct Body {
    nodes: Vec<Node>,
    call_args: Vec<i64>,
    for_parts: Vec<i64>,
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
                BinOp::Band => (3, 12),
                BinOp::Bor => (3, 13),
                BinOp::Bxor => (3, 14),
                other => panic!(
                    "increment handles arithmetic, comparisons, booleans, bitwise, got {other:?}"
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
            // Node layout for an If (kind 4): arg = cond, lhs = then, rhs = else.
            // Each branch is a full block (it may bind its own locals). A
            // statement-form `if` without an `else` synthesizes a Unit else block,
            // so both branches produce a unit value.
            let cond = flatten(condition, scope, next_slot, ctx);
            let t = flatten_block(then_block, scope, next_slot, ctx);
            let el = match else_block {
                Some(eb) => flatten_block(eb, scope, next_slot, ctx),
                None => node(ctx, 20, 0, 0, 0),
            };
            ctx.nodes.push(Node {
                kind: 4,
                arg: cond,
                lhs: t,
                rhs: el,
            });
            (ctx.nodes.len() - 1) as i64
        }
        Expr::UnaryOp { op, operand, .. } if matches!(op, UnaryOp::Not | UnaryOp::Neg) => {
            // UnaryNot (kind 6) / UnaryNeg (kind 10): operand in lhs.
            let kind = match op {
                UnaryOp::Not => 6,
                UnaryOp::Neg => 10,
                _ => unreachable!(),
            };
            let operand = flatten(operand, scope, next_slot, ctx);
            ctx.nodes.push(Node {
                kind,
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
        Expr::FieldAccess { object, field, .. } => {
            let data_name = match object.as_ref() {
                Expr::Ident { name, .. } => name,
                other => panic!("increment handles data field access only, got {other:?}"),
            };
            // A `const data` field read inlines to a constant (Literal, kind 1);
            // a `private`/`shared` field read is a DataRead (kind 11) of its slot.
            match const_data_value(ctx, data_name, field) {
                Some(value) => node(ctx, 1, value, 0, 0),
                None => {
                    let slot = data_slot(ctx, data_name, field);
                    node(ctx, 11, slot, 0, 0)
                }
            }
        }
        Expr::ArrayIndex { object, index, .. } => {
            // A `d.arr[i]` read. IndexRead (kind 13): arg = base + len*65536,
            // lhs = index. The object is a `d.arr` field access.
            let (data_name, field) = match object.as_ref() {
                Expr::FieldAccess {
                    object: inner,
                    field,
                    ..
                } => match inner.as_ref() {
                    Expr::Ident { name, .. } => (name, field),
                    other => panic!("increment handles data array access only, got {other:?}"),
                },
                other => panic!("increment handles data array access only, got {other:?}"),
            };
            let (base, len) = array_base_len(ctx, data_name, field);
            let index_node = flatten(index, scope, next_slot, ctx);
            ctx.nodes.push(Node {
                kind: 13,
                arg: base + len * 65536,
                lhs: index_node,
                rhs: 0,
            });
            (ctx.nodes.len() - 1) as i64
        }
        other => {
            panic!("increment handles literal/local/binop/if/not/call/data-read, got {other:?}")
        }
    }
}

/// Flatten a block into a cons-list of statements ending in the tail expression,
/// and return its root node index. Each statement becomes a link: a `let` becomes a
/// `LetIn` (kind 5, slot is a fresh local), a data assignment becomes a
/// `DataAssignIn` (kind 12, slot is the data-layout slot). `let`s are assigned the
/// next monotonic slot and leave the scope on exit; slots are never reused.
fn flatten_block(
    block: &Block,
    scope: &mut Vec<(String, i64)>,
    next_slot: &mut i64,
    ctx: &mut Ctx,
) -> i64 {
    let mark = scope.len();
    // Each entry is (node kind, slot, value node): kind 5 LetIn (local slot) or
    // kind 12 DataAssignIn (data slot).
    let mut stmts: Vec<(i64, i64, i64)> = Vec::new();
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
                stmts.push((5, slot, value));
            }
            Stmt::DataFieldAssign {
                data_name,
                field,
                value,
                ..
            } => {
                let value_node = flatten(value, scope, next_slot, ctx);
                let slot = data_slot(ctx, data_name, field);
                stmts.push((12, slot, value_node));
            }
            Stmt::DataFieldIndexAssign {
                data_name,
                field,
                indices,
                value,
                ..
            } => {
                assert_eq!(
                    indices.len(),
                    1,
                    "increment handles single-dimension array assignment"
                );
                let value_node = flatten(value, scope, next_slot, ctx);
                let index_node = flatten(&indices[0], scope, next_slot, ctx);
                // IndexStore (kind 15): sequences value then index.
                ctx.nodes.push(Node {
                    kind: 15,
                    arg: 0,
                    lhs: value_node,
                    rhs: index_node,
                });
                let store_node = (ctx.nodes.len() - 1) as i64;
                let (base, len) = array_base_len(ctx, data_name, field);
                // IndexAssignIn (kind 14): arg = base + len*65536, value = store node.
                stmts.push((14, base + len * 65536, store_node));
            }
            Stmt::For(fs) => {
                let (start_expr, limit_expr) = match &fs.iterable {
                    Iterable::Range(s, l) => (s.as_ref(), l.as_ref()),
                    other => panic!("increment handles range for-loops only, got {other:?}"),
                };
                // The loop variable and the limit are two monotonic locals (i first).
                let i_slot = *next_slot;
                *next_slot += 1;
                let lim_slot = *next_slot;
                *next_slot += 1;
                // The bounds are evaluated before the loop, with i not yet in scope.
                let start_node = flatten(start_expr, scope, next_slot, ctx);
                let limit_node = flatten(limit_expr, scope, next_slot, ctx);
                // i is in scope for the condition, body, and increment.
                scope.push((fs.var.clone(), i_slot));
                // Synthetic condition `i >= limit` (BinOp GtEq, operator code 11).
                let i_a = node(ctx, 2, i_slot, 0, 0);
                let lim_a = node(ctx, 2, lim_slot, 0, 0);
                let cond_node = node(ctx, 3, 11, i_a, lim_a);
                let body_node = flatten_block(&fs.body, scope, next_slot, ctx);
                // Synthetic increment `i + 1` (BinOp Add, operator code 1).
                let i_b = node(ctx, 2, i_slot, 0, 0);
                let one = node(ctx, 1, 1, 0, 0);
                let incr_node = node(ctx, 3, 1, i_b, one);
                scope.pop();
                // Reserve the 7-word for_parts entry: i_slot, limit slot, start,
                // limit, condition, body, increment. The continuation is threaded
                // by the cons-list fold (via rhs), not stored here.
                let fp_start = ctx.for_parts.len() as i64;
                ctx.for_parts.push(i_slot);
                ctx.for_parts.push(lim_slot);
                ctx.for_parts.push(start_node);
                ctx.for_parts.push(limit_node);
                ctx.for_parts.push(cond_node);
                ctx.for_parts.push(body_node);
                ctx.for_parts.push(incr_node);
                // ForIn (kind 16): arg = for_parts entry start; lhs unused.
                stmts.push((16, fp_start, 0));
            }
            Stmt::Expr(expr) => {
                // A bare expression statement (a call for effect or a
                // statement-form `if`): ExprStmtIn (kind 21), lhs = expression.
                let expr_node = flatten(expr, scope, next_slot, ctx);
                stmts.push((21, 0, expr_node));
            }
            other => panic!("increment handles let/data-assign/for/expr statements, got {other:?}"),
        }
    }
    // A statement-only block (no tail expression) has the unit value, emitted as a
    // Unit node (PushImmediate(0)); this is the body of a `for` loop.
    let tail = match &block.tail_expr {
        Some(t) => flatten(t, scope, next_slot, ctx),
        None => node(ctx, 20, 0, 0, 0),
    };
    // Fold the statements into a cons-list from the innermost (tail) outward.
    let mut cont = tail;
    for &(kind, slot, value) in stmts.iter().rev() {
        ctx.nodes.push(Node {
            kind,
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
fn build_body(
    block: &Block,
    params: &[Param],
    chunk_names: Vec<String>,
    data_slots: Vec<String>,
    const_data: Vec<(String, i64)>,
) -> Body {
    let mut ctx = Ctx {
        nodes: Vec::new(),
        call_args: Vec::new(),
        for_parts: Vec::new(),
        chunk_names,
        data_slots,
        const_data,
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
        for_parts: ctx.for_parts,
        root,
    }
}

fn decode_op(w: i64) -> Op {
    let (tag, operand) = (w % 64, w / 64);
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
        23 => Op::CheckedNeg,
        24 => Op::BitAnd,
        25 => Op::BitOr,
        26 => Op::BitXor,
        27 => Op::GetData(operand as u16),
        28 => Op::SetData(operand as u16),
        29 => Op::GetDataIndexed((operand % 65536) as u16, (operand / 65536) as u16),
        30 => Op::SetDataIndexed((operand % 65536) as u16, (operand / 65536) as u16),
        31 => Op::Loop(operand as u16),
        32 => Op::BreakIf(operand as u16),
        33 => Op::EndLoop(operand as u16),
        34 => Op::PushImmediate(operand as u8),
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
    for (k, &part) in body.for_parts.iter().enumerate() {
        vm.set_shared(&mut shared, FOR_PARTS + k, Value::Int(part))
            .expect("for_part");
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
        // Unary negation: CheckedNeg then PopN(2).
        ("fn main(a: Word) -> Word { -a }", 5, Int(-5)),
        ("fn main(a: Word) -> Word { 3 - -a }", 4, Int(7)),
        // Per-limb bitwise operators: single-op, no PopN.
        ("fn main(a: Word) -> Word { a band 6 }", 5, Int(4)),
        ("fn main(a: Word) -> Word { a bor 1 }", 4, Int(5)),
        ("fn main(a: Word) -> Word { a bxor 3 }", 5, Int(6)),
        // Scalar data read/write: SetData then GetData.
        (
            "private data d { x: Word } fn main(v: Word) -> Word { d.x = v; d.x }",
            42,
            Int(42),
        ),
        // Two data fields, written then read and combined.
        (
            "private data d { a: Word, b: Word } fn main(v: Word) -> Word { d.a = v; d.b = v + 1; d.a + d.b }",
            20,
            Int(41),
        ),
        // Data write interleaved with a let, and a read in the tail.
        (
            "private data d { x: Word } fn main(v: Word) -> Word { let y = v * 2; d.x = y; d.x + 1 }",
            20,
            Int(41),
        ),
        // Array indexed write then read (constant index).
        (
            "private data d { xs: [Word; 4] } fn main(v: Word) -> Word { d.xs[2] = v; d.xs[2] }",
            42,
            Int(42),
        ),
        // Array indexed write/read with a runtime index expression.
        (
            "private data d { xs: [Word; 4] } fn main(i: Word) -> Word { d.xs[i] = i + 10; d.xs[i] }",
            3,
            Int(13),
        ),
        // Array element used in arithmetic in the tail.
        (
            "private data d { xs: [Word; 4] } fn main(v: Word) -> Word { d.xs[0] = v; d.xs[1] = v + 1; d.xs[0] + d.xs[1] }",
            20,
            Int(41),
        ),
        // for loop over a constant range accumulating into a scalar data field.
        (
            "private data d { s: Word } fn main() -> Word { for i in 0..4 { d.s = d.s + i; } d.s }",
            0,
            Int(6),
        ),
        // for loop writing each array element by its index.
        (
            "private data d { xs: [Word; 4] } fn main() -> Word { for i in 0..4 { d.xs[i] = i * 2; } d.xs[3] }",
            0,
            Int(6),
        ),
        // Nested-ish: a let before the loop, the loop accumulating into the field.
        (
            "private data d { s: Word } fn main(v: Word) -> Word { let base = v; for i in 0..3 { d.s = d.s + base; } d.s }",
            10,
            Int(30),
        ),
        // const data read: inlines to the field's literal value.
        (
            "const data cd { k: Word = 7 } fn main(v: Word) -> Word { v + cd.k }",
            35,
            Int(42),
        ),
        // for loop bounded by a const data field: statically bounded, so it runs.
        (
            "const data cd { n: Word = 4 } private data d { s: Word } fn main() -> Word { for i in 0..cd.n { d.s = d.s + i; } d.s }",
            0,
            Int(6),
        ),
        // bare call statement (expression statement): its value is discarded.
        (
            "fn bump(x: Word) -> Word { x + 1 } fn main(v: Word) -> Word { bump(v); bump(v); v + 2 }",
            40,
            Int(42),
        ),
        // statement-form if without else, for effect.
        (
            "private data d { s: Word } fn main(v: Word) -> Word { if v > 0 { d.s = 42; } d.s }",
            1,
            Int(42),
        ),
        // statement-form if with else, for effect.
        (
            "private data d { s: Word } fn main(v: Word) -> Word { if v > 0 { d.s = 1; } else { d.s = 2; } d.s }",
            0,
            Int(2),
        ),
    ];
    for &(src, arg, expected) in cases {
        let program = parse(&tokenize(src).expect("lex")).expect("parse");
        let reference = compile_src(src);
        let idx = main_index(&reference);
        let reference_ops = reference.chunks[idx].ops.clone();
        let chunk_names: Vec<String> = reference.chunks.iter().map(|c| c.name.clone()).collect();
        let data_slots: Vec<String> = reference
            .data_layout
            .as_ref()
            .map(|dl| dl.slots.iter().map(|s| s.name.clone()).collect())
            .unwrap_or_default();
        // Scalar `const data` fields, resolved to their literal values.
        let const_data: Vec<(String, i64)> = program
            .data_decls
            .iter()
            .filter(|d| d.visibility == DataVisibility::Const)
            .flat_map(|d| {
                d.fields.iter().filter_map(move |f| match &f.initializer {
                    Some(ConstInitializer::Scalar(Literal::Int(n))) => {
                        Some((format!("{}.{}", d.name, f.name), *n))
                    }
                    _ => None,
                })
            })
            .collect();

        let main_fn = program
            .functions
            .iter()
            .find(|f| f.name == "main")
            .expect("main fn");
        let body = build_body(
            &main_fn.body,
            &main_fn.params,
            chunk_names,
            data_slots,
            const_data,
        );

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

/// Milestone probe: attempt to compile `codegen.kel`'s own atomic `fn`s through the
/// stage, one function body at a time, and report which compile byte-identically
/// against the Rust reference and which hit a not-yet-supported construct. This is
/// a coverage report toward self-hosting, not a pass/fail gate on full coverage; it
/// only asserts that the covered functions stay covered.
#[test]
fn self_compile_codegen_atomic_functions() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let src = std::fs::read_to_string("compiler/kel/codegen.kel").expect("read codegen.kel");
    let program = parse(&tokenize(&src).expect("lex codegen.kel")).expect("parse codegen.kel");
    let reference = compile_src(&src);
    let chunk_names: Vec<String> = reference.chunks.iter().map(|c| c.name.clone()).collect();
    let data_slots: Vec<String> = reference
        .data_layout
        .as_ref()
        .map(|dl| dl.slots.iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default();
    let const_data: Vec<(String, i64)> = program
        .data_decls
        .iter()
        .filter(|d| d.visibility == DataVisibility::Const)
        .flat_map(|d| {
            d.fields.iter().filter_map(move |f| match &f.initializer {
                Some(ConstInitializer::Scalar(Literal::Int(n))) => {
                    Some((format!("{}.{}", d.name, f.name), *n))
                }
                _ => None,
            })
        })
        .collect();

    // Silence per-attempt panic backtraces; the summary below is the report.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let mut ok: Vec<String> = Vec::new();
    let mut gaps: Vec<(String, String)> = Vec::new();
    for f in &program.functions {
        if f.category != FunctionCategory::Fn {
            gaps.push((
                f.name.clone(),
                "non-atomic (yield/loop) function".to_string(),
            ));
            continue;
        }
        let cn = chunk_names.clone();
        let ds = data_slots.clone();
        let cd = const_data.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let body = build_body(&f.body, &f.params, cn, ds, cd);
            if body.nodes.len() > 64 || body.for_parts.len() > 64 || body.call_args.len() > 64 {
                return Err(format!(
                    "exceeds the 64-slot adapter arrays ({} nodes)",
                    body.nodes.len()
                ));
            }
            let (emitted, _pool, _lc) = run_codegen(&body, f.params.len());
            let ref_ops = reference
                .chunks
                .iter()
                .find(|c| c.name == f.name)
                .map(|c| c.ops.clone())
                .unwrap_or_default();
            if emitted == ref_ops {
                Ok(())
            } else {
                Err("op stream differs from the reference".to_string())
            }
        }));
        match result {
            Ok(Ok(())) => ok.push(f.name.clone()),
            Ok(Err(reason)) => gaps.push((f.name.clone(), reason)),
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "panic".to_string());
                gaps.push((f.name.clone(), msg));
            }
        }
    }

    std::panic::set_hook(prev_hook);

    eprintln!(
        "\n=== self-compile probe: {} atomic fns compile byte-identically ===",
        ok.len()
    );
    for n in &ok {
        eprintln!("  OK   {n}");
    }
    eprintln!("=== {} functions with gaps ===", gaps.len());
    for (n, r) in &gaps {
        eprintln!("  GAP  {n}: {r}");
    }
    assert!(
        !ok.is_empty(),
        "expected at least some atomic fns of codegen.kel to self-compile"
    );
}
