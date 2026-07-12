//! Merged parser stage (`compiler/kel/parse.kel`), increment 1: one streaming `loop`
//! that parses a whole top-level function declaration INCLUDING an atomic body (a single
//! integer literal or a parameter reference) in a single pass, rather than the two
//! composed loops (parser.kel finding declarations and body.kel parsing each body) the
//! host currently chains.
//!
//! A throwaway adapter maps the reference tokenizer into the stage's `(kind, value)`
//! token stream. The stage emits header records `dkind + val*64` (1/2/3 START of a
//! fn/yield/loop, 4 PARAM, 5 END, 6 PTYPE, 7 RETTYPE, 8 ASIZE, 15 DONE, 16 BSTART) and,
//! bracketed by a BSTART and the body's terminal Done, body node records `kind + arg*64`
//! (the body.kel `enum Node` codes: 1 Literal, 2 Local, 15 Done). The host decode keeps
//! an `in_body` flag mirroring the stage's: a BSTART switches it into body mode, the
//! body's Done switches it back. Each function is checked against the reference parse for
//! its category, name, parameter names, and the atomic body node. Types are not checked
//! here; they are covered by the separate parser-stage test. Later increments add the
//! operator grammar, statements, and the data/enum/use declaration kinds.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::ast::{Expr, FunctionCategory, Literal, Pattern};
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

/// Map the reference token stream into the stage's `(kind, value)` pairs, interning
/// identifier names into `names`. The trailing `Eof` is dropped so the token count is
/// exactly the real tokens.
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
            TokenKind::LBracket => (11, 0),
            TokenKind::IntLit(n) => (12, *n),
            TokenKind::RBracket => (18, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    (kinds, vals)
}

/// A body node: its kind (1 Literal, 2 Local) and argument (a literal value or a frame
/// slot). Atomic bodies are a single node.
type BodyNode = (i64, i64);

/// A parsed function: its category (1 fn, 2 yield, 3 loop), interned name id, its value
/// parameter name ids in order, and its atomic body node.
type Func = (i64, i64, Vec<i64>, BodyNode);

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
    // The declaration under construction: (category, name, params, optional body node).
    let mut cur: Option<(i64, i64, Vec<i64>, Option<BodyNode>)> = None;
    let mut in_body = false;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(kinds.len() * 3 + 16) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let code = w.rem_euclid(64);
                let val = w.div_euclid(64);
                if in_body {
                    // Body mode: atomic node records, ended by the body's Done.
                    match code {
                        0 => {} // PENDING
                        1 | 2 => {
                            // A Literal (1) or Local (2) leaf; the atomic body's root.
                            cur.as_mut().expect("body node before START").3 = Some((code, val));
                        }
                        15 => in_body = false, // the body's Done ends body mode
                        other => panic!("unexpected body node kind {other}"),
                    }
                } else {
                    match code {
                        0 => {} // PENDING
                        1..=3 => cur = Some((code, val, Vec::new(), None)),
                        4 => cur.as_mut().expect("PARAM before START").2.push(val),
                        6 | 7 | 8 => {} // PTYPE/RETTYPE/ASIZE: not checked this increment
                        16 => in_body = true, // BSTART: a body forest follows
                        5 => {
                            let (cat, name, params, body) = cur.take().expect("END before START");
                            parsed.funcs.push((
                                cat,
                                name,
                                params,
                                body.expect("a body before END"),
                            ));
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

/// Build the same `Parsed` from the reference parse, interning names to match the stage's
/// ids. Only atomic bodies (a literal or a parameter reference tail) are modeled.
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
        // The atomic body: the block's tail expression is a literal or a parameter ref.
        let tail = f
            .body
            .tail_expr
            .as_ref()
            .expect("an atomic body has a tail");
        let node: BodyNode = match &**tail {
            Expr::Literal {
                value: Literal::Int(n),
                ..
            } => (1, *n),
            Expr::Ident { name, .. } => {
                let slot = param_names
                    .iter()
                    .position(|p| *p == name.as_str())
                    .unwrap_or_else(|| panic!("identifier {name} is not a parameter"))
                    as i64;
                (2, slot)
            }
            other => panic!("increment 1 handles only an atomic body, got {other:?}"),
        };
        funcs.push((cat, id(&f.name), params, node));
    }
    Parsed { funcs }
}

// A function whose body is a single integer literal: START, RETTYPE, BSTART, the Literal
// node, the body Done, END.
#[test]
fn an_atomic_literal_body_parses_in_one_pass() {
    let src = "fn answer() -> Word { 42 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs.len(), 1);
    assert_eq!(got.funcs[0].3, (1, 42)); // Literal 42
}

// A function whose body is a parameter reference resolves to that parameter's frame slot.
#[test]
fn an_atomic_param_body_resolves_the_slot() {
    let src = "fn second(a: Word, b: Word) -> Word { b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs[0].3, (2, 1)); // Local slot 1 (the second parameter)
    assert_eq!(got.funcs[0].2.len(), 2); // two parameters
}

// The body walk returns control to the header for the next declaration: two functions,
// each with an atomic body, parse in sequence.
#[test]
fn the_body_walk_returns_to_the_header() {
    let src = "fn one() -> Word { 1 } fn ident(x: Word) -> Word { x }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs.len(), 2);
    assert_eq!(got.funcs[0].3, (1, 1)); // first: Literal 1
    assert_eq!(got.funcs[1].3, (2, 0)); // second: Local slot 0
}

// A `yield` function category is carried through the merged parse.
#[test]
fn a_yield_function_category_is_carried() {
    let src = "yield gen(r: Word) -> Word { r }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs[0].0, 2); // yield category
    assert_eq!(got.funcs[0].3, (2, 0)); // Local slot 0
}
