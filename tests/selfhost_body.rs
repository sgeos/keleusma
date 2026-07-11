//! Body-expression parser (`compiler/kel/body.kel`), increment 16: a function body over
//! the full operator-precedence expression grammar, scalar and indexed data-field
//! reads, `let` bindings and scalar data-field assignments, the `if`/`else` conditional
//! with nested statement-block branches, function calls, the bounded
//! `for v in start..end limit CAP { body }` loop, and a statement-only block whose
//! implicit value is Unit — lowered to the abstract-syntax node forest codegen consumes,
//! with `call_args` and `limit_parts` side arrays.
//!
//! A throwaway adapter tokenises the program, feeds the function body's tokens, the
//! parameter-name table, and the data-field layout (computed from the compiled
//! program's flat data segment) to the `body.kel` `loop`, and decodes the postorder
//! node-record stream into a node forest. Each leaf record (Literal, Local, DataRead)
//! pushes a node and its index onto a stack; an interior binary record (BinOp, Andalso,
//! Orelse) pops two operands, a unary record (Not, Neg) pops one, and a LetIn record
//! pops the running continuation and the bound value — each pushing the combined node.
//! The forest is checked against a reference flattening of the same body's block, with
//! parameters occupying the first frame slots and each `let` binding the next, and a
//! `d.f` read resolved to its data-segment slot — the same lowering the codegen
//! conformance harness performs — so the operator grammar, the `let` slot allocation
//! and scope resolution, the LetIn cons-list, and the data-field slot resolution are all
//! verified against the reference. The drivers run on a wider-stack thread because the
//! host compiler's recursive-descent parse of the deeply nested stage overflows the
//! default test-thread stack.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::ast::{BinOp, Block, Expr, Iterable, Literal, Pattern, Stmt, UnaryOp};
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `src` block in body.kel: len at 0, the two
// length-512 token arrays, the parameter count, the length-32 parameter table, then
// the field count and the four length-64 field-layout arrays.
const LEN: usize = 0;
const KINDS: usize = 1;
const VALS: usize = 1 + 512;
const PARAM_COUNT: usize = 1 + 512 + 512;
const PARAMS: usize = 1 + 512 + 512 + 1;
const FIELD_COUNT: usize = 1 + 512 + 512 + 1 + 32;
const FDATA: usize = 1 + 512 + 512 + 1 + 32 + 1;
const FFIELD: usize = 1 + 512 + 512 + 1 + 32 + 1 + 64;
const FBASE: usize = 1 + 512 + 512 + 1 + 32 + 1 + 64 + 64;
const FLEN: usize = 1 + 512 + 512 + 1 + 32 + 1 + 64 + 64 + 64;
const CHUNK_COUNT: usize = 1 + 512 + 512 + 1 + 32 + 1 + 64 + 64 + 64 + 64;
const CHUNKS: usize = 1 + 512 + 512 + 1 + 32 + 1 + 64 + 64 + 64 + 64 + 1;
const LIMIT_ID: usize = 1 + 512 + 512 + 1 + 32 + 1 + 64 + 64 + 64 + 64 + 1 + 64;

/// One node of the abstract-syntax forest: the codegen contract's `(kind, arg, lhs,
/// rhs)`. A leaf has `lhs == rhs == 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Node {
    kind: i64,
    arg: i64,
    lhs: i64,
    rhs: i64,
}

/// The flat data-segment slot names of `program`, from the compiled data layout. A
/// program with no `data` block has no layout, and is deliberately not compiled so a
/// body exercising a construct the compiler cannot yet lower remains testable.
fn data_slot_names(program: &keleusma::ast::Program) -> Vec<String> {
    if program.data_decls.is_empty() {
        return Vec::new();
    }
    compile(program)
        .expect("compile program")
        .data_layout
        .as_ref()
        .map(|dl| dl.slots.iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default()
}

/// The parameter name at a slot, for the reference scope.
fn param_name(p: &keleusma::ast::Param) -> &str {
    match &p.pattern {
        Pattern::Variable(n, _) => n,
        other => panic!("test uses simple parameter patterns only, got {other:?}"),
    }
}

/// Run `work` on a thread with a generous stack. `body.kel`'s deeply nested statement
/// dispatch makes the host compiler's recursive-descent parse of it exceed the default
/// test-thread stack; a wider stack keeps the drivers robust as the stage grows.
fn with_big_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(work)
        .expect("spawn")
        .join()
        .expect("join")
}

/// Drive `body.kel` over the caller's body in `func_src`, returning the node forest,
/// the `call_args` side array, and the root index.
fn run_body(func_src: &str) -> (Vec<Node>, Vec<i64>, Vec<i64>, i64) {
    let src = func_src.to_string();
    with_big_stack(move || run_body_inner(&src))
}

/// The body of [`run_body`]; identifier names are interned so the parameter table and
/// the body's identifier tokens share ids.
fn run_body_inner(func_src: &str) -> (Vec<Node>, Vec<i64>, Vec<i64>, i64) {
    let program = parse(&tokenize(func_src).expect("lex")).expect("parse");
    // The caller whose body is parsed is the last function; the chunk table is every
    // function name in declaration order (matching the compiler's chunk indices).
    let f = program.functions.last().expect("a function");
    let chunk_names: Vec<String> = program.functions.iter().map(|f| f.name.clone()).collect();
    let param_names: Vec<String> = f.params.iter().map(|p| param_name(p).to_string()).collect();

    // Tokenise the whole function, interning identifiers, then keep the tokens from the
    // body's opening `{` (the first `{`, since a signature contains none).
    let mut names: Vec<String> = Vec::new();
    let mut intern = |s: &str| -> i64 {
        if let Some(i) = names.iter().position(|n| n == s) {
            i as i64
        } else {
            names.push(s.to_string());
            (names.len() - 1) as i64
        }
    };
    let tokens = tokenize(func_src).expect("lex");
    let mut kinds = Vec::new();
    let mut vals = Vec::new();
    for tok in &tokens {
        let (kind, val) = match &tok.kind {
            TokenKind::Fn => (0, 0),
            TokenKind::Yield => (5, 0),
            TokenKind::Loop => (6, 0),
            TokenKind::Dot => (40, 0),
            TokenKind::LBracket => (41, 0),
            TokenKind::RBracket => (42, 0),
            TokenKind::If => (43, 0),
            TokenKind::Else => (44, 0),
            TokenKind::Comma => (10, 0),
            TokenKind::For => (45, 0),
            TokenKind::In => (46, 0),
            TokenKind::DotDot => (47, 0),
            TokenKind::LowerIdent(s) | TokenKind::UpperIdent(s) => (1, intern(s)),
            TokenKind::LBrace => (2, 0),
            TokenKind::RBrace => (3, 0),
            TokenKind::LParen => (7, 0),
            TokenKind::RParen => (8, 0),
            TokenKind::IntLit(n) => (12, *n),
            TokenKind::Plus => (21, 0),
            TokenKind::Minus => (22, 0),
            TokenKind::Star => (23, 0),
            TokenKind::Slash => (24, 0),
            TokenKind::Percent => (25, 0),
            TokenKind::EqEq => (26, 0),
            TokenKind::NotEq => (27, 0),
            TokenKind::Lt => (28, 0),
            TokenKind::Gt => (29, 0),
            TokenKind::LtEq => (30, 0),
            TokenKind::GtEq => (31, 0),
            TokenKind::Not => (32, 0),
            TokenKind::Band => (33, 0),
            TokenKind::Bor => (34, 0),
            TokenKind::Bxor => (35, 0),
            TokenKind::Andalso => (36, 0),
            TokenKind::Orelse => (37, 0),
            TokenKind::Let => (38, 0),
            TokenKind::Semicolon => (39, 0),
            TokenKind::Eq => (17, 0),
            TokenKind::Colon => (9, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    // The caller's body opens at the first `{` after the last function keyword.
    let fn_kw = kinds
        .iter()
        .rposition(|&k| k == 0 || k == 5 || k == 6)
        .expect("a function keyword");
    let body_rel = kinds[fn_kw..]
        .iter()
        .position(|&k| k == 2)
        .expect("a function body opens with `{`");
    let body_start = fn_kw + body_rel;
    let body_kinds = &kinds[body_start..];
    let body_vals = &vals[body_start..];
    let param_ids: Vec<i64> = param_names.iter().map(|n| intern(n)).collect();
    let chunk_ids: Vec<i64> = chunk_names.iter().map(|n| intern(n)).collect();
    // The interned id of the contextual `limit` keyword, so the parser can recognise
    // the end of a loop's range (harmless when the program has no loop).
    let limit_id = intern("limit");

    // The data-field layout, from the compiled program's flat data segment. Each slot
    // name is `data.field` (scalar) or `data.field[k]` (array element); the field
    // table records, per distinct `(data, field)`, its element-0 slot and slot count.
    // A program with no data blocks needs no layout (and is only parsed, not compiled,
    // so a body exercising a not-yet-compilable construct is still testable).
    let data_slots = data_slot_names(&program);
    // (data id, field id, base slot, length).
    let mut fields: Vec<(i64, i64, i64, i64)> = Vec::new();
    for (i, slot) in data_slots.iter().enumerate() {
        let (data, rest) = slot.split_once('.').expect("a data slot name has a dot");
        let field = rest.split('[').next().unwrap();
        let did = intern(data);
        let fid = intern(field);
        if let Some(e) = fields
            .iter_mut()
            .find(|(d, f, _, _)| *d == did && *f == fid)
        {
            e.3 += 1;
        } else {
            fields.push((did, fid, i as i64, 1));
        }
    }

    let stage = std::fs::read_to_string("compiler/kel/body.kel").expect("read body.kel");
    let module = compile(&parse(&tokenize(&stage).expect("lex body.kel")).expect("parse body.kel"))
        .expect("compile body.kel");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify body.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, LEN, Value::Int(body_kinds.len() as i64))
        .expect("len");
    for (i, (&k, &v)) in body_kinds.iter().zip(body_vals.iter()).enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(k))
            .expect("kind");
        vm.set_shared(&mut shared, VALS + i, Value::Int(v))
            .expect("val");
    }
    vm.set_shared(&mut shared, PARAM_COUNT, Value::Int(param_ids.len() as i64))
        .expect("param_count");
    for (i, &id) in param_ids.iter().enumerate() {
        vm.set_shared(&mut shared, PARAMS + i, Value::Int(id))
            .expect("param");
    }
    vm.set_shared(&mut shared, FIELD_COUNT, Value::Int(fields.len() as i64))
        .expect("field_count");
    for (i, &(did, fid, base, len)) in fields.iter().enumerate() {
        vm.set_shared(&mut shared, FDATA + i, Value::Int(did))
            .expect("fdata");
        vm.set_shared(&mut shared, FFIELD + i, Value::Int(fid))
            .expect("ffield");
        vm.set_shared(&mut shared, FBASE + i, Value::Int(base))
            .expect("fbase");
        vm.set_shared(&mut shared, FLEN + i, Value::Int(len))
            .expect("flen");
    }
    vm.set_shared(&mut shared, CHUNK_COUNT, Value::Int(chunk_ids.len() as i64))
        .expect("chunk_count");
    for (i, &id) in chunk_ids.iter().enumerate() {
        vm.set_shared(&mut shared, CHUNKS + i, Value::Int(id))
            .expect("chunk");
    }
    vm.set_shared(&mut shared, LIMIT_ID, Value::Int(limit_id))
        .expect("limit_id");

    let mut nodes: Vec<Node> = Vec::new();
    let mut call_args: Vec<i64> = Vec::new();
    let mut limit_parts: Vec<i64> = Vec::new();
    let mut slot_buf: Vec<i64> = Vec::new();
    let mut stack: Vec<i64> = Vec::new();
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    // Each token yields at least once, and an operator spreads its shunting-yard
    // pops and push, a `let` its drain, and the tail its LetIn fold, across extra
    // iterations; with a Reset between every yield, the state budget is a generous
    // multiple of the token count.
    for _ in 0..(body_kinds.len() * 12 + 128) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let kind = w.rem_euclid(64);
                let arg = w.div_euclid(64);
                match kind {
                    0 => {} // PENDING
                    1 | 2 | 11 | 20 => {
                        // A leaf node: Literal (1), Local (2), DataRead (11), or Unit
                        // (20, a statement-only block's implicit unit value). Push it and
                        // its index.
                        nodes.push(Node {
                            kind,
                            arg,
                            lhs: 0,
                            rhs: 0,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    3 | 8 | 9 => {
                        // An interior binary node: BinOp (3), Andalso (8), or Orelse
                        // (9). Pop the two operands (rhs then lhs).
                        let rhs = stack.pop().expect("a binary node needs a right operand");
                        let lhs = stack.pop().expect("a binary node needs a left operand");
                        nodes.push(Node {
                            kind,
                            arg,
                            lhs,
                            rhs,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    6 | 10 => {
                        // A unary node: Not (6) or Neg (10). Pop one operand into `lhs`.
                        let operand = stack.pop().expect("a unary operator needs an operand");
                        nodes.push(Node {
                            kind,
                            arg: 0,
                            lhs: operand,
                            rhs: 0,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    13 => {
                        // An IndexRead: pop the index expression into `lhs`; arg is the
                        // packed base + len*65536.
                        let index = stack.pop().expect("IndexRead needs an index");
                        nodes.push(Node {
                            kind,
                            arg,
                            lhs: index,
                            rhs: 0,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    4 => {
                        // An If: pop the three sub-nodes (else, then, cond). The
                        // condition goes in `arg`, then in `lhs`, else in `rhs`.
                        let els = stack.pop().expect("If needs an else branch");
                        let then = stack.pop().expect("If needs a then branch");
                        let cond = stack.pop().expect("If needs a condition");
                        nodes.push(Node {
                            kind,
                            arg: cond,
                            lhs: then,
                            rhs: els,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    5 | 12 => {
                        // A statement node: LetIn (5) or DataFieldAssignIn (12). Pop the
                        // continuation (rhs) and the bound/assigned value (lhs); arg is
                        // the local or data slot.
                        let rhs = stack.pop().expect("a statement node needs a continuation");
                        let lhs = stack.pop().expect("a statement node needs a value");
                        nodes.push(Node {
                            kind,
                            arg,
                            lhs,
                            rhs,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    7 => {
                        // A Call: arg packs chunk + count*256. Pop `count` argument
                        // nodes (reversed to source order) into call_args, recording the
                        // start; lhs = start, rhs = count, node arg = chunk index.
                        let chunk = arg.rem_euclid(256);
                        let count = arg.div_euclid(256);
                        let start = call_args.len() as i64;
                        let mut popped: Vec<i64> = (0..count)
                            .map(|_| stack.pop().expect("a call argument"))
                            .collect();
                        popped.reverse();
                        call_args.extend(popped);
                        nodes.push(Node {
                            kind,
                            arg: chunk,
                            lhs: start,
                            rhs: count,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    23 => {
                        // A ForLimit statement (fold): pop the continuation; lhs is an
                        // unused placeholder (0); arg is the limit_parts entry start.
                        let cont = stack.pop().expect("ForLimit needs a continuation");
                        nodes.push(Node {
                            kind,
                            arg,
                            lhs: 0,
                            rhs: cont,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    32 => {
                        // A loop slot; the host collects five before the build signal.
                        slot_buf.push(arg);
                    }
                    33 => {
                        // Build signal: pop the seven loop nodes (reversed to
                        // start/end/body/cap/0/1/2 order) and assemble the 12-word
                        // limit_parts entry from the five slots then the seven nodes.
                        let mut popped: Vec<i64> =
                            (0..7).map(|_| stack.pop().expect("a loop node")).collect();
                        popped.reverse();
                        limit_parts.extend(&slot_buf);
                        limit_parts.extend(&popped);
                        slot_buf.clear();
                    }
                    15 => {
                        // DONE: the single remaining stack entry is the body root.
                        assert_eq!(stack.len(), 1, "the body has exactly one root node");
                        return (nodes, call_args, limit_parts, stack[0]);
                    }
                    other => panic!("unexpected node kind {other}"),
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected VM state {other:?}"),
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("body parser did not reach DONE within the iteration budget");
}

/// The flat data-segment slot of a scalar `data.field`, its position in the compiled
/// data-layout slot names.
fn data_slot(data_slots: &[String], data: &str, field: &str) -> i64 {
    let name = format!("{data}.{field}");
    data_slots
        .iter()
        .position(|n| n == &name)
        .unwrap_or_else(|| panic!("no data slot named `{name}`")) as i64
}

/// The element-0 slot (base) and length of an array `data.field`, from its per-element
/// slot names `data.field[0]`, `data.field[1]`, ...
fn array_base_len(data_slots: &[String], data: &str, field: &str) -> (i64, i64) {
    let prefix = format!("{data}.{field}[");
    let base = data_slots
        .iter()
        .position(|n| n.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no array data slot with prefix `{prefix}`"))
        as i64;
    let len = data_slots.iter().filter(|n| n.starts_with(&prefix)).count() as i64;
    (base, len)
}

/// Flatten an expression into `nodes` in postorder, returning the index of its root
/// node. This mirrors the codegen conformance harness's `flatten` for the subset the
/// body parser handles so far: integer literals, parameter and local references, the
/// unary and binary operators, and scalar data-field reads.
fn flatten(
    expr: &Expr,
    scope: &mut Vec<(String, i64)>,
    nodes: &mut Vec<Node>,
    data_slots: &[String],
    chunk_names: &[String],
    call_args: &mut Vec<i64>,
    next_slot: &mut i64,
    limit_parts: &mut Vec<i64>,
) -> i64 {
    match expr {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => {
            nodes.push(Node {
                kind: 1,
                arg: *n,
                lhs: 0,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
        Expr::Ident { name, .. } => {
            // The scope maps each bound name to its frame slot (parameters first, then
            // `let` bindings and `for` variables); the most recent binding of a name
            // wins. Slots are explicit because a `for..limit` claims anonymous slots
            // that carry no name.
            let slot = scope
                .iter()
                .rev()
                .find(|(n, _)| n == name)
                .map(|(_, s)| *s)
                .expect("identifier is a parameter or a let binding this increment");
            nodes.push(Node {
                kind: 2,
                arg: slot,
                lhs: 0,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            let lhs = flatten(
                left,
                scope,
                nodes,
                data_slots,
                chunk_names,
                call_args,
                next_slot,
                limit_parts,
            );
            let rhs = flatten(
                right,
                scope,
                nodes,
                data_slots,
                chunk_names,
                call_args,
                next_slot,
                limit_parts,
            );
            // Most operators are a BinOp node (kind 3) with an operator code; the
            // short-circuit booleans are their own node kinds (8 Andalso, 9 Orelse).
            let (kind, code) = match op {
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
                BinOp::Band => (3, 12),
                BinOp::Bor => (3, 13),
                BinOp::Bxor => (3, 14),
                BinOp::Andalso => (8, 0),
                BinOp::Orelse => (9, 0),
                other => panic!(
                    "increment handles arithmetic, comparison, bitwise, and short-circuit, got {other:?}"
                ),
            };
            nodes.push(Node {
                kind,
                arg: code,
                lhs,
                rhs,
            });
            (nodes.len() - 1) as i64
        }
        Expr::UnaryOp { op, operand, .. } if matches!(op, UnaryOp::Not | UnaryOp::Neg) => {
            let child = flatten(
                operand,
                scope,
                nodes,
                data_slots,
                chunk_names,
                call_args,
                next_slot,
                limit_parts,
            );
            let kind = match op {
                UnaryOp::Not => 6,
                UnaryOp::Neg => 10,
                _ => unreachable!(),
            };
            nodes.push(Node {
                kind,
                arg: 0,
                lhs: child,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
        Expr::FieldAccess { object, field, .. } => {
            // A scalar `data.field` read is a DataRead (kind 11) of the field's slot.
            let data = match object.as_ref() {
                Expr::Ident { name, .. } => name,
                other => panic!("increment handles data field access only, got {other:?}"),
            };
            let slot = data_slot(data_slots, data, field);
            nodes.push(Node {
                kind: 11,
                arg: slot,
                lhs: 0,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
        Expr::ArrayIndex { object, index, .. } => {
            // A `data.field[i]` read is an IndexRead (kind 13): arg = base + len*65536,
            // lhs = the index expression. The object is a `data.field` access.
            let (data, field) = match object.as_ref() {
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
            let (base, len) = array_base_len(data_slots, data, field);
            let index_node = flatten(
                index,
                scope,
                nodes,
                data_slots,
                chunk_names,
                call_args,
                next_slot,
                limit_parts,
            );
            nodes.push(Node {
                kind: 13,
                arg: base + len * 65536,
                lhs: index_node,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            // An If (kind 4): arg = condition, lhs = then branch, rhs = else branch.
            // Each branch is a full statement block.
            let cond = flatten(
                condition,
                scope,
                nodes,
                data_slots,
                chunk_names,
                call_args,
                next_slot,
                limit_parts,
            );
            let then = flatten_block(
                then_block,
                scope,
                nodes,
                data_slots,
                chunk_names,
                call_args,
                next_slot,
                limit_parts,
            );
            let els = match else_block {
                Some(eb) => flatten_block(
                    eb,
                    scope,
                    nodes,
                    data_slots,
                    chunk_names,
                    call_args,
                    next_slot,
                    limit_parts,
                ),
                None => panic!("increment requires an else branch"),
            };
            nodes.push(Node {
                kind: 4,
                arg: cond,
                lhs: then,
                rhs: els,
            });
            (nodes.len() - 1) as i64
        }
        Expr::Call { name, args, .. } => {
            // A Call (kind 7): flatten the arguments (their nodes emitted, indices
            // appended contiguously to call_args), then arg = chunk index, lhs = the
            // args' start in call_args, rhs = the count.
            let chunk = chunk_names
                .iter()
                .position(|n| n == name)
                .expect("callee is a known chunk") as i64;
            let arg_nodes: Vec<i64> = args
                .iter()
                .map(|a| {
                    flatten(
                        a,
                        scope,
                        nodes,
                        data_slots,
                        chunk_names,
                        call_args,
                        next_slot,
                        limit_parts,
                    )
                })
                .collect();
            let start = call_args.len() as i64;
            let count = arg_nodes.len() as i64;
            call_args.extend(arg_nodes);
            nodes.push(Node {
                kind: 7,
                arg: chunk,
                lhs: start,
                rhs: count,
            });
            (nodes.len() - 1) as i64
        }
        other => panic!(
            "increment handles literals, references, operators, data reads, if, and calls, got {other:?}"
        ),
    }
}

/// Flatten a block — an `if` branch or the function body — into `nodes`, returning its
/// cons-list root: its `let` and data-assignment statements (each a LetIn or
/// DataFieldAssignIn) wrap the tail, last to first. A `let` claims the next frame slot
/// (`scope.len()`, monotonic since the scope is never truncated) and extends the scope,
/// so the body parser's last-wins resolution over the full binding list matches for
/// every admissible program.
fn flatten_block(
    block: &Block,
    scope: &mut Vec<(String, i64)>,
    nodes: &mut Vec<Node>,
    data_slots: &[String],
    chunk_names: &[String],
    call_args: &mut Vec<i64>,
    next_slot: &mut i64,
    limit_parts: &mut Vec<i64>,
) -> i64 {
    // Each entry is (statement node kind, arg, value node index).
    let mut stmts: Vec<(i64, i64, i64)> = Vec::new();
    for st in &block.stmts {
        match st {
            Stmt::Let(l) => {
                let value = flatten(
                    &l.value,
                    scope,
                    nodes,
                    data_slots,
                    chunk_names,
                    call_args,
                    next_slot,
                    limit_parts,
                );
                let name = match &l.pattern {
                    Pattern::Variable(n, _) => n.clone(),
                    other => panic!("test uses simple let patterns only, got {other:?}"),
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
                let value_node = flatten(
                    value,
                    scope,
                    nodes,
                    data_slots,
                    chunk_names,
                    call_args,
                    next_slot,
                    limit_parts,
                );
                let slot = data_slot(data_slots, data_name, field);
                stmts.push((12, slot, value_node));
            }
            Stmt::For(fs) if fs.limit.is_some() => {
                // A bare `for v in start..end limit CAP { body }`. Mirrors the codegen
                // conformance harness: the loop variable, the end, the counter, the cap,
                // and the outcome are five consecutive frame slots (the variable first,
                // allocated after the start ops and before the end ops). The body sees
                // the variable in scope. The 12-word `limit_parts` entry records the five
                // slots then the start, end, body, cap, 0, 1, and 2 nodes.
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
                    other => {
                        panic!("the self-host stage requires a literal `limit`, got {other:?}")
                    }
                };
                let start_node = flatten(
                    start_expr,
                    scope,
                    nodes,
                    data_slots,
                    chunk_names,
                    call_args,
                    next_slot,
                    limit_parts,
                );
                let var = *next_slot;
                *next_slot += 1;
                let end_node = flatten(
                    end_expr,
                    scope,
                    nodes,
                    data_slots,
                    chunk_names,
                    call_args,
                    next_slot,
                    limit_parts,
                );
                let end_slot = *next_slot;
                *next_slot += 1;
                let ctr = *next_slot;
                *next_slot += 1;
                let cap_slot = *next_slot;
                *next_slot += 1;
                let oc = *next_slot;
                *next_slot += 1;
                scope.push((fs.var.clone(), var));
                let body_node = flatten_block(
                    &fs.body,
                    scope,
                    nodes,
                    data_slots,
                    chunk_names,
                    call_args,
                    next_slot,
                    limit_parts,
                );
                scope.pop();
                let mut lit = |v: i64| -> i64 {
                    nodes.push(Node {
                        kind: 1,
                        arg: v,
                        lhs: 0,
                        rhs: 0,
                    });
                    (nodes.len() - 1) as i64
                };
                let cap_node = lit(cap);
                let zero_node = lit(0);
                let one_node = lit(1);
                let two_node = lit(2);
                let lp_start = limit_parts.len() as i64;
                for v in [
                    var, end_slot, ctr, cap_slot, oc, start_node, end_node, body_node, cap_node,
                    zero_node, one_node, two_node,
                ] {
                    limit_parts.push(v);
                }
                stmts.push((23, lp_start, 0));
            }
            other => panic!(
                "increment handles let, data-assign, and for-limit statements, got {other:?}"
            ),
        }
    }
    // A block with no trailing expression (a statement-only block, such as an effectful
    // `for` body) has the unit value, emitted as a Unit node (kind 20).
    let mut cont = match &block.tail_expr {
        Some(tail) => flatten(
            tail,
            scope,
            nodes,
            data_slots,
            chunk_names,
            call_args,
            next_slot,
            limit_parts,
        ),
        None => {
            nodes.push(Node {
                kind: 20,
                arg: 0,
                lhs: 0,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
    };
    for &(kind, arg, value) in stmts.iter().rev() {
        nodes.push(Node {
            kind,
            arg,
            lhs: value,
            rhs: cont,
        });
        cont = (nodes.len() - 1) as i64;
    }
    cont
}

/// The reference forest for a body: flatten the function's block of `let` bindings and
/// tail expression, with parameters occupying the first frame slots and each binding
/// the next slot, then wrap the tail in a LetIn cons-list innermost (last) binding
/// first — the same lowering the codegen conformance harness performs.
fn reference_body(func_src: &str) -> (Vec<Node>, Vec<i64>, Vec<i64>, i64) {
    let src = func_src.to_string();
    with_big_stack(move || reference_body_inner(&src))
}

fn reference_body_inner(func_src: &str) -> (Vec<Node>, Vec<i64>, Vec<i64>, i64) {
    let program = parse(&tokenize(func_src).expect("lex")).expect("parse");
    let data_slots = data_slot_names(&program);
    let chunk_names: Vec<String> = program.functions.iter().map(|f| f.name.clone()).collect();
    let f = program.functions.last().expect("a function");
    let mut scope: Vec<(String, i64)> = f
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| (param_name(p).to_string(), i as i64))
        .collect();
    let mut next_slot = f.params.len() as i64;
    let mut nodes = Vec::new();
    let mut call_args = Vec::new();
    let mut limit_parts = Vec::new();
    let root = flatten_block(
        &f.body,
        &mut scope,
        &mut nodes,
        &data_slots,
        &chunk_names,
        &mut call_args,
        &mut next_slot,
        &mut limit_parts,
    );
    (nodes, call_args, limit_parts, root)
}

// A body that is a single integer literal is one Literal node.
#[test]
fn a_literal_body_is_one_literal_node() {
    let src = "fn answer() -> Word { 42 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    assert_eq!(root, 0);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 1,
            arg: 42,
            lhs: 0,
            rhs: 0
        }]
    );
}

// A body that is a single parameter reference is one Local node at the parameter's
// slot; the second parameter resolves to slot 1.
#[test]
fn a_parameter_reference_resolves_to_its_slot() {
    let src = "fn second(a: Word, b: Word) -> Word { b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    assert_eq!(root, 0);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 2,
            arg: 1,
            lhs: 0,
            rhs: 0
        }]
    );
}

// The first parameter resolves to slot 0.
#[test]
fn the_first_parameter_resolves_to_slot_zero() {
    let src = "fn ident(x: Word) -> Word { x }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, _root) = run_body(src);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }]
    );
}

// A zero literal round-trips (the record encoding does not lose a zero argument).
#[test]
fn a_zero_literal_round_trips() {
    let src = "fn zero() -> Word { 0 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, _root) = run_body(src);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 1,
            arg: 0,
            lhs: 0,
            rhs: 0
        }]
    );
}

// A single binary operator over two parameters is one BinOp node over two Locals.
#[test]
fn a_binary_operator_combines_two_operands() {
    let src = "fn sum(a: Word, b: Word) -> Word { a + b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // Postorder: Local a (0), Local b (1), BinOp Add (2, lhs 0, rhs 1).
    assert_eq!(root, 2);
    assert_eq!(
        nodes[2],
        Node {
            kind: 3,
            arg: 1,
            lhs: 0,
            rhs: 1
        }
    );
}

// Multiplication binds tighter than addition: `a + b * c` parses as `a + (b * c)`.
#[test]
fn multiplication_binds_tighter_than_addition() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a + b * c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // The root is Add; its right child is the Mul of b and c.
    let add = nodes[root as usize];
    assert_eq!(add.arg, 1); // Add
    let mul = nodes[add.rhs as usize];
    assert_eq!(mul.arg, 2); // Mul
}

// Subtraction is left-associative: `a - b - c` parses as `(a - b) - c`.
#[test]
fn subtraction_is_left_associative() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a - b - c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // The root is a Sub whose left child is itself a Sub (left association).
    let outer = nodes[root as usize];
    assert_eq!(outer.arg, 3); // Sub
    let inner = nodes[outer.lhs as usize];
    assert_eq!(inner.arg, 3); // Sub
}

// Comparison binds loosest: `a + b == c` parses as `(a + b) == c`.
#[test]
fn comparison_binds_loosest() {
    let src = "fn f(a: Word, b: Word, c: Word) -> bool { a + b == c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let eq = nodes[root as usize];
    assert_eq!(eq.arg, 6); // Eq
    let add = nodes[eq.lhs as usize];
    assert_eq!(add.arg, 1); // Add
}

// A literal operand mixes with parameters across every arithmetic precedence level.
#[test]
fn a_mixed_precedence_expression_matches_the_reference() {
    let src = "fn f(a: Word, b: Word) -> Word { a * 2 + b / 3 - 1 }";
    assert_eq!(run_body(src), reference_body(src));
}

// Parentheses override precedence: `(a + b) * c` parses as `(a + b) * c`, so the root
// is the Mul and its left child is the Add.
#[test]
fn parentheses_override_precedence() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { (a + b) * c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let mul = nodes[root as usize];
    assert_eq!(mul.arg, 2); // Mul
    let add = nodes[mul.lhs as usize];
    assert_eq!(add.arg, 1); // Add
}

// A parenthesised group on the right of a tighter operator: `a * (b + c)`.
#[test]
fn parentheses_on_the_right_group_the_addition() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a * (b + c) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let mul = nodes[root as usize];
    assert_eq!(mul.arg, 2); // Mul
    let add = nodes[mul.rhs as usize];
    assert_eq!(add.arg, 1); // Add
}

// Nested and redundant parentheses collapse to the inner expression.
#[test]
fn nested_parentheses_collapse() {
    let src = "fn f(x: Word) -> Word { ((x)) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, _root) = run_body(src);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }]
    );
}

// Two parenthesised groups combine under a single operator: `(a + b) * (c - d)`.
#[test]
fn two_parenthesised_groups_combine() {
    let src = "fn f(a: Word, b: Word, c: Word, d: Word) -> Word { (a + b) * (c - d) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let mul = nodes[root as usize];
    assert_eq!(mul.arg, 2); // Mul
    assert_eq!(nodes[mul.lhs as usize].arg, 1); // Add
    assert_eq!(nodes[mul.rhs as usize].arg, 3); // Sub
}

// Unary minus in operand position is Neg (kind 10) with the operand in `lhs`.
#[test]
fn unary_minus_is_negation() {
    let src = "fn f(x: Word) -> Word { -x }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let neg = nodes[root as usize];
    assert_eq!(neg.kind, 10);
    assert_eq!(
        nodes[neg.lhs as usize],
        Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }
    );
}

// `not` is a prefix unary Not (kind 6).
#[test]
fn not_is_a_prefix_unary() {
    let src = "fn f(a: Word, b: Word) -> bool { not a == b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // `not` binds tighter than `==`, so the root is Eq and its left child is Not.
    let eq = nodes[root as usize];
    assert_eq!(eq.arg, 6); // Eq
    assert_eq!(nodes[eq.lhs as usize].kind, 6); // Not
}

// Unary minus binds tighter than a binary operator: `-a * b` is `(-a) * b`.
#[test]
fn unary_minus_binds_tighter_than_multiplication() {
    let src = "fn f(a: Word, b: Word) -> Word { -a * b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let mul = nodes[root as usize];
    assert_eq!(mul.arg, 2); // Mul
    assert_eq!(nodes[mul.lhs as usize].kind, 10); // Neg
}

// Binary minus is still recognised after an operand: `a - -b` is `a - (-b)`.
#[test]
fn binary_and_unary_minus_coexist() {
    let src = "fn f(a: Word, b: Word) -> Word { a - -b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let sub = nodes[root as usize];
    assert_eq!(sub.kind, 3);
    assert_eq!(sub.arg, 3); // Sub
    assert_eq!(nodes[sub.rhs as usize].kind, 10); // Neg on the right
}

// Unary operators nest right-associatively: `- -x` is `-(-x)`.
#[test]
fn unary_operators_nest() {
    let src = "fn f(x: Word) -> Word { - -x }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let outer = nodes[root as usize];
    assert_eq!(outer.kind, 10);
    assert_eq!(nodes[outer.lhs as usize].kind, 10); // inner Neg
}

// The bitwise operators are BinOp codes 12 (band), 13 (bor), 14 (bxor).
#[test]
fn bitwise_operators_are_recognised() {
    let src = "fn f(a: Word, b: Word) -> Word { a band b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    assert_eq!(
        nodes[root as usize],
        Node {
            kind: 3,
            arg: 12,
            lhs: 0,
            rhs: 1
        }
    );
}

// Within the bitwise level `band` binds tighter than `bor`: `a bor b band c` is
// `a bor (b band c)`.
#[test]
fn band_binds_tighter_than_bor() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a bor b band c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let bor = nodes[root as usize];
    assert_eq!(bor.arg, 13); // Bor
    assert_eq!(nodes[bor.rhs as usize].arg, 12); // Band on the right
}

// The bitwise operators bind tighter than comparison: `a band b == c` is
// `(a band b) == c`.
#[test]
fn bitwise_binds_tighter_than_comparison() {
    let src = "fn f(a: Word, b: Word, c: Word) -> bool { a band b == c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let eq = nodes[root as usize];
    assert_eq!(eq.arg, 6); // Eq
    assert_eq!(nodes[eq.lhs as usize].arg, 12); // Band
}

// The short-circuit booleans are their own node kinds (8 andalso, 9 orelse) and are
// looser than comparison: `a == b andalso c == d`.
#[test]
fn short_circuit_and_is_looser_than_comparison() {
    let src = "fn f(a: Word, b: Word, c: Word, d: Word) -> bool { a == b andalso c == d }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let andalso = nodes[root as usize];
    assert_eq!(andalso.kind, 8); // Andalso
    assert_eq!(nodes[andalso.lhs as usize].arg, 6); // Eq on the left
    assert_eq!(nodes[andalso.rhs as usize].arg, 6); // Eq on the right
}

// `andalso` binds tighter than `orelse`: `a andalso b orelse c` is
// `(a andalso b) orelse c`.
#[test]
fn andalso_binds_tighter_than_orelse() {
    let src = "fn f(a: bool, b: bool, c: bool) -> bool { a andalso b orelse c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let orelse = nodes[root as usize];
    assert_eq!(orelse.kind, 9); // Orelse
    assert_eq!(nodes[orelse.lhs as usize].kind, 8); // Andalso
}

// A single `let` binding wraps the tail in a LetIn node; the binding takes the slot
// after the parameters, and the tail references it as a Local.
#[test]
fn a_single_let_binding_wraps_the_tail() {
    let src = "fn f(a: Word) -> Word { let b = a + 1; b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let letin = nodes[root as usize];
    assert_eq!(letin.kind, 5);
    assert_eq!(letin.arg, 1); // slot 1 (after the single parameter a at slot 0)
    // The continuation is the tail Local referring to slot 1.
    assert_eq!(
        nodes[letin.rhs as usize],
        Node {
            kind: 2,
            arg: 1,
            lhs: 0,
            rhs: 0
        }
    );
    // The bound value is `a + 1`, an Add.
    assert_eq!(nodes[letin.lhs as usize].arg, 1); // Add
}

// A binding with a type annotation is read the same; the annotation is skipped.
#[test]
fn a_typed_let_binding_is_read() {
    let src = "fn f() -> Word { let x: Word = 7; x }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let letin = nodes[root as usize];
    assert_eq!(letin.kind, 5);
    assert_eq!(letin.arg, 0); // slot 0 (no parameters)
}

// Several bindings nest as a cons-list, outermost binding first, each referencing the
// prior ones.
#[test]
fn multiple_let_bindings_nest() {
    let src = "fn f() -> Word { let a = 1; let b = a + 2; let c = b * 3; a + b + c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // Root is the LetIn for a (slot 0); its continuation is the LetIn for b (slot 1);
    // then c (slot 2); then the tail.
    let la = nodes[root as usize];
    assert_eq!((la.kind, la.arg), (5, 0));
    let lb = nodes[la.rhs as usize];
    assert_eq!((lb.kind, lb.arg), (5, 1));
    let lc = nodes[lb.rhs as usize];
    assert_eq!((lc.kind, lc.arg), (5, 2));
}

// A later binding of a name shadows an earlier binding: the tail resolves to the most
// recent slot.
#[test]
fn a_later_binding_shadows_an_earlier_one() {
    let src = "fn f() -> Word { let x = 1; let x = x + 1; x }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // The tail `x` resolves to the second binding's slot (1), not the first (0).
    let outer = nodes[root as usize]; // LetIn slot 0
    let inner = nodes[outer.rhs as usize]; // LetIn slot 1
    assert_eq!(inner.arg, 1);
    assert_eq!(
        nodes[inner.rhs as usize],
        Node {
            kind: 2,
            arg: 1,
            lhs: 0,
            rhs: 0
        }
    );
}

// A scalar `data.field` read is a DataRead node at the field's data-segment slot.
#[test]
fn a_scalar_data_read_uses_the_field_slot() {
    let src = "shared data d { x: Word, y: Word } fn f() -> Word { d.y }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // `d.y` is the second field, slot 1.
    assert_eq!(
        nodes[root as usize],
        Node {
            kind: 11,
            arg: 1,
            lhs: 0,
            rhs: 0
        }
    );
}

// A data read is an ordinary operand: it combines with parameters under operators.
#[test]
fn a_data_read_is_an_operand() {
    let src = "shared data st { count: Word } fn f(n: Word) -> Word { st.count + n }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let add = nodes[root as usize];
    assert_eq!(add.arg, 1); // Add
    assert_eq!(nodes[add.lhs as usize].kind, 11); // DataRead st.count
    assert_eq!(nodes[add.rhs as usize].kind, 2); // Local n
}

// Distinct fields resolve to distinct slots by declaration order.
#[test]
fn distinct_fields_resolve_to_distinct_slots() {
    let src = "shared data d { p: Word, q: Word, r: Word } fn f() -> Word { d.p + d.r }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let add = nodes[root as usize];
    // d.p is slot 0; d.r is slot 2 (after d.p at 0 and d.q at 1).
    assert_eq!(nodes[add.lhs as usize].arg, 0);
    assert_eq!(nodes[add.lhs as usize].kind, 11);
    assert_eq!(nodes[add.rhs as usize].arg, 2);
    assert_eq!(nodes[add.rhs as usize].kind, 11);
}

// A data read binds into a `let`, and the tail references the binding.
#[test]
fn a_data_read_binds_into_a_let() {
    let src = "shared data d { v: Word } fn f() -> Word { let c = d.v + 1; c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let letin = nodes[root as usize];
    assert_eq!(letin.kind, 5);
    // The bound value is `d.v + 1`, an Add whose left is the DataRead.
    let add = nodes[letin.lhs as usize];
    assert_eq!(add.arg, 1); // Add
    assert_eq!(nodes[add.lhs as usize].kind, 11); // DataRead d.v
}

// An indexed `data.arr[i]` read is an IndexRead node; its arg packs base and length,
// and its child is the index expression.
#[test]
fn an_indexed_data_read_is_an_index_read() {
    let src = "shared data d { xs: [Word; 4] } fn f(i: Word) -> Word { d.xs[i] }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let idx = nodes[root as usize];
    assert_eq!(idx.kind, 13);
    // xs is the only field, so base 0 and length 4: arg = 0 + 4*65536.
    assert_eq!(idx.arg, 4 * 65536);
    // The index is the Local i (slot 0).
    assert_eq!(
        nodes[idx.lhs as usize],
        Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }
    );
}

// The index may be an expression, parsed with full precedence inside the brackets.
#[test]
fn the_index_is_a_full_expression() {
    let src = "shared data d { xs: [Word; 8] } fn f(i: Word) -> Word { d.xs[i + 1] }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let idx = nodes[root as usize];
    assert_eq!(idx.kind, 13);
    // The index child is the Add of i and 1.
    assert_eq!(nodes[idx.lhs as usize].arg, 1); // Add
}

// A data read may appear inside the index, exercising the nested index stack.
#[test]
fn a_data_read_nests_inside_an_index() {
    let src = "shared data d { xs: [Word; 16], i: Word } fn f() -> Word { d.xs[d.i] }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let idx = nodes[root as usize];
    assert_eq!(idx.kind, 13);
    // The index is a scalar DataRead of d.i (slot 16, after the 16 xs elements).
    assert_eq!(
        nodes[idx.lhs as usize],
        Node {
            kind: 11,
            arg: 16,
            lhs: 0,
            rhs: 0
        }
    );
}

// An indexed read combines with an operator as an ordinary operand.
#[test]
fn an_indexed_read_is_an_operand() {
    let src = "shared data d { xs: [Word; 4] } fn f(i: Word) -> Word { d.xs[i] + 1 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let add = nodes[root as usize];
    assert_eq!(add.arg, 1); // Add
    assert_eq!(nodes[add.lhs as usize].kind, 13); // IndexRead
}

// A scalar data assignment is a statement: a DataFieldAssignIn node wrapping the tail,
// with the assigned value in `lhs` and the target field's slot in `arg`.
#[test]
fn a_scalar_data_assignment_is_a_statement() {
    let src = "shared data d { x: Word } fn f() -> Word { d.x = 5; d.x }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let assign = nodes[root as usize];
    assert_eq!(assign.kind, 12);
    assert_eq!(assign.arg, 0); // d.x is slot 0
    // The assigned value is the literal 5; the continuation is the tail read of d.x.
    assert_eq!(
        nodes[assign.lhs as usize],
        Node {
            kind: 1,
            arg: 5,
            lhs: 0,
            rhs: 0
        }
    );
    assert_eq!(nodes[assign.rhs as usize].kind, 11); // DataRead d.x
}

// The assigned value is a full expression that may read data and parameters.
#[test]
fn an_assignment_value_is_an_expression() {
    let src = "shared data d { x: Word } fn f(n: Word) -> Word { d.x = n * 2 + d.x; n }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let assign = nodes[root as usize];
    assert_eq!(assign.kind, 12);
    // The value is `n * 2 + d.x`, an Add.
    assert_eq!(nodes[assign.lhs as usize].arg, 1); // Add
}

// Assignments and `let` bindings mix in one block, folded outermost binding first.
#[test]
fn assignments_and_lets_mix_in_a_block() {
    let src = "shared data d { x: Word } fn f(n: Word) -> Word { let a = n + 1; d.x = a; a }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // Root is the LetIn for `a` (slot 1, after parameter n at 0); its continuation is
    // the DataFieldAssignIn for d.x (slot 0).
    let letin = nodes[root as usize];
    assert_eq!((letin.kind, letin.arg), (5, 1));
    let assign = nodes[letin.rhs as usize];
    assert_eq!((assign.kind, assign.arg), (12, 0));
    // The assigned value is the Local `a` (slot 1).
    assert_eq!(
        nodes[assign.lhs as usize],
        Node {
            kind: 2,
            arg: 1,
            lhs: 0,
            rhs: 0
        }
    );
}

// An `if`/`else` conditional is an If node with the condition in arg and the branches
// in lhs/rhs.
#[test]
fn an_if_expression_is_an_if_node() {
    let src = "fn f(x: Word) -> Word { if x > 0 { x } else { 0 } }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let iff = nodes[root as usize];
    assert_eq!(iff.kind, 4);
    assert_eq!(nodes[iff.arg as usize].arg, 9); // Gt condition
    assert_eq!(
        nodes[iff.lhs as usize],
        Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }
    ); // then: x
    assert_eq!(
        nodes[iff.rhs as usize],
        Node {
            kind: 1,
            arg: 0,
            lhs: 0,
            rhs: 0
        }
    ); // else: 0
}

// The condition and branches may be full expressions.
#[test]
fn if_branches_are_expressions() {
    let src = "fn f(a: Word, b: Word) -> Word { if a + b == 3 { a * b } else { a - b } }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let iff = nodes[root as usize];
    assert_eq!(iff.kind, 4);
    assert_eq!(nodes[iff.arg as usize].arg, 6); // Eq condition
    assert_eq!(nodes[iff.lhs as usize].arg, 2); // then: Mul
    assert_eq!(nodes[iff.rhs as usize].arg, 3); // else: Sub
}

// A conditional nests in the then branch; the inner If is the outer If's then child.
#[test]
fn conditionals_nest_in_the_then_branch() {
    let src = "fn f(x: Word) -> Word { if x > 0 { if x > 5 { 2 } else { 1 } } else { 0 } }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let outer = nodes[root as usize];
    assert_eq!(outer.kind, 4);
    // The then branch is itself an If.
    assert_eq!(nodes[outer.lhs as usize].kind, 4);
    // The else branch is the literal 0.
    assert_eq!(
        nodes[outer.rhs as usize],
        Node {
            kind: 1,
            arg: 0,
            lhs: 0,
            rhs: 0
        }
    );
}

// A conditional nests in the else branch (an else-if chain).
#[test]
fn conditionals_nest_in_the_else_branch() {
    let src = "fn f(x: Word) -> Word { if x > 5 { 2 } else { if x > 0 { 1 } else { 0 } } }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let outer = nodes[root as usize];
    assert_eq!(outer.kind, 4);
    // The else branch is itself an If.
    assert_eq!(nodes[outer.rhs as usize].kind, 4);
}

// A conditional binds into a `let` and combines under an operator.
#[test]
fn a_conditional_binds_and_combines() {
    let src = "fn f(x: Word) -> Word { let m = if x > 0 { x } else { 0 }; m + 1 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let letin = nodes[root as usize];
    assert_eq!(letin.kind, 5);
    // The bound value is the If.
    assert_eq!(nodes[letin.lhs as usize].kind, 4);
}

// A branch is a full statement block: its `let` binding folds into a cons-list root,
// which is the branch of the If.
#[test]
fn an_if_branch_is_a_statement_block() {
    let src = "fn f(x: Word) -> Word { if x > 0 { let y = x + 1; y } else { 0 } }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let iff = nodes[root as usize];
    assert_eq!(iff.kind, 4);
    // The then branch is a LetIn (the block's cons-list root), not a bare expression.
    let then = nodes[iff.lhs as usize];
    assert_eq!(then.kind, 5);
    // The binding's slot is 1 (after the parameter x at slot 0).
    assert_eq!(then.arg, 1);
    // The else branch is the bare literal 0.
    assert_eq!(
        nodes[iff.rhs as usize],
        Node {
            kind: 1,
            arg: 0,
            lhs: 0,
            rhs: 0
        }
    );
}

// Both branches carry statements, and the slots continue monotonically across them.
#[test]
fn both_branches_carry_statements() {
    let src = "fn f(x: Word) -> Word { if x > 0 { let a = x; a } else { let b = 0; b } }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let iff = nodes[root as usize];
    // then: LetIn a at slot 1; else: LetIn b at slot 2 (monotonic, not reused).
    assert_eq!(
        (nodes[iff.lhs as usize].kind, nodes[iff.lhs as usize].arg),
        (5, 1)
    );
    assert_eq!(
        (nodes[iff.rhs as usize].kind, nodes[iff.rhs as usize].arg),
        (5, 2)
    );
}

// A branch block writes data and reads a parameter, then a later statement resumes the
// enclosing block correctly.
#[test]
fn a_branch_block_with_an_assignment() {
    let src = "shared data d { x: Word } \
        fn f(n: Word) -> Word { let a = if n > 0 { d.x = n; n } else { 0 }; a }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // Outer LetIn binds `a` to the If; the If's then branch is a DataFieldAssignIn.
    let letin = nodes[root as usize];
    assert_eq!(letin.kind, 5);
    let iff = nodes[letin.lhs as usize];
    assert_eq!(iff.kind, 4);
    assert_eq!(nodes[iff.lhs as usize].kind, 12); // then branch: DataFieldAssignIn
}

// A statement follows the conditional in the enclosing block: the block state resumes
// after the branches fold.
#[test]
fn a_statement_follows_a_conditional() {
    let src = "shared data d { x: Word } \
        fn f(n: Word) -> Word { let a = if n > 0 { n } else { 0 }; d.x = a; a }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // Root LetIn(a=If); continuation DataFieldAssignIn(d.x = a); then tail a.
    let letin = nodes[root as usize];
    assert_eq!((letin.kind, letin.arg), (5, 1));
    assert_eq!(nodes[letin.lhs as usize].kind, 4); // If
    assert_eq!(nodes[letin.rhs as usize].kind, 12); // DataFieldAssignIn
}

// A single-argument call is a Call node at the callee's chunk index, with the argument
// recorded in call_args.
#[test]
fn a_call_is_a_call_node() {
    let src = "fn helper(x: Word) -> Word { x + 1 } fn main(n: Word) -> Word { helper(n) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, call_args, _limit_parts, root) = run_body(src);
    let call = nodes[root as usize];
    assert_eq!(call.kind, 7);
    assert_eq!(call.arg, 0); // helper is chunk 0
    assert_eq!(call.rhs, 1); // one argument
    // The single argument (call_args[start]) is the Local n.
    let arg_node = call_args[call.lhs as usize];
    assert_eq!(
        nodes[arg_node as usize],
        Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }
    );
}

// A no-argument call has an empty argument slice.
#[test]
fn a_no_argument_call() {
    let src = "fn zero() -> Word { 0 } fn main() -> Word { zero() }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    let call = nodes[root as usize];
    assert_eq!(call.kind, 7);
    assert_eq!(call.rhs, 0); // no arguments
}

// Multiple arguments are recorded in source order, each a full expression.
#[test]
fn multiple_arguments_in_order() {
    let src = "fn add3(a: Word, b: Word, c: Word) -> Word { a + b + c } \
        fn main(n: Word) -> Word { add3(n, n + 1, n * 2) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, call_args, _limit_parts, root) = run_body(src);
    let call = nodes[root as usize];
    assert_eq!(call.rhs, 3);
    // call_args[start..start+3] are n, (n+1), (n*2) in order.
    let a0 = nodes[call_args[call.lhs as usize] as usize];
    let a1 = nodes[call_args[(call.lhs + 1) as usize] as usize];
    let a2 = nodes[call_args[(call.lhs + 2) as usize] as usize];
    assert_eq!(a0.kind, 2); // Local n
    assert_eq!(a1.arg, 1); // Add
    assert_eq!(a2.arg, 2); // Mul
}

// A call is an operand: it combines under operators and nests as an argument.
#[test]
fn a_call_is_an_operand_and_nests() {
    let src = "fn helper(x: Word) -> Word { x + 1 } \
        fn main(n: Word) -> Word { helper(helper(n)) + 2 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, _limit_parts, root) = run_body(src);
    // Root Add; left is the outer Call whose argument is the inner Call.
    let add = nodes[root as usize];
    assert_eq!(add.arg, 1); // Add
    assert_eq!(nodes[add.lhs as usize].kind, 7); // outer Call
}

// A call binds into a `let` and reads data as an argument.
#[test]
fn a_call_binds_and_reads_data() {
    let src = "shared data d { v: Word } fn helper(x: Word) -> Word { x + 1 } \
        fn main() -> Word { let r = helper(d.v); r }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, call_args, _limit_parts, root) = run_body(src);
    let letin = nodes[root as usize];
    assert_eq!(letin.kind, 5);
    let call = nodes[letin.lhs as usize];
    assert_eq!(call.kind, 7);
    // The argument is a DataRead of d.v.
    assert_eq!(nodes[call_args[call.lhs as usize] as usize].kind, 11);
}

// A bare `for v in start..end limit CAP { body }` lowers to a ForLimit statement
// (kind 23) wrapping the block tail, with a 12-word limit_parts entry recording the
// five loop slots then the start, end, body, cap, 0, 1, and 2 nodes. The loop
// variable occupies the first anonymous slot and is visible in the body.
#[test]
fn a_for_limit_loop_lowers_to_a_forlimit_statement() {
    let src = "fn count() -> Word { for i in 0..10 limit 100 { i } 0 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, limit_parts, root) = run_body(src);
    // The root is the ForLimit statement wrapping the tail literal 0.
    let forlimit = nodes[root as usize];
    assert_eq!(forlimit.kind, 23);
    assert_eq!(forlimit.arg, 0); // the sole entry starts at limit_parts[0]
    assert_eq!(forlimit.lhs, 0); // ForLimit has no value node
    assert_eq!(nodes[forlimit.rhs as usize].kind, 1); // the tail is literal 0
    // The 12-word entry: var, end_slot, ctr, cap_slot, oc, then the seven nodes.
    assert_eq!(limit_parts.len(), 12);
    assert_eq!(&limit_parts[0..5], &[0, 1, 2, 3, 4]);
    // Node 5 (start) is literal 0, node 6 (end) is literal 10, node 7 (body) is the
    // loop variable at slot 0, node 8 (cap) is literal 100.
    assert_eq!(nodes[limit_parts[5] as usize].arg, 0); // start literal 0
    assert_eq!(nodes[limit_parts[6] as usize].arg, 10); // end literal 10
    assert_eq!(nodes[limit_parts[7] as usize].kind, 2); // body: Local
    assert_eq!(nodes[limit_parts[7] as usize].arg, 0); // ...at the variable slot 0
    assert_eq!(nodes[limit_parts[8] as usize].arg, 100); // cap literal
    assert_eq!(nodes[limit_parts[9] as usize].arg, 0); // 0
    assert_eq!(nodes[limit_parts[10] as usize].arg, 1); // 1
    assert_eq!(nodes[limit_parts[11] as usize].arg, 2); // 2
}

// The loop variable combines under operators inside the body, and the range bounds
// may be parameter references rather than literals.
#[test]
fn a_for_limit_body_uses_the_variable_and_parameter_bounds() {
    let src = "fn ramp(n: Word) -> Word { for i in 0..n limit 50 { i * 2 + 1 } n }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, limit_parts, _root) = run_body(src);
    assert_eq!(limit_parts.len(), 12);
    // The end bound (node index limit_parts[6]) is the parameter n at slot 0.
    let end = nodes[limit_parts[6] as usize];
    assert_eq!(end.kind, 2);
    assert_eq!(end.arg, 0);
    // The body root (limit_parts[7]) is an Add whose left is a Mul.
    let body = nodes[limit_parts[7] as usize];
    assert_eq!(body.kind, 3);
    assert_eq!(body.arg, 1); // Add
    assert_eq!(nodes[body.lhs as usize].arg, 2); // Mul
}

// A `let` binding before the loop claims slot 0, so the loop's five slots start at 1;
// the reference agreement checks the slot arithmetic through the shared next_slot.
#[test]
fn a_let_before_a_for_limit_shifts_the_loop_slots() {
    let src = "fn shifted() -> Word { let base = 7; for i in 0..3 limit 8 { base + i } base }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, limit_parts, _root) = run_body(src);
    assert_eq!(limit_parts.len(), 12);
    // base is slot 0; the loop variable is slot 1, the four anon slots 2..5.
    assert_eq!(&limit_parts[0..5], &[1, 2, 3, 4, 5]);
    // The body `base + i` is an Add of Local(0) and Local(1).
    let body = nodes[limit_parts[7] as usize];
    assert_eq!(body.kind, 3);
    assert_eq!(nodes[body.lhs as usize].arg, 0); // base at slot 0
    assert_eq!(nodes[body.rhs as usize].arg, 1); // i at slot 1
}

// Two sequential loops each contribute a 12-word limit_parts entry; the second entry
// starts at word 12, and the loop-variable slots do not collide because next_slot is
// monotonic across both loops.
#[test]
fn two_sequential_for_limit_loops_have_disjoint_slots() {
    let src = "fn twice() -> Word { for i in 0..3 limit 4 { i } for j in 0..5 limit 6 { j } 0 }";
    assert_eq!(run_body(src), reference_body(src));
    let (_nodes, _call_args, limit_parts, _root) = run_body(src);
    assert_eq!(limit_parts.len(), 24);
    // First loop: slots 0..5. Second loop: slots 5..10.
    assert_eq!(&limit_parts[0..5], &[0, 1, 2, 3, 4]);
    assert_eq!(&limit_parts[12..17], &[5, 6, 7, 8, 9]);
}

// A statement-only `for` body (an effectful data-field assignment with no trailing
// expression) folds its statements around a Unit tail (kind 20) into the body node.
#[test]
fn a_for_limit_body_may_be_statement_only() {
    let src = "shared data acc { sum: Word } \
        fn run() -> Word { for i in 0..3 limit 4 { acc.sum = acc.sum + i; } 0 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, limit_parts, _root) = run_body(src);
    assert_eq!(limit_parts.len(), 12);
    // The body node (limit_parts[7]) is a DataFieldAssignIn (kind 12) whose continuation
    // is a Unit node (kind 20).
    let body = nodes[limit_parts[7] as usize];
    assert_eq!(body.kind, 12);
    assert_eq!(nodes[body.rhs as usize].kind, 20);
    // Its value is the Add of the data read and the loop variable.
    assert_eq!(nodes[body.lhs as usize].kind, 3);
    assert_eq!(nodes[body.lhs as usize].arg, 1); // Add
}

// A `for` body may carry a `let` binding before its tail expression; the body fold
// wraps the tail in the LetIn just as a top-level block does, and the loop variable and
// the body `let` occupy distinct monotonic slots.
#[test]
fn a_for_limit_body_may_bind_a_let() {
    let src = "fn ramp() -> Word { for i in 0..3 limit 4 { let d = i * 2; d } 0 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, limit_parts, _root) = run_body(src);
    // The loop variable is slot 0; the four anon slots 1..4; the body `let d` is slot 5.
    assert_eq!(&limit_parts[0..5], &[0, 1, 2, 3, 4]);
    let body = nodes[limit_parts[7] as usize];
    assert_eq!(body.kind, 5); // LetIn
    assert_eq!(body.arg, 5); // d at slot 5 (after the loop's five slots)
    // The bound value is i * 2 (Mul of Local(0) and Literal 2).
    assert_eq!(nodes[body.lhs as usize].arg, 2); // Mul
    // The continuation is the tail `d`, a Local at slot 5.
    assert_eq!(nodes[body.rhs as usize].kind, 2);
    assert_eq!(nodes[body.rhs as usize].arg, 5);
}

// A statement-only `for` body with two statements folds both around the Unit tail in
// source order (last statement innermost).
#[test]
fn a_for_limit_body_may_have_several_statements() {
    let src = "shared data acc { a: Word, b: Word } \
        fn run() -> Word { for i in 0..3 limit 4 { acc.a = i; acc.b = i; } 0 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _call_args, limit_parts, _root) = run_body(src);
    let body = nodes[limit_parts[7] as usize];
    // Outer assignment is acc.a (slot 0); its continuation is the acc.b assignment
    // (slot 1); whose continuation is Unit.
    assert_eq!(body.kind, 12);
    assert_eq!(body.arg, 0); // acc.a
    let inner = nodes[body.rhs as usize];
    assert_eq!(inner.kind, 12);
    assert_eq!(inner.arg, 1); // acc.b
    assert_eq!(nodes[inner.rhs as usize].kind, 20); // Unit
}
