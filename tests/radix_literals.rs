//! Radix-prefixed integer literals in the self-hosted lexer.
//!
//! # What was wrong
//!
//! `lexer.kel` had no support for them at all. It consumed the leading `0`, stopped, and
//! interned the remainder as an IDENTIFIER: `0xFF` became the number **zero** followed by a
//! name `xFF`. Measured with `lex_token_trace`, not inferred.
//!
//! `wire.kel` uses thirty-five of them, so the largest stage in the corpus could not
//! self-compile. That failure had been recorded as a capacity bound, which it never was.
//!
//! # Why it went unmeasured
//!
//! **The construct-support boundary contained no radix literal.** Ninety-six cases and not
//! one, so the gap was unverified by construction — the fourth instance of that class after
//! the boolean literal, the `Byte` cast, and the bare `for` form.
//!
//! # Proportionality
//!
//! `self_hosted_compile` cross-checks against the reference and refuses on divergence, so a
//! command-line user got a loud error rather than a wrong artifact. Direct callers of
//! `self_host_compile` received a module with an undefined name where a constant belonged.

#![cfg(all(feature = "self-host", feature = "compile"))]

use keleusma::token::TokenKind;

/// The integer and identifier tokens the REFERENCE produces, in order.
fn reference_tokens(src: &str) -> Vec<String> {
    keleusma::lexer::tokenize(src)
        .expect("the reference must accept a probe before it says anything about the stage")
        .iter()
        .filter_map(|t| match &t.kind {
            TokenKind::IntLit(v) => Some(format!("int {v}")),
            TokenKind::ByteLit(v) => Some(format!("byte {v}")),
            TokenKind::LowerIdent(n) | TokenKind::UpperIdent(n) => Some(format!("ident {n}")),
            _ => None,
        })
        .collect()
}

/// The same, from the self-hosted stage.
fn stage_tokens(src: &str) -> Vec<String> {
    let (names, toks) = keleusma::selfhost::lex_token_trace(src);
    toks.iter()
        .filter_map(|(k, v)| match k {
            12 => Some(format!("int {v}")),
            1 => Some(format!(
                "ident {}",
                names.get(*v as usize).cloned().unwrap_or_default()
            )),
            _ => None,
        })
        .collect()
}

/// Every accepted radix form lexes to the same tokens as the reference.
///
/// **Compared against the reference rather than a hand-written expectation.** A table of
/// expected values would encode my reading of the rules, and the rules are exactly what was
/// misjudged: the point is agreement, not plausibility.
#[test]
fn every_radix_form_agrees_with_the_reference() {
    for src in [
        "fn f() -> Word { 0xFF }",
        "fn f() -> Word { 0xff }",
        "fn f() -> Word { 0XAB }",
        "fn f() -> Word { 0Xab }",
        "fn f() -> Word { 0x0 }",
        "fn f() -> Word { 0xdeadBEEF }",
        "fn f() -> Word { 0b1010 }",
        "fn f() -> Word { 0B1101 }",
        "fn f() -> Word { 0b0 }",
        "fn f() -> Word { 1 + 0xFFFF }",
        "fn f() -> Word { 0xFF + 0b11 }",
        // Decimal must be untouched by the change.
        "fn f() -> Word { 255 }",
        "fn f() -> Word { 0 }",
        "fn f() -> Word { 1024 }",
    ] {
        assert_eq!(
            stage_tokens(src),
            reference_tokens(src),
            "the stage and the reference disagree on `{src}`"
        );
    }
}

/// **`0B` IS NOT UNCONDITIONALLY A BINARY PREFIX**, and guessing otherwise breaks `0Byte`.
///
/// The reference treats `0B` as binary only when a binary digit follows; otherwise the `B`
/// begins the `Byte` numeric suffix, so `0Byte` is the byte literal zero. A stage that took
/// `0B` as binary unconditionally would turn `0Byte` into a malformed binary literal.
///
/// This test asserts the DISAMBIGUATION, not the suffix support: the stage has never
/// supported numeric suffixes, so it and the reference still differ on `0Byte`. What must
/// hold is that the stage does not read the `B` as a radix prefix.
#[test]
fn an_uppercase_b_without_a_binary_digit_is_not_a_radix_prefix() {
    let stage = stage_tokens("fn f() -> Byte { 0Byte }");
    assert!(
        stage.contains(&"int 0".to_string()),
        "`0Byte` must still yield the integer zero, got {stage:?}"
    );
    assert!(
        stage.contains(&"ident Byte".to_string()),
        "the `B` must begin the `Byte` suffix rather than a binary literal, got {stage:?}"
    );
}

/// No part of a numeric literal is interned as a name.
///
/// **An operations-only comparison would pass while this was broken.** The old stage
/// interned `xFF` as a real identifier, so a module could carry the right instructions and
/// a polluted `NAMES` region.
#[test]
fn no_part_of_a_radix_literal_is_interned_as_a_name() {
    for (src, fragment) in [
        ("fn f() -> Word { 0xFF }", "xFF"),
        ("fn f() -> Word { 0XAB }", "XAB"),
        ("fn f() -> Word { 0b1010 }", "b1010"),
        ("fn f() -> Word { 0xdeadBEEF }", "xdeadBEEF"),
    ] {
        let (names, _) = keleusma::selfhost::lex_token_trace(src);
        assert!(
            !names.iter().any(|n| n == fragment),
            "`{fragment}` was interned as a name from `{src}`; the name table is polluted \
             even if the instructions are right. Names: {names:?}"
        );
    }
}

/// A hexadecimal literal reaches the compiled module with its value intact.
///
/// The lexer test above proves the token; this proves the whole pipeline carries it, since
/// a correct token discarded downstream would satisfy the former and not the latter.
#[test]
fn a_hex_literal_survives_the_whole_self_hosted_pipeline() {
    const SRC: &str = "fn f() -> Word { 0xFFFF }\nfn main() -> Word { f() }";
    let reference = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(SRC).expect("lex")).expect("parse"),
    )
    .expect("reference compile");
    let mine = keleusma::selfhost::self_host_compile(SRC);
    assert_eq!(
        keleusma::wire_format::module_to_wire_bytes(&mine).expect("mine"),
        keleusma::wire_format::module_to_wire_bytes(&reference).expect("reference"),
        "a hexadecimal literal does not self-compile byte-identically"
    );
}

/// `wire.kel` COMPILES, and the radix repair is why it got past `crc_begin`.
///
/// **This pin recorded a failure and the failure is gone.** It is re-aimed rather than
/// deleted: the property worth keeping is that the *radix* cause specifically does not
/// return, which a test asserting only "it compiles" would not distinguish from a
/// regression that reintroduced it alongside some other repair.
///
/// `wire.kel` is **not** byte-identical -- two chunks still diverge. That is pinned in
/// `tests/wire_self_compile_status.rs`, which owns the whole-file claim; this test owns only
/// the radix half of it.
#[test]
fn wire_kel_no_longer_fails_on_a_radix_literal() {
    const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");

    // The literals themselves must still lex correctly in the file that needed them.
    let (names, _) = keleusma::selfhost::lex_token_trace(WIRE);
    for fragment in ["xFFFFFFFF", "xEDB88320"] {
        assert!(
            !names.iter().any(|n| n == fragment),
            "`{fragment}` is interned as a name again, so the radix repair regressed in the \
             file that motivated it"
        );
    }

    // And it compiles, which it could not before the repair.
    keleusma::selfhost::self_host_compile(WIRE);
}
