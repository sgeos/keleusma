#![cfg(all(feature = "compile", feature = "verify"))]
//! Length-dependent WCET cost for string operations (#49).
//!
//! A text comparison (`Op::CmpEq`/`Op::CmpNe`), concatenation (`Op::Add` on
//! text), and `Op::Len` on text run in time proportional to a string length,
//! which the flat per-opcode cost table cannot capture. The verifier's WCET
//! pass now adds `text_byte_cycles * length` for a statically-bounded length
//! (a comparison is bounded by the shorter operand, so a literal operand bounds
//! it) and reports the per-iteration WCET as non-boundable when a length cannot
//! be statically bounded (two unbounded operands). The compiler folds a
//! non-boundable per-iteration WCET into the module's WCET-overflow header
//! rather than rejecting the program, so these tests exercise the verifier
//! entry point directly.

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::wcet_stream_iteration;

/// Per-iteration WCET of the compiled module's `main` Stream chunk, or `Err`
/// when the bound is not statically extractable.
fn stream_wcet(src: &str) -> Result<u32, ()> {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("main chunk");
    wcet_stream_iteration(main).map_err(|_| ())
}

#[test]
fn text_comparison_against_a_literal_is_bounded() {
    // A native-returned text has unbounded length, but comparing it against a
    // fixed literal is bounded: the VM compares length-first, so the literal
    // caps the work. The per-iteration WCET is finite.
    let wcet = stream_wcet(
        "use host_text() -> Text\n\
         loop main(seed: Word) -> Word { if host_text() == \"admin\" { yield seed } else { yield 0 } }",
    );
    assert!(
        wcet.is_ok(),
        "comparison against a literal is bounded by the literal length"
    );
}

#[test]
fn text_comparison_cost_scales_with_the_bounding_literal_length() {
    // The only difference between the two programs is the literal compared
    // against the native text: a 1-byte literal versus a 5-byte literal. At one
    // nominal cycle per byte the per-iteration WCET must differ by exactly four
    // cycles, isolating the length-dependent term.
    let short = stream_wcet(
        "use host_text() -> Text\n\
         loop main(seed: Word) -> Word { if host_text() == \"x\" { yield seed } else { yield 0 } }",
    )
    .expect("bounded");
    let long = stream_wcet(
        "use host_text() -> Text\n\
         loop main(seed: Word) -> Word { if host_text() == \"admin\" { yield seed } else { yield 0 } }",
    )
    .expect("bounded");
    assert_eq!(
        long - short,
        4,
        "a 5-byte literal costs four more cycles than a 1-byte literal at one cycle per byte \
         (long={long}, short={short})"
    );
}

#[test]
fn comparison_of_two_unbounded_texts_is_not_boundable() {
    // Both operands are native-returned texts of unbounded length; the
    // comparison is O(length) with no static bound, so the per-iteration WCET
    // is reported as non-boundable.
    let wcet = stream_wcet(
        "use host_text() -> Text\n\
         loop main(seed: Word) -> Word { let a = host_text(); let b = host_text(); \
          if a == b { yield seed } else { yield 0 } }",
    );
    assert!(
        wcet.is_err(),
        "two unbounded-length text operands cannot be statically bounded"
    );
}
