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
use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `ast` block's field order in codegen.kel
// (one slot per scalar, arrays contiguous). root=0, then the four length-512 node
// arrays (`kinds`/`args`/`lhs`/`rhs`, sized for the stage's largest own function),
// then the length-64 `call_args`/`for_parts`/`match_parts` side arrays, then
// param_count.
const KINDS: usize = 1;
const ARGS: usize = 513;
const LHS: usize = 1025;
const RHS: usize = 1537;
const CALL_ARGS: usize = 2049;
const FOR_PARTS: usize = 2113;
const MATCH_PARTS: usize = 2177;
const LIMIT_PARTS: usize = 2241;
const HEAD_PARTS: usize = 2305;
const PARAM_COUNT: usize = 2369;
const CATEGORY: usize = 2370;

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
                // IndexAssignIn (kind 14): arg = base + len*65536, value = store node.
                stmts.push((14, base + len * 65536, store_node));
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
        27 => Op::GetData(operand as u16),
        28 => Op::SetData(operand as u16),
        29 => Op::GetDataIndexed((operand % 65536) as u16, (operand / 65536) as u16),
        30 => Op::SetDataIndexed((operand % 65536) as u16, (operand / 65536) as u16),
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
    for _ in 0..4096 {
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
            if body.nodes.len() > 512
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
    const EXPECTED_SELF_COMPILE: usize = 34;
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

// Lexer `src` block slots: len(1) + bytes(4096) then the intern table.
const BR_LEX_ISTART: usize = 1 + 4096;
const BR_LEX_ILEN: usize = 1 + 4096 + 512;
const BR_LEX_ICOUNT: usize = 1 + 4096 + 512 + 512;
// Parser `toks` block slots.
const BR_P_LEN: usize = 0;
const BR_P_KINDS: usize = 1;
const BR_P_VALS: usize = 1 + 2048;
const BR_P_LIMIT_ID: usize = 1 + 2048 + 2048;
const BR_P_CHUNK_COUNT: usize = 1 + 2048 + 2048 + 1;
const BR_P_CHUNKS: usize = 1 + 2048 + 2048 + 2;
const BR_P_REQUIRE_ID: usize = 1 + 2048 + 2048 + 2 + 256;

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

/// Drive lexer.kel then parse.kel over a single-function `src`, returning that
/// function's body records (the postorder (kind, arg) node stream) and its value
/// parameter count. All parser inputs come from the lexer's output.
fn parse_function_records(src: &str) -> (Vec<(i64, i64)>, usize) {
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
        vm.set_shared(&mut shared, BR_P_KINDS + i, Value::Int(k))
            .unwrap();
        vm.set_shared(&mut shared, BR_P_VALS + i, Value::Int(v))
            .unwrap();
    }

    let mut records: Vec<(i64, i64)> = Vec::new();
    let mut params = 0usize;
    let (mut in_body, mut in_data, mut in_enum, mut in_use) = (false, false, false, false);
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(tokens.len() * 4 + 64) {
        if let VmState::Yielded(Value::Int(w)) = state {
            let (code, val) = (w.rem_euclid(64), w.div_euclid(64));
            if in_body {
                match code {
                    0 => {}
                    15 => in_body = false,
                    _ => records.push((code, val)),
                }
            } else if in_data {
                in_data = code != 5;
            } else if in_enum {
                in_enum = code != 5;
            } else if in_use {
                in_use = code != 5;
            } else {
                match code {
                    4 => params += 1,
                    9 => in_data = true,
                    10 => in_use = true,
                    12 => in_enum = true,
                    16 => in_body = true,
                    15 => return (records, params),
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
/// The root is the one node left on the stack. This increment covers the
/// arithmetic, comparison, bitwise, unary, short-circuit, block, and if kinds; a
/// record of any other kind is rejected until a later increment adds it.
fn reconstruct_body(records: &[(i64, i64)], category: i64) -> Body {
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<i64> = Vec::new();
    for &(kind, arg) in records {
        let idx = match kind {
            1 | 2 | 20 => {
                nodes.push(Node {
                    kind,
                    arg,
                    lhs: 0,
                    rhs: 0,
                });
                (nodes.len() - 1) as i64
            }
            3 | 5 | 8 | 9 | 21 => {
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
            6 | 10 => {
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
            other => panic!("reconstruct_body: unsupported node kind {other}"),
        };
        stack.push(idx);
    }
    assert_eq!(stack.len(), 1, "exactly one root node remains");
    Body {
        nodes,
        call_args: Vec::new(),
        for_parts: Vec::new(),
        match_parts: Vec::new(),
        limit_parts: Vec::new(),
        head_parts: Vec::new(),
        category,
        root: stack[0],
    }
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
        let (records, param_count) = parse_function_records(src);
        let body = reconstruct_body(&records, 0);
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
        let (records, param_count) = parse_function_records(src);
        let body = reconstruct_body(&records, 0);
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
