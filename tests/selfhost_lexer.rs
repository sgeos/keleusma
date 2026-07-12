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
//! (`compiler/kel/lexer.kel`, through increment 5). It compiles the lexer on the
//! current runtime, drives it over a source held in shared data, and checks the
//! streamed token encoding. Increment 5 unifies the wire to `tok + payload*64`
//! where `tok` is the parser's Tok discriminant (with 63 PENDING and 62 EOF above
//! the Tok range), so the lexer's output stream is the parser's input stream: the
//! two-byte operators, keywords, interned identifiers, the single-byte
//! punctuation, the `->` arrow, and the lone `_` all carry their Tok codes. Guards
//! that the lexer keeps compiling and tokenizing as the runtime evolves toward
//! V0.3.0.
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
                // Unified wire (increment 5): `tok + payload*64`, with 63 PENDING
                // (skipped) and 62 EOF (recorded as (62, 0), then stop).
                if t == 63 {
                } else if t == 62 {
                    tokens.push((62, 0));
                    break; // EOF
                } else {
                    tokens.push((t % 64, t / 64));
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected {:?}", other),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    eprintln!("tokens (tok,payload) = {:?}", tokens);
    // Unified Tok wire (increment 5): let (Tok::Let 38), x (Tok::Ident 1, interned
    // id 0), = (Tok::Eq 17), 42 (Tok::IntLit 12, value 42), EOF (62).
    assert_eq!(tokens, vec![(38, 0), (1, 0), (17, 0), (12, 42), (62, 0)]);
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
                // Unified wire (increment 5): `tok + payload*64`, with 63 PENDING
                // (skipped) and 62 EOF (recorded as (62, 0), then stop).
                if t == 63 {
                } else if t == 62 {
                    tokens.push((62, 0));
                    break; // EOF
                } else {
                    tokens.push((t % 64, t / 64));
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

// Increment 2/5: maximal munch over the two-byte operators, each mapped to its
// parser Tok code. `==`->EqEq 26, `!=`->NotEq 27, `<=`->LtEq 30, `>=`->GtEq 31,
// `::`->ColCol 51, `..`->DotDot 47, `=>`->FatArrow 49.
#[test]
fn self_hosted_lexer_increment_2_compound_operators() {
    // Every compound operator, separated by identifiers so each stands alone.
    let tokens = lex_tokens(b"a==b!=c<=d>=e::f..g=>h");
    // The eight identifiers are all distinct, so they intern to ids 0..7.
    assert_eq!(
        tokens,
        vec![
            (1, 0),  // Ident a -> id 0
            (26, 0), // ==  EqEq
            (1, 1),  // Ident b -> id 1
            (27, 0), // !=  NotEq
            (1, 2),  // Ident c -> id 2
            (30, 0), // <=  LtEq
            (1, 3),  // Ident d -> id 3
            (31, 0), // >=  GtEq
            (1, 4),  // Ident e -> id 4
            (51, 0), // ::  ColCol
            (1, 5),  // Ident f -> id 5
            (47, 0), // ..  DotDot
            (1, 6),  // Ident g -> id 6
            (49, 0), // =>  FatArrow
            (1, 7),  // Ident h -> id 7
            (62, 0), // EOF
        ]
    );
}

// The maximal munch must not over-consume: a `=`, `<`, `:`, or `.` not followed
// by its partner stays a single-byte token (mapped to its own Tok code), including
// at end of input where the lookahead sees the past-the-end sentinel 0.
#[test]
fn self_hosted_lexer_increment_2_single_byte_punctuation_unaffected() {
    // `a = b < c : d . e` — lone operators, and a trailing `=` at end of input.
    let tokens = lex_tokens(b"a=b<c:d.e=");
    // Five distinct identifiers intern to ids 0..4.
    assert_eq!(
        tokens,
        vec![
            (1, 0),  // Ident a -> id 0
            (17, 0), // lone =  Eq
            (1, 1),  // Ident b -> id 1
            (28, 0), // lone <  Lt
            (1, 2),  // Ident c -> id 2
            (9, 0),  // lone :  Colon
            (1, 3),  // Ident d -> id 3
            (40, 0), // lone .  Dot
            (1, 4),  // Ident e -> id 4
            (17, 0), // trailing = at end of input (peek sees sentinel 0)  Eq
            (62, 0), // EOF
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
            (0, 0),  // fn      (Tok::Fn = 0)
            (38, 0), // let     (Tok::Let = 38)
            (44, 0), // else    (Tok::Else = 44)
            (16, 0), // const   (Tok::Const = 16)
            (14, 0), // shared  (Tok::Shared = 14)
            (15, 0), // private (Tok::Private = 15)
            (1, 0),  // Ident x (the only non-keyword run, interned to id 0)
            (12, 9), // IntLit 9
            (52, 0), // as      (Tok::As = 52), ending exactly at end of input
            (62, 0), // EOF
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
    // All four are distinct non-keyword runs (Tok::Ident 1), interned to ids 0..3.
    assert_eq!(
        tokens,
        vec![
            (1, 0),  // Ident i     (prefix of if/in)
            (1, 1),  // Ident fnn   (fn plus a byte)
            (1, 2),  // Ident ix    (same length as if, second byte differs)
            (1, 3),  // Ident loops (loop plus a byte)
            (62, 0), // EOF
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
            (1, 0),  // foo -> new id 0
            (1, 1),  // bar -> new id 1
            (1, 0),  // foo -> id 0
            (38, 0), // let (Tok::Let, not interned)
            (1, 2),  // baz -> new id 2
            (1, 1),  // bar -> id 1
            (1, 0),  // foo -> id 0
            (62, 0), // EOF
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
            (1, 0),  // cat -> id 0
            (1, 1),  // dog -> id 1
            (1, 2),  // cot -> id 2 (same length as cat, differs at byte 1)
            (1, 1),  // dog -> id 1
            (1, 0),  // cat -> id 0, ending exactly at end of input
            (62, 0), // EOF
        ]
    );
}

// Increment 5: the unified Tok wire. Single-byte punctuation, the `->` arrow, and
// the lone `_` all map to their parser Tok codes, so the lexer's stream is the
// parser's input directly. `->` has no dedicated Tok, so it folds to the catch-all
// 4 (which the parser skips); a lone `_` is Underscore (50).
#[test]
fn self_hosted_lexer_increment_5_punctuation_arrow_and_underscore() {
    let tokens = lex_tokens(b"(x) -> _ { , ; [ ] }");
    assert_eq!(
        tokens,
        vec![
            (7, 0),  // (  LParen
            (1, 0),  // x  Ident id 0
            (8, 0),  // )  RParen
            (4, 0),  // -> Arrow (catch-all)
            (50, 0), // _  Underscore
            (2, 0),  // {  LBrace
            (10, 0), // ,  Comma
            (39, 0), // ;  Semicolon
            (41, 0), // [  LBracket
            (42, 0), // ]  RBracket
            (3, 0),  // }  RBrace
            (62, 0), // EOF
        ]
    );
}

// The single-character operator set each maps to its Tok code, and a `-` not
// immediately followed by `>` stays Minus rather than being munched into an arrow.
#[test]
fn self_hosted_lexer_increment_5_operator_set() {
    let tokens = lex_tokens(b"+ - * / % < >");
    assert_eq!(
        tokens,
        vec![
            (21, 0), // +  Plus
            (22, 0), // -  Minus (space before *, so not an arrow)
            (23, 0), // *  Star
            (24, 0), // /  Slash
            (25, 0), // %  Percent
            (28, 0), // <  Lt
            (29, 0), // >  Gt
            (62, 0), // EOF
        ]
    );
}

// A lone `_` is Underscore, but `_foo` and `_bar` are ordinary identifiers: the
// underscore special case fires only for a run of exactly one `_` byte.
#[test]
fn self_hosted_lexer_increment_5_underscore_versus_identifier() {
    let tokens = lex_tokens(b"_foo _ _bar _");
    assert_eq!(
        tokens,
        vec![
            (1, 0),  // _foo  Ident id 0
            (50, 0), // _     Underscore
            (1, 1),  // _bar  Ident id 1
            (50, 0), // _     Underscore, ending exactly at end of input
            (62, 0), // EOF
        ]
    );
}

// Regression for the two identifier-boundary bugs the unified wire surfaced: an
// underscore mid-run (`is_alpha`, `chunk_count`) and a trailing digit (`kw2`) are
// each one Ident, not a split. These are identifiers the stages themselves use, so
// the lexer must tokenize its own vocabulary correctly. Interning still holds: the
// repeated `is_alpha` resolves to its first id.
#[test]
fn self_hosted_lexer_increment_5_identifiers_with_underscores_and_digits() {
    let tokens = lex_tokens(b"is_alpha kw2 chunk_count is_alpha");
    assert_eq!(
        tokens,
        vec![
            (1, 0),  // is_alpha    -> id 0
            (1, 1),  // kw2         -> id 1 (trailing digit stays in the identifier)
            (1, 2),  // chunk_count -> id 2
            (1, 0),  // is_alpha    -> id 0 (repeat resolves to the first id)
            (62, 0), // EOF
        ]
    );
}
