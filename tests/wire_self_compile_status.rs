//! **`wire.kel` COMPILES THROUGH THE SELF-HOSTED PIPELINE, AND IS NOT BYTE-IDENTICAL.**
//!
//! Both halves matter and they are recorded together on purpose. The claim
//! "`wire.kel` self-compiles byte-identically" was once invented in this repository and
//! reached a doc comment, a pull-request body and all three resume channels before anyone
//! checked it. This file exists so the distinction cannot be blurred again.
//!
//! # What changed
//!
//! The largest stage in the corpus, at 486 chunks, had never compiled at all. Three causes
//! were named and cleared in sequence, and **two of the three were first diagnosed wrongly**:
//!
//! | recorded cause | verdict |
//! |---|---|
//! | a capacity bound, read off the `1024` in an index message | **wrong** |
//! | the lexer having no hexadecimal or binary literal support | correct, fixed |
//! | a cap of 256 on the declaration count | **wrong** |
//! | a `Call` record whose chunk field overflowed at index 256 | correct, fixed |
//!
//! # What remains, stated as measured and not explained
//!
//! Two of 486 chunks differ, and the self-hosted compiler emits FEWER operations for both.
//! Extracted verbatim into a small program each one compiles byte-identically, so the
//! divergence is **context-dependent** and its mechanism is **unknown**. Guessing at the
//! construct has already failed here: the shared `for i in 0..16` proved innocent under four
//! separate probes.

#![cfg(all(feature = "self-host", feature = "compile"))]

const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");

/// `wire.kel` compiles, and the chunk count matches the reference.
///
/// **Pinned in the direction that breaks on regression.** If it stops compiling, the cause
/// that returned should be named rather than discovered again from scratch.
#[test]
fn wire_kel_compiles_through_the_self_hosted_pipeline() {
    let mine = keleusma::selfhost::self_host_compile(WIRE);
    let reference = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(WIRE).expect("lex")).expect("parse"),
    )
    .expect("reference compile");

    assert_eq!(
        mine.chunks.len(),
        reference.chunks.len(),
        "the two compilers no longer agree on how many chunks `wire.kel` has"
    );
    assert_eq!(mine.chunks.len(), 486, "`wire.kel`'s chunk count moved");
}

/// **IT IS NOT BYTE-IDENTICAL, AND EXACTLY TWO CHUNKS DIVERGE.**
///
/// Pinned in the FAILING direction: closing the gap breaks this test rather than passing
/// silently, and the message says what to do. The two names are asserted so a *different*
/// pair diverging is a failure rather than a quiet substitution.
#[test]
fn wire_kel_is_not_yet_byte_identical_and_two_chunks_name_the_gap() {
    let mine = keleusma::selfhost::self_host_compile(WIRE);
    let reference = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(WIRE).expect("lex")).expect("parse"),
    )
    .expect("reference compile");

    let diverging: Vec<&str> = mine
        .chunks
        .iter()
        .zip(reference.chunks.iter())
        .filter(|(m, r)| m.ops != r.ops)
        .map(|(_, r)| r.name.as_str())
        .collect();

    assert!(
        !diverging.is_empty(),
        "`wire.kel` now self-compiles byte-identically. That closes the last stage outside \
         the oracle: add it to `assert_stage_byte_identical`'s corpus in \
         `tests/selfhost_codegen.rs` and delete this pin rather than relaxing it"
    );
    assert_eq!(
        diverging,
        vec!["emit_prologue", "prologue_disagreed"],
        "a different set of chunks diverges than the pair recorded here. Say which, and \
         whether the recorded pair was fixed or merely displaced"
    );

    // The direction is part of the finding: the stage emits FEWER operations, which is a
    // dropped construct rather than a mistranslated one, and narrows where to look.
    for (m, r) in mine.chunks.iter().zip(reference.chunks.iter()) {
        if m.ops != r.ops {
            assert!(
                m.ops.len() < r.ops.len(),
                "chunk `{}` now emits MORE operations than the reference ({} vs {}). The \
                 divergence changed character, so the note in this file is stale",
                r.name,
                m.ops.len(),
                r.ops.len()
            );
        }
    }
}

/// The divergence is CONTEXT-DEPENDENT: each function compiles byte-identically alone.
///
/// **This is what stops the next reader from bisecting the function bodies.** Four probes
/// over the construct the two share -- a bare `for` over a constant range -- all came back
/// identical, and so did both functions extracted verbatim.
#[test]
fn the_diverging_functions_compile_identically_in_isolation() {
    const EXTRACT: &str = r#"data wire { bytes: [Byte; 64] }
private data st { dis: Word }
fn neq(a: Word, b: Word) -> Word { if a == b { 0 } else { 1 } }
fn voted_byte(i: Word) -> Word { wire.bytes[i] as Word }
fn prologue_disagreed() -> Word {
    st.dis = 0;
    for i in 0..16 {
        st.dis = st.dis
            bor neq(wire.bytes[i] as Word, voted_byte(i))
            bor neq(wire.bytes[16 + i] as Word, voted_byte(i))
            bor neq(wire.bytes[32 + i] as Word, voted_byte(i));
    }
    st.dis
}
fn main() -> Word { prologue_disagreed() }
"#;
    let reference = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(EXTRACT).expect("lex")).expect("parse"),
    )
    .expect("reference compile");
    let mine = keleusma::selfhost::self_host_compile(EXTRACT);
    assert_eq!(
        keleusma::wire_format::module_to_wire_bytes(&mine).expect("mine"),
        keleusma::wire_format::module_to_wire_bytes(&reference).expect("reference"),
        "`prologue_disagreed` now diverges in isolation too. That is a BETTER position than \
         the recorded one -- the defect became reproducible in a small program -- so update \
         this pin and bisect the body"
    );
}
