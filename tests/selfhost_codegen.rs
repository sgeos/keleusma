// The self-hosted codegen is a full-width host tool, gated to a 64-bit runtime.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
// The self-hosting driver and scaffold-assembly helpers carry composite tuple types that a
// `type` alias would only scatter; allow the complexity lint for this test file.
#![allow(clippy::type_complexity)]
//! Stage 3 codegen (`compiler/kel/codegen.kel`). A throwaway adapter flattens the
//! reference's block into the shared-data node arrays and statement metadata, the
//! Keleusma stage walks it recursion-free, interns each literal into its own
//! deduplicating constant pool, and emits the ops followed by the pool. The host
//! builds the module from the stage's ops and pool, checks structural equality
//! against the Rust compiler, and runs it. Increment 27 adds statement-form
//! expressions: a bare call or a statement-form `if` (both `Stmt::Expr`) lowers to
//! the expression ops then `PopN(1)`; an `if` without `else` synthesizes a Unit
//! else so both branches produce a unit value.
//!
//! This file also hosts the parse-into-codegen bridge (see `reconstruct_body`),
//! which replaces the throwaway adapter with the real parser: lexer.kel tokenizes
//! the source, parse.kel emits the body's postorder node records, the host rebuilds
//! the codegen forest from that stream, and codegen.kel emits the ops. For the
//! arithmetic node kinds this whole self-hosted path is byte-identical to the Rust
//! compiler; later increments extend the reconstruction to the remaining kinds.

use keleusma::Arena;
use keleusma::ast::{
    BinOp, Block, ConstInitializer, DataVisibility, Expr, FunctionCategory, FunctionDef, Iterable,
    Literal, Param, Pattern, Stmt, UnaryOp,
};
use keleusma::bytecode::{ConstValue, Module, NewCompositeOperand, Op, StructField, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::value_layout::{CompositeKind, ScalarKind};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `ast` block's field order in codegen.kel
// (one slot per scalar, arrays contiguous). root=0, then the four length-512 node
// arrays (`kinds`/`args`/`lhs`/`rhs`, sized for the stage's largest own function),
// then the length-64 `call_args`/`for_parts`/`match_parts` side arrays, then
// param_count.
const KINDS: usize = 1;
const ARGS: usize = 1 + 1024;
const LHS: usize = 1 + 1024 * 2;
const RHS: usize = 1 + 1024 * 3;
const CALL_ARGS: usize = 1 + 1024 * 4;
const FOR_PARTS: usize = 1 + 1024 * 4 + 64;
const MATCH_PARTS: usize = 1 + 1024 * 4 + 64 * 2;
const LIMIT_PARTS: usize = 1 + 1024 * 4 + 64 * 3;
const HEAD_PARTS: usize = 1 + 1024 * 4 + 64 * 4;
const PARAM_COUNT: usize = 1 + 1024 * 4 + 64 * 5;
const CATEGORY: usize = 1 + 1024 * 4 + 64 * 5 + 1;

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
    match_parts: Vec<i64>,
    limit_parts: Vec<i64>,
    head_parts: Vec<i64>,
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
    match_parts: Vec<i64>,
    limit_parts: Vec<i64>,
    head_parts: Vec<i64>,
    category: i64,
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
        Expr::Cast { expr, .. } => {
            // A `Byte as Word` widening: Cast node (kind 26), operand in lhs. The stages
            // only cast a Byte data-read to Word, which lowers to a single ByteToWord op.
            let operand = flatten(expr, scope, next_slot, ctx);
            ctx.nodes.push(Node {
                kind: 26,
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
            // A `d.arr[i]` read. IndexRead (kind 13): arg = base + len*2^24,
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
                arg: base + len * 16_777_216,
                lhs: index_node,
                rhs: 0,
            });
            (ctx.nodes.len() - 1) as i64
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            // The reference lowers a `match` by stashing the scrutinee in a fresh
            // temp local, so declare it right after the scrutinee (mirroring
            // `compile_expr`'s `declare_local("__match")`) and before the arm
            // results. Slots are monotonic, matching the reference's high-water
            // `local_count`.
            let scrut = flatten(scrutinee, scope, next_slot, ctx);
            let temp = *next_slot;
            *next_slot += 1;
            // Split the literal arms from the single trailing wildcard.
            let mut lit_arms: Vec<(i64, i64)> = Vec::new();
            let mut wildcard: Option<i64> = None;
            for arm in arms {
                assert!(
                    arm.guard.is_none(),
                    "increment handles unguarded match arms only"
                );
                match &arm.pattern {
                    Pattern::Literal(Literal::Int(v), _) => {
                        let lit_node = node(ctx, 1, *v, 0, 0);
                        let res_node = flatten(&arm.expr, scope, next_slot, ctx);
                        lit_arms.push((lit_node, res_node));
                    }
                    Pattern::Wildcard(_) => {
                        assert!(
                            wildcard.is_none(),
                            "increment handles a single wildcard arm"
                        );
                        wildcard = Some(flatten(&arm.expr, scope, next_slot, ctx));
                    }
                    other => panic!(
                        "increment handles integer-literal and wildcard match arms, got {other:?}"
                    ),
                }
            }
            let wc = wildcard.expect("match needs a wildcard arm");
            // The match_parts entry: temp slot at [0], wildcard result at [1], then
            // one (literal node, result node) pair per literal arm from [2].
            let base = ctx.match_parts.len() as i64;
            ctx.match_parts.push(temp);
            ctx.match_parts.push(wc);
            for (lit_node, res_node) in &lit_arms {
                ctx.match_parts.push(*lit_node);
                ctx.match_parts.push(*res_node);
            }
            let arm_count = lit_arms.len() as i64;
            // MatchIn (kind 22): arg = match_parts entry start, lhs = scrutinee,
            // rhs = literal-arm count.
            ctx.nodes.push(Node {
                kind: 22,
                arg: base,
                lhs: scrut,
                rhs: arm_count,
            });
            (ctx.nodes.len() - 1) as i64
        }
        Expr::Yield { value, .. } => {
            // YieldExpr (kind 24): the yielded expression in lhs.
            let inner = flatten(value, scope, next_slot, ctx);
            node(ctx, 24, 0, inner, 0)
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
                // IndexAssignIn (kind 14): arg = base + len*2^24, value = store node.
                stmts.push((14, base + len * 16_777_216, store_node));
            }
            Stmt::For(fs) if fs.limit.is_some() => {
                // A `for i in start..end limit CAP { body }` in the bare form.
                // Mirrors the reference `compile_for_limit` local order: the loop
                // variable, the end, the counter, the cap, and the outcome are
                // five consecutive locals (the variable first, allocated after
                // the start ops and before the end ops).
                assert!(
                    fs.on_arms.is_empty(),
                    "the self-host stage supports the bare `limit` form only"
                );
                let (start_expr, end_expr) = match &fs.iterable {
                    Iterable::Range(s, e) => (s.as_ref(), e.as_ref()),
                    other => panic!("a `limit` clause requires a range loop, got {other:?}"),
                };
                let cap = match &fs.limit {
                    Some(Expr::Literal {
                        value: Literal::Int(n),
                        ..
                    }) => *n,
                    Some(Expr::FieldAccess { object, field, .. }) => match object.as_ref() {
                        Expr::Ident { name, .. } => const_data_value(ctx, name, field)
                            .unwrap_or_else(|| panic!("`limit` must be a const-data field")),
                        other => panic!("`limit` must be a constant, got {other:?}"),
                    },
                    other => panic!("`limit` must be a compile-time constant, got {other:?}"),
                };
                let start_node = flatten(start_expr, scope, next_slot, ctx);
                let var = *next_slot;
                *next_slot += 1;
                let end_node = flatten(end_expr, scope, next_slot, ctx);
                let end_slot = *next_slot;
                *next_slot += 1;
                let ctr = *next_slot;
                *next_slot += 1;
                let cap_slot = *next_slot;
                *next_slot += 1;
                let oc = *next_slot;
                *next_slot += 1;
                scope.push((fs.var.clone(), var));
                let body_node = flatten_block(&fs.body, scope, next_slot, ctx);
                scope.pop();
                let cap_node = node(ctx, 1, cap, 0, 0);
                let zero_node = node(ctx, 1, 0, 0, 0);
                let one_node = node(ctx, 1, 1, 0, 0);
                let two_node = node(ctx, 1, 2, 0, 0);
                // The 12-word limit_parts entry: the five slots, then the start,
                // end, and body nodes, then the cap/0/1/2 literal nodes.
                let lp_start = ctx.limit_parts.len() as i64;
                for v in [
                    var, end_slot, ctr, cap_slot, oc, start_node, end_node, body_node, cap_node,
                    zero_node, one_node, two_node,
                ] {
                    ctx.limit_parts.push(v);
                }
                // ForLimit (kind 23): arg = limit_parts entry start; lhs unused.
                stmts.push((23, lp_start, 0));
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
    category: i64,
    chunk_names: Vec<String>,
    data_slots: Vec<String>,
    const_data: Vec<(String, i64)>,
) -> Body {
    let mut ctx = Ctx {
        nodes: Vec::new(),
        call_args: Vec::new(),
        for_parts: Vec::new(),
        match_parts: Vec::new(),
        limit_parts: Vec::new(),
        head_parts: Vec::new(),
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
        match_parts: ctx.match_parts,
        limit_parts: ctx.limit_parts,
        head_parts: ctx.head_parts,
        category,
        root,
    }
}

/// Flatten a multiheaded function (its heads share a name), building the MultiHead
/// dispatch (category 3). Each head copies the parameters into fresh consecutive
/// locals (mirroring the reference's per-head param binding), then flattens its
/// optional `when` guard and its `yield`-expression body in the scope of those
/// copies. The 4-word head_parts entry is (param-copy start slot, guarded flag,
/// guard node, body node).
fn build_multihead(
    heads: &[&FunctionDef],
    params: &[Param],
    chunk_names: Vec<String>,
    data_slots: Vec<String>,
    const_data: Vec<(String, i64)>,
) -> Body {
    let mut ctx = Ctx {
        nodes: Vec::new(),
        call_args: Vec::new(),
        for_parts: Vec::new(),
        match_parts: Vec::new(),
        limit_parts: Vec::new(),
        head_parts: Vec::new(),
        chunk_names,
        data_slots,
        const_data,
    };
    let pc = params.len() as i64;
    // The parameters occupy slots 0..pc; each head's copies and body lets follow.
    let mut next_slot = pc;
    let mut entries: Vec<(i64, i64, i64, i64)> = Vec::new();
    for head in heads {
        let param_start = next_slot;
        next_slot += pc;
        // The parameters resolve to this head's copies for its guard and body.
        let mut scope: Vec<(String, i64)> = params
            .iter()
            .enumerate()
            .map(|(i, p)| (param_name(p).to_string(), param_start + i as i64))
            .collect();
        let (guarded, guard_node) = match &head.guard {
            Some(g) => (1, flatten(g, &mut scope, &mut next_slot, &mut ctx)),
            None => (0, 0),
        };
        let body_node = flatten_block(&head.body, &mut scope, &mut next_slot, &mut ctx);
        entries.push((param_start, guarded, guard_node, body_node));
    }
    for (ps, g, gn, bn) in &entries {
        ctx.head_parts.push(*ps);
        ctx.head_parts.push(*g);
        ctx.head_parts.push(*gn);
        ctx.head_parts.push(*bn);
    }
    // MultiHead (kind 25): arg = head_parts base (0), rhs = head count.
    let root = node(&mut ctx, 25, 0, 0, heads.len() as i64);
    Body {
        nodes: ctx.nodes,
        call_args: ctx.call_args,
        for_parts: ctx.for_parts,
        match_parts: ctx.match_parts,
        limit_parts: ctx.limit_parts,
        head_parts: ctx.head_parts,
        category: 3,
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
        27 => Op::GetData(operand as u32),
        28 => Op::SetData(operand as u32),
        // Base and length pack with a 2^24 radix so a data slot or length beyond
        // 65535 (a shared segment over 64 KB) does not spill base into length.
        29 => Op::GetDataIndexed((operand % 16777216) as u32, (operand / 16777216) as u32),
        30 => Op::SetDataIndexed((operand % 16777216) as u32, (operand / 16777216) as u32),
        31 => Op::Loop(operand as u16),
        32 => Op::BreakIf(operand as u16),
        33 => Op::EndLoop(operand as u16),
        34 => Op::PushImmediate(operand as u8),
        // Match control flow. The stage gives these their own op-word tags so
        // `emit_op` can backpatch a bare `If`/`EndIf` and multiple unconditional
        // `Break`s, but they decode to the same reference ops as the structured
        // forms (an `mif` is an `Op::If`, an `mloop` an `Op::Loop`, and so on).
        35 => Op::Break(operand as u16),
        36 => Op::Trap(operand as u16),
        37 => Op::If(operand as u16),
        38 => Op::EndIf,
        39 => Op::Loop(operand as u16),
        40 => Op::EndLoop(operand as u16),
        // The `for ... limit` counter header is a conditional break to the loop
        // exit; its own stage tag lets `emit_op` set its exit target while
        // preserving the `BreakIf` decode.
        41 => Op::BreakIf(operand as u16),
        // The `yield`/`loop` machinery.
        42 => Op::Yield,
        43 => Op::Stream,
        44 => Op::Reset,
        45 => Op::ByteToWord,
        // A flat struct construction: the operand packs count + byte_size*65536. The
        // composite kind is Struct this increment (the only lowered composite).
        46 => Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Struct,
            count: (operand % 65536) as u16,
            byte_size: (operand / 65536) as u16,
        }),
        // A flat struct field read: the operand packs offset + kind_tag*65536.
        47 => Op::GetField(StructField::Flat {
            offset: (operand % 65536) as u16,
            kind: match operand / 65536 {
                0 => ScalarKind::Unit,
                1 => ScalarKind::Bool,
                2 => ScalarKind::Byte,
                3 => ScalarKind::Int,
                4 => ScalarKind::Fixed,
                5 => ScalarKind::Float,
                6 => ScalarKind::Text,
                7 => ScalarKind::Opaque,
                other => panic!("bad scalar kind tag {other}"),
            },
        }),
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
    for (k, &part) in body.match_parts.iter().enumerate() {
        vm.set_shared(&mut shared, MATCH_PARTS + k, Value::Int(part))
            .expect("match_part");
    }
    for (k, &part) in body.limit_parts.iter().enumerate() {
        vm.set_shared(&mut shared, LIMIT_PARTS + k, Value::Int(part))
            .expect("limit_part");
    }
    for (k, &part) in body.head_parts.iter().enumerate() {
        vm.set_shared(&mut shared, HEAD_PARTS + k, Value::Int(part))
            .expect("head_part");
    }
    vm.set_shared(&mut shared, PARAM_COUNT, Value::Int(param_count as i64))
        .expect("param_count");
    vm.set_shared(&mut shared, CATEGORY, Value::Int(body.category))
        .expect("category");

    // Phase 1: ops until Return.
    let mut ops = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..65536 {
        match st {
            VmState::Yielded(Value::Int(w)) => {
                if w != 0 {
                    let op = decode_op(w);
                    // The op stream's terminator depends on the function category:
                    // an `fn` ends in `Return`, a `loop` in `Reset`, and a
                    // multiheaded dispatch in `Trap(NoMatchingHead=1)`. A
                    // multihead's per-head `Return`s are interior ops, so it must
                    // read past them to the final trap.
                    let done = match body.category {
                        2 => op == Op::Reset,
                        3 => op == Op::Trap(1),
                        _ => op == Op::Return,
                    };
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

// The first slice of struct codegen: a flat all-scalar struct construction `P { x: 1, y: 2 }`
// lowers to its field expressions in declaration order then a NewComposite(Flat{Struct,
// count, byte_size}), byte-identically to the reference compiler. Driven from a hand-built
// AST because parse.kel does not yet emit a struct-construction node in a body; the struct's
// packed byte size (16 = two 8-byte Words) is supplied by the layout, exactly as an earlier
// stage will supply it once the parser produces the node.
#[test]
fn struct_construction_lowers_to_newcomposite() {
    let m = compile_src("struct P { x: Word, y: Word }\nfn make() -> P { P { x: 1, y: 2 } }");
    let make = m
        .chunks
        .iter()
        .find(|c| c.name == "make")
        .expect("make chunk");

    let body = Body {
        nodes: vec![
            Node {
                kind: 1,
                arg: 1,
                lhs: 0,
                rhs: 0,
            }, // Literal 1 (field x)
            Node {
                kind: 1,
                arg: 2,
                lhs: 0,
                rhs: 0,
            }, // Literal 2 (field y)
            Node {
                kind: 27,
                arg: 16,
                lhs: 0,
                rhs: 2,
            }, // StructInit: byte_size 16, start 0, count 2
        ],
        call_args: vec![0, 1],
        for_parts: Vec::new(),
        match_parts: Vec::new(),
        limit_parts: Vec::new(),
        head_parts: Vec::new(),
        category: 1, // fn
        root: 2,
    };
    let (ops, _pool, _lc) = run_codegen(&body, 0);
    assert_eq!(
        ops, make.ops,
        "struct construction lowers like the reference"
    );
    assert!(
        ops.iter().any(|op| matches!(
            op,
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size: 16,
            })
        )),
        "emits a flat 2-field 16-byte struct NewComposite"
    );
}

// The second struct-codegen slice: a flat struct field read `p.f` lowers to the object ops
// then `GetField(Flat{offset, kind})`, byte-identically to the reference. Two fields at
// distinct byte offsets (x at 0, y at 8 on a 64-bit target) confirm the packed offset+kind
// operand. Driven from a hand-built AST; the baked offset and ScalarKind are supplied on
// the node by the layout, as an earlier stage will supply them.
#[test]
fn struct_field_access_lowers_to_getfield() {
    let m = compile_src(
        "struct P { x: Word, y: Word }\n\
         fn getx(p: P) -> Word { p.x }\n\
         fn gety(p: P) -> Word { p.y }",
    );
    let getx = m.chunks.iter().find(|c| c.name == "getx").expect("getx");
    let gety = m.chunks.iter().find(|c| c.name == "gety").expect("gety");

    // `p.x`: field at byte offset 0, read as an Int (ScalarKind tag 3).
    let bx = Body {
        nodes: vec![
            Node {
                kind: 2,
                arg: 0,
                lhs: 0,
                rhs: 0,
            }, // Local slot 0 (p)
            Node {
                kind: 28,
                arg: 3 * 65536,
                lhs: 0,
                rhs: 0,
            }, // FieldAccess offset 0, kind Int
        ],
        call_args: Vec::new(),
        for_parts: Vec::new(),
        match_parts: Vec::new(),
        limit_parts: Vec::new(),
        head_parts: Vec::new(),
        category: 1,
        root: 1,
    };
    let (ops_x, _p, _l) = run_codegen(&bx, 1);
    assert_eq!(ops_x, getx.ops, "p.x lowers like the reference");
    assert!(ops_x.iter().any(|op| matches!(
        op,
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int
        })
    )));

    // `p.y`: field at byte offset 8 (after the 8-byte x field).
    let by = Body {
        nodes: vec![
            Node {
                kind: 2,
                arg: 0,
                lhs: 0,
                rhs: 0,
            }, // Local slot 0 (p)
            Node {
                kind: 28,
                arg: 8 + 3 * 65536,
                lhs: 0,
                rhs: 0,
            }, // FieldAccess offset 8, kind Int
        ],
        call_args: Vec::new(),
        for_parts: Vec::new(),
        match_parts: Vec::new(),
        limit_parts: Vec::new(),
        head_parts: Vec::new(),
        category: 1,
        root: 1,
    };
    let (ops_y, _p, _l) = run_codegen(&by, 1);
    assert_eq!(ops_y, gety.ops, "p.y lowers like the reference");
    assert!(ops_y.iter().any(|op| matches!(
        op,
        Op::GetField(StructField::Flat {
            offset: 8,
            kind: ScalarKind::Int
        })
    )));
}

// -- Struct layout computation ------------------------------------------------
//
// The third struct-codegen slice: compute the flat packed layout of an all-scalar struct
// -- its total byte size (for the NewComposite operand) and per field the byte offset and
// ScalarKind (for the GetField operand) -- from the field types the parser captures. This
// is the bridge between the parser's captured struct fields and the codegen operands. It is
// prototyped host-side (a Rust helper) as the scaffold assembly and analyze driver were,
// to be ported to a `.kel` layout pass later. Fields pack with no alignment padding, exactly
// as the reference `value_layout` sums preceding field sizes. Scalar fields only this slice.

/// The ScalarKind and byte size of a scalar field type on the 64-bit host target
/// (word and float are eight bytes). Mirrors `value_layout::ScalarKind::size_in_bytes`.
fn scalar_kind_size(ty: &str) -> (ScalarKind, u16) {
    match ty {
        "Word" => (ScalarKind::Int, 8),
        "Byte" => (ScalarKind::Byte, 1),
        "Bool" => (ScalarKind::Bool, 1),
        "Float" => (ScalarKind::Float, 8),
        other => panic!("struct layout: unsupported scalar field type `{other}`"),
    }
}

/// The flat packed layout of an all-scalar struct: its total byte size, and per field in
/// declaration order the field's byte offset and ScalarKind.
fn struct_scalar_layout(field_types: &[&str]) -> (u16, Vec<(u16, ScalarKind)>) {
    let mut offset = 0u16;
    let mut fields = Vec::with_capacity(field_types.len());
    for ty in field_types {
        let (kind, size) = scalar_kind_size(ty);
        fields.push((offset, kind));
        offset += size;
    }
    (offset, fields) // `offset` is now the total packed byte size
}

/// The byte size the reference baked into a construction's `NewComposite` op.
fn ref_struct_byte_size(m: &Module, ctor: &str) -> u16 {
    let c = m
        .chunks
        .iter()
        .find(|c| c.name == ctor)
        .expect("ctor chunk");
    c.ops
        .iter()
        .find_map(|op| match op {
            Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) => Some(*byte_size),
            _ => None,
        })
        .expect("a NewComposite op")
}

/// The (offset, kind) the reference baked into a getter's `GetField` op.
fn ref_field_offset_kind(m: &Module, getter: &str) -> (u16, ScalarKind) {
    let c = m
        .chunks
        .iter()
        .find(|c| c.name == getter)
        .expect("getter chunk");
    c.ops
        .iter()
        .find_map(|op| match op {
            Op::GetField(StructField::Flat { offset, kind }) => Some((*offset, *kind)),
            _ => None,
        })
        .expect("a GetField op")
}

// The computed layout matches the offsets and byte size the reference bakes. An all-Word
// struct validates the byte size (via a construction) and the equal-stride offsets (via
// getters); a mixed Byte/Word struct validates that offsets accumulate across differing
// field sizes (a one-byte field then a word).
#[test]
fn struct_layout_matches_the_reference_baked_layout() {
    // All-Word struct: byte size 24, offsets 0/8/16, all Int.
    let (bytes, fields) = struct_scalar_layout(&["Word", "Word", "Word"]);
    assert_eq!(bytes, 24);
    assert_eq!(
        fields,
        vec![
            (0, ScalarKind::Int),
            (8, ScalarKind::Int),
            (16, ScalarKind::Int)
        ]
    );
    let m = compile_src(
        "struct P { x: Word, y: Word, z: Word }\n\
         fn make() -> P { P { x: 1, y: 2, z: 3 } }\n\
         fn gx(p: P) -> Word { p.x }\n\
         fn gy(p: P) -> Word { p.y }\n\
         fn gz(p: P) -> Word { p.z }",
    );
    assert_eq!(bytes, ref_struct_byte_size(&m, "make"));
    assert_eq!(fields[0], ref_field_offset_kind(&m, "gx"));
    assert_eq!(fields[1], ref_field_offset_kind(&m, "gy"));
    assert_eq!(fields[2], ref_field_offset_kind(&m, "gz"));

    // Mixed Byte/Word struct: the word field starts at offset 1, right after the byte.
    let (_mbytes, mfields) = struct_scalar_layout(&["Byte", "Word"]);
    assert_eq!(mfields, vec![(0, ScalarKind::Byte), (1, ScalarKind::Int)]);
    let mm = compile_src(
        "struct M { b: Byte, w: Word }\n\
         fn gb(m: M) -> Byte { m.b }\n\
         fn gw(m: M) -> Word { m.w }",
    );
    assert_eq!(mfields[0], ref_field_offset_kind(&mm, "gb"));
    assert_eq!(mfields[1], ref_field_offset_kind(&mm, "gw"));
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
        // match over an integer scrutinee: the reference lowers it to a virtual
        // loop of GetLocal/Const/CmpEq/If tests, each arm ending in a Break, then a
        // dead Trap tail. A literal arm hit.
        (
            "fn main(input: Word) -> Word { match input { 0 => 10, 1 => 20, 2 => 30, _ => 99 } }",
            1,
            Int(20),
        ),
        // The same match, falling through every literal arm to the wildcard.
        (
            "fn main(input: Word) -> Word { match input { 0 => 10, 1 => 20, 2 => 30, _ => 99 } }",
            5,
            Int(99),
        ),
        // Arithmetic in an arm result and in the wildcard.
        (
            "fn main(a: Word) -> Word { match a { 2 => a * 10, _ => a + 5 } }",
            2,
            Int(20),
        ),
        (
            "fn main(a: Word) -> Word { match a { 2 => a * 10, _ => a + 5 } }",
            3,
            Int(8),
        ),
        // match in value position, bound in a let, then used: the temp local for the
        // scrutinee sits before the let-bound slot, matching the reference numbering.
        (
            "fn main(a: Word) -> Word { let x = match a { 7 => 1, _ => a + 100 }; x + 1 }",
            7,
            Int(2),
        ),
        (
            "fn main(a: Word) -> Word { let x = match a { 7 => 1, _ => a + 100 }; x + 1 }",
            40,
            Int(141),
        ),
        // A repeated literal across pattern and result interns to one pool entry, as
        // the reference does (the `5` pattern and the `5` addend share a slot).
        (
            "fn main(a: Word) -> Word { match a { 5 => a + 5, _ => a } }",
            5,
            Int(10),
        ),
        // Bare `for ... limit` over a runtime range: the counter header, the range
        // exit, the two increments, the boundary and break reclassifications, and
        // the dead limit trap, reproduced byte for byte. Completes within the cap.
        (
            "private data d { s: Word } \
             fn main(n: Word) -> Word { for i in 0..n limit 8 { d.s = d.s + i; } d.s }",
            3,
            Int(3),
        ),
        // The count == cap boundary completes (does not trap) at exactly the cap.
        (
            "private data d { s: Word } \
             fn main(n: Word) -> Word { for i in 0..n limit 8 { d.s = d.s + i; } d.s }",
            8,
            Int(28),
        ),
        // A limit loop whose body guards on the loop variable, the codegen idiom.
        (
            "private data d { xs: [Word; 8], s: Word } \
             fn main(n: Word) -> Word { for i in 0..n limit 8 { if i < 3 { d.s = d.s + d.xs[i]; } } d.s }",
            2,
            Int(0),
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
        // The conformance cases are all atomic `fn` bodies (category 0).
        let body = build_body(
            &main_fn.body,
            &main_fn.params,
            0,
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

/// Hard-gated multihead conformance. The multiheaded `yield` dispatch (increment
/// 32) is the newest and historically most fragile path, and its only real subject
/// so far is `emit_next` through the non-gating self-compile probe. This pins a
/// hand-written multihead in the byte-identity corpus, and deliberately uses a
/// parameter-referencing guard (`when r == 0`) rather than the data-field guards
/// `emit_next` uses, so the head parameter-copy and guard-dispatch fall-through are
/// exercised on a distinct guard shape.
#[test]
fn a_synthetic_multiheaded_function_compiles_byte_identically() {
    let src = "yield g(r: Word) -> Word when r == 0 { yield r } \
        yield g(r: Word) -> Word when r > 5 { yield r } \
        yield g(r: Word) -> Word { yield 0 } \
        loop main(r: Word) -> Word { g(r) }";
    let reference = compile_src(src);
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let chunk_names: Vec<String> = reference.chunks.iter().map(|c| c.name.clone()).collect();

    // The three `g` heads compile to a single reference chunk; drive the stage over
    // them and require the emitted ops to match it byte for byte.
    let heads: Vec<&FunctionDef> = program.functions.iter().filter(|f| f.name == "g").collect();
    assert_eq!(heads.len(), 3, "the multihead has three heads");
    let body = build_multihead(
        &heads,
        &heads[0].params,
        chunk_names,
        Vec::new(),
        Vec::new(),
    );
    let (emitted, _pool, _lc) = run_codegen(&body, heads[0].params.len());
    let ref_ops = reference
        .chunks
        .iter()
        .find(|c| c.name == "g")
        .expect("g chunk")
        .ops
        .clone();
    assert_eq!(
        emitted, ref_ops,
        "synthetic multihead ops diverged from the reference"
    );
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
    // Group functions by name (order-preserving); a multiheaded function's heads
    // share a name and are compiled together as one MultiHead dispatch.
    let mut names_ordered: Vec<String> = Vec::new();
    for f in &program.functions {
        if !names_ordered.contains(&f.name) {
            names_ordered.push(f.name.clone());
        }
    }
    for name in &names_ordered {
        let heads: Vec<&FunctionDef> = program
            .functions
            .iter()
            .filter(|f| &f.name == name)
            .collect();
        let first = heads[0];
        let cn = chunk_names.clone();
        let ds = data_slots.clone();
        let cd = const_data.clone();
        let name_c = name.clone();
        let heads_ref = &heads;
        let result = catch_unwind(AssertUnwindSafe(|| {
            // Multiheaded (any category) -> MultiHead dispatch (category 3); a
            // single atomic `fn` -> category 0; a single-headed `loop` -> the
            // Stream/Reset wrapper (category 2). A lone single-head `yield` is not
            // exercised by codegen.kel and is not yet lowered.
            let body = if heads_ref.len() > 1 {
                build_multihead(heads_ref, &first.params, cn, ds, cd)
            } else {
                match first.category {
                    FunctionCategory::Fn => build_body(&first.body, &first.params, 0, cn, ds, cd),
                    FunctionCategory::Loop if first.guard.is_none() => {
                        build_body(&first.body, &first.params, 2, cn, ds, cd)
                    }
                    _ => return Err("non-atomic single-head (yield) function".to_string()),
                }
            };
            if body.nodes.len() > 1024
                || body.for_parts.len() > 64
                || body.call_args.len() > 64
                || body.match_parts.len() > 64
                || body.head_parts.len() > 64
                || body.limit_parts.len() > 64
            {
                return Err(format!(
                    "exceeds the adapter arrays ({} nodes)",
                    body.nodes.len()
                ));
            }
            let (emitted, _pool, _lc) = run_codegen(&body, first.params.len());
            let ref_ops = reference
                .chunks
                .iter()
                .find(|c| c.name == name_c)
                .map(|c| c.ops.clone())
                .unwrap_or_default();
            if emitted == ref_ops {
                Ok(())
            } else {
                let diff = (0..emitted.len().max(ref_ops.len()))
                    .find(|&i| emitted.get(i) != ref_ops.get(i))
                    .unwrap_or(0);
                Err(format!(
                    "op stream differs at {} (emitted {:?} vs ref {:?}); lens {}/{}",
                    diff,
                    emitted.get(diff),
                    ref_ops.get(diff),
                    emitted.len(),
                    ref_ops.len()
                ))
            }
        }));
        match result {
            Ok(Ok(())) => ok.push(name.clone()),
            Ok(Err(reason)) => gaps.push((name.clone(), reason)),
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "panic".to_string());
                gaps.push((name.clone(), msg));
            }
        }
    }

    std::panic::set_hook(prev_hook);

    eprintln!(
        "\n=== self-compile probe: {} functions compile byte-identically ===",
        ok.len()
    );
    for n in &ok {
        eprintln!("  OK   {n}");
    }
    eprintln!("=== {} functions with gaps ===", gaps.len());
    for (n, r) in &gaps {
        eprintln!("  GAP  {n}: {r}");
    }
    // Regression gate. The codegen stage is fully self-hosting: every one of its
    // functions round-trips through itself byte-identically. Assert that no
    // function gaps and pin the count, so a silent partial regression (a function
    // that stops self-compiling, or one that disappears) fails the test rather than
    // passing unnoticed once attention moves to the parser. If codegen.kel gains or
    // loses a function deliberately, update EXPECTED_SELF_COMPILE below.
    // 37 as of increment 34 (added `push_struct_init` and `push_field_access` for flat
    // struct construction and field access).
    const EXPECTED_SELF_COMPILE: usize = 37;
    assert!(
        gaps.is_empty(),
        "codegen self-compile regressed; functions that no longer round-trip: {gaps:?}"
    );
    assert_eq!(
        ok.len(),
        EXPECTED_SELF_COMPILE,
        "codegen self-compile count changed (expected {EXPECTED_SELF_COMPILE}); \
         update the gate deliberately if codegen.kel changed. self-compiled: {ok:?}"
    );
}

// ---------------------------------------------------------------------------
// Parse-into-codegen bridge (arithmetic subset).
//
// The final composition step: parse.kel emits a function body as a postorder
// stream of (kind, arg) node records, and codegen.kel consumes a forest of
// (kind, arg, lhs, rhs) nodes. `reconstruct_body` rebuilds that forest from the
// postorder stream with a node-index stack -- a leaf pushes itself, an operator
// pops its children and links them -- exactly the inverse of the reference
// flatten. Driving lexer.kel then parse.kel then codegen.kel over real source and
// asserting byte-identity with the Rust compiler proves the whole self-hosted
// front-to-back path for the arithmetic node kinds; later increments extend the
// reconstruction to the control-flow, data-access, call, and yield kinds.
// ---------------------------------------------------------------------------

// Lexer `src` block slots: len(1) + bytes(65536) then the intern table.
const BR_LEX_ISTART: usize = 1 + 98304;
const BR_LEX_ILEN: usize = 1 + 98304 + 1280;
const BR_LEX_ICOUNT: usize = 1 + 98304 + 1280 + 1280;
// Parser `toks` block slots: len(1), then the packed token array (one `tok+payload*64`
// word per token), then the scalar and chunk-table inputs.
const BR_P_LEN: usize = 0;
const BR_P_PACKED: usize = 1;
const BR_P_LIMIT_ID: usize = 1 + 12288;
const BR_P_CHUNK_COUNT: usize = 1 + 12288 + 1;
const BR_P_CHUNKS: usize = 1 + 12288 + 2;
const BR_P_REQUIRE_ID: usize = 1 + 12288 + 2 + 256;

fn br_shared_word(vm: &Vm<'_, '_>, buf: &[u8], slot: usize) -> i64 {
    match vm.get_shared(buf, slot).expect("get_shared") {
        Value::Int(n) => n,
        other => panic!("expected Int at slot {slot}, got {other:?}"),
    }
}

/// Tokenize `src` with lexer.kel; return its `(tok, payload)` stream (no EOF) and
/// the id-to-spelling table recovered from the exposed intern table.
fn br_lex(src: &str) -> (Vec<(i64, i64)>, Vec<String>) {
    let bytes = src.as_bytes();
    let m =
        compile_src(&std::fs::read_to_string("compiler/kel/lexer.kel").expect("read lexer.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify lexer.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(bytes.len() as i64))
        .unwrap();
    for (i, &b) in bytes.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b)).unwrap();
    }
    let mut toks = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(bytes.len() * 4 + 16) {
        if let VmState::Yielded(Value::Int(t)) = st {
            if t == 63 {
            } else if t == 62 {
                break;
            } else {
                toks.push((t.rem_euclid(64), t.div_euclid(64)));
            }
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    let icount = br_shared_word(&vm, &shared, BR_LEX_ICOUNT) as usize;
    let mut names = Vec::with_capacity(icount);
    for id in 0..icount {
        let start = br_shared_word(&vm, &shared, BR_LEX_ISTART + id) as usize;
        let len = br_shared_word(&vm, &shared, BR_LEX_ILEN + id) as usize;
        names.push(String::from_utf8(bytes[start..start + len].to_vec()).unwrap());
    }
    (toks, names)
}

/// Function-name ids in declaration order, from a brace-depth scan of the tokens.
fn br_chunks(tokens: &[(i64, i64)]) -> Vec<i64> {
    let (mut chunks, mut depth) = (Vec::new(), 0i64);
    for w in tokens.windows(2) {
        match w[0].0 {
            2 => depth += 1,
            3 => depth -= 1,
            0 | 5 | 6 if depth == 0 && w[1].0 == 1 => chunks.push(w[1].1),
            _ => {}
        }
    }
    chunks
}

/// Drive lexer.kel then parse.kel over `src`, returning the last function's body
/// records (the postorder (kind, arg) node stream), its value parameter count, and
/// its codegen category (0 fn, 2 loop, 3 multihead, mapped from the parser's START
/// category of 1 fn / 2 yield / 3 loop). All parser inputs come from the lexer.
fn parse_function_records(src: &str) -> (Vec<(i64, i64)>, usize, i64) {
    let (tokens, names) = br_lex(src);
    let id_of = |s: &str| {
        names
            .iter()
            .position(|n| n == s)
            .map(|i| i as i64)
            .unwrap_or(-1)
    };
    let chunks = br_chunks(&tokens);

    let module = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| compile_src(&std::fs::read_to_string("compiler/kel/parse.kel").expect("read")))
        .expect("spawn")
        .join()
        .expect("join");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parse.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, BR_P_LEN, Value::Int(tokens.len() as i64))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_LIMIT_ID, Value::Int(id_of("limit")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_REQUIRE_ID, Value::Int(id_of("require")))
        .unwrap();
    vm.set_shared(
        &mut shared,
        BR_P_CHUNK_COUNT,
        Value::Int(chunks.len() as i64),
    )
    .unwrap();
    for (i, &c) in chunks.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_CHUNKS + i, Value::Int(c))
            .unwrap();
    }
    for (i, &(k, v)) in tokens.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_PACKED + i, Value::Int(k + v * 64))
            .unwrap();
    }

    // Track each function's records and value-parameter count, keeping the last
    // one at DONE. Multi-function sources (a callee then `main`) supply the whole
    // chunk table so parse.kel resolves the call, and `main` is the last function.
    let mut last: (Vec<(i64, i64)>, usize, i64) = (Vec::new(), 0, 0);
    let mut records: Vec<(i64, i64)> = Vec::new();
    let mut params = 0usize;
    // Codegen category, mapped from the parser's START category: fn 1 -> 0,
    // yield 2 -> 3 (multihead), loop 3 -> 2.
    let mut cur_cat = 0i64;
    let (mut in_body, mut in_data, mut in_enum, mut in_use) = (false, false, false, false);
    let mut in_guard = false;
    // A struct (STRUCTSTART 18), trait (19), or impl (20) declaration is skipped until its
    // END so its fields/methods are not mistaken for a function's params or body.
    let mut in_skip_decl = false;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(tokens.len() * 16 + 256) {
        if let VmState::Yielded(Value::Int(w)) = state {
            let (code, val) = (w.rem_euclid(64), w.div_euclid(64));
            if in_body {
                match code {
                    0 => {}
                    15 => in_body = false,
                    _ => records.push((code, val)),
                }
            } else if in_guard {
                in_guard = code != 15; // skip the `when` guard forest until its Done
            } else if in_data {
                in_data = code != 5;
            } else if in_enum {
                in_enum = code != 5;
            } else if in_use {
                in_use = code != 5;
            } else if in_skip_decl {
                in_skip_decl = code != 5;
            } else {
                match code {
                    1..=3 => {
                        cur_cat = match code {
                            3 => 2, // loop
                            2 => 3, // yield -> multihead
                            _ => 0, // fn
                        };
                        records = Vec::new();
                        params = 0;
                    }
                    4 => params += 1,
                    9 => in_data = true,
                    10 => in_use = true,
                    12 => in_enum = true,
                    16 => in_body = true,
                    17 => in_guard = true,
                    18 | 19 | 20 => in_skip_decl = true, // struct/trait/impl declaration
                    5 => last = (std::mem::take(&mut records), params, cur_cat),
                    15 => return last,
                    _ => {}
                }
            }
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("parse.kel did not reach DONE");
}

/// Rebuild the codegen node forest from parse.kel's postorder record stream with a
/// node-index stack, the inverse of the reference flatten. A leaf (Literal 1,
/// Local 2, Unit 20) pushes itself. A binary node pops its right then left child:
/// the operators (BinOp 3, Andalso 8, Orelse 9), and the block-fold statements
/// LetIn (5, whose `arg` is the local slot) and ExprStmt (21), whose left child is
/// the value and right child the continuation. A unary operator (Not 6, Neg 10)
/// pops its single operand into `lhs`. An If (4) pops its else, then, and cond
/// children and takes the cond node index as its `arg` (the record's `arg` is 0).
/// The root is the one node left on the stack. The data-access kinds fit the same
/// groups: a scalar DataRead (11) is a leaf carrying its slot; a DataAssign (12)
/// folds like a LetIn carrying its slot; and an IndexRead (13) is unary over the
/// index, carrying `base + len*2^24` in `arg`. A Call (7) packs `chunk + count*256`
/// in its record `arg`; it pops its `count` argument nodes, reverses them to source
/// order into the `call_args` side array, and takes that slice's start as `lhs` and
/// the count as `rhs`. An indexed write pairs an IndexStore signal (36), which
/// folds the value and index into a kind-15 node (value in lhs, index in rhs),
/// with an IndexAssign (14) that folds like a LetIn carrying `base + len*2^24`.
/// A `for .. limit` loop streams its start, end, body, and the four literals cap/
/// 0/1/2 as nodes, then five SlotRecords (32) carrying its frame slots, then a
/// ForBuild (33) that assembles the 12-word limit_parts entry (the five slots then
/// the start, end, body, cap, 0, 1, 2 node indices); the ForLimit statement (23)
/// folds into the block popping only the continuation. A `match` streams its
/// scrutinee, a (literal, result) pair per literal arm, and the wildcard result,
/// then a MatchBuild (34) packing `temp*1024 + lit_count`, which assembles the
/// match_parts entry (temp, wildcard, then the pairs) and builds the MatchIn node.
/// A YieldExpr (24) is unary, holding the yielded expression in `lhs`. This
/// increment covers the arithmetic, comparison, bitwise, unary, short-circuit,
/// block, if, scalar and indexed data-access, call, for-limit, integer-match, and
/// yield kinds; a record of any other kind is rejected until a later increment adds
/// it (the multiheaded dispatch and its head_parts remain).
fn reconstruct_body(records: &[(i64, i64)], category: i64) -> Body {
    reconstruct_body_with_structs(records, category, &[])
}

// Reconstruct a body that may contain struct constructions, resolving each StructInit's
// flat byte size from `struct_bytesizes` (indexed by struct declaration order).
fn reconstruct_body_with_structs(
    records: &[(i64, i64)],
    category: i64,
    struct_bytesizes: &[u16],
) -> Body {
    let mut nodes: Vec<Node> = Vec::new();
    let mut call_args: Vec<i64> = Vec::new();
    let mut limit_parts: Vec<i64> = Vec::new();
    let mut match_parts: Vec<i64> = Vec::new();
    let root = reconstruct_into(
        records,
        &mut nodes,
        &mut call_args,
        &mut limit_parts,
        &mut match_parts,
        struct_bytesizes,
    );
    Body {
        nodes,
        call_args,
        for_parts: Vec::new(),
        match_parts,
        limit_parts,
        head_parts: Vec::new(),
        category,
        root,
    }
}

/// Reconstruct one postorder record forest into the shared `nodes` array (and the
/// shared side arrays), returning the root node index. Node indices are absolute in
/// `nodes`, so several forests -- a multihead's per-head guards and bodies -- can be
/// reconstructed into one array and referenced by `head_parts`.
fn reconstruct_into(
    records: &[(i64, i64)],
    nodes: &mut Vec<Node>,
    call_args: &mut Vec<i64>,
    limit_parts: &mut Vec<i64>,
    match_parts: &mut Vec<i64>,
    struct_bytesizes: &[u16],
) -> i64 {
    let mut stack: Vec<i64> = Vec::new();
    let mut pending_slots: Vec<i64> = Vec::new();
    for &(kind, arg) in records {
        // A SlotRecord (32) carries a frame slot for the pending `for` loop, and a
        // ForBuild (33) assembles the 12-word limit_parts entry from the five
        // collected slots and the seven loop nodes on the stack. Neither is a node.
        if kind == 32 {
            pending_slots.push(arg);
            continue;
        }
        if kind == 33 {
            let two = stack.pop().expect("for two");
            let one = stack.pop().expect("for one");
            let zero = stack.pop().expect("for zero");
            let cap = stack.pop().expect("for cap");
            let body = stack.pop().expect("for body");
            let end = stack.pop().expect("for end");
            let start = stack.pop().expect("for start");
            assert_eq!(pending_slots.len(), 5, "a for loop has five SlotRecords");
            limit_parts.extend([
                pending_slots[0],
                pending_slots[1],
                pending_slots[2],
                pending_slots[3],
                pending_slots[4],
                start,
                end,
                body,
                cap,
                zero,
                one,
                two,
            ]);
            pending_slots.clear();
            continue;
        }
        let idx = match kind {
            1 | 2 | 11 | 20 => {
                nodes.push(Node {
                    kind,
                    arg,
                    lhs: 0,
                    rhs: 0,
                });
                (nodes.len() - 1) as i64
            }
            3 | 5 | 8 | 9 | 12 | 14 | 21 => {
                let r = stack.pop().expect("binary rhs");
                let l = stack.pop().expect("binary lhs");
                nodes.push(Node {
                    kind,
                    arg,
                    lhs: l,
                    rhs: r,
                });
                (nodes.len() - 1) as i64
            }
            36 => {
                // IndexStore signal: fold the value and index already on the stack
                // into a kind-15 IndexStore node (value in lhs, index in rhs). The
                // record kind is 36 because 15 collides with the record DONE marker;
                // the node it produces is a real kind-15.
                let value = stack.pop().expect("index-store value");
                let index = stack.pop().expect("index-store index");
                nodes.push(Node {
                    kind: 15,
                    arg: 0,
                    lhs: value,
                    rhs: index,
                });
                (nodes.len() - 1) as i64
            }
            23 => {
                // ForLimit statement: arg is the limit_parts entry start (already
                // assembled at the ForBuild). It folds into the block popping only
                // the continuation; its lhs is unused.
                let cont = stack.pop().expect("forlimit continuation");
                nodes.push(Node {
                    kind: 23,
                    arg,
                    lhs: 0,
                    rhs: cont,
                });
                (nodes.len() - 1) as i64
            }
            34 => {
                // MatchBuild signal: arg packs `temp*1024 + lit_count`. The stack
                // holds the scrutinee, then a (literal, result) pair per literal arm,
                // then the wildcard result on top. Assemble the match_parts entry
                // (temp, wildcard, then the literal/result pairs) and build the
                // MatchIn node (kind 22): arg = entry start, lhs = scrutinee, rhs =
                // literal-arm count. Unlike the for loop, the build signal itself
                // produces the value node.
                let temp = arg.div_euclid(1024);
                let lit_count = arg.rem_euclid(1024);
                let wc = stack.pop().expect("match wildcard result");
                let mut pairs: Vec<(i64, i64)> = Vec::new();
                for _ in 0..lit_count {
                    let res = stack.pop().expect("match arm result");
                    let lit = stack.pop().expect("match arm literal");
                    pairs.push((lit, res));
                }
                pairs.reverse();
                let scrut = stack.pop().expect("match scrutinee");
                let base = match_parts.len() as i64;
                match_parts.push(temp);
                match_parts.push(wc);
                for (lit, res) in &pairs {
                    match_parts.push(*lit);
                    match_parts.push(*res);
                }
                nodes.push(Node {
                    kind: 22,
                    arg: base,
                    lhs: scrut,
                    rhs: lit_count,
                });
                (nodes.len() - 1) as i64
            }
            6 | 10 | 13 | 24 | 26 => {
                let c = stack.pop().expect("unary operand");
                nodes.push(Node {
                    kind,
                    arg,
                    lhs: c,
                    rhs: 0,
                });
                (nodes.len() - 1) as i64
            }
            4 => {
                let el = stack.pop().expect("if else");
                let th = stack.pop().expect("if then");
                let cond = stack.pop().expect("if cond");
                nodes.push(Node {
                    kind: 4,
                    arg: cond,
                    lhs: th,
                    rhs: el,
                });
                (nodes.len() - 1) as i64
            }
            7 => {
                // Call: arg packs `chunk + count*256`. Pop the count argument node
                // indices (last-pushed is the last argument), reverse them to source
                // order, and append them to the packed call_args side array. The Call
                // node's lhs is that slice's start and rhs the argument count.
                let chunk = arg.rem_euclid(256);
                let count = arg.div_euclid(256);
                let mut popped: Vec<i64> =
                    (0..count).map(|_| stack.pop().expect("call arg")).collect();
                popped.reverse();
                let args_start = call_args.len() as i64;
                call_args.extend(popped);
                nodes.push(Node {
                    kind: 7,
                    arg: chunk,
                    lhs: args_start,
                    rhs: count,
                });
                (nodes.len() - 1) as i64
            }
            27 => {
                // StructInit (the layout bridge): arg packs `struct_index * 1024 + count`.
                // Resolve the struct's flat byte size from its declaration index, pop the
                // field-value nodes into call_args in source order, and build the codegen
                // StructInit node whose arg is the byte size, lhs the field slice start,
                // rhs the field count -- exactly what `push_struct_init` in codegen.kel
                // consumes.
                let index = arg.div_euclid(1024);
                let count = arg.rem_euclid(1024);
                let byte_size = *struct_bytesizes
                    .get(index as usize)
                    .unwrap_or_else(|| panic!("no layout for struct index {index}"));
                let mut popped: Vec<i64> = (0..count)
                    .map(|_| stack.pop().expect("struct field value"))
                    .collect();
                popped.reverse();
                let args_start = call_args.len() as i64;
                call_args.extend(popped);
                nodes.push(Node {
                    kind: 27,
                    arg: byte_size as i64,
                    lhs: args_start,
                    rhs: count,
                });
                (nodes.len() - 1) as i64
            }
            other => panic!("reconstruct_body: unsupported node kind {other}"),
        };
        stack.push(idx);
    }
    assert_eq!(stack.len(), 1, "exactly one root node remains");
    stack[0]
}

// The whole self-hosted front-to-back path for the arithmetic node kinds: lexer.kel
// tokenizes real source, parse.kel produces the body's postorder records, the host
// rebuilds the forest, and codegen.kel emits the ops -- which must be byte-identical
// to the Rust compiler's, and the built module must compute the right value.
#[test]
fn parse_into_codegen_arithmetic_matches_the_reference() {
    // (source, input, expected) -- all `fn` bodies returning Word.
    let cases: &[(&str, i64, i64)] = &[
        ("fn main(x: Word) -> Word { x + 1 }", 41, 42),
        ("fn main(x: Word) -> Word { x * 2 + 1 }", 20, 41),
        ("fn main(x: Word) -> Word { x * 2 + 2 }", 20, 42),
        ("fn main(a: Word) -> Word { a - 1 }", 43, 42),
        ("fn main(a: Word) -> Word { a / 2 }", 84, 42),
        ("fn main(a: Word) -> Word { a % 5 }", 47, 2),
        ("fn main(a: Word) -> Word { (a - 2) / 2 }", 86, 42),
        ("fn main(a: Word) -> Word { -a }", 5, -5),
        ("fn main(a: Word) -> Word { 3 - -a }", 4, 7),
        ("fn main(a: Word) -> Word { a band 6 }", 5, 4),
        ("fn main(a: Word) -> Word { a bor 1 }", 4, 5),
        ("fn main(a: Word) -> Word { a bxor 3 }", 5, 6),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The layout bridge, end to end: lexer.kel tokenizes, parse.kel emits the construction
// records (a StructInit carrying the struct's declaration index), the host layout bridge
// resolves the flat byte size from that index, and codegen.kel emits the ops -- byte-
// identical to the reference for a flat all-scalar struct construction. This is the seam
// that joins the construction PARSER (parse.kel) to the construction CODEGEN (codegen.kel's
// NewComposite lowering); the byte-size resolution is host-side for now, to be ported to a
// .kel layout pass in the self-hosted reconstruct stage later.
#[test]
fn parse_into_codegen_struct_construction_matches_the_reference() {
    let src = "struct P { x: Word, y: Word }\n\
               fn make() -> P { P { x: 1, y: 2 } }";
    // Struct flat byte sizes in declaration order (P is index 0), from the layout helper.
    let (p_bytes, _p_fields) = struct_scalar_layout(&["Word", "Word"]);
    let struct_bytesizes = vec![p_bytes];

    let (records, param_count, category) = parse_function_records(src);
    // parse.kel emits [(Literal 1), (Literal 2), (StructInit index 0 * 1024 + count 2)].
    assert_eq!(
        records,
        vec![(1, 1), (1, 2), (27, 2)],
        "make's construction records"
    );

    let body = reconstruct_body_with_structs(&records, category, &struct_bytesizes);
    let (ops, _pool, _lc) = run_codegen(&body, param_count);

    let reference = compile_src(src);
    let make = reference
        .chunks
        .iter()
        .find(|c| c.name == "make")
        .expect("make chunk");
    assert_eq!(
        ops, make.ops,
        "self-hosted struct construction matches the reference"
    );
    assert!(
        ops.iter().any(|op| matches!(
            op,
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: 2,
                byte_size: 16,
            })
        )),
        "emits a flat 2-field 16-byte struct NewComposite"
    );
}

// The bridge over the block and control-flow node kinds: `let` bindings (LetIn),
// bare expression statements (ExprStmt), the statement-only-block Unit, and
// `if`/`else` including an else-less form and nesting. Same self-hosted path and
// byte-identity check as the arithmetic bridge.
#[test]
fn parse_into_codegen_blocks_and_control_flow_match_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // let bindings: value ops then SetLocal, tail reads the slot.
        (
            "fn main(input: Word) -> Word { let x = input + 1; x * 2 }",
            20,
            42,
        ),
        (
            "fn main(input: Word) -> Word { let x = input + 1; let y = x + x; y }",
            20,
            42,
        ),
        // if/else as the tail, both branches taken across inputs.
        (
            "fn main(a: Word) -> Word { if a < 5 { 2 } else { 3 } }",
            3,
            2,
        ),
        (
            "fn main(a: Word) -> Word { if a < 5 { 2 } else { 3 } }",
            10,
            3,
        ),
        // arithmetic in the branches.
        (
            "fn main(a: Word) -> Word { if a < 5 { a + 1 } else { a - 1 } }",
            10,
            9,
        ),
        // nested if in the else branch.
        (
            "fn main(a: Word) -> Word { if a < 5 { 1 } else { if a < 10 { 2 } else { 3 } } }",
            7,
            2,
        ),
        // a let bound inside each branch (monotonic slots).
        (
            "fn main(a: Word) -> Word { if a < 5 { let y = a + 1; y } else { let z = a - 1; z } }",
            3,
            4,
        ),
        // a let outside, then an if whose branches each bind a local.
        (
            "fn main(a: Word) -> Word { let x = a + 1; if x < 5 { let y = x + 1; y } else { let z = x - 1; z } }",
            10,
            10,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The bridge over the scalar and indexed-read data-access kinds: a scalar DataRead
// (11) and DataAssign (12), and an indexed IndexRead (13) with a constant or
// runtime index. parse.kel resolves each field to its data-layout slot and bakes it
// into the record, so the reconstruction needs no slot table. Same self-hosted path
// and byte-identity check. (Indexed writes -- the IndexStore signal and IndexAssign
// -- are a later increment.)
#[test]
fn parse_into_codegen_data_access_reads_match_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // scalar write then read.
        (
            "private data d { x: Word } fn main(v: Word) -> Word { d.x = v; d.x }",
            42,
            42,
        ),
        // two scalar fields, written then read and combined.
        (
            "private data d { a: Word, b: Word } fn main(v: Word) -> Word { d.a = v; d.b = v + 1; d.a + d.b }",
            20,
            41,
        ),
        // a scalar write interleaved with a let, read in the tail.
        (
            "private data d { x: Word } fn main(v: Word) -> Word { let y = v * 2; d.x = y; d.x + 1 }",
            20,
            41,
        ),
        // indexed read with a runtime index; the array is zero-initialized, so the
        // read yields 0 and the result is the scalar field's value. The scalar write
        // satisfies the rule that a `private data` block must be mutated somewhere.
        (
            "private data d { xs: [Word; 4], n: Word } fn main(i: Word) -> Word { d.n = i; d.xs[i] + d.n }",
            3,
            3,
        ),
        // indexed read with a constant index.
        (
            "private data d { xs: [Word; 4], n: Word } fn main(v: Word) -> Word { d.n = v; d.xs[2] + d.n }",
            9,
            9,
        ),
        // an indexed read used in arithmetic in the tail.
        (
            "private data d { xs: [Word; 4], n: Word } fn main(i: Word) -> Word { d.n = 7; d.xs[i] + d.n }",
            1,
            7,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The bridge over function calls: `main` calls a callee, so parse.kel resolves the
// call target against the chunk table (recovered from the token stream) and packs
// the chunk index and argument count into the Call record; the host rebuilds the
// call_args side array. Same self-hosted path and byte-identity check, now over a
// multi-function source where `main` is the last function.
#[test]
fn parse_into_codegen_calls_match_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // single-argument call.
        (
            "fn inc(x: Word) -> Word { x + 1 } fn main(a: Word) -> Word { inc(a) }",
            41,
            42,
        ),
        // two-argument call, arguments pushed left to right.
        (
            "fn add(x: Word, y: Word) -> Word { x + y } fn main(a: Word) -> Word { add(a, 2) }",
            40,
            42,
        ),
        // nested call: the inner result is the outer argument.
        (
            "fn inc(x: Word) -> Word { x + 1 } fn main(a: Word) -> Word { inc(inc(a)) }",
            40,
            42,
        ),
        // a call whose argument is an expression, plus arithmetic around it.
        (
            "fn dbl(x: Word) -> Word { x * 2 } fn main(a: Word) -> Word { dbl(a + 1) + 1 }",
            20,
            43,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The bridge over the indexed write, completing the data-access family: an
// IndexStore signal folds the value and index into a kind-15 node, and IndexAssign
// folds it into the block. Same self-hosted path and byte-identity check.
#[test]
fn parse_into_codegen_indexed_writes_match_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // constant index write then read.
        (
            "private data d { xs: [Word; 4] } fn main(v: Word) -> Word { d.xs[2] = v; d.xs[2] }",
            42,
            42,
        ),
        // runtime index write (value an expression) then read.
        (
            "private data d { xs: [Word; 4] } fn main(i: Word) -> Word { d.xs[i] = i + 10; d.xs[i] }",
            3,
            13,
        ),
        // two element writes then a combined read.
        (
            "private data d { xs: [Word; 4] } fn main(v: Word) -> Word { d.xs[0] = v; d.xs[1] = v + 1; d.xs[0] + d.xs[1] }",
            20,
            41,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The bridge over the bounded `for .. limit` loop: the first construct whose
// reconstruction consumes SlotRecords and a build signal to assemble a fixed-size
// side-array entry (limit_parts). Same self-hosted path and byte-identity check.
#[test]
fn parse_into_codegen_for_limit_matches_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // runtime range with a static cap, accumulating into a scalar field.
        (
            "private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 8 { d.s = d.s + i; } d.s }",
            4,
            6,
        ),
        // writing each array element by its index in the loop.
        (
            "private data d { xs: [Word; 8] } fn main(n: Word) -> Word { for i in 0..n limit 8 { d.xs[i] = i * 2; } d.xs[3] }",
            4,
            6,
        ),
        // a let before the loop, the loop accumulating a constant.
        (
            "private data d { s: Word } fn main(v: Word) -> Word { let base = v; for i in 0..3 limit 8 { d.s = d.s + base; } d.s }",
            10,
            30,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The bridge over the integer `match` expression: the MatchBuild signal assembles
// the match_parts entry (temp slot, wildcard, then the literal/result pairs) and
// builds the MatchIn node. Same self-hosted path and byte-identity check.
#[test]
fn parse_into_codegen_match_matches_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // two literal arms and a wildcard; the matched arm is taken.
        (
            "fn main(n: Word) -> Word { match n { 0 => 10, 1 => 20, _ => 30 } }",
            1,
            20,
        ),
        // the wildcard arm is taken and reads the scrutinee.
        (
            "fn main(n: Word) -> Word { match n { 0 => 100, _ => n + 1 } }",
            5,
            6,
        ),
        // a match as a let value, its result used in the tail.
        (
            "fn main(n: Word) -> Word { let r = match n { 1 => 100, _ => n }; r + 1 }",
            1,
            101,
        ),
        // arithmetic in an arm result.
        (
            "fn main(n: Word) -> Word { match n { 2 => n * 10, _ => 0 } }",
            2,
            20,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The bridge over `yield` in a `loop` function: YieldExpr is unary, and the loop
// category (2) makes run_codegen read to the terminating Reset. The whole
// self-hosted path over a coroutine, byte-identical to the Rust compiler; a loop
// yields rather than finishing, so the yielded value is checked.
#[test]
fn parse_into_codegen_yield_in_a_loop_matches_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        ("loop main(r: Word) -> Word { yield r + 1 }", 41, 42),
        ("loop main(r: Word) -> Word { yield r * 2 }", 21, 42),
        // a yield of a larger arithmetic expression.
        ("loop main(r: Word) -> Word { yield r * 2 + r }", 14, 42),
        // a let before the yield.
        (
            "loop main(r: Word) -> Word { let x = r + 1; yield x * 2 }",
            20,
            42,
        ),
        // `yield` of a block-form if: the yield marker must span the whole if, so the
        // Yield op follows the branch. Regression for the fixed marker-drain bug.
        (
            "loop main(r: Word) -> Word { yield if r < 5 { 1 } else { 2 } }",
            3,
            1,
        ),
        (
            "loop main(r: Word) -> Word { yield if r < 5 { 1 } else { 2 } }",
            9,
            2,
        ),
        // `yield` of a match: the match's scrutinee and arm drains must stop at the
        // yield marker, which the block-tail drain then emits as the YieldExpr.
        (
            "loop main(r: Word) -> Word { yield match r { 0 => 100, _ => r + 1 } }",
            5,
            6,
        ),
        // `yield` of a call: the call closes on its `)`, then the yield spans it.
        (
            "fn inc(x: Word) -> Word { x + 1 } loop main(r: Word) -> Word { yield inc(r) }",
            41,
            42,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        assert_eq!(category, 2, "loop maps to codegen category 2");
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Yielded(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// Harden the bridge on combined and nested constructs: the individual kinds each
// pass, but their compositions exercise the reconstruction's stack discipline under
// interaction. All compose cleanly and are byte-identical to the reference -- an if
// inside a for body, a call in a for body, a match over a let value, a call whose
// argument is a match, a call inside an if branch, and a data read as a match arm
// result. The match-as-call-argument case (`f(match x { .. })`) is a regression for
// the fixed marker-drain bug in parse.kel: a sub-expression drain used to pop the
// enclosing call marker as a spurious BinOp, so the Call was never emitted; it now
// stops at the marker, which the closing `)` consumes into the Call.
#[test]
fn parse_into_codegen_combined_constructs_match_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // an if inside a for-limit body, accumulating conditionally.
        (
            "private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 8 { if i > 1 { d.s = d.s + i; } } d.s }",
            4,
            5,
        ),
        // a call in a for-limit body.
        (
            "fn add1(x: Word) -> Word { x + 1 } private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 8 { d.s = add1(d.s); } d.s }",
            4,
            4,
        ),
        // a match over a let value with a wildcard reading it.
        (
            "fn main(n: Word) -> Word { let c = n + 1; match c { 3 => 100, _ => c * 2 } }",
            1,
            4,
        ),
        // a call whose argument is a match expression. Regression for the fixed
        // marker-drain bug: the call marker must survive the match's scrutinee and
        // arm drains so the Call node is emitted after the match.
        (
            "fn inc(x: Word) -> Word { x + 1 } fn main(n: Word) -> Word { inc(match n { 0 => 10, _ => 20 }) }",
            5,
            21,
        ),
        // a call inside an if branch.
        (
            "fn dbl(x: Word) -> Word { x * 2 } fn main(n: Word) -> Word { if n < 10 { dbl(n) } else { n } }",
            5,
            10,
        ),
        // a data read as a match arm result.
        (
            "private data d { k: Word } fn main(n: Word) -> Word { d.k = 7; match n { 0 => d.k, _ => d.k + n } }",
            3,
            10,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);

        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );

        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        match vm.call(&[Value::Int(input)]).expect("call built") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

/// A parsed function from the record stream: parser category (1 fn, 2 yield, 3 loop),
/// name id, value-parameter count, and the postorder records of its `when` guard (empty
/// when unguarded) and its body.
struct ParsedFn {
    cat: i64,
    name: i64,
    params: usize,
    // The type-name id of each value parameter (from the header PTYPE records) and of
    // the return (the RETTYPE record), for the driver's own chunk-signature assembly.
    param_types: Vec<i64>,
    return_type: i64,
    guard: Vec<(i64, i64)>,
    body: Vec<(i64, i64)>,
}

/// Drive lexer.kel then parse.kel over `src` and return every function it yields, each
/// with its guard and body records, plus the interned-name table. Multiheaded functions
/// appear as several same-named entries in declaration order.
fn parse_functions(src: &str) -> (Vec<ParsedFn>, Vec<String>, Vec<(i64, i64)>, Vec<(i64, i64)>) {
    let (tokens, names) = br_lex(src);
    let id_of = |s: &str| {
        names
            .iter()
            .position(|n| n == s)
            .map(|i| i as i64)
            .unwrap_or(-1)
    };
    // The chunk table must be in the module's actual chunk order so a resolved call index
    // matches the assembled module. The Rust compiler orders chunks by name, not by
    // declaration, so the table (host-supplied resolved-reference data) is taken from the
    // reference module's chunk order rather than the token scan's declaration order.
    let reference_chunks = compile_src(src);
    let chunks: Vec<i64> = reference_chunks
        .chunks
        .iter()
        .map(|c| id_of(&c.name))
        .collect();
    let module = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| compile_src(&std::fs::read_to_string("compiler/kel/parse.kel").expect("read")))
        .expect("spawn")
        .join()
        .expect("join");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parse.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, BR_P_LEN, Value::Int(tokens.len() as i64))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_LIMIT_ID, Value::Int(id_of("limit")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_REQUIRE_ID, Value::Int(id_of("require")))
        .unwrap();
    vm.set_shared(
        &mut shared,
        BR_P_CHUNK_COUNT,
        Value::Int(chunks.len() as i64),
    )
    .unwrap();
    for (i, &c) in chunks.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_CHUNKS + i, Value::Int(c))
            .unwrap();
    }
    for (i, &(k, v)) in tokens.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_PACKED + i, Value::Int(k + v * 64))
            .unwrap();
    }

    let mut fns: Vec<ParsedFn> = Vec::new();
    let mut cur: Option<ParsedFn> = None;
    // Every data block's header records (DSTART, then PARAM/PTYPE/ASIZE per field, then
    // END), concatenated in declaration order, for the driver's own data-layout assembly.
    let mut data_records: Vec<(i64, i64)> = Vec::new();
    // Every enum's header records (ENUMSTART, then EVARIANT/EDISC per variant, then END),
    // for the driver's own enum-layout assembly.
    let mut enum_records: Vec<(i64, i64)> = Vec::new();
    let (mut in_body, mut in_guard, mut in_data, mut in_enum, mut in_use) =
        (false, false, false, false, false);
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(tokens.len() * 16 + 256) {
        if let VmState::Yielded(Value::Int(w)) = state {
            let (code, val) = (w.rem_euclid(64), w.div_euclid(64));
            if in_body {
                match code {
                    0 => {}
                    15 => in_body = false,
                    _ => cur.as_mut().unwrap().body.push((code, val)),
                }
            } else if in_guard {
                match code {
                    0 => {}
                    15 => in_guard = false,
                    _ => cur.as_mut().unwrap().guard.push((code, val)),
                }
            } else if in_data {
                if code == 5 {
                    data_records.push((5, 0));
                    in_data = false;
                } else if code != 0 {
                    data_records.push((code, val));
                }
            } else if in_enum {
                if code == 5 {
                    enum_records.push((5, 0));
                    in_enum = false;
                } else if code != 0 {
                    enum_records.push((code, val));
                }
            } else if in_use {
                in_use = code != 5;
            } else {
                match code {
                    1..=3 => {
                        cur = Some(ParsedFn {
                            cat: code,
                            name: val,
                            params: 0,
                            param_types: Vec::new(),
                            return_type: 0,
                            guard: Vec::new(),
                            body: Vec::new(),
                        })
                    }
                    4 => cur.as_mut().unwrap().params += 1,
                    6 => cur.as_mut().unwrap().param_types.push(val),
                    7 => cur.as_mut().unwrap().return_type = val,
                    9 => {
                        in_data = true;
                        data_records.push((9, val));
                    }
                    10 => in_use = true,
                    12 => {
                        in_enum = true;
                        enum_records.push((12, val));
                    }
                    16 => in_body = true,
                    17 => in_guard = true,
                    5 => fns.push(cur.take().unwrap()),
                    15 => return (fns, names, data_records, enum_records),
                    _ => {}
                }
            }
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("parse.kel did not reach DONE");
}

/// Combine the heads of a multiheaded function into the codegen Body codegen.kel's
/// multihead dispatch consumes. Each head's guard and body are reconstructed into one
/// shared node array, its frame slots remapped by the head's `param_start` -- the heads'
/// parameter copies occupy `(i+1)*pc .. (i+2)*pc` (this covers heads whose only frame
/// slots are the parameters; heads with their own lets or loops need the additional
/// per-head temp offset, a later refinement). Each head contributes a four-word
/// head_parts entry (param_start, guarded, guard node, body node), and a MultiHead node
/// (kind 25, rhs = head count) is the root; the category is 3.
fn build_multihead_bridge(heads: &[&ParsedFn], pc: usize) -> Body {
    let mut nodes: Vec<Node> = Vec::new();
    let mut call_args: Vec<i64> = Vec::new();
    let mut limit_parts: Vec<i64> = Vec::new();
    let mut match_parts: Vec<i64> = Vec::new();
    let mut head_parts: Vec<i64> = Vec::new();
    let mut entries: Vec<(i64, i64, i64, i64)> = Vec::new();
    for (i, h) in heads.iter().enumerate() {
        let base = ((i + 1) * pc) as i64;
        // Remap frame-slot references (Local) into this head's parameter window.
        let remap = |recs: &[(i64, i64)]| -> Vec<(i64, i64)> {
            recs.iter()
                .map(|&(k, a)| if k == 2 { (k, a + base) } else { (k, a) })
                .collect()
        };
        let (guarded, guard_node) = if h.guard.is_empty() {
            (0i64, 0i64)
        } else {
            let g = remap(&h.guard);
            let root = reconstruct_into(
                &g,
                &mut nodes,
                &mut call_args,
                &mut limit_parts,
                &mut match_parts,
                &[],
            );
            (1i64, root)
        };
        let b = remap(&h.body);
        let body_node = reconstruct_into(
            &b,
            &mut nodes,
            &mut call_args,
            &mut limit_parts,
            &mut match_parts,
            &[],
        );
        entries.push((base, guarded, guard_node, body_node));
    }
    for (ps, g, gn, bn) in &entries {
        head_parts.push(*ps);
        head_parts.push(*g);
        head_parts.push(*gn);
        head_parts.push(*bn);
    }
    nodes.push(Node {
        kind: 25,
        arg: 0,
        lhs: 0,
        rhs: heads.len() as i64,
    });
    let root = (nodes.len() - 1) as i64;
    Body {
        nodes,
        call_args,
        for_parts: Vec::new(),
        match_parts,
        limit_parts,
        head_parts,
        category: 3,
        root,
    }
}

/// Reconstruct the multiheaded function named `head_name` in `src` from the record stream
/// and assert its emitted ops are byte-identical to the Rust compiler's single chunk.
fn assert_multihead_matches(src: &str, head_name: &str) {
    let (fns, names, _, _) = parse_functions(src);
    let heads: Vec<&ParsedFn> = fns
        .iter()
        .filter(|f| names[f.name as usize] == head_name)
        .collect();
    assert!(heads.len() >= 2, "a multihead has at least two heads");
    let pc = heads[0].params;
    let body = build_multihead_bridge(&heads, pc);
    let (emitted, _pool, _lc) = run_codegen(&body, pc);
    let reference = compile_src(src);
    let ops = reference
        .chunks
        .iter()
        .find(|c| c.name == head_name)
        .expect("head chunk")
        .ops
        .clone();
    assert_eq!(emitted, ops, "multihead ops for `{src}`");
}

// The final reconstruction kind: the multiheaded dispatch. parse.kel emits each head as a
// separate declaration with its guard and body forests; the host groups the same-named
// heads, reconstructs them into one node array with per-head slot remapping, and assembles
// the head_parts side array with a MultiHead root. The emitted ops must be byte-identical
// to the Rust compiler's single chunk.
#[test]
fn parse_into_codegen_multihead_matches_the_reference() {
    // Parameter-referencing guards and simple yield bodies.
    assert_multihead_matches(
        "yield g(r: Word) -> Word when r == 0 { yield r } \
         yield g(r: Word) -> Word when r > 5 { yield r } \
         yield g(r: Word) -> Word { yield 0 } \
         loop main(r: Word) -> Word { g(r) }",
        "g",
    );
    // Data-field guards and data-read bodies, the shape codegen.kel's own `emit_next`
    // dispatch uses: the data reads resolve to shared data slots (not frame slots), so
    // they pass through the per-head slot remapping untouched.
    assert_multihead_matches(
        "shared data st { pos: Word, len: Word } \
         yield emit(r: Word) -> Word when st.pos < st.len { yield st.pos } \
         yield emit(r: Word) -> Word { yield 0 } \
         loop main(r: Word) -> Word { emit(r) }",
        "emit",
    );
}

/// Self-host-compile a whole program: drive the pipeline over every function, reconstruct
/// each into its codegen Body (grouping same-named heads into one multihead), run
/// codegen.kel, and splice the self-hosted ops, constant pool, and local_count into the
/// reference module chunk of that name. Native chunks (absent from the source) keep the
/// reference's ops. The result is a runnable module whose every source-defined chunk was
/// emitted by the self-hosted pipeline.
fn self_host_compile(src: &str) -> Module {
    let (fns, names, _, _) = parse_functions(src);
    let mut module = compile_src(src);
    let mut i = 0;
    while i < fns.len() {
        let name = names[fns[i].name as usize].clone();
        // Group consecutive same-named heads (a multiheaded function is one chunk).
        let mut group: Vec<&ParsedFn> = vec![&fns[i]];
        let mut j = i + 1;
        while j < fns.len() && names[fns[j].name as usize] == name {
            group.push(&fns[j]);
            j += 1;
        }
        i = j;
        let pc = group[0].params;
        // A yield head compiles as a multihead chunk; a fn or loop as a single body.
        // The reconstruction runs through the self-hosted reconstruct.kel stage rather
        // than the Rust `reconstruct_into`, so the whole self-host compile path is
        // Keleusma from lexing through code generation and the host only moves data
        // between stages.
        let body = if group[0].cat == 2 {
            reconstruct_via_kel_multihead(&group, pc)
        } else {
            let category = if group[0].cat == 3 { 2 } else { 0 };
            reconstruct_via_kel(&group[0].body, category, pc)
        };
        let (ops, pool, lc) = run_codegen(&body, pc);
        let idx = module
            .chunks
            .iter()
            .position(|c| c.name == name)
            .unwrap_or_else(|| panic!("no chunk named `{name}`"));
        module.chunks[idx].ops = ops;
        module.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        module.chunks[idx].local_count = lc as u16;
    }
    module
}

/// Self-host-compile `src` and assert every chunk's ops, constant pool, and local_count
/// are byte-identical to the Rust-hosted compiler's, returning the self-hosted module.
fn assert_self_host_byte_identical(src: &str) -> Module {
    let module = self_host_compile(src);
    let reference = compile_src(src);
    assert_eq!(module.chunks.len(), reference.chunks.len(), "chunk count");
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order");
        assert_eq!(m.ops, r.ops, "ops for chunk `{}`", r.name);
        assert_eq!(m.constants, r.constants, "pool for chunk `{}`", r.name);
        assert_eq!(
            m.local_count, r.local_count,
            "local_count for chunk `{}`",
            r.name
        );
    }
    module
}

/// Run a self-hosted module through `call_with_shared` (it may have shared data) and
/// assert its first yielded value.
fn assert_self_host_yields(module: Module, arg: i64, expected: i64) {
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify self-hosted module");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    match vm
        .call_with_shared(&mut shared, &[Value::Int(arg)])
        .expect("call")
    {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, expected, "self-hosted result"),
        other => panic!("unexpected result: {other:?}"),
    }
}

// The bootstrap's first increment: whole multi-declaration programs self-compiled. Every
// source-defined chunk is emitted by the self-hosted pipeline (lexer.kel, parse.kel, the
// forest reconstruction, and codegen.kel) and assembled into one module byte-identical to
// the Rust-hosted compiler's, and the module runs.
#[test]
fn self_host_compiles_a_whole_program_byte_identically() {
    // A shared data block, a plain `fn` with an if, a two-headed data-guarded `yield`, and
    // a `loop main` yielding a call.
    let m1 = assert_self_host_byte_identical(
        "shared data st { pos: Word, len: Word } \
         fn clamp(x: Word) -> Word { if x > 8 { 8 } else { x } } \
         yield emit(r: Word) -> Word when st.pos < st.len { yield st.pos } \
         yield emit(r: Word) -> Word { yield 0 } \
         loop main(r: Word) -> Word { yield clamp(r) }",
    );
    assert_self_host_yields(m1, 10, 8); // clamp(10) = 8

    // A richer program: private-data accumulation over a `for` loop that calls another
    // function, a `match` function, and a `loop main` yielding the total. Exercises module
    // assembly over chunks with a for-limit loop, calls, a match, and private data.
    let m2 = assert_self_host_byte_identical(
        "private data acc { sum: Word } \
         fn add(x: Word, y: Word) -> Word { x + y } \
         fn total(n: Word) -> Word { acc.sum = 0; for i in 0..n limit 8 { acc.sum = add(acc.sum, i); } acc.sum } \
         fn pick(k: Word) -> Word { match k { 0 => 100, _ => k * 2 } } \
         loop main(n: Word) -> Word { yield total(n) }",
    );
    assert_self_host_yields(m2, 4, 6); // total(4) = 0+1+2+3 = 6

    // An enum program: enum-variant casts (`Kind::Lo() as Word`), a match over the cast
    // result, and calls. parse.kel accumulates the enum table and folds each cast to its
    // discriminant literal, so the whole program self-compiles at module scope.
    let m3 = assert_self_host_byte_identical(
        "enum Kind { Lo = 0, Hi = 1 } \
         fn classify(n: Word) -> Word { if n < 10 { Kind::Lo() as Word } else { Kind::Hi() as Word } } \
         fn pick(k: Word) -> Word { match k { 0 => 100, _ => 200 } } \
         loop main(n: Word) -> Word { yield pick(classify(n)) }",
    );
    assert_self_host_yields(m3, 5, 100); // classify(5)=Lo=0 -> pick(0)=100

    // A const-data program: a `const data` block whose scalar fields inline to their
    // integer-literal values when read (`cfg.base`, `cfg.step`), exactly as the reference
    // compiler folds them to `Const`. This is the shape codegen.kel's own `const data wire`
    // opcode table uses throughout, and is required to self-compile the codegen stage.
    let m4 = assert_self_host_byte_identical(
        "const data cfg { base: Word = 10, step: Word = 2 } \
         fn bump(x: Word) -> Word { x + cfg.base } \
         fn stride(x: Word) -> Word { x * cfg.step + cfg.base } \
         loop main(x: Word) -> Word { yield stride(bump(x)) }",
    );
    assert_self_host_yields(m4, 5, 40); // bump(5)=15; stride(15)=15*2+10=40
}

// Nested `for .. limit`: the loop-context stack lets an inner loop nest in an outer
// loop's body without displacing it. Byte-identical to the reference, and the built
// module computes the doubly-nested accumulation.
#[test]
fn parse_into_codegen_nested_for_matches_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // inner loop accumulates a scalar; n*n increments.
        (
            "private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 4 { for j in 0..n limit 4 { d.s = d.s + 1; } } d.s }",
            3,
            9,
        ),
        // the inner loop's bound and body use the outer variable.
        (
            "private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 4 { for j in 0..i limit 4 { d.s = d.s + i; } } d.s }",
            3,
            5,
        ),
        // a statement before the nested loops, and a scalar write after the inner loop.
        (
            "private data d { s: Word } fn main(n: Word) -> Word { d.s = 1; for i in 0..n limit 4 { for j in 0..n limit 4 { d.s = d.s + 1; } d.s = d.s + 10; } d.s }",
            2,
            25,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);
        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        let mut shared = vec![0u8; vm.shared_data_bytes()];
        match vm
            .call_with_shared(&mut shared, &[Value::Int(input)])
            .expect("call")
        {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The bridge over a `Byte as Word` cast, the last construct blocking lexer.kel: a Byte
// data-read widened to Word lowers to a single ByteToWord op. parse.kel emits a Cast node
// (26) on `as`, codegen.kel emits ByteToWord, and both match the Rust compiler.
#[test]
fn parse_into_codegen_byte_cast_matches_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // a Byte element read, widened and used in arithmetic (the peek_at shape).
        (
            "shared data d { bs: [Byte; 8], n: Word } fn main(p: Word) -> Word { d.n = p; d.bs[0] as Word + 1 }",
            0,
            1,
        ),
        // a runtime index, and the cast under a comparison.
        (
            "shared data d { bs: [Byte; 8], n: Word } fn main(p: Word) -> Word { d.n = p; if d.bs[p] as Word < 5 { 1 } else { 0 } }",
            0,
            1,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);
        let reference = compile_src(src);
        let idx = reference
            .chunks
            .iter()
            .position(|c| c.name == "main")
            .unwrap();
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        assert_eq!(
            local_count, reference.chunks[idx].local_count as i64,
            "local_count for `{src}`"
        );
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        let mut shared = vec![0u8; vm.shared_data_bytes()];
        match vm
            .call_with_shared(&mut shared, &[Value::Int(input)])
            .expect("call")
        {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The whole of lexer.kel, the tokenizer stage, self-compiled: every one of its chunks is
// emitted by the self-hosted pipeline (lexer.kel tokenizing, parse.kel parsing, the forest
// reconstruction, and codegen.kel) byte-identically to the Rust-hosted compiler. This is
// the first whole stage-file self-compile, and it exercises the full body grammar the
// tokenizer uses: yields inside if branches, `for .. limit` loops nested inside `if`
// branches inside outer loops, Byte-as-Word casts, private and shared data, and calls.
#[test]
fn self_host_compiles_lexer_kel_byte_identically() {
    let src = std::fs::read_to_string("compiler/kel/lexer.kel").expect("read lexer.kel");
    let module = self_host_compile(&src);
    let reference = compile_src(&src);
    assert_eq!(module.chunks.len(), reference.chunks.len(), "chunk count");
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order");
        assert_eq!(m.ops, r.ops, "ops for chunk `{}`", r.name);
        assert_eq!(m.constants, r.constants, "pool for chunk `{}`", r.name);
        assert_eq!(
            m.local_count, r.local_count,
            "local_count for chunk `{}`",
            r.name
        );
    }
}

// A `yield` written as the tail of an if branch (`if c { yield a } else { yield b }`) folds
// its YieldExpr into that branch, not around the whole conditional. parse.kel captures the
// operator-stack depth at each branch open and drains the YieldMark at the branch fold only
// when the `yield` was pushed inside the branch, distinguishing this from the outer form
// `yield if c { a } else { b }` (covered elsewhere) where the whole If is the yield operand.
// This was the lexer.kel `main` blocker.
#[test]
fn parse_into_codegen_yield_in_if_branch_matches_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        (
            "loop main(r: Word) -> Word { if r == 0 { yield 62 } else { yield 63 } }",
            0,
            62,
        ),
        (
            "loop main(r: Word) -> Word { if r == 0 { yield 62 } else { yield 63 } }",
            1,
            63,
        ),
        // nested: a yield in each branch of an if nested in the else branch.
        (
            "loop main(r: Word) -> Word { if r == 0 { yield 62 } else { if r == 1 { yield 63 } else { yield 64 } } }",
            2,
            64,
        ),
        // a yield tail preceded by a let and a data-assign statement, inside a branch.
        (
            "shared data st { k: Word, v: Word } loop main(r: Word) -> Word { if r == 0 { let x = st.v; st.k = 0; yield 12 + x * 64 } else { yield 62 } }",
            0,
            12,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);
        let reference = compile_src(src);
        let idx = main_index(&reference);
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        let mut shared = vec![0u8; vm.shared_data_bytes()];
        match vm
            .call_with_shared(&mut shared, &[Value::Int(input)])
            .expect("call")
        {
            VmState::Yielded(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// A `for .. limit` loop written inside an `if` branch closes as a for body, not the branch,
// so its parts and ForLimit statement stream inside the branch. parse.kel stamps each block
// open with a monotonic sequence and closes the innermost (largest-stamp) block at each `}`.
// This was the lexer.kel `intern_id` blocker, whose outer-loop body is an `if` guarding a
// nested `for`.
#[test]
fn parse_into_codegen_for_in_if_branch_matches_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        // a for as the sole content of an if-then branch (no else).
        (
            "private data d { s: Word } fn main(n: Word) -> Word { if n > 0 { for j in 0..n limit 8 { d.s = d.s + 1; } } d.s }",
            3,
            3,
        ),
        // a for as an if-then branch with an explicit else.
        (
            "private data d { s: Word } fn main(n: Word) -> Word { if n > 0 { for j in 0..n limit 8 { d.s = d.s + 1; } } else { d.s = 9; } d.s }",
            0,
            9,
        ),
        // an if (no else) holding a nested for, inside an outer for -- the intern_id shape.
        (
            "private data d { s: Word } fn main(n: Word) -> Word { d.s = 0; for i in 0..n limit 4 { if i > 0 { for j in 0..n limit 4 { d.s = d.s + 1; } } } d.s }",
            3,
            6,
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);
        let reference = compile_src(src);
        let idx = main_index(&reference);
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let need = required_persistent_capacity_for(&built);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("resize");
        let mut vm = Vm::new(built, &arena).expect("verify built");
        let mut shared = vec![0u8; vm.shared_data_bytes()];
        match vm
            .call_with_shared(&mut shared, &[Value::Int(input)])
            .expect("call")
        {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// A `match` whose arm results are function calls (`k => f(x)`), and matches with many
// arms. codegen.kel's `walk_step` dispatch is a 23-arm match of exactly this shape; the
// earlier whole-program tests only exercised literal-valued arms, so this pins the
// call-valued and high-arity forms independently.
#[test]
fn parse_into_codegen_match_call_arms_match_the_reference() {
    let cases: &[(&str, i64, i64)] = &[
        (
            "fn g(x: Word) -> Word { x + 1 } fn h(x: Word) -> Word { x + 2 } fn main(k: Word) -> Word { match k { 0 => g(k), 1 => h(k), _ => g(k) } }",
            1,
            3, // h(1) = 3
        ),
        (
            "fn main(k: Word) -> Word { match k { 0 => 10, 1 => 11, 2 => 12, 3 => 13, 4 => 14, 5 => 15, 6 => 16, 7 => 17, 8 => 18, 9 => 19, _ => 99 } }",
            7,
            17,
        ),
        (
            "fn g(x: Word) -> Word { x + 1 } fn main(k: Word) -> Word { match k { 0 => g(k), 1 => g(k), 2 => g(k), 3 => g(k), 4 => g(k), 5 => g(k), 6 => g(k), 7 => g(k), 8 => g(k), 9 => g(k), _ => g(k) } }",
            5,
            6, // g(5) = 6
        ),
    ];
    for &(src, input, expected) in cases {
        let (records, param_count, category) = parse_function_records(src);
        let body = reconstruct_body(&records, category);
        let (emitted, pool, local_count) = run_codegen(&body, param_count);
        let reference = compile_src(src);
        let idx = main_index(&reference);
        assert_eq!(emitted, reference.chunks[idx].ops, "ops for `{src}`");
        let mut built = compile_src(src);
        built.chunks[idx].ops = emitted;
        built.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        built.chunks[idx].local_count = local_count as u16;
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(built, &arena).expect("verify");
        match vm.call(&[Value::Int(input)]).expect("call") {
            VmState::Finished(Value::Int(n)) => assert_eq!(n, expected, "value for `{src}`"),
            other => panic!("unexpected result for `{src}`: {other:?}"),
        }
    }
}

// The whole of codegen.kel, the code-generator stage, self-compiled: every one of its 35
// chunks is emitted by the self-hosted pipeline byte-identically to the Rust-hosted
// compiler. This is the second whole stage-file self-compile (after lexer.kel) and the
// first that is itself a code generator, so the self-hosted codegen reproduces its own
// bytecode. It exercises the full body grammar at scale: a 23-arm dispatch `match` with
// call-valued arms, `for .. limit` emitters with ~78-statement bodies, nested data-array
// reads and writes, and the multiheaded `emit_next` phase machine. Reaching it required
// packing the parser token stream (codegen.kel is 4404 tokens), raising the lexer source
// and intern buffers (42KB, ~900 identifiers), raising parse.kel's per-block statement
// table past 64, and rewriting codegen.kel's one `|>` pipe to the `match` it desugars to.
#[test]
fn self_host_compiles_codegen_kel_byte_identically() {
    let src = std::fs::read_to_string("compiler/kel/codegen.kel").expect("read codegen.kel");
    let module = self_host_compile(&src);
    let reference = compile_src(&src);
    assert_eq!(module.chunks.len(), reference.chunks.len(), "chunk count");
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order");
        assert_eq!(m.ops, r.ops, "ops for chunk `{}`", r.name);
        assert_eq!(m.constants, r.constants, "pool for chunk `{}`", r.name);
        assert_eq!(
            m.local_count, r.local_count,
            "local_count for chunk `{}`",
            r.name
        );
    }
}

// The whole of parse.kel, the parser stage, self-compiled: every one of its 42 chunks is
// emitted by the self-hosted pipeline byte-identically to the Rust-hosted compiler. This
// is the THIRD and last whole stage-file self-compile, so all three self-hosted stages
// (lexer.kel, codegen.kel, parse.kel) now reproduce their own bytecode -- the precondition
// for the self-compiling fixed point. parse.kel is the largest stage (68 KB source, ~10040
// tokens, functions up to ~440 nodes and ~540 ops); reaching it required the wire-format V2
// 24-bit data operands (its shared segment exceeds 64 KB), the widened base+length pack in
// the parse-to-codegen bridge (a data slot or array length past 65535 no longer spills its
// base into the length field), and the enlarged lexer source, parser token, and codegen
// ast/op buffers.
#[test]
fn self_host_compiles_parse_kel_byte_identically() {
    let src = std::fs::read_to_string("compiler/kel/parse.kel").expect("read parse.kel");
    let module = self_host_compile(&src);
    let reference = compile_src(&src);
    assert_eq!(module.chunks.len(), reference.chunks.len(), "chunk count");
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order");
        assert_eq!(m.ops, r.ops, "ops for chunk `{}`", r.name);
        assert_eq!(m.constants, r.constants, "pool for chunk `{}`", r.name);
        assert_eq!(
            m.local_count, r.local_count,
            "local_count for chunk `{}`",
            r.name
        );
    }
}

// reconstruct.kel, the reconstruction stage, self-compiled: every chunk is emitted by
// the self-hosted pipeline (which itself now runs through reconstruct.kel) byte-
// identically to the Rust-hosted compiler. reconstruct.kel reconstructing its own
// source is the fourth stage-file self-compile, so every Keleusma source in the
// compiler self-compiles. It stays within the bridged grammar (fn/loop, if/else,
// `orelse`, nested `for .. limit`, shared and private data with indexed access, and
// helper calls -- no match, enum, struct, or generic), and is the smallest stage.
#[test]
fn self_host_compiles_reconstruct_kel_byte_identically() {
    let src =
        std::fs::read_to_string("compiler/kel/reconstruct.kel").expect("read reconstruct.kel");
    let module = self_host_compile(&src);
    let reference = compile_src(&src);
    assert_eq!(module.chunks.len(), reference.chunks.len(), "chunk count");
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order");
        assert_eq!(m.ops, r.ops, "ops for chunk `{}`", r.name);
        assert_eq!(m.constants, r.constants, "pool for chunk `{}`", r.name);
        assert_eq!(
            m.local_count, r.local_count,
            "local_count for chunk `{}`",
            r.name
        );
    }
}

// -- reconstruct.kel: the self-hosted reconstruction stage --------------------
//
// parse.kel emits postorder (kind, arg) records; codegen.kel consumes a
// random-access (kind, arg, lhs, rhs) forest. `reconstruct.kel` is the Keleusma
// stage that bridges them, replacing the host-side Rust `reconstruct_into`. These
// tests drive it and assert its `ast` output equals the Rust `reconstruct_body` for
// the same records, grown construct by construct.

// Flat shared-slot offsets of reconstruct.kel's `rin` then `ast` blocks (declaration
// order). `rin` = rec_count(1) + in_category(1) + in_param_count(1) + rec_kind[1024]
// + rec_arg[1024]; `ast` mirrors codegen.kel's block.
const RC_REC_COUNT: usize = 0;
const RC_IN_CATEGORY: usize = 1;
const RC_IN_PARAM: usize = 2;
const RC_REC_KIND: usize = 3;
const RC_REC_ARG: usize = 3 + 1024;
const RC_AST_BASE: usize = 3 + 1024 * 2;
const RC_AST_ROOT: usize = RC_AST_BASE;
const RC_AST_KINDS: usize = RC_AST_BASE + 1;
const RC_AST_ARGS: usize = RC_AST_BASE + 1 + 1024;
const RC_AST_LHS: usize = RC_AST_BASE + 1 + 1024 * 2;
const RC_AST_RHS: usize = RC_AST_BASE + 1 + 1024 * 3;
const RC_AST_CALL_ARGS: usize = RC_AST_BASE + 1 + 1024 * 4;
const RC_AST_MATCH_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 2;
const RC_AST_LIMIT_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 3;
const RC_AST_HEAD_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 4;
const RC_AST_CATEGORY: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 5 + 1;
// Multiheaded-dispatch input, appended after out_category.
const RC_HEAD_COUNT: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 5 + 2;
const RC_HEAD_GUARD_START: usize = RC_HEAD_COUNT + 1;
const RC_HEAD_GUARD_LEN: usize = RC_HEAD_COUNT + 1 + 16;
const RC_HEAD_BODY_START: usize = RC_HEAD_COUNT + 1 + 16 * 2;
const RC_HEAD_BODY_LEN: usize = RC_HEAD_COUNT + 1 + 16 * 3;

/// Drive reconstruct.kel over one function's postorder records and read back the
/// reconstructed forest as a `Body`. This increment reads only the node arrays and
/// the root/category; the side arrays (call/for/match) arrive with those kinds.
// Compile reconstruct.kel once and clone the module per call: `self_host_compile`
// drives it for every function, so recompiling each time dominates the runtime.
fn reconstruct_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| {
            compile_src(
                &std::fs::read_to_string("compiler/kel/reconstruct.kel")
                    .expect("read reconstruct.kel"),
            )
        })
        .clone()
}

fn reconstruct_via_kel(records: &[(i64, i64)], category: i64, param_count: usize) -> Body {
    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, RC_REC_COUNT, Value::Int(records.len() as i64))
        .unwrap();
    vm.set_shared(&mut shared, RC_IN_CATEGORY, Value::Int(category))
        .unwrap();
    vm.set_shared(&mut shared, RC_IN_PARAM, Value::Int(param_count as i64))
        .unwrap();
    for (i, &(k, a)) in records.iter().enumerate() {
        vm.set_shared(&mut shared, RC_REC_KIND + i, Value::Int(k))
            .unwrap();
        vm.set_shared(&mut shared, RC_REC_ARG + i, Value::Int(a))
            .unwrap();
    }
    let node_count = match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call")
    {
        VmState::Yielded(Value::Int(n)) => n as usize,
        other => panic!("unexpected reconstruct.kel state: {other:?}"),
    };
    let rd = |vm: &Vm<'_, '_>, shared: &[u8], slot: usize| -> i64 {
        match vm.get_shared(shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    let root = rd(&vm, &shared, RC_AST_ROOT);
    let mut nodes = Vec::with_capacity(node_count);
    for i in 0..node_count {
        nodes.push(Node {
            kind: rd(&vm, &shared, RC_AST_KINDS + i),
            arg: rd(&vm, &shared, RC_AST_ARGS + i),
            lhs: rd(&vm, &shared, RC_AST_LHS + i),
            rhs: rd(&vm, &shared, RC_AST_RHS + i),
        });
    }
    // Read each 64-entry side array in full; the caller compares only the prefix the
    // Rust reconstruction populated.
    let read_side = |vm: &Vm<'_, '_>, shared: &[u8], base: usize| -> Vec<i64> {
        (0..64).map(|k| rd(vm, shared, base + k)).collect()
    };
    let call_args = read_side(&vm, &shared, RC_AST_CALL_ARGS);
    let match_parts = read_side(&vm, &shared, RC_AST_MATCH_PARTS);
    let limit_parts = read_side(&vm, &shared, RC_AST_LIMIT_PARTS);
    Body {
        nodes,
        call_args,
        for_parts: Vec::new(),
        match_parts,
        limit_parts,
        head_parts: Vec::new(),
        category: rd(&vm, &shared, RC_AST_CATEGORY),
        root,
    }
}

/// Assert reconstruct.kel and the Rust reconstruction agree for `src`'s body.
fn assert_reconstruct_kel_matches(src: &str) {
    let (records, pc, cat) = parse_function_records(src);
    let via_kel = reconstruct_via_kel(&records, cat, pc);
    let via_rust = reconstruct_body(&records, cat);
    assert_eq!(
        via_kel.nodes.len(),
        via_rust.nodes.len(),
        "node count for `{src}`"
    );
    for (i, (a, b)) in via_kel.nodes.iter().zip(via_rust.nodes.iter()).enumerate() {
        assert_eq!(
            (a.kind, a.arg, a.lhs, a.rhs),
            (b.kind, b.arg, b.lhs, b.rhs),
            "node {i} for `{src}`"
        );
    }
    assert_eq!(via_kel.root, via_rust.root, "root for `{src}`");
    assert_eq!(via_kel.category, via_rust.category, "category for `{src}`");
    // The reconstruct.kel side arrays are read at full width; compare the prefix the
    // Rust reconstruction populated (the rest is zero-fill).
    assert_eq!(
        via_kel.call_args[..via_rust.call_args.len()],
        via_rust.call_args[..],
        "call_args for `{src}`"
    );
    assert_eq!(
        via_kel.match_parts[..via_rust.match_parts.len()],
        via_rust.match_parts[..],
        "match_parts for `{src}`"
    );
    assert_eq!(
        via_kel.limit_parts[..via_rust.limit_parts.len()],
        via_rust.limit_parts[..],
        "limit_parts for `{src}`"
    );
}

// reconstruct.kel increment 1: the atomic, operator, block, and `if` node kinds --
// leaves (Literal/Local/DataRead/Unit), binaries (BinOp/LetIn/Andalso/Orelse/
// DataAssignIn/ExprStmt), unaries (Not/Neg/Cast), and If -- reconstruct to the same
// forest the Rust bridge builds.
#[test]
fn reconstruct_kel_matches_rust_for_atomic_and_if_bodies() {
    let cases: &[&str] = &[
        "fn main(x: Word) -> Word { x + 1 }",
        "fn main(x: Word) -> Word { let y = x + 1; y * 2 }",
        "fn main(x: Word) -> Word { if x > 2 { x } else { 0 } }",
        "fn main(x: Word) -> Word { let y = x + 1; if y > 2 { y } else { 0 } }",
        "fn main(x: Word) -> Word { if not (x > 0) { 0 - x } else { x } }",
        "fn main(x: Word) -> Word { (x > 0) andalso (x < 10) }",
        "fn main(x: Word) -> Word { (x < 0) orelse (x > 10) }",
        "fn main(x: Word) -> Word { if x > 0 { if x > 10 { 2 } else { 1 } } else { 0 } }",
    ];
    for src in cases {
        assert_reconstruct_kel_matches(src);
    }
}

// reconstruct.kel increment 2: function calls (the call_args side array, argument
// nodes stored in source order) and indexed data writes (the IndexStore signal
// folding value and index into a kind-15 node).
#[test]
fn reconstruct_kel_matches_rust_for_calls_and_indexed_writes() {
    let cases: &[&str] = &[
        "fn g(x: Word) -> Word { x + 1 } fn main(x: Word) -> Word { g(x) }",
        "fn add(a: Word, b: Word) -> Word { a + b } fn main(x: Word) -> Word { add(x, 1) }",
        "fn g(x: Word) -> Word { x } fn main(x: Word) -> Word { g(g(x)) }",
        "fn g(a: Word, b: Word, c: Word) -> Word { a } fn main(x: Word) -> Word { g(x, x + 1, x + 2) }",
        "shared data d { a: [Word; 8] } fn main(i: Word) -> Word { d.a[i] = 3; d.a[i] }",
    ];
    for src in cases {
        assert_reconstruct_kel_matches(src);
    }
}

// reconstruct.kel increment 3: bounded `for .. limit` loops (the SlotRecord/ForBuild
// signals assembling the twelve-word limit_parts entry, and the ForLimit statement)
// and integer `match` (the MatchBuild signal assembling the match_parts entry and the
// MatchIn node). With these the whole single-headed body grammar reconstructs.
#[test]
fn reconstruct_kel_matches_rust_for_loops_and_matches() {
    let cases: &[&str] = &[
        "private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 8 { d.s = d.s + i; } d.s }",
        "private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 4 { for j in 0..n limit 4 { d.s = d.s + 1; } } d.s }",
        "fn main(k: Word) -> Word { match k { 0 => 100, 1 => 200, _ => k * 2 } }",
        "fn g(x: Word) -> Word { x + 1 } fn main(k: Word) -> Word { match k { 0 => g(k), 1 => g(k), _ => g(k) } }",
        "private data d { s: Word } fn main(n: Word) -> Word { for i in 0..n limit 8 { if i > 0 { d.s = d.s + i; } } d.s }",
    ];
    for src in cases {
        assert_reconstruct_kel_matches(src);
    }
}

/// Drive reconstruct.kel over a group of same-named heads (a multiheaded function),
/// feeding each head's guard and body record ranges, and read back the reconstructed
/// multihead `Body`.
fn reconstruct_via_kel_multihead(heads: &[&ParsedFn], pc: usize) -> Body {
    // Concatenate every head's guard then body records, tracking the per-head offsets.
    let mut recs: Vec<(i64, i64)> = Vec::new();
    let mut gs = Vec::new();
    let mut gl = Vec::new();
    let mut bs = Vec::new();
    let mut bl = Vec::new();
    for h in heads {
        gs.push(recs.len());
        gl.push(h.guard.len());
        recs.extend_from_slice(&h.guard);
        bs.push(recs.len());
        bl.push(h.body.len());
        recs.extend_from_slice(&h.body);
    }

    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &mut Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&mut vm, &mut shared, RC_REC_COUNT, recs.len() as i64);
    set(&mut vm, &mut shared, RC_IN_CATEGORY, 3);
    set(&mut vm, &mut shared, RC_IN_PARAM, pc as i64);
    for (i, &(k, a)) in recs.iter().enumerate() {
        set(&mut vm, &mut shared, RC_REC_KIND + i, k);
        set(&mut vm, &mut shared, RC_REC_ARG + i, a);
    }
    set(&mut vm, &mut shared, RC_HEAD_COUNT, heads.len() as i64);
    for h in 0..heads.len() {
        set(&mut vm, &mut shared, RC_HEAD_GUARD_START + h, gs[h] as i64);
        set(&mut vm, &mut shared, RC_HEAD_GUARD_LEN + h, gl[h] as i64);
        set(&mut vm, &mut shared, RC_HEAD_BODY_START + h, bs[h] as i64);
        set(&mut vm, &mut shared, RC_HEAD_BODY_LEN + h, bl[h] as i64);
    }
    let node_count = match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call")
    {
        VmState::Yielded(Value::Int(n)) => n as usize,
        other => panic!("unexpected reconstruct.kel state: {other:?}"),
    };
    let rd = |vm: &Vm<'_, '_>, shared: &[u8], slot: usize| -> i64 {
        match vm.get_shared(shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    let root = rd(&vm, &shared, RC_AST_ROOT);
    let mut nodes = Vec::with_capacity(node_count);
    for i in 0..node_count {
        nodes.push(Node {
            kind: rd(&vm, &shared, RC_AST_KINDS + i),
            arg: rd(&vm, &shared, RC_AST_ARGS + i),
            lhs: rd(&vm, &shared, RC_AST_LHS + i),
            rhs: rd(&vm, &shared, RC_AST_RHS + i),
        });
    }
    let read_side = |vm: &Vm<'_, '_>, shared: &[u8], base: usize| -> Vec<i64> {
        (0..64).map(|k| rd(vm, shared, base + k)).collect()
    };
    Body {
        nodes,
        call_args: read_side(&vm, &shared, RC_AST_CALL_ARGS),
        for_parts: Vec::new(),
        match_parts: read_side(&vm, &shared, RC_AST_MATCH_PARTS),
        limit_parts: read_side(&vm, &shared, RC_AST_LIMIT_PARTS),
        head_parts: read_side(&vm, &shared, RC_AST_HEAD_PARTS),
        category: rd(&vm, &shared, RC_AST_CATEGORY),
        root,
    }
}

// reconstruct.kel increment 4: the multiheaded dispatch. A group of same-named
// `yield` heads reconstructs to the same forest, head_parts entries, and MultiHead
// root the Rust `build_multihead_bridge` builds -- each head's frame slots remapped
// into its parameter window, guards reconstructed when present. With this
// reconstruct.kel covers the whole grammar the bridge does.
#[test]
fn reconstruct_kel_matches_rust_for_multihead() {
    let cases: &[&str] = &[
        "yield emit(r: Word) -> Word when r > 0 { yield r } yield emit(r: Word) -> Word { yield 0 }",
        "shared data st { pos: Word, len: Word } \
         yield emit(r: Word) -> Word when st.pos < st.len { yield st.pos } \
         yield emit(r: Word) -> Word { yield 62 }",
        "yield pick(k: Word) -> Word when k > 5 { yield k * 2 } \
         yield pick(k: Word) -> Word when k > 0 { yield k } \
         yield pick(k: Word) -> Word { yield 0 }",
    ];
    for src in cases {
        let (fns, names, _, _) = parse_functions(src);
        // Group the last run of same-named heads (the multiheaded function).
        let last_name = names[fns.last().unwrap().name as usize].clone();
        let group: Vec<&ParsedFn> = fns
            .iter()
            .filter(|f| names[f.name as usize] == last_name)
            .collect();
        let pc = group[0].params;
        let via_kel = reconstruct_via_kel_multihead(&group, pc);
        let via_rust = build_multihead_bridge(&group, pc);
        assert_eq!(
            via_kel.nodes.len(),
            via_rust.nodes.len(),
            "node count for `{src}`"
        );
        for (i, (a, b)) in via_kel.nodes.iter().zip(via_rust.nodes.iter()).enumerate() {
            assert_eq!(
                (a.kind, a.arg, a.lhs, a.rhs),
                (b.kind, b.arg, b.lhs, b.rhs),
                "node {i} for `{src}`"
            );
        }
        assert_eq!(via_kel.root, via_rust.root, "root for `{src}`");
        assert_eq!(
            via_kel.head_parts[..via_rust.head_parts.len()],
            via_rust.head_parts[..],
            "head_parts for `{src}`"
        );
        assert_eq!(
            via_kel.match_parts[..via_rust.match_parts.len()],
            via_rust.match_parts[..],
            "match_parts for `{src}`"
        );
    }
}

// -- data-layout assembly from parse.kel's data-block records -----------------
//
// The self-host compile still borrows the module's data layout from the reference.
// The first step to assembling it from the stages: parse.kel already emits each data
// block's header (DSTART packing name and visibility, a PARAM per field name, a PTYPE
// per type, an ASIZE per array field, END), so the driver can build the slot table
// itself. This is the same two-pass expansion the Rust compiler does -- shared blocks
// first, then private, const blocks producing no runtime slots, array fields expanding
// to one `block.field[k]` slot per element.

/// Assemble the data-slot table from parse.kel's data-block record stream, mapping the
/// interned block, field, and (unused here) type ids through the name table.
fn assemble_data_slots(
    data_records: &[(i64, i64)],
    names: &[String],
) -> Vec<keleusma::bytecode::DataSlot> {
    use keleusma::bytecode::{DataSlot, SlotVisibility};
    struct Blk {
        name_id: i64,
        vis: i64,
        fields: Vec<(i64, i64)>, // (field name id, element count)
    }
    let mut blocks: Vec<Blk> = Vec::new();
    for &(code, val) in data_records {
        match code {
            9 => blocks.push(Blk {
                name_id: val / 4,
                vis: val % 4,
                fields: Vec::new(),
            }),
            4 => blocks.last_mut().unwrap().fields.push((val, 1)),
            8 => blocks.last_mut().unwrap().fields.last_mut().unwrap().1 = val,
            // 6 (PTYPE) is not needed for the slot names; 5 (END) is a boundary.
            _ => {}
        }
    }
    let mut slots = Vec::new();
    // Pass 0 shared, pass 1 private; visibility 2 (const) yields no runtime slots.
    for pass_vis in [0i64, 1i64] {
        let visibility = if pass_vis == 0 {
            SlotVisibility::Shared
        } else {
            SlotVisibility::Private
        };
        for b in blocks.iter().filter(|b| b.vis == pass_vis) {
            let bname = &names[b.name_id as usize];
            for &(fid, count) in &b.fields {
                let fname = &names[fid as usize];
                if count == 1 {
                    slots.push(DataSlot {
                        name: format!("{bname}.{fname}"),
                        visibility,
                    });
                } else {
                    for k in 0..count {
                        slots.push(DataSlot {
                            name: format!("{bname}.{fname}[{k}]"),
                            visibility,
                        });
                    }
                }
            }
        }
    }
    slots
}

// The data-slot table the driver assembles from parse.kel's data-block records is
// byte-identical (name and visibility, in order) to the one the Rust-hosted compiler
// bakes, for every stage source: shared-then-private ordering, array expansion, and
// const blocks producing no slots.
#[test]
fn assembled_data_slots_match_the_reference() {
    let cases = [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ];
    for path in cases {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (_fns, names, data_records, _) = parse_functions(&src);
        let slots = assemble_data_slots(&data_records, &names);
        let reference = compile_src(&src);
        let ref_slots = &reference.data_layout.as_ref().expect("data layout").slots;
        assert_eq!(slots.len(), ref_slots.len(), "slot count for {path}");
        for (i, (a, b)) in slots.iter().zip(ref_slots.iter()).enumerate() {
            assert_eq!(a.name, b.name, "slot {i} name for {path}");
            assert_eq!(a.visibility, b.visibility, "slot {i} visibility for {path}");
        }
    }
}

/// Assemble the per-shared-slot byte layout (offset, kind tag, len) from parse.kel's
/// data-block records. The shared segment is the single shared block's fields expanded
/// to one entry per element at consecutive byte offsets; the stage data fields are all
/// `Word` (ScalarKind::Int, tag 3, eight bytes at the 64-bit reference width) or `Byte`
/// (tag 2, one byte), and each is a scalar (len 0).
fn assemble_shared_layout(
    data_records: &[(i64, i64)],
    names: &[String],
) -> Vec<keleusma::bytecode::SharedSlotLayout> {
    use keleusma::bytecode::SharedSlotLayout;
    struct Blk {
        vis: i64,
        fields: Vec<(i64, i64)>, // (type name id, element count)
    }
    let mut blocks: Vec<Blk> = Vec::new();
    for &(code, val) in data_records {
        match code {
            9 => blocks.push(Blk {
                vis: val % 4,
                fields: Vec::new(),
            }),
            4 => blocks.last_mut().unwrap().fields.push((0, 1)),
            6 => blocks.last_mut().unwrap().fields.last_mut().unwrap().0 = val,
            8 => blocks.last_mut().unwrap().fields.last_mut().unwrap().1 = val,
            _ => {}
        }
    }
    let scalar = |type_id: i64| -> (u8, u32) {
        match names[type_id as usize].as_str() {
            "Word" => (3, 8),
            "Byte" => (2, 1),
            other => panic!("unhandled shared field type `{other}`"),
        }
    };
    let mut layout = Vec::new();
    let mut offset: u32 = 0;
    for b in blocks.iter().filter(|b| b.vis == 0) {
        for &(tid, count) in &b.fields {
            let (tag, size) = scalar(tid);
            for _ in 0..count {
                layout.push(SharedSlotLayout {
                    offset,
                    kind: tag,
                    len: 0,
                });
                offset += size;
            }
        }
    }
    layout
}

// The per-shared-slot byte layout the driver assembles from parse.kel's records equals
// the reference compiler's, offset/kind/len for every element slot, for all four stage
// sources (including lexer.kel's 98304-byte buffer whose later fields sit past 64 KB).
#[test]
fn assembled_shared_layout_matches_the_reference() {
    let cases = [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ];
    for path in cases {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (_fns, names, data_records, _) = parse_functions(&src);
        let layout = assemble_shared_layout(&data_records, &names);
        let reference = compile_src(&src);
        let ref_layout = &reference
            .data_layout
            .as_ref()
            .expect("data layout")
            .shared_layout;
        assert_eq!(
            layout.len(),
            ref_layout.len(),
            "shared_layout len for {path}"
        );
        for (i, (a, b)) in layout.iter().zip(ref_layout.iter()).enumerate() {
            assert_eq!(
                (a.offset, a.kind, a.len),
                (b.offset, b.kind, b.len),
                "shared_layout entry {i} for {path}"
            );
        }
    }
}

/// Assemble the per-private-slot load-time initial values from parse.kel's records:
/// one entry per private slot (arrays expanded), the type's zero -- `Int(0)` for a
/// `Word` slot, `Byte(0)` for a `Byte` slot. The stage private fields carry no
/// `= literal` initializer, so every entry is a zero.
fn assemble_private_init(
    data_records: &[(i64, i64)],
    names: &[String],
) -> Vec<keleusma::bytecode::ConstValue> {
    use keleusma::bytecode::ConstValue;
    struct Blk {
        vis: i64,
        fields: Vec<(i64, i64)>, // (type name id, element count)
    }
    let mut blocks: Vec<Blk> = Vec::new();
    for &(code, val) in data_records {
        match code {
            9 => blocks.push(Blk {
                vis: val % 4,
                fields: Vec::new(),
            }),
            4 => blocks.last_mut().unwrap().fields.push((0, 1)),
            6 => blocks.last_mut().unwrap().fields.last_mut().unwrap().0 = val,
            8 => blocks.last_mut().unwrap().fields.last_mut().unwrap().1 = val,
            _ => {}
        }
    }
    let mut init = Vec::new();
    for b in blocks.iter().filter(|b| b.vis == 1) {
        for &(tid, count) in &b.fields {
            let zero = match names[tid as usize].as_str() {
                "Word" => ConstValue::Int(0),
                "Byte" => ConstValue::Byte(0),
                other => panic!("unhandled private field type `{other}`"),
            };
            for _ in 0..count {
                init.push(zero.clone());
            }
        }
    }
    init
}

/// Assemble a whole `DataLayout` from parse.kel's data-block records. The stages have
/// no private composite fields, so `private_composite_layout` is empty.
fn assemble_data_layout(
    data_records: &[(i64, i64)],
    names: &[String],
) -> keleusma::bytecode::DataLayout {
    keleusma::bytecode::DataLayout {
        slots: assemble_data_slots(data_records, names),
        shared_layout: assemble_shared_layout(data_records, names),
        private_composite_layout: Vec::new(),
        private_init: assemble_private_init(data_records, names),
    }
}

// The whole DataLayout the driver assembles from parse.kel's records -- slot table,
// per-shared-slot byte layout, and per-private-slot initial values -- is byte-identical
// to the reference compiler's for every stage source, so the driver no longer needs the
// reference for the module's data layout.
#[test]
fn assembled_data_layout_matches_the_reference() {
    let cases = [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ];
    for path in cases {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (_fns, names, data_records, _) = parse_functions(&src);
        let dl = assemble_data_layout(&data_records, &names);
        let reference = compile_src(&src);
        let ref_dl = reference.data_layout.as_ref().expect("data layout");
        assert_eq!(dl.slots.len(), ref_dl.slots.len(), "slot count for {path}");
        for (i, (a, b)) in dl.slots.iter().zip(ref_dl.slots.iter()).enumerate() {
            assert_eq!(
                (&a.name, a.visibility),
                (&b.name, b.visibility),
                "slot {i} for {path}"
            );
        }
        assert_eq!(
            dl.shared_layout.len(),
            ref_dl.shared_layout.len(),
            "shared_layout len for {path}"
        );
        for (i, (a, b)) in dl
            .shared_layout
            .iter()
            .zip(ref_dl.shared_layout.iter())
            .enumerate()
        {
            assert_eq!(
                (a.offset, a.kind, a.len),
                (b.offset, b.kind, b.len),
                "shared_layout {i} for {path}"
            );
        }
        assert_eq!(
            dl.private_composite_layout.len(),
            ref_dl.private_composite_layout.len(),
            "private_composite_layout for {path}"
        );
        assert_eq!(
            dl.private_init, ref_dl.private_init,
            "private_init for {path}"
        );
    }
}

// -- enum-layout assembly from parse.kel's enum records -----------------------
//
// parse.kel emits each enum as ENUMSTART (the type name), then EVARIANT per variant
// (the running implicit discriminant, from 0, is used) and an optional EDISC that
// overrides it and reseeds the running value, then END. The stage enums are unit enums
// (no payload), so `min_payload` is zero. The driver assembles the enum-layout table
// itself, matching the reference's `build_enum_layouts`.

/// Assemble the enum-layout table from parse.kel's enum record stream.
fn assemble_enum_layouts(
    enum_records: &[(i64, i64)],
    names: &[String],
) -> Vec<keleusma::bytecode::EnumLayout> {
    use keleusma::bytecode::{EnumLayout, EnumVariantDisc};
    let mut layouts: Vec<EnumLayout> = Vec::new();
    let mut running = 0i64;
    for &(code, val) in enum_records {
        match code {
            12 => {
                layouts.push(EnumLayout {
                    type_name: names[val as usize].clone(),
                    variants: Vec::new(),
                    min_payload: 0,
                });
                running = 0;
            }
            13 => {
                layouts.last_mut().unwrap().variants.push(EnumVariantDisc {
                    name: names[val as usize].clone(),
                    disc: running,
                });
                running += 1;
            }
            14 => {
                let vs = &mut layouts.last_mut().unwrap().variants;
                vs.last_mut().unwrap().disc = val;
                running = val + 1;
            }
            _ => {}
        }
    }
    // The reference orders the enum-layout table by type name, not declaration order.
    layouts.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    layouts
}

// The enum-layout table the driver assembles from parse.kel's records equals the
// reference compiler's -- type name, each variant's name and discriminant (implicit and
// explicit), and the zero min_payload -- for the stages that declare enums.
#[test]
fn assembled_enum_layouts_match_the_reference() {
    // parse.kel declares enums (Tok, OpCode, Node); the other stages declare none, so
    // the assembled table is empty and still matches.
    let cases = [
        "compiler/kel/parse.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/lexer.kel",
        "compiler/kel/codegen.kel",
    ];
    for path in cases {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (_fns, names, _data, enum_records) = parse_functions(&src);
        let layouts = assemble_enum_layouts(&enum_records, &names);
        let reference = compile_src(&src);
        assert_eq!(
            layouts.len(),
            reference.enum_layouts.len(),
            "enum count for {path}"
        );
        for (a, b) in layouts.iter().zip(reference.enum_layouts.iter()) {
            assert_eq!(a.type_name, b.type_name, "enum name for {path}");
            assert_eq!(a.min_payload, b.min_payload, "min_payload for {path}");
            assert_eq!(
                a.variants.len(),
                b.variants.len(),
                "variant count for {path}"
            );
            for (va, vb) in a.variants.iter().zip(b.variants.iter()) {
                assert_eq!(
                    (&va.name, va.disc),
                    (&vb.name, vb.disc),
                    "variant for {path}"
                );
            }
        }
    }
}

// -- chunk-signature assembly from parse.kel's header records -----------------
//
// The typed-verifier per-chunk signature is the flat shape of each parameter, the
// return, and (for a Stream chunk) the resume value. For the stages every boundary
// value is a scalar `Word` or `Byte`, so the shapes are the corresponding ScalarKind
// tags; only a `loop` (Stream) chunk resumes with its first parameter's shape, an `fn`
// or `yield` records `Top`. Signatures are parallel to the module chunks, ordered by
// chunk name; a multiheaded function is one chunk described by its first head.

/// The flat shape of a stage boundary type: `Word` -> Int scalar (tag 3), `Byte` ->
/// Byte scalar (tag 2), anything else the conservative `Top` the reference records for
/// an unresolvable type.
fn wire_shape_of(type_id: i64, names: &[String]) -> keleusma::bytecode::WireShape {
    use keleusma::bytecode::WireShape;
    match names.get(type_id as usize).map(String::as_str) {
        Some("Word") => WireShape::Scalar { kind: 3 },
        Some("Byte") => WireShape::Scalar { kind: 2 },
        _ => WireShape::Top,
    }
}

/// Assemble the per-chunk signature table from the parsed functions, grouping
/// same-named heads into one chunk and ordering by chunk name to match the module.
fn assemble_signatures(
    fns: &[ParsedFn],
    names: &[String],
) -> Vec<keleusma::bytecode::ChunkSignature> {
    use keleusma::bytecode::{ChunkSignature, WireShape};
    let mut chunks: Vec<(String, ChunkSignature)> = Vec::new();
    let mut i = 0;
    while i < fns.len() {
        let name = names[fns[i].name as usize].clone();
        let first = &fns[i];
        // Skip the rest of this head group.
        let mut j = i + 1;
        while j < fns.len() && names[fns[j].name as usize] == name {
            j += 1;
        }
        i = j;
        let params: Vec<WireShape> = first
            .param_types
            .iter()
            .map(|&t| wire_shape_of(t, names))
            .collect();
        let ret = wire_shape_of(first.return_type, names);
        // Only a `loop` (category 3, a Stream chunk) resumes with its first parameter.
        let resume = if first.cat == 3 {
            params.first().copied().unwrap_or(WireShape::Top)
        } else {
            WireShape::Top
        };
        chunks.push((
            name,
            ChunkSignature {
                params,
                ret,
                resume,
            },
        ));
    }
    chunks.sort_by(|a, b| a.0.cmp(&b.0));
    chunks.into_iter().map(|(_, s)| s).collect()
}

// The per-chunk signature table the driver assembles equals the reference compiler's --
// parameter shapes, return shape, and resume shape, in chunk-name order -- for every
// stage source.
#[test]
fn assembled_signatures_match_the_reference() {
    let cases = [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ];
    for path in cases {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (fns, names, _data, _enums) = parse_functions(&src);
        let sigs = assemble_signatures(&fns, &names);
        let reference = compile_src(&src);
        assert_eq!(
            sigs.len(),
            reference.signatures.len(),
            "signature count for {path}"
        );
        for (i, (a, b)) in sigs.iter().zip(reference.signatures.iter()).enumerate() {
            assert_eq!(a.params, b.params, "params of chunk {i} for {path}");
            assert_eq!(a.ret, b.ret, "ret of chunk {i} for {path}");
            assert_eq!(a.resume, b.resume, "resume of chunk {i} for {path}");
        }
    }
}

// The schema hash is a pure function of the DataLayout, which the driver assembles
// itself, so the driver computes the module's schema hash from its own assembled layout
// -- matching the reference for every stage source.
#[test]
fn assembled_schema_hash_matches_the_reference() {
    for path in [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ] {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (_fns, names, data_records, _enums) = parse_functions(&src);
        let dl = assemble_data_layout(&data_records, &names);
        let hash = keleusma::bytecode::compute_schema_hash(Some(&dl));
        let reference = compile_src(&src);
        assert_eq!(hash, reference.schema_hash, "schema hash for {path}");
    }
}

/// Set a module's declared WCET/WCMU header from the self-hosted analyze.kel stage: the
/// per-iteration maximum across the module's Stream chunks, mirroring the reference
/// compiler's fold (`compiler.rs` sets `wcet_cycles`/`wcmu_bytes` to that maximum).
fn assemble_resource_bounds(module: &mut Module) {
    let mut max_wcet = 0i64;
    let mut max_wcmu = 0i64;
    for c in &module.chunks {
        if c.block_type != keleusma::bytecode::BlockType::Stream {
            continue;
        }
        let (wcet, stack, heap, reject) = analyze_via_kel(c);
        assert!(!reject, "analyze.kel rejected a stage Stream chunk");
        max_wcet = max_wcet.max(wcet);
        max_wcmu = max_wcmu.max(stack + heap);
    }
    module.wcet_cycles = max_wcet as u32;
    module.wcmu_bytes = max_wcmu as u32;
}

// -- integration: the self-assembled scaffold serializes byte-identically ------
//
// Each analytical scaffold component (DataLayout, enum-layout table, typed-verifier
// signatures, schema hash, chunk-table metadata, and now the WCET/WCMU declared-bound
// numbers) was proved byte-identical to the reference as a struct. This splices all of them,
// together with the self-hosted chunk ops, into a module and serializes it, asserting the
// wire bytes equal the reference compiler's -- so the assembled components agree not only
// field by field but through the wire encoding. With `assemble_resource_bounds` computing
// the header WCET/WCMU from analyze.kel, no field of the serialized module is borrowed from
// the reference for these loop-free stages; the reference module is used only as the
// comparison oracle.
#[test]
fn self_assembled_scaffold_serializes_byte_identically() {
    for path in [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ] {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (fns, names, data_records, enum_records) = parse_functions(&src);
        // The self-hosted chunk ops, with the reference scaffold; then splice in the
        // driver-assembled components.
        let mut module = self_host_compile(&src);
        let dl = assemble_data_layout(&data_records, &names);
        module.schema_hash = keleusma::bytecode::compute_schema_hash(Some(&dl));
        module.data_layout = Some(dl);
        module.enum_layouts = assemble_enum_layouts(&enum_records, &names);
        module.signatures = assemble_signatures(&fns, &names);
        assemble_resource_bounds(&mut module);

        let self_bytes = module.to_bytes().expect("serialize self-assembled module");
        let ref_bytes = compile_src(&src)
            .to_bytes()
            .expect("serialize reference module");
        assert_eq!(self_bytes, ref_bytes, "serialized module for {path}");
    }
}

// -- chunk-table metadata assembly --------------------------------------------
//
// The last driver-borrowed chunk fields (beyond the self-hosted ops/constants/
// local_count) are the parameter count, block type, and parameter type tags. All three
// come from parse.kel: the count and types from the header PARAM/PTYPE records, and the
// block type from the category (fn -> Func, yield -> Reentrant, loop -> Stream). A
// multiheaded function is one chunk described by its first head; the table is ordered by
// chunk name.

/// Assemble each chunk's (name, param_count, block_type, param_type tags) from the
/// parsed functions, in chunk-name order.
#[allow(clippy::type_complexity)]
fn assemble_chunk_metadata(
    fns: &[ParsedFn],
    names: &[String],
) -> Vec<(
    String,
    u8,
    keleusma::bytecode::BlockType,
    Vec<keleusma::bytecode::TypeTag>,
)> {
    use keleusma::bytecode::{BlockType, TypeTag};
    let tag_of = |type_id: i64| -> TypeTag {
        match names.get(type_id as usize).map(String::as_str) {
            Some("Word") => TypeTag::Word,
            Some("Byte") => TypeTag::Byte,
            _ => TypeTag::Composite,
        }
    };
    let mut chunks = Vec::new();
    let mut i = 0;
    while i < fns.len() {
        let name = names[fns[i].name as usize].clone();
        let first = &fns[i];
        let mut j = i + 1;
        while j < fns.len() && names[fns[j].name as usize] == name {
            j += 1;
        }
        i = j;
        let block_type = match first.cat {
            1 => BlockType::Func,
            2 => BlockType::Reentrant,
            _ => BlockType::Stream,
        };
        let param_types: Vec<TypeTag> = first.param_types.iter().map(|&t| tag_of(t)).collect();
        chunks.push((name, first.params as u8, block_type, param_types));
    }
    chunks.sort_by(|a, b| a.0.cmp(&b.0));
    chunks
}

// The chunk-table metadata the driver assembles equals the reference compiler's --
// parameter count, block type, and parameter type tags, in chunk-name order -- for every
// stage source. With this, only the two WCET/WCMU declared-bound numbers remain borrowed.
#[test]
fn assembled_chunk_metadata_matches_the_reference() {
    for path in [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ] {
        let src = std::fs::read_to_string(path).expect("read stage");
        let (fns, names, _data, _enums) = parse_functions(&src);
        let meta = assemble_chunk_metadata(&fns, &names);
        let reference = compile_src(&src);
        assert_eq!(meta.len(), reference.chunks.len(), "chunk count for {path}");
        for (i, (m, c)) in meta.iter().zip(reference.chunks.iter()).enumerate() {
            assert_eq!(m.0, c.name, "chunk {i} name for {path}");
            assert_eq!(m.1, c.param_count, "param_count of `{}` for {path}", c.name);
            assert_eq!(m.2, c.block_type, "block_type of `{}` for {path}", c.name);
            assert_eq!(m.3, c.param_types, "param_types of `{}` for {path}", c.name);
        }
    }
}

// -- self-hosted WCET/WCMU analysis (analyze.kel) -----------------------------
//
// analyze.kel reformulates the reference verifier's recursive `wcet_region`/`wcmu_region`
// max traversals as one explicit region-frame stack, computing a Stream chunk's
// per-iteration WCET and WCMU from a marshalled op table. Each per-op quantity is the
// authoritative `Op::cost()`/`stack_growth()`/`stack_shrink()`/`heap_alloc()`; the stage
// self-hosts only the control-flow algorithm. This increment covers the loop-free structured
// forms (straight-line, `if`/`else`, bare `if`, `break`/`trap`), exact for the four stages'
// loop-free Stream bodies; `loop` regions land in a later increment.
const WA_OP_COUNT: usize = 0;
const WA_STREAM_POS: usize = 1;
const WA_RESET_POS: usize = 2;
const WA_LOCAL_COUNT: usize = 3;
const WA_VSB: usize = 4;
const WA_ARENA_CAPACITY: usize = 5;
const WA_REGION_START: usize = 6;
const WA_REGION_END: usize = 7;
const WA_COST: usize = 8;
const WA_CLASS: usize = 8 + 1024;
const WA_ARG: usize = 8 + 1024 * 2;
const WA_GROWTH: usize = 8 + 1024 * 3;
const WA_SHRINK: usize = 8 + 1024 * 4;
const WA_HEAP: usize = 8 + 1024 * 5;
const WA_OPK: usize = 8 + 1024 * 6;
const WA_SLOT: usize = 8 + 1024 * 7;
const WA_CVAL: usize = 8 + 1024 * 8;
const WA_CINT: usize = 8 + 1024 * 9;
const WA_CALLEE_SLOTS: usize = 8 + 1024 * 10;
const WA_CALLEE_HEAP: usize = 8 + 1024 * 11;
const WA_OUT_WCET: usize = 8 + 1024 * 12;
const WA_OUT_STACK: usize = 8 + 1024 * 12 + 1;
const WA_OUT_HEAP: usize = 8 + 1024 * 12 + 2;
const WA_OUT_REJECT: usize = 8 + 1024 * 12 + 3;
const WA_OUT_VALID: usize = 8 + 1024 * 12 + 4;

fn analyze_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| {
            compile_src(
                &std::fs::read_to_string("compiler/kel/analyze.kel").expect("read analyze.kel"),
            )
        })
        .clone()
}

/// Classify an op for analyze.kel as `(class, arg)`. The class tags the control-flow role
/// (0 plain, 1 If, 2 Else, 3 EndIf, 4 Loop, 5 EndLoop, 6 Break, 7 BreakIf, 8 Trap, 9 Call);
/// `arg` carries the branch target for If and Loop, and the matching EndIf position for Else.
fn analyze_class(op: &keleusma::bytecode::Op) -> (i64, i64) {
    use keleusma::bytecode::Op;
    match op {
        Op::If(t) => (1, *t as i64),
        Op::Else(e) => (2, *e as i64),
        Op::EndIf => (3, 0),
        Op::Loop(x) => (4, *x as i64),
        Op::EndLoop(_) => (5, 0),
        Op::Break(_) => (6, 0),
        Op::BreakIf(_) => (7, 0),
        Op::Trap(_) => (8, 0),
        Op::Call(_, _) => (9, 0),
        _ => (0, 0),
    }
}

/// Fine-grained op detail for analyze.kel's loop-bound extraction: `(opk, slot, cval, cint)`.
/// `opk` tags the opcode (1 GetLocal, 2 SetLocal, 3 Const, 4 CmpGe, 5 BreakIf, 6 CheckedAdd,
/// 7 PopN, 8 EndLoop, 9 Loop, 0 other); `slot` the GetLocal/SetLocal slot; `cval` the Const
/// integer value or PopN count; `cint` 1 if a Const resolves to an integer.
fn analyze_opk(
    op: &keleusma::bytecode::Op,
    chunk: &keleusma::bytecode::Chunk,
) -> (i64, i64, i64, i64) {
    use keleusma::bytecode::{ConstValue, Op};
    match op {
        Op::GetLocal(s) => (1, *s as i64, 0, 0),
        Op::SetLocal(s) => (2, *s as i64, 0, 0),
        Op::Const(idx) => match chunk.constants.get(*idx as usize) {
            Some(ConstValue::Int(v)) => (3, 0, *v, 1),
            _ => (3, 0, 0, 0),
        },
        Op::CmpGe => (4, 0, 0, 0),
        Op::BreakIf(_) => (5, 0, 0, 0),
        Op::CheckedAdd => (6, 0, 0, 0),
        Op::PopN(n) => (7, 0, *n as i64, 0),
        Op::EndLoop(_) => (8, 0, 0, 0),
        Op::Loop(_) => (9, 0, 0, 0),
        _ => (0, 0, 0, 0),
    }
}

/// The operand-stack `(growth, shrink)` analyze.kel accounts for `op` under the empty
/// resolver. Identical to `Op::stack_growth()`/`stack_shrink()` except for a native call: the
/// reference WCMU native arm uses `during_peak = offset + 1` and `offset += 1 - n` with `n`
/// the whole argument-count byte (the error-reify high bit included), which the generic
/// accounting reproduces exactly as `growth = 1, shrink = n_full_byte`.
fn analyze_stack_effect(op: &keleusma::bytecode::Op) -> (i64, i64) {
    use keleusma::bytecode::Op;
    match op {
        Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => (1, *n as i64),
        _ => (op.stack_growth() as i64, op.stack_shrink() as i64),
    }
}

/// The per-op arena-heap bytes analyze.kel accounts for `op`: the op's own construction
/// allocation (`Op::heap_alloc`) plus the copy-out a `GetData`/`GetDataIndexed` performs when
/// it reads a flat-composite shared slot (`shared_composite_copyout_bytes`). `shared_layout`
/// is empty for the shallow empty-resolver form (copy-out zero, matching
/// `wcmu_stream_iteration`) and the module's real layout for the transitive validator.
fn analyze_op_heap(
    op: &keleusma::bytecode::Op,
    chunk: &keleusma::bytecode::Chunk,
    shared_layout: &[keleusma::bytecode::SharedSlotLayout],
) -> i64 {
    use keleusma::bytecode::{Op, SHARED_SLOT_COMPOSITE_FLAG};
    let slot_copyout = |slot: usize| -> i64 {
        shared_layout
            .get(slot)
            .filter(|e| e.kind & SHARED_SLOT_COMPOSITE_FLAG != 0)
            .map_or(0, |e| e.len as i64)
    };
    let copyout = match op {
        Op::GetData(s) => slot_copyout(*s as usize),
        Op::GetDataIndexed(base, len) => (0..*len as usize)
            .map(|i| slot_copyout(*base as usize + i))
            .max()
            .unwrap_or(0),
        _ => 0,
    };
    op.heap_alloc(chunk) as i64 + copyout
}

/// Run analyze.kel over one chunk against `arena_capacity`, returning `(wcet, stack_bytes,
/// heap_bytes, reject, valid)`. The region is the Stream-to-Reset body for a Stream chunk and
/// the whole op range for a Func/Reentrant chunk, matching `compute_chunk_wcmu`. `chunk_wcmu`
/// resolves each `Op::Call` to the callee chunk's already-computed `(stack_bytes, heap_bytes)`
/// (indexed by chunk index); pass `&[]` for the shallow empty-resolver form, where every
/// callee folds in as zero. `shared_layout` sizes composite-shared-read copy-out (empty for
/// the shallow form).
fn run_analyze_kel(
    chunk: &keleusma::bytecode::Chunk,
    arena_capacity: i64,
    chunk_wcmu: &[(i64, i64)],
    shared_layout: &[keleusma::bytecode::SharedSlotLayout],
) -> (i64, i64, i64, bool, bool) {
    use keleusma::bytecode::{BlockType, Op};
    let vsb = keleusma::bytecode::VALUE_SLOT_SIZE_BYTES as i64;
    // The analysed region and the Stream/Reset positions (used only for a Stream chunk's WCET
    // overhead term; a Func/Reentrant chunk analyses its whole op range).
    let (region_start, region_end, sp, rp) = match chunk.block_type {
        BlockType::Stream => {
            let sp = chunk
                .ops
                .iter()
                .position(|o| matches!(o, Op::Stream))
                .expect("Stream op");
            let rp = chunk
                .ops
                .iter()
                .position(|o| matches!(o, Op::Reset))
                .expect("Reset op");
            (sp + 1, rp, sp, rp)
        }
        BlockType::Func | BlockType::Reentrant => (0, chunk.ops.len(), 0, 0),
    };
    assert!(chunk.ops.len() <= 1024, "analyze.kel op-table capacity");
    let m = analyze_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify analyze.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&vm, &mut shared, WA_OP_COUNT, chunk.ops.len() as i64);
    set(&vm, &mut shared, WA_STREAM_POS, sp as i64);
    set(&vm, &mut shared, WA_RESET_POS, rp as i64);
    set(&vm, &mut shared, WA_LOCAL_COUNT, chunk.local_count as i64);
    set(&vm, &mut shared, WA_VSB, vsb);
    set(&vm, &mut shared, WA_ARENA_CAPACITY, arena_capacity);
    set(&vm, &mut shared, WA_REGION_START, region_start as i64);
    set(&vm, &mut shared, WA_REGION_END, region_end as i64);
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        let (opk, slot, cval, cint) = analyze_opk(op, chunk);
        let (growth, shrink) = analyze_stack_effect(op);
        set(&vm, &mut shared, WA_COST + i, op.cost() as i64);
        set(&vm, &mut shared, WA_CLASS + i, class);
        set(&vm, &mut shared, WA_ARG + i, arg);
        set(&vm, &mut shared, WA_GROWTH + i, growth);
        set(&vm, &mut shared, WA_SHRINK + i, shrink);
        set(
            &vm,
            &mut shared,
            WA_HEAP + i,
            analyze_op_heap(op, chunk, shared_layout),
        );
        set(&vm, &mut shared, WA_OPK + i, opk);
        set(&vm, &mut shared, WA_SLOT + i, slot);
        set(&vm, &mut shared, WA_CVAL + i, cval);
        set(&vm, &mut shared, WA_CINT + i, cint);
        // A Call folds in the callee's transitive WCMU (in slots for the stack term, bytes
        // for the heap term). An unresolved callee (shallow mode) contributes zero.
        if let Op::Call(callee, _) = op {
            let (cs, ch) = chunk_wcmu.get(*callee as usize).copied().unwrap_or((0, 0));
            set(&vm, &mut shared, WA_CALLEE_SLOTS + i, cs / vsb);
            set(&vm, &mut shared, WA_CALLEE_HEAP + i, ch);
        }
    }
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call analyze.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected analyze.kel state: {other:?}"),
    }
    let rd = |slot: usize| -> i64 {
        match vm.get_shared(&shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    (
        rd(WA_OUT_WCET),
        rd(WA_OUT_STACK),
        rd(WA_OUT_HEAP),
        rd(WA_OUT_REJECT) != 0,
        rd(WA_OUT_VALID) != 0,
    )
}

/// The analysis view: the per-iteration bounds `(wcet, stack_bytes, heap_bytes, reject)` for a
/// Stream chunk. Shallow (empty resolver) and against an unbounded capacity.
fn analyze_via_kel(chunk: &keleusma::bytecode::Chunk) -> (i64, i64, i64, bool) {
    let (wcet, stack, heap, reject, _valid) = run_analyze_kel(chunk, i64::MAX, &[], &[]);
    (wcet, stack, heap, reject)
}

/// The self-hosted validator verdict for a Stream chunk against `arena_capacity` (shallow):
/// true iff analyze.kel proves a finite per-iteration bound whose stack-plus-heap budget fits.
fn validate_via_kel(chunk: &keleusma::bytecode::Chunk, arena_capacity: i64) -> bool {
    run_analyze_kel(chunk, arena_capacity, &[], &[]).4
}

/// The self-hosted drop-in replacement for `verify_resource_bounds`: analyze.kel decides each
/// chunk's WCMU transitively (callee bodies folded at every `Op::Call`, resolved in
/// topological order so callees precede callers), and the module is admitted iff no chunk has
/// an inextractable bound and every Stream chunk's budget fits `arena_capacity`.
fn validate_module_via_kel(module: &Module, arena_capacity: i64) -> bool {
    use keleusma::bytecode::{BlockType, Op};
    let n = module.chunks.len();
    // Topological order over the call graph (callees before callers), rejecting recursion --
    // the same DFS postorder `topological_call_order` computes.
    let mut visited = vec![0u8; n]; // 0 unseen, 1 on-stack, 2 done
    let mut order = Vec::new();
    fn visit(module: &Module, i: usize, visited: &mut [u8], order: &mut Vec<usize>) -> bool {
        if visited[i] == 1 {
            return false; // cycle
        }
        if visited[i] == 2 {
            return true;
        }
        visited[i] = 1;
        for op in &module.chunks[i].ops {
            if let Op::Call(callee, _) = op {
                let c = *callee as usize;
                if c < module.chunks.len() && !visit(module, c, visited, order) {
                    return false;
                }
            }
        }
        visited[i] = 2;
        order.push(i);
        true
    }
    for i in 0..n {
        if visited[i] != 2 && !visit(module, i, &mut visited, &mut order) {
            return false; // a recursive call graph is inadmissible
        }
    }
    // Resolve each chunk's transitive WCMU in topological order, then admit.
    let shared_layout = module
        .data_layout
        .as_ref()
        .map_or(&[][..], |dl| &dl.shared_layout);
    let mut chunk_wcmu = vec![(0i64, 0i64); n];
    let mut valid = true;
    for &idx in &order {
        let chunk = &module.chunks[idx];
        let (_wcet, stack, heap, reject, chunk_valid) =
            run_analyze_kel(chunk, arena_capacity, &chunk_wcmu, shared_layout);
        if reject {
            valid = false; // an inextractable bound anywhere fails module_wcmu
        }
        if chunk.block_type == BlockType::Stream && !chunk_valid {
            valid = false; // a Stream chunk whose transitive budget exceeds the capacity
        }
        chunk_wcmu[idx] = (stack, heap);
    }
    valid
}

// analyze.kel reproduces both `wcet_stream_iteration` and `wcmu_stream_iteration` exactly for
// every stage's loop-free Stream chunk. Reference WCET is 154/43/14/14 and WCMU stack bytes
// 288/128/96/64 (heap 0) for lexer/reconstruct/codegen/parse.
#[test]
fn analyze_via_kel_matches_the_reference() {
    for path in [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ] {
        let src = std::fs::read_to_string(path).expect("read stage");
        let m = compile_src(&src);
        for c in &m.chunks {
            if c.block_type != keleusma::bytecode::BlockType::Stream {
                continue;
            }
            let (wcet, stack, heap, reject) = analyze_via_kel(c);
            assert!(!reject, "unexpected reject for {path} `{}`", c.name);
            let ref_wcet =
                keleusma::verify::wcet_stream_iteration(c).expect("reference wcet") as i64;
            let (ref_stack, ref_heap) =
                keleusma::verify::wcmu_stream_iteration(c).expect("reference wcmu");
            assert_eq!(wcet, ref_wcet, "wcet for {path} chunk `{}`", c.name);
            assert_eq!(
                stack, ref_stack as i64,
                "wcmu stack for {path} `{}`",
                c.name
            );
            assert_eq!(heap, ref_heap as i64, "wcmu heap for {path} `{}`", c.name);
        }
    }
}

// analyze.kel handles `loop` regions with a self-hosted iteration-bound extraction. These
// synthetic Stream programs carry a `for .. limit` loop in the loop body (the compiler emits
// the canonical for-range header analyze.kel recognizes), and analyze.kel must reproduce
// `wcet_stream_iteration`/`wcmu_stream_iteration` exactly for each -- exercising the loop
// multiply, the runtime-range `break` inside the loop, and nested control flow.
#[test]
fn analyze_via_kel_matches_the_reference_for_loops() {
    let cases: &[&str] = &[
        // A plain accumulation loop.
        r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word, arr: [Word; 16] }
loop main(resume: Word) -> Word {
    d.s = 0;
    for i in 0..10 limit 16 { d.s = d.s + io.arr[i]; }
    io.out = d.s;
    yield d.s
}"#,
        // A loop whose body carries a conditional (nested if inside the loop).
        r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word, arr: [Word; 32] }
loop main(resume: Word) -> Word {
    d.s = 0;
    for i in 0..20 limit 32 {
        if io.arr[i] > 0 { d.s = d.s + io.arr[i]; } else { d.s = d.s + 1; }
    }
    io.out = d.s;
    yield d.s
}"#,
        // Straight-line code before and after the loop, and a different cap.
        r#"require word >= 32;
private data d { s: Word, t: Word }
shared data io { out: Word, arr: [Word; 8] }
loop main(resume: Word) -> Word {
    d.t = 7;
    d.s = d.t;
    for i in 0..5 limit 8 { d.s = d.s + io.arr[i] * 2; }
    io.out = d.s + d.t;
    yield d.s
}"#,
        // A nested loop: the inner loop is consumed within the outer loop body (each loop
        // frame consumes only its own body's breaks), exercising frame-stack recursion.
        r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word }
loop main(resume: Word) -> Word {
    d.s = 0;
    for i in 0..3 limit 4 {
        for j in 0..3 limit 4 { d.s = d.s + 1; }
    }
    io.out = d.s;
    yield d.s
}"#,
        // A for-in over a literal array: the compiler bakes the length as a constant (the
        // range end), so the same pattern-1 header applies; the array's NewComposite
        // exercises the heap term (a non-zero WCMU heap).
        r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word }
loop main(resume: Word) -> Word {
    d.s = 0;
    let a = [1, 2, 3, 4];
    for x in a { d.s = d.s + x; }
    io.out = d.s;
    yield d.s
}"#,
    ];
    for src in cases {
        let m = compile_src(src);
        for c in &m.chunks {
            if c.block_type != keleusma::bytecode::BlockType::Stream {
                continue;
            }
            let (wcet, stack, heap, reject) = analyze_via_kel(c);
            assert!(!reject, "unexpected reject for `{src}`");
            let ref_wcet =
                keleusma::verify::wcet_stream_iteration(c).expect("reference wcet") as i64;
            let (ref_stack, ref_heap) =
                keleusma::verify::wcmu_stream_iteration(c).expect("reference wcmu");
            assert_eq!(wcet, ref_wcet, "wcet for `{src}`");
            assert_eq!(stack, ref_stack as i64, "wcmu stack for `{src}`");
            assert_eq!(heap, ref_heap as i64, "wcmu heap for `{src}`");
        }
    }
}

// analyze.kel is itself compiled byte-identically by the self-hosted pipeline -- the same
// fixed-point property the other four stages have. This makes the resource-bound analysis a
// fifth self-compiling Keleusma stage, not merely Keleusma code the reference compiles.
#[test]
fn self_host_compiles_analyze_kel_byte_identically() {
    let src = std::fs::read_to_string("compiler/kel/analyze.kel").expect("read analyze.kel");
    let module = self_host_compile(&src);
    let reference = compile_src(&src);
    assert_eq!(module.chunks.len(), reference.chunks.len(), "chunk count");
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order");
        assert_eq!(m.ops, r.ops, "ops for chunk `{}`", r.name);
        assert_eq!(m.constants, r.constants, "pool for chunk `{}`", r.name);
        assert_eq!(
            m.local_count, r.local_count,
            "local_count for chunk `{}`",
            r.name
        );
    }
}

// The fail-closed reject path: a loop whose bound cannot be statically extracted must be
// rejected, not silently under-bounded. Every compilable Keleusma Stream loop emits the
// canonical for-range header, so no source program reaches the reject path; this constructs
// it by mutating a compiled loop's header (replacing the induction `GetLocal` with a
// non-canonical op) and asserts analyze.kel rejects exactly when the reference does.
#[test]
fn analyze_via_kel_rejects_an_inextractable_loop_like_the_reference() {
    use keleusma::bytecode::Op;
    let src = r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word, arr: [Word; 16] }
loop main(resume: Word) -> Word {
    d.s = 0;
    for i in 0..10 limit 16 { d.s = d.s + io.arr[i]; }
    io.out = d.s;
    yield d.s
}"#;
    let m = compile_src(src);
    let mut chunk = m
        .chunks
        .iter()
        .find(|c| c.block_type == keleusma::bytecode::BlockType::Stream)
        .expect("stream chunk")
        .clone();
    // Sanity: the unmutated loop extracts a bound (reference and analyze both succeed).
    assert!(keleusma::verify::wcet_stream_iteration(&chunk).is_ok());
    assert!(
        !analyze_via_kel(&chunk).3,
        "unmutated loop should not reject"
    );

    // Break the canonical header: the op after Loop is the induction `GetLocal`. Replace it
    // with a non-GetLocal so the bound is inextractable while the body still falls through.
    let loop_ip = chunk
        .ops
        .iter()
        .position(|o| matches!(o, Op::Loop(_)))
        .expect("loop op");
    assert!(matches!(chunk.ops[loop_ip + 1], Op::GetLocal(_)));
    chunk.ops[loop_ip + 1] = Op::Const(0);

    // The reference now rejects (no statically extractable iteration bound); so must analyze.
    assert!(
        keleusma::verify::wcet_stream_iteration(&chunk).is_err(),
        "reference should reject the mutated loop"
    );
    let (_, _, _, reject) = analyze_via_kel(&chunk);
    assert!(reject, "analyze.kel should reject the mutated loop");
}

// Native-call stack effects. No compiler stage calls a host native, so this injects a
// CallVerifiedNative op (with the empty resolver both analyze and the reference use, no
// registration is needed) and asserts analyze.kel reproduces the reference's WCET and WCMU
// on the same mutated chunk -- for a plain native (n = 2) and an error-reify native (n =
// 0x82), whose high bit the reference's native WCMU arm folds into the pop count.
#[test]
fn analyze_via_kel_matches_the_reference_for_native_calls() {
    use keleusma::bytecode::Op;
    let src = r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word }
loop main(resume: Word) -> Word {
    d.s = 5;
    io.out = d.s;
    yield d.s
}"#;
    for n_args in [2u8, 0x82u8] {
        let m = compile_src(src);
        let mut chunk = m
            .chunks
            .iter()
            .find(|c| c.block_type == keleusma::bytecode::BlockType::Stream)
            .expect("stream chunk")
            .clone();
        // Replace the first Const in the body with a native call.
        let at = chunk
            .ops
            .iter()
            .position(|o| matches!(o, Op::Const(_)))
            .expect("a Const to mutate");
        chunk.ops[at] = Op::CallVerifiedNative(0, n_args);

        let (wcet, stack, heap, reject) = analyze_via_kel(&chunk);
        assert!(!reject, "native chunk should not reject (n={n_args:#x})");
        let ref_wcet =
            keleusma::verify::wcet_stream_iteration(&chunk).expect("reference wcet") as i64;
        let (ref_stack, ref_heap) =
            keleusma::verify::wcmu_stream_iteration(&chunk).expect("reference wcmu");
        assert_eq!(wcet, ref_wcet, "wcet with native n={n_args:#x}");
        assert_eq!(
            stack, ref_stack as i64,
            "wcmu stack with native n={n_args:#x}"
        );
        assert_eq!(heap, ref_heap as i64, "wcmu heap with native n={n_args:#x}");
    }
}

// The self-hosted validator: analyze.kel's `out_valid` verdict for a Stream chunk against an
// arena capacity must agree with the reference `verify_resource_bounds`, which admits a
// module iff every Stream chunk has a provable finite bound whose stack-plus-heap budget
// fits. The programs are call-free (no `Op::Call`), so the shallow per-iteration WCMU the
// validator uses equals the transitive `module_wcmu` the reference uses, and the two verdicts
// must match exactly -- at capacities just below, at, and above the budget, and for an
// inextractable (mutated) loop that both must reject.
#[test]
fn validate_via_kel_matches_verify_resource_bounds() {
    use keleusma::bytecode::{BlockType, Op};
    // Call-free single-Stream programs: one straight-line, one with a bounded loop.
    let srcs: &[&str] = &[
        r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word }
loop main(resume: Word) -> Word {
    d.s = 5;
    io.out = d.s;
    yield d.s
}"#,
        r#"require word >= 32;
private data d { s: Word }
shared data io { out: Word, arr: [Word; 16] }
loop main(resume: Word) -> Word {
    d.s = 0;
    for i in 0..10 limit 16 { d.s = d.s + io.arr[i]; }
    io.out = d.s;
    yield d.s
}"#,
    ];
    for src in srcs {
        let m = compile_src(src);
        let main = m
            .chunks
            .iter()
            .find(|c| c.block_type == BlockType::Stream)
            .expect("stream chunk");
        assert!(
            !main.ops.iter().any(|o| matches!(o, Op::Call(..))),
            "test program must be call-free for the shallow validator to match the reference"
        );
        let (stack, heap) = keleusma::verify::wcmu_stream_iteration(main).expect("wcmu");
        let total = (stack + heap) as i64;
        // Below, at, and above the exact budget.
        for cap in [total - 1, total, total + 4096] {
            let self_valid = validate_via_kel(main, cap);
            let ref_valid = keleusma::verify::verify_resource_bounds(&m, cap as usize).is_ok();
            assert_eq!(self_valid, ref_valid, "validity at cap={cap} for `{src}`");
        }
    }

    // An inextractable loop: both the validator and the reference must reject it (invalid)
    // regardless of capacity.
    let src = srcs[1];
    let m = compile_src(src);
    let mut broken = m.clone();
    let ci = broken
        .chunks
        .iter()
        .position(|c| c.block_type == BlockType::Stream)
        .unwrap();
    let loop_ip = broken.chunks[ci]
        .ops
        .iter()
        .position(|o| matches!(o, Op::Loop(_)))
        .unwrap();
    broken.chunks[ci].ops[loop_ip + 1] = Op::Const(0);
    let big = 1i64 << 30;
    assert!(
        !validate_via_kel(&broken.chunks[ci], big),
        "validator must reject an inextractable loop"
    );
    assert!(
        keleusma::verify::verify_resource_bounds(&broken, big as usize).is_err(),
        "reference must reject an inextractable loop"
    );
}

// The self-hosted validator as a drop-in replacement for `verify_resource_bounds` on
// CALL-BEARING modules. analyze.kel folds each callee's transitive WCMU at every Op::Call
// (resolved in topological order), so the module verdict must match the reference for the
// four self-hosted stage modules (whose `main` calls many helpers, some with loops) and for
// synthetic call programs -- at capacities below, at, and above the module's Stream budget.
#[test]
fn validate_module_via_kel_is_a_drop_in_for_verify_resource_bounds() {
    use keleusma::bytecode::BlockType;
    let mut modules: Vec<Module> = Vec::new();
    for path in [
        "compiler/kel/lexer.kel",
        "compiler/kel/reconstruct.kel",
        "compiler/kel/codegen.kel",
        "compiler/kel/parse.kel",
    ] {
        modules.push(compile_src(
            &std::fs::read_to_string(path).expect("read stage"),
        ));
    }
    let synth: &[&str] = &[
        // main -> a plain Func helper.
        r#"require word >= 32;
shared data io { out: Word }
fn add3(a: Word, b: Word, c: Word) -> Word { a + b + c }
loop main(resume: Word) -> Word { io.out = add3(1, 2, 3); yield io.out }"#,
        // main -> a helper that itself contains a bounded loop.
        r#"require word >= 32;
shared data io { out: Word }
fn work(n: Word) -> Word {
    for i in 0..n limit 16 { let z = i + i; }
    n + 1
}
loop main(resume: Word) -> Word { io.out = work(10); yield io.out }"#,
        // A two-level call chain (main -> f -> g).
        r#"require word >= 32;
shared data io { out: Word }
fn g(x: Word) -> Word { x + 1 }
fn f(x: Word) -> Word { g(x) + g(x + 1) }
loop main(resume: Word) -> Word { io.out = f(3); yield io.out }"#,
    ];
    for s in synth {
        modules.push(compile_src(s));
    }
    for m in &modules {
        // Confirm the module actually exercises calls, then find its max Stream budget.
        assert!(
            m.chunks.iter().any(|c| c
                .ops
                .iter()
                .any(|o| matches!(o, keleusma::bytecode::Op::Call(..)))),
            "test module must contain at least one Op::Call"
        );
        let per = keleusma::verify::module_wcmu(m, &[]).expect("reference module_wcmu");
        let mut max_total = 0i64;
        for (i, c) in m.chunks.iter().enumerate() {
            if c.block_type == BlockType::Stream {
                max_total = max_total.max((per[i].0 + per[i].1) as i64);
            }
        }
        for cap in [max_total - 1, max_total, max_total + 4096] {
            let self_valid = validate_module_via_kel(m, cap);
            let ref_valid = keleusma::verify::verify_resource_bounds(m, cap as usize).is_ok();
            assert_eq!(self_valid, ref_valid, "validity at cap={cap}");
        }
    }
}

// The composite-shared-read copy-out heap term: reading a whole flat-composite shared slot
// (`GetData` on a struct/array slot) copies its bytes to the arena, which `module_wcmu`
// counts and `wcmu_stream_iteration` (empty resolver) does not. analyze.kel's transitive
// validator folds the same term, so it must still match `verify_resource_bounds` for a
// program whose shared read is composite -- where the shallow bound would under-count.
#[test]
fn validate_module_via_kel_folds_composite_shared_copyout() {
    use keleusma::bytecode::BlockType;
    let src = r#"require word >= 32;
struct P { x: Word, y: Word }
shared data io { p: P, out: Word }
loop main(resume: Word) -> Word {
    let q = io.p;
    io.out = q.x + q.y;
    yield io.out
}"#;
    let m = compile_src(src);
    // Confirm this program actually exercises a non-zero composite copy-out: the transitive
    // module_wcmu heap for the Stream chunk must exceed the shallow wcmu_stream_iteration heap.
    let per = keleusma::verify::module_wcmu(&m, &[]).expect("module_wcmu");
    let mut max_total = 0i64;
    for (i, c) in m.chunks.iter().enumerate() {
        if c.block_type == BlockType::Stream {
            let (_s, shallow_heap) = keleusma::verify::wcmu_stream_iteration(c).expect("shallow");
            assert!(
                per[i].1 > shallow_heap,
                "expected a composite copy-out to make transitive heap exceed the shallow heap"
            );
            max_total = max_total.max((per[i].0 + per[i].1) as i64);
        }
    }
    for cap in [max_total - 1, max_total, max_total + 4096] {
        let self_valid = validate_module_via_kel(&m, cap);
        let ref_valid = keleusma::verify::verify_resource_bounds(&m, cap as usize).is_ok();
        assert_eq!(
            self_valid, ref_valid,
            "composite-shared validity at cap={cap}"
        );
    }
}
