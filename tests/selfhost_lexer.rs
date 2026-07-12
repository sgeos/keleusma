// The self-hosted compiler is a full-width host tool. Its byte-level and
// op-encoding arithmetic overflows a narrow declared word, and the compiler
// rejects a target wider than the runtime, so these tests are meaningful only on
// a 64-bit runtime, not under the `narrow-word-*` (embedded-target) feature
// configs.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
//! Regression test for the self-hosted compiler's Stage 1 lexer
//! (`compiler/kel/lexer.kel`, through increment 2). It compiles the lexer on the
//! current runtime, drives it over a source held in shared data, and checks the
//! streamed token encoding: increment 1's IDENT/INT/PUNCT/EOF wire and
//! increment 2's maximal munch over the two-byte operators. Guards that the lexer
//! keeps compiling and tokenizing as the runtime evolves toward V0.3.0.
use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

#[test]
fn self_hosted_lexer_increment_1() {
    let src = std::fs::read_to_string("compiler/kel/lexer.kel").expect("read lexer.kel");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify");

    let input = b"let x = 42";
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(input.len() as i64))
        .expect("len");
    for (i, &byte) in input.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(byte))
            .expect("byte");
    }

    // Collect non-PENDING tokens as (kind, value) until EOF.
    let mut tokens: Vec<(i64, i64)> = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..64 {
        match st {
            VmState::Yielded(Value::Int(t)) => {
                let (kind, value) = (t % 16, t / 16);
                if kind != 0 {
                    tokens.push((kind, value));
                    if kind == 1 {
                        break; // EOF
                    }
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected {:?}", other),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    eprintln!("tokens (kind,value) = {:?}", tokens);
    // IDENT len3 "let", IDENT len1 "x", PUNCT '=' , INT 42, EOF
    assert_eq!(
        tokens,
        vec![(2, 3), (2, 1), (4, b'=' as i64), (3, 42), (1, 0)]
    );
}

/// Drive the increment-1 lexer over `input` and collect its non-PENDING
/// `(kind, value)` tokens through EOF. Shared by the increment-2 assertions.
fn lex_tokens(input: &[u8]) -> Vec<(i64, i64)> {
    let src = std::fs::read_to_string("compiler/kel/lexer.kel").expect("read lexer.kel");
    let m = compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(input.len() as i64))
        .expect("len");
    for (i, &byte) in input.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(byte))
            .expect("byte");
    }

    let mut tokens: Vec<(i64, i64)> = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(input.len() * 4 + 16) {
        match st {
            VmState::Yielded(Value::Int(t)) => {
                let (kind, value) = (t % 16, t / 16);
                if kind != 0 {
                    tokens.push((kind, value));
                    if kind == 1 {
                        break; // EOF
                    }
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected {:?}", other),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    tokens
}

// Increment 2: maximal munch over the two-byte operators. Each of `==`, `!=`,
// `<=`, `>=`, `::`, `..`, `=>` becomes one OP2 token (kind 5) carrying its
// compound code, rather than two single-byte PUNCT tokens. This is the first
// step toward the lexer emitting the parser's unified token vocabulary.
#[test]
fn self_hosted_lexer_increment_2_compound_operators() {
    // Every compound operator, separated by identifiers so each stands alone.
    let tokens = lex_tokens(b"a==b!=c<=d>=e::f..g=>h");
    assert_eq!(
        tokens,
        vec![
            (2, 1), // IDENT a
            (5, 0), // ==
            (2, 1), // IDENT b
            (5, 1), // !=
            (2, 1), // IDENT c
            (5, 2), // <=
            (2, 1), // IDENT d
            (5, 3), // >=
            (2, 1), // IDENT e
            (5, 4), // ::
            (2, 1), // IDENT f
            (5, 5), // ..
            (2, 1), // IDENT g
            (5, 6), // =>
            (2, 1), // IDENT h
            (1, 0), // EOF
        ]
    );
}

// The maximal munch must not over-consume: a `=`, `<`, `:`, or `.` not followed
// by its partner stays a single-byte PUNCT, including at end of input where the
// lookahead sees the past-the-end sentinel 0.
#[test]
fn self_hosted_lexer_increment_2_single_byte_punctuation_unaffected() {
    // `a = b < c : d . e` — lone operators, and a trailing `=` at end of input.
    let tokens = lex_tokens(b"a=b<c:d.e=");
    assert_eq!(
        tokens,
        vec![
            (2, 1),           // IDENT a
            (4, b'=' as i64), // lone =
            (2, 1),           // IDENT b
            (4, b'<' as i64), // lone <
            (2, 1),           // IDENT c
            (4, b':' as i64), // lone :
            (2, 1),           // IDENT d
            (4, b'.' as i64), // lone .
            (2, 1),           // IDENT e
            (4, b'=' as i64), // trailing = at end of input (peek sees sentinel 0)
            (1, 0),           // EOF
        ]
    );
}
