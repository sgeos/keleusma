// The self-hosted compiler is a full-width host tool. Its byte-level and
// op-encoding arithmetic overflows a narrow declared word, so these tests are
// meaningful only on a 64-bit runtime, not under the `narrow-word-*` configs.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
//! End-to-end lexer/parser boundary test: the self-hosted lexer (`kel/lexer.kel`,
//! increment 5) must produce, for real stage source, exactly the token stream the
//! host adapter `adapt_tokens` in `selfhost_parse.rs` produces from the runtime
//! tokenizer. That adapter is the reference parse.kel is validated against, so
//! proving the lexer reproduces it token for token, interned id for interned id,
//! establishes that the lexer is a verified drop-in for the adapter and the two
//! self-hosted stages can be composed with the host only orchestrating the
//! yield/resume loop. This is the guarantee that makes an actual lexer-into-parser
//! pipeline sound.

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

/// The reference token stream: the runtime tokenizer mapped exactly as the parser
/// harness's `adapt_tokens` maps it, to `(Tok code, payload)` pairs with
/// identifiers interned in first-seen order. This is a verbatim copy of that
/// adapter's mapping (kept in sync by construction), minus the trailing EOF, which
/// the adapter drops with `continue`.
fn reference_stream(src: &str) -> Vec<(i64, i64)> {
    let mut names: Vec<String> = Vec::new();
    let mut intern = |s: &str| -> i64 {
        if let Some(i) = names.iter().position(|n| n == s) {
            i as i64
        } else {
            names.push(s.to_string());
            (names.len() - 1) as i64
        }
    };
    let tokens = tokenize(src).expect("lex");
    let mut out = Vec::new();
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
            TokenKind::Data => (13, 0),
            TokenKind::Shared => (14, 0),
            TokenKind::Private => (15, 0),
            TokenKind::Const => (16, 0),
            TokenKind::Eq => (17, 0),
            TokenKind::Use => (19, 0),
            TokenKind::LBracket => (41, 0),
            TokenKind::RBracket => (42, 0),
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
            TokenKind::Dot => (40, 0),
            TokenKind::If => (43, 0),
            TokenKind::Else => (44, 0),
            TokenKind::For => (45, 0),
            TokenKind::In => (46, 0),
            TokenKind::DotDot => (47, 0),
            TokenKind::Match => (48, 0),
            TokenKind::FatArrow => (49, 0),
            TokenKind::Underscore => (50, 0),
            TokenKind::ColonColon => (51, 0),
            TokenKind::As => (52, 0),
            TokenKind::Enum => (53, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        out.push((kind, val));
    }
    out
}

/// Drive `kel/lexer.kel` over `src` and collect its `(tok, payload)` stream,
/// dropping the trailing EOF marker so it lines up with `reference_stream`.
fn lex_stream(src: &str) -> Vec<(i64, i64)> {
    let bytes = src.as_bytes();
    let source = std::fs::read_to_string("compiler/kel/lexer.kel").expect("read lexer.kel");
    let m = compile(&parse(&tokenize(&source).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify");

    assert!(
        bytes.len() <= 4096,
        "source exceeds the lexer's 4096-byte cap"
    );
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(bytes.len() as i64))
        .expect("len");
    for (i, &b) in bytes.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b))
            .expect("byte");
    }

    let mut out = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(bytes.len() * 4 + 16) {
        match st {
            VmState::Yielded(Value::Int(t)) => {
                if t == 63 {
                    // PENDING; skip.
                } else if t == 62 {
                    return out; // EOF
                } else {
                    out.push((t % 64, t / 64));
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected {:?}", other),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("lexer did not reach EOF within the iteration budget");
}

/// Assert the self-hosted lexer reproduces the reference adapter's stream exactly.
fn assert_lexer_matches_adapter(src: &str) {
    assert_eq!(lex_stream(src), reference_stream(src), "source: {src:?}");
}

#[test]
fn lexer_matches_the_adapter_on_a_simple_function() {
    assert_lexer_matches_adapter("fn f(x: Word) -> Word { x + 1 }");
}

#[test]
fn lexer_matches_the_adapter_on_identifiers_with_underscores_and_digits() {
    // The identifier-boundary fixes must agree with the runtime tokenizer on
    // underscores (mid-run and leading) and trailing digits.
    assert_lexer_matches_adapter(
        "fn is_ident_cont(b: Word) -> Word { \
            if is_alpha(b) == 1 { 1 } else { if is_digit(b) == 1 { 1 } else { 0 } } }",
    );
}

#[test]
fn lexer_matches_the_adapter_on_a_when_guarded_yield_head() {
    // `when` is a reserved word the stages use but the parser has no Tok for, so
    // both the adapter and the lexer fold it to the catch-all 4 rather than
    // interning it as an identifier; this checks that alignment.
    assert_lexer_matches_adapter("yield emit(resume: Word) -> Word when pos < len { yield buf }");
}

#[test]
fn lexer_matches_the_adapter_on_the_operator_and_punctuation_surface() {
    // Every compound and single-byte operator, the brackets, the arrow, and a
    // match arm with `=>` and `_`, so the whole punctuation surface is exercised.
    assert_lexer_matches_adapter(
        "fn g(a: Word) -> Word { \
            let r = a * 2 + 1 - 3 / 1 % 4; \
            if r >= 2 andalso r <= 9 orelse r != 0 { \
                match r { 0 => a, _ => r band 1 bor 2 bxor 3 } \
            } else { a[r] } }",
    );
}

#[test]
fn lexer_matches_the_adapter_on_a_data_block_and_enum() {
    assert_lexer_matches_adapter(
        "enum Kind { Lo = 0, Hi = 1 } \
         shared data src { buf: [Word; 16], len: Word } \
         private data st { pos: Word } \
         fn scan(t: Word) -> Word { \
            for i in 0..src.len limit 16 { if src.buf[i] == t { st.pos = i; } } st.pos }",
    );
}

#[test]
fn lexer_matches_the_adapter_on_a_verbatim_stage_function() {
    // A real function copied from lexer.kel itself: keyword classification by run
    // length, dispatching over `peek_at` calls, with underscores, digits, and the
    // `0 - 1` sentinel. The lexer tokenizing its own source into the parser's
    // vocabulary, identically to the reference adapter.
    assert_lexer_matches_adapter(
        "fn keyword_code(start: Word, len: Word) -> Word { \
            if len == 2 { kw2(start) } else { \
            if len == 3 { kw3(start) } else { \
            if len == 4 { kw4(start) } else { \
            0 - 1 } } } }",
    );
}
