//! Stage 2 parser (`compiler/kel/parser.kel`), increment 2: the top-level
//! declaration header (function category and value-parameter count).
//!
//! A throwaway adapter maps the reference tokenizer's output into the parser
//! stage's `(kind, value)` token stream, the Keleusma `loop` consumes it one token
//! per iteration, and it yields one declaration word (`dkind + pcount*16 +
//! name*1024`) per top-level function declaration: the category from the
//! `fn`/`yield`/`loop` keyword, the value-parameter count from the parameter
//! parentheses, and the interned name, while the signature and body are still
//! skipped rather than parsed (a later increment). The host decodes each
//! declaration and checks its (category, parameter count, name) against the
//! reference parse's functions, including const-generic and tuple-type colons that
//! must not inflate the count and multiheaded functions whose heads are separate
//! declarations.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::ast::FunctionCategory;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `toks` block in parser.kel: len at 0,
// then the two length-2048 arrays.
const LEN: usize = 0;
const KINDS: usize = 1;
const VALS: usize = 1 + 2048;

/// Map the reference token stream into the parser stage's `(kind, value)` pairs,
/// interning identifier names into `names` so the parser's yielded name id can be
/// resolved back to a string. The trailing `Eof` token is dropped so the token
/// count is exactly the real tokens; the parser reports DONE at `cursor == len`.
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
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    (kinds, vals)
}

/// Drive the parser stage over `src`, returning the (category, param count, name
/// id) of each yielded declaration.
fn run_parser(src: &str, names: &mut Vec<String>) -> Vec<(i64, i64, i64)> {
    let (kinds, vals) = adapt_tokens(src, names);
    let stage = std::fs::read_to_string("compiler/kel/parser.kel").expect("read parser.kel");
    let module =
        compile(&parse(&tokenize(&stage).expect("lex parser.kel")).expect("parse parser.kel"))
            .expect("compile parser.kel");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parser.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, LEN, Value::Int(kinds.len() as i64))
        .expect("len");
    for (i, (&k, &v)) in kinds.iter().zip(vals.iter()).enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(k))
            .expect("kind");
        vm.set_shared(&mut shared, VALS + i, Value::Int(v))
            .expect("val");
    }

    let mut decls = Vec::new();
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(kinds.len() * 2 + 16) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let dkind = w.rem_euclid(16);
                if dkind == 15 {
                    return decls; // DONE
                }
                if dkind != 0 {
                    let pcount = w.div_euclid(16).rem_euclid(64);
                    let name = w.div_euclid(1024);
                    decls.push((dkind, pcount, name));
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

/// The reference parse's top-level functions, in order, as (category, value-param
/// count, interned name id). The category encoding matches the stage's dkind: fn 1,
/// yield 2, loop 3.
fn reference_functions(src: &str, names: &[String]) -> Vec<(i64, i64, i64)> {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    program
        .functions
        .iter()
        .map(|f| {
            let cat = match f.category {
                FunctionCategory::Fn => 1,
                FunctionCategory::Yield => 2,
                FunctionCategory::Loop => 3,
            };
            let pc = f.params.len() as i64;
            let id = names
                .iter()
                .position(|n| n == &f.name)
                .expect("function name was interned") as i64;
            (cat, pc, id)
        })
        .collect()
}

// A single function: the parser recognises one declaration and yields its name.
#[test]
fn a_single_function_is_recognised() {
    let src = "fn main() -> Word { 42 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 1);
}

// Several functions in order, including parameters and a nested-brace body.
#[test]
fn functions_are_yielded_in_order() {
    let src = "fn inc(x: Word) -> Word { x + 1 } \
        fn choose(a: Word) -> Word { if a > 0 { a } else { 0 } } \
        fn main() -> Word { choose(inc(2)) }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 3);
}

// A body with deeply nested braces (match arms, blocks) still ends at the correct
// closing brace, so the next declaration is recognised.
#[test]
fn nested_braces_do_not_confuse_the_boundary() {
    let src = "fn a(n: Word) -> Word { match n { 0 => 1, _ => n } } \
        fn b(n: Word) -> Word { if n > 0 { if n > 1 { 2 } else { 1 } } else { 0 } }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 2);
}

// An empty program yields no declarations and reaches DONE immediately.
#[test]
fn an_empty_program_yields_no_declarations() {
    let mut names = Vec::new();
    let got = run_parser("", &mut names);
    assert!(got.is_empty());
}

// The three function categories are distinguished by their keyword.
#[test]
fn categories_are_distinguished() {
    let src = "fn a() -> Word { 0 } \
        yield b(r: Word) -> Word { yield r } \
        loop c(r: Word) -> Word { yield r }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    // Categories: fn 1, yield 2, loop 3.
    assert_eq!(got.iter().map(|d| d.0).collect::<Vec<_>>(), vec![1, 2, 3]);
}

// The value-parameter count is read from the parameter parentheses.
#[test]
fn parameter_count_is_read() {
    let src = "fn a() -> Word { 0 } \
        fn b(x: Word) -> Word { x } \
        fn c(x: Word, y: Word, z: Word) -> Word { x + y + z }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.iter().map(|d| d.1).collect::<Vec<_>>(), vec![0, 1, 3]);
}

// A type parameter's colon and a tuple-typed parameter do not inflate the count:
// only the value parameters' own `:` colons are counted.
#[test]
fn type_parameter_and_tuple_type_colons_are_not_counted() {
    let src = "fn h<const n: Word>(x: Word) -> Word { x } \
        fn t(p: (Word, Word)) -> Word { 0 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    // h has one value parameter (x); the `const n: Word` colon sits before the
    // parentheses. t has one value parameter (p) whose tuple type carries no colon.
    assert_eq!(got.iter().map(|d| d.1).collect::<Vec<_>>(), vec![1, 1]);
}

// Each head of a multiheaded function is a separate declaration; the reference
// lists each head, and the parser yields each, with the same category and count.
#[test]
fn a_multiheaded_function_yields_each_head() {
    let src = "yield g(r: Word) -> Word when r > 0 { yield r } \
        yield g(r: Word) -> Word { yield 0 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 2);
    // Both heads: yield category, one parameter.
    assert!(got.iter().all(|d| d.0 == 2 && d.1 == 1));
}
