//! Stage 2 parser (`compiler/kel/parser.kel`), increment 1: top-level
//! function-declaration recognition.
//!
//! A throwaway adapter maps the reference tokenizer's output into the parser
//! stage's `(kind, value)` token stream, the Keleusma `loop` consumes it one token
//! per iteration, and it yields one declaration word (`dkind + name*16`) per
//! top-level `fn` declaration, brace-matching the parameters, return type, and body
//! rather than parsing them (a later increment). The host decodes the declarations
//! and checks their kind-and-name sequence against the reference parse.
//!
//! This is the parser's analogue of the lexer's increment 1: it establishes the
//! token-consuming `loop`, the per-declaration yield contract, and the boundary and
//! kind recognition, before the recursive body parsing of later increments.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
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
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    (kinds, vals)
}

/// Drive the parser stage over `src`, returning the (kind, name id) of each yielded
/// declaration.
fn run_parser(src: &str, names: &mut Vec<String>) -> Vec<(i64, i64)> {
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
                    decls.push((dkind, w.div_euclid(16)));
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

/// The reference parse's top-level function names, in order, resolved to the
/// interned name ids the adapter assigned.
fn reference_functions(src: &str, names: &[String]) -> Vec<(i64, i64)> {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    program
        .functions
        .iter()
        .map(|f| {
            let id = names
                .iter()
                .position(|n| n == &f.name)
                .expect("function name was interned") as i64;
            (1, id) // dkind 1 = function
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
