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
//! (`compiler/kel/lexer.kel`, through increment 4). It compiles the lexer on the
//! current runtime, drives it over a source held in shared data, and checks the
//! streamed token encoding: increment 1's IDENT/INT/PUNCT/EOF wire, increment 2's
//! maximal munch over the two-byte operators, increment 3's keyword classification
//! to the parser's Tok codes, and increment 4's identifier interning (an IDENT
//! carries a stable id, not a length). Guards that the lexer keeps compiling and
//! tokenizing as the runtime evolves toward V0.3.0.
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
    // KEYWORD let (Tok::Let = 38, classified since increment 3), IDENT x (interned
    // to id 0 since increment 4), PUNCT '=', INT 42, EOF.
    assert_eq!(
        tokens,
        vec![(6, 38), (2, 0), (4, b'=' as i64), (3, 42), (1, 0)]
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
    // The eight identifiers are all distinct, so they intern to ids 0..7.
    assert_eq!(
        tokens,
        vec![
            (2, 0), // IDENT a -> id 0
            (5, 0), // ==
            (2, 1), // IDENT b -> id 1
            (5, 1), // !=
            (2, 2), // IDENT c -> id 2
            (5, 2), // <=
            (2, 3), // IDENT d -> id 3
            (5, 3), // >=
            (2, 4), // IDENT e -> id 4
            (5, 4), // ::
            (2, 5), // IDENT f -> id 5
            (5, 5), // ..
            (2, 6), // IDENT g -> id 6
            (5, 6), // =>
            (2, 7), // IDENT h -> id 7
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
    // Five distinct identifiers intern to ids 0..4.
    assert_eq!(
        tokens,
        vec![
            (2, 0),           // IDENT a -> id 0
            (4, b'=' as i64), // lone =
            (2, 1),           // IDENT b -> id 1
            (4, b'<' as i64), // lone <
            (2, 2),           // IDENT c -> id 2
            (4, b':' as i64), // lone :
            (2, 3),           // IDENT d -> id 3
            (4, b'.' as i64), // lone .
            (2, 4),           // IDENT e -> id 4
            (4, b'=' as i64), // trailing = at end of input (peek sees sentinel 0)
            (1, 0),           // EOF
        ]
    );
}

// Increment 3: keyword classification. An identifier run that spells a keyword is
// emitted as a KEYWORD token (kind 6) carrying the parser's Tok code, while a
// non-keyword run and a keyword-like prefix (`i`, `fna`, `ip`) stay IDENT.
#[test]
fn self_hosted_lexer_increment_3_keyword_classification() {
    // One keyword from each run length plus a plain identifier and an integer, so
    // the dispatch-by-length and the keyword-versus-IDENT split are both covered.
    let tokens = lex_tokens(b"fn let else const shared private x 9 as");
    assert_eq!(
        tokens,
        vec![
            (6, 0),  // KEYWORD fn      (Tok::Fn = 0)
            (6, 38), // KEYWORD let     (Tok::Let = 38)
            (6, 44), // KEYWORD else    (Tok::Else = 44)
            (6, 16), // KEYWORD const   (Tok::Const = 16)
            (6, 14), // KEYWORD shared  (Tok::Shared = 14)
            (6, 15), // KEYWORD private (Tok::Private = 15)
            (2, 0),  // IDENT x (the only non-keyword run, interned to id 0)
            (3, 9),  // INT 9
            (6, 52), // KEYWORD as      (Tok::As = 52), ending exactly at end of input
            (1, 0),  // EOF
        ]
    );
}

// A keyword's proper prefix, superstring, or same-length near-miss is an ordinary
// identifier, not a keyword: the match is exact over the whole run, not a prefix
// test. `i` (prefix of `if`/`in`), `fnn` (superstring of `fn`), and `ix`
// (same-length near-miss of `if`) must all stay IDENT.
#[test]
fn self_hosted_lexer_increment_3_keyword_matching_is_exact() {
    let tokens = lex_tokens(b"i fnn ix loops");
    // All four are distinct non-keyword runs, interned to ids 0..3.
    assert_eq!(
        tokens,
        vec![
            (2, 0), // IDENT i     (prefix of if/in)
            (2, 1), // IDENT fnn   (fn plus a byte)
            (2, 2), // IDENT ix    (same length as if, second byte differs)
            (2, 3), // IDENT loops (loop plus a byte)
            (1, 0), // EOF
        ]
    );
}

// Increment 4: identifier interning. A non-keyword IDENT now carries a stable id
// (its index in the intern table) rather than a byte length, so repeated
// identifiers share an id and distinct ones get sequential ids in first-seen
// order. Keywords are classified before interning, so `let` never consumes an id.
#[test]
fn self_hosted_lexer_increment_4_identifier_interning() {
    let tokens = lex_tokens(b"foo bar foo let baz bar foo");
    assert_eq!(
        tokens,
        vec![
            (2, 0),  // foo -> new id 0
            (2, 1),  // bar -> new id 1
            (2, 0),  // foo -> id 0
            (6, 38), // KEYWORD let (Tok::Let, not interned)
            (2, 2),  // baz -> new id 2
            (2, 1),  // bar -> id 1
            (2, 0),  // foo -> id 0
            (1, 0),  // EOF
        ]
    );
}

// Interning keys on content, not length: same-length distinct runs get distinct
// ids, and a repeat resolves to its original id regardless of intervening
// identifiers (the table is a set, not a stack). The last run ends exactly at end
// of input, exercising the interning path in the flush branch.
#[test]
fn self_hosted_lexer_increment_4_interning_is_by_content() {
    let tokens = lex_tokens(b"cat dog cot dog cat");
    assert_eq!(
        tokens,
        vec![
            (2, 0), // cat -> id 0
            (2, 1), // dog -> id 1
            (2, 2), // cot -> id 2 (same length as cat, differs at byte 1)
            (2, 1), // dog -> id 1
            (2, 0), // cat -> id 0, ending exactly at end of input
            (1, 0), // EOF
        ]
    );
}
