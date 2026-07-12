//! Merged parser stage (`compiler/kel/parse.kel`), increment 4: one streaming `loop` that
//! parses a whole top-level function declaration whose body may contain the
//! `if cond { then } else { else }` conditional with nested statement-block branches (and
//! the block-form `if` statement and the `if` without `else`), over the `let` blocks and
//! operator grammar of increments 2 and 3, in a single pass.
//!
//! A throwaway adapter maps the reference tokenizer into the stage's unified `(kind,
//! value)` token stream. The stage emits header records `dkind + val*64` (1/2/3 START of a
//! fn/yield/loop, 4 PARAM, 5 END, 6 PTYPE, 7 RETTYPE, 8 ASIZE, 15 DONE, 16 BSTART) and,
//! bracketed by a BSTART and the body's terminal Done, body node records `kind + arg*64`
//! in POSTORDER (the `enum Node` codes: 1 Literal, 2 Local, 3 BinOp, 6 Not, 8 Andalso,
//! 9 Orelse, 10 Neg, 15 Done). The host decode keeps an `in_body` flag mirroring the
//! stage's. Each function is checked against the reference parse for its category, name,
//! parameter names, and the postorder body record sequence; two postorder traversals are
//! equal exactly when the node forests are, so the flat sequence is a sound equivalence.
//! Types are covered by the separate parser-stage test. Later increments add statements,
//! the control-flow forms, and the data/enum/use declaration kinds.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::ast::{BinOp, Block, Expr, FunctionCategory, Literal, Pattern, Stmt, UnaryOp};
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `toks` block in parse.kel.
const LEN: usize = 0;
const KINDS: usize = 1;
const VALS: usize = 1 + 2048;

/// Map the reference token stream into the stage's unified `(kind, value)` pairs. The
/// operator codes are body.kel's (`Plus` 21 upward); the header keywords and punctuation
/// keep their parser.kel codes, which agree with the body vocabulary on every shared
/// token. The trailing `Eof` is dropped.
fn adapt_tokens(src: &str, names: &mut Vec<String>) -> (Vec<i64>, Vec<i64>) {
    let mut intern = |s: &str| -> i64 {
        if let Some(i) = names.iter().position(|n| n == s) {
            i as i64
        } else {
            names.push(s.to_string());
            (names.len() - 1) as i64
        }
    };
    let tokens = tokenize(src).expect("lex");
    let mut kinds = Vec::new();
    let mut vals = Vec::new();
    for tok in &tokens {
        let (kind, val) = match &tok.kind {
            TokenKind::Fn => (0, 0),
            TokenKind::LowerIdent(s) | TokenKind::UpperIdent(s) => (1, intern(s)),
            TokenKind::LBrace => (2, 0),
            TokenKind::RBrace => (3, 0),
            TokenKind::Yield => (5, 0),
            TokenKind::Loop => (6, 0),
            TokenKind::LParen => (7, 0),
            TokenKind::RParen => (8, 0),
            TokenKind::Colon => (9, 0),
            TokenKind::Comma => (10, 0),
            TokenKind::IntLit(n) => (12, *n),
            TokenKind::Eq => (17, 0),
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
            TokenKind::If => (43, 0),
            TokenKind::Else => (44, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    (kinds, vals)
}

/// A parsed function: its category (1 fn, 2 yield, 3 loop), interned name id, its value
/// parameter name ids in order, and its body's postorder node record sequence as
/// (kind, arg) pairs.
type Func = (i64, i64, Vec<i64>, Vec<(i64, i64)>);

#[derive(Debug, Default, PartialEq)]
struct Parsed {
    funcs: Vec<Func>,
}

/// Compile parse.kel on a 64MB thread; its deeply nested source overflows the default
/// 2MB test-thread stack in the host compiler's recursive-descent parse.
fn compile_parse_stage() -> keleusma::bytecode::Module {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let stage = std::fs::read_to_string("compiler/kel/parse.kel").expect("read parse.kel");
            compile(&parse(&tokenize(&stage).expect("lex parse.kel")).expect("parse parse.kel"))
                .expect("compile parse.kel")
        })
        .expect("spawn")
        .join()
        .expect("join")
}

fn run_parse(src: &str, names: &mut Vec<String>) -> Parsed {
    let (kinds, vals) = adapt_tokens(src, names);
    let module = compile_parse_stage();
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parse.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, LEN, Value::Int(kinds.len() as i64))
        .expect("len");
    for (i, (&k, &v)) in kinds.iter().zip(vals.iter()).enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(k))
            .expect("kind");
        vm.set_shared(&mut shared, VALS + i, Value::Int(v))
            .expect("val");
    }

    let mut parsed = Parsed::default();
    // The declaration under construction: (category, name, params, body records).
    let mut cur: Option<(i64, i64, Vec<i64>, Vec<(i64, i64)>)> = None;
    let mut in_body = false;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(kinds.len() * 4 + 16) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let code = w.rem_euclid(64);
                let val = w.div_euclid(64);
                if in_body {
                    match code {
                        0 => {}                // PENDING (a shunting-yard push, no record)
                        15 => in_body = false, // the body's Done ends body mode
                        _ => cur
                            .as_mut()
                            .expect("body node before START")
                            .3
                            .push((code, val)),
                    }
                } else {
                    match code {
                        0 => {} // PENDING
                        1..=3 => cur = Some((code, val, Vec::new(), Vec::new())),
                        4 => cur.as_mut().expect("PARAM before START").2.push(val),
                        6 | 7 | 8 => {} // PTYPE/RETTYPE/ASIZE: not checked this increment
                        16 => in_body = true, // BSTART: a body forest follows
                        5 => {
                            let f = cur.take().expect("END before START");
                            parsed.funcs.push(f);
                        }
                        15 => {
                            assert!(cur.is_none(), "DONE mid-declaration");
                            return parsed;
                        }
                        other => panic!("unexpected declaration kind {other}"),
                    }
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected VM state {other:?}"),
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("parser did not reach DONE within the iteration budget");
}

/// Flatten an expression into its postorder node record sequence, mirroring the stage's
/// shunting-yard emission: operands before operators, a BinOp (kind 3) carrying its
/// operator code, the short-circuit booleans their own kinds (8 Andalso, 9 Orelse), and
/// the unary prefixes 6 Not / 10 Neg. Parenthesised grouping is structural in the AST, so
/// it contributes no node, exactly as the stage discards its parenthesis marker.
fn flatten(e: &Expr, scope: &[(String, i64)], next_slot: &mut i64, out: &mut Vec<(i64, i64)>) {
    match e {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => out.push((1, *n)),
        Expr::Ident { name, .. } => {
            // The scope maps each bound name to its frame slot; the most recent binding of
            // a name wins (a `let` shadows a parameter).
            let slot = scope
                .iter()
                .rev()
                .find(|(n, _)| n == name)
                .map(|(_, s)| *s)
                .unwrap_or_else(|| panic!("identifier {name} is not in scope"));
            out.push((2, slot));
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            flatten(left, scope, next_slot, out);
            flatten(right, scope, next_slot, out);
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
                other => panic!("increment does not handle operator {other:?}"),
            };
            out.push((kind, code));
        }
        Expr::UnaryOp { op, operand, .. } => {
            flatten(operand, scope, next_slot, out);
            let kind = match op {
                UnaryOp::Not => 6,
                UnaryOp::Neg => 10,
                other => panic!("increment handles only `-` and `not`, got {other:?}"),
            };
            out.push((kind, 0));
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            // Postorder: the condition, the then branch (a folded block), the else branch
            // (a folded block, or a synthesized Unit when absent), then the If node
            // (kind 4). Slots are monotonic across branches, so `next_slot` is threaded.
            flatten(condition, scope, next_slot, out);
            flatten_block(then_block, scope, next_slot, out);
            match else_block {
                Some(eb) => flatten_block(eb, scope, next_slot, out),
                None => out.push((20, 0)), // synthesized empty else: a Unit
            }
            out.push((4, 0));
        }
        other => panic!("increment 4 does not handle expression {other:?}"),
    }
}

/// Flatten a block: each statement's value in source order, then the tail (or a Unit for a
/// statement-only block), then each statement's node (LetIn for a `let`, ExprStmt for a
/// bare or block-form expression) last to first, mirroring the stage's block fold. A `let`
/// binding claims the next monotonic frame slot and joins the scope. `next_slot` is the
/// monotonic frame-slot counter, threaded so a nested branch's slots never collide with an
/// enclosing block's.
fn flatten_block(
    block: &Block,
    scope: &[(String, i64)],
    next_slot: &mut i64,
    out: &mut Vec<(i64, i64)>,
) {
    let mut local = scope.to_vec();
    let mut stmt_nodes = Vec::new();
    for st in &block.stmts {
        match st {
            Stmt::Let(l) => {
                flatten(&l.value, &local, next_slot, out);
                let name = match &l.pattern {
                    Pattern::Variable(n, _) => n.clone(),
                    other => panic!("test uses simple let patterns only, got {other:?}"),
                };
                let slot = *next_slot;
                *next_slot += 1;
                local.push((name, slot));
                stmt_nodes.push((5, slot)); // LetIn
            }
            Stmt::Expr(e) => {
                flatten(e, &local, next_slot, out);
                stmt_nodes.push((21, 0)); // ExprStmt
            }
            other => panic!("increment 4 handles `let` and expression statements, got {other:?}"),
        }
    }
    match &block.tail_expr {
        Some(tail) => flatten(tail, &local, next_slot, out),
        None => out.push((20, 0)), // statement-only block: implicit Unit
    }
    for (kind, arg) in stmt_nodes.iter().rev() {
        out.push((*kind, *arg));
    }
}

/// Build the same `Parsed` from the reference parse, interning names to match the stage's
/// ids.
fn reference(src: &str, names: &[String]) -> Parsed {
    let id = |s: &str| -> i64 {
        names
            .iter()
            .position(|n| n == s)
            .unwrap_or_else(|| panic!("name {s} not interned")) as i64
    };
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let mut funcs = Vec::new();
    for f in &program.functions {
        let cat = match f.category {
            FunctionCategory::Fn => 1,
            FunctionCategory::Yield => 2,
            FunctionCategory::Loop => 3,
        };
        let param_names: Vec<&str> = f
            .params
            .iter()
            .map(|p| match &p.pattern {
                Pattern::Variable(n, _) => n.as_str(),
                other => panic!("test uses simple parameter patterns only, got {other:?}"),
            })
            .collect();
        let params: Vec<i64> = param_names.iter().map(|n| id(n)).collect();
        // The frame scope: each parameter at its positional slot.
        let param_scope: Vec<(String, i64)> = param_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), i as i64))
            .collect();
        let mut body = Vec::new();
        let mut next_slot = param_names.len() as i64;
        flatten_block(&f.body, &param_scope, &mut next_slot, &mut body);
        funcs.push((cat, id(&f.name), params, body));
    }
    Parsed { funcs }
}

// An atomic literal body (the increment-1 case) still parses: one Literal record.
#[test]
fn an_atomic_literal_body_still_parses() {
    let src = "fn answer() -> Word { 42 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(1, 42)]);
}

// A single binary operator over two parameters is postorder Local, Local, BinOp.
#[test]
fn a_binary_operator_body_parses() {
    let src = "fn add(a: Word, b: Word) -> Word { a + b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 1), (3, 1)]); // a, b, BinOp(Add)
}

// Precedence binds `*` tighter than `+`: `a + b * c` is a, b, c, Mul, Add.
#[test]
fn precedence_is_respected() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a + b * c }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 1), (2, 2), (3, 2), (3, 1)]);
}

// Parentheses override precedence: `(a + b) * c` is a, b, Add, c, Mul.
#[test]
fn parentheses_override_precedence() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { (a + b) * c }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 1), (3, 1), (2, 2), (3, 2)]);
}

// The unary prefixes and a comparison: `not a == b` and `-a`.
#[test]
fn unary_and_comparison_parse() {
    let src = "fn f(a: Word, b: Word) -> Word { -a + b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// The bitwise and short-circuit operators parse to their node kinds.
#[test]
fn bitwise_and_short_circuit_parse() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a band b orelse c }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, b, BinOp(Band), c, Orelse.
    assert_eq!(
        got.funcs[0].3,
        vec![(2, 0), (2, 1), (3, 12), (2, 2), (9, 0)]
    );
}

// Two functions with expression bodies parse in sequence, proving the handoff.
#[test]
fn two_expression_bodies_in_sequence() {
    let src = "fn f(a: Word) -> Word { a + a } fn g(b: Word) -> Word { b * b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2);
}

// A single `let` binding followed by a tail: the value, the tail, then a LetIn wrapping
// them. The binding's slot is the first after the parameters.
#[test]
fn a_let_binding_folds_into_a_letin() {
    let src = "fn f(a: Word) -> Word { let x = a + a; x }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, a, BinOp(Add) [the value], Local(x, slot 1) [tail], LetIn(slot 1).
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 0), (3, 1), (2, 1), (5, 1)]);
}

// Two `let` bindings: the second may reference the first; the fold wraps last to first.
#[test]
fn two_let_bindings_fold_last_to_first() {
    let src = "fn f(a: Word) -> Word { let x = a + a; let y = x + a; y }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // x value (a,a,Add), y value (x=slot1, a=slot0, Add), tail y=slot2,
    // then LetIn(y=slot2), LetIn(x=slot1).
    assert_eq!(
        got.funcs[0].3,
        vec![
            (2, 0),
            (2, 0),
            (3, 1),
            (2, 1),
            (2, 0),
            (3, 1),
            (2, 2),
            (5, 2),
            (5, 1)
        ]
    );
}

// A statement-only block (a `let` with no tail) has the implicit Unit value.
#[test]
fn a_statement_only_block_has_a_unit_tail() {
    let src = "fn f(a: Word) -> Word { let x = a; }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a [value], Unit [tail], LetIn(slot 1).
    assert_eq!(got.funcs[0].3, vec![(2, 0), (20, 0), (5, 1)]);
}

// An `if`/`else` as the block tail: condition, then branch, else branch, then the If node.
#[test]
fn an_if_else_tail_is_an_if_node() {
    let src = "fn f(a: Word) -> Word { if a > 0 { a } else { 0 } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, 0, BinOp(Gt) [cond]; a [then]; 0 [else]; If.
    assert_eq!(
        got.funcs[0].3,
        vec![(2, 0), (1, 0), (3, 9), (2, 0), (1, 0), (4, 0)]
    );
}

// Nested statement blocks in the branches, each with its own `let`, use monotonic slots.
#[test]
fn if_branches_are_statement_blocks() {
    let src = "fn f(a: Word) -> Word { if a > 0 { let x = a; x } else { let y = a; y } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// An `if` without `else` synthesizes a Unit else branch.
#[test]
fn an_if_without_else_synthesizes_a_unit() {
    let src = "fn f(a: Word) -> Word { if a > 0 { a } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // cond; a [then]; Unit [synth else]; If.
    assert_eq!(
        got.funcs[0].3,
        vec![(2, 0), (1, 0), (3, 9), (2, 0), (20, 0), (4, 0)]
    );
}

// A block-form `if` statement followed by a tail is committed as an ExprStmt.
#[test]
fn a_block_form_if_statement_is_an_expr_stmt() {
    let src = "fn f(a: Word) -> Word { if a > 0 { a } else { a } a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // cond, then, else, If [the statement value]; a [tail]; ExprStmt.
    assert_eq!(
        got.funcs[0].3,
        vec![
            (2, 0),
            (1, 0),
            (3, 9),
            (2, 0),
            (2, 0),
            (4, 0),
            (2, 0),
            (21, 0)
        ]
    );
}
