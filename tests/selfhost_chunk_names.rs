//! The chunk-index to chunk-name mapping the type channel needs, checked against
//! the reference compiler.
//!
//! # Why this exists as its own file
//!
//! A `Call` node carries the callee's **chunk index**; the type channel is keyed
//! by **names**. Turning one into the other is a numbering, and a numbering
//! derived from a description of itself is a guess.
//!
//! This line has been wrong about a numbering more than once — most sharply when
//! two node caps were conflated and the other line was told their figure was
//! wrong when it was right. The remedy that worked there is the one used here:
//! compare the derivation against the thing it claims to describe, over real
//! input, and assert the comparison is non-vacuous.

#![cfg(all(feature = "self-host", feature = "compile"))]

const CORPUS: &[(&str, &str)] = &[
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    (
        "verify_types",
        include_str!("../src/selfhost/kel/verify_types.kel"),
    ),
    // `wire.kel` IS BACK. It was excluded while `chunk_names_from_pipeline`
    // derived the numbering by hand and got it wrong there; the function now
    // delegates to `first_pass`, which already computed this table, and the
    // exclusion went with the hand-rolled derivation.
    ("wire", include_str!("../src/selfhost/kel/wire.kel")),
];

/// **THE DERIVED NUMBERING IS THE COMPILER'S NUMBERING.**
///
/// Both halves matter. The equality is the claim; the length and content guards
/// are what stop a derivation that returned an empty list from satisfying it.
#[test]
fn the_derived_chunk_names_match_the_reference_compiler() {
    let mut checked = 0usize;
    let mut total_chunks = 0usize;

    for (stage, src) in CORPUS {
        let module = keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
        )
        .expect("compile");
        let want: Vec<String> = module.chunks.iter().map(|c| c.name.clone()).collect();
        let got = keleusma::selfhost::chunk_names_from_pipeline(src);

        assert!(
            !want.is_empty(),
            "{stage}: the reference compiled no chunks, so this case compares nothing"
        );
        assert_eq!(
            got.len(),
            want.len(),
            "{stage}: the pipeline derived {} chunk names against the compiler's {}",
            got.len(),
            want.len()
        );
        assert_eq!(
            got, want,
            "{stage}: the derived chunk numbering disagrees with the compiler's. A `Call` \
             node's chunk index would resolve to the wrong callee name"
        );

        total_chunks += want.len();
        checked += 1;
    }

    assert_eq!(checked, CORPUS.len(), "not every stage was checked");
    // THE BOUND WENT 500 -> 200 -> 500 AND THE ROUND TRIP IS THE RECORD. It was
    // lowered when `wire` was excluded, because `wire.kel`'s 486 chunks carried it
    // almost single-handedly; `wire` is back, so it is back. A guard that only one
    // corpus member satisfies is fragile, and saying so is worth more than a
    // silently restored number.
    assert!(
        total_chunks > 500,
        "only {total_chunks} chunks were compared across the corpus, too few to be these \
         stages; the walk is measuring something other than what it names"
    );
}

/// **THE GROUPING IS LOAD-BEARING, AND A NAIVE ONE-CHUNK-PER-FUNCTION DERIVATION
/// WOULD PASS ON MOST INPUT.**
///
/// A chunk is a run of *consecutive same-named heads* — the multi-arm form, where
/// several `fn` heads share a name. On a source where no name repeats, grouping
/// and not-grouping produce the same list, so the corpus above could pass while
/// the rule was wrong.
///
/// This gives the rule an input that separates the two.
#[test]
fn consecutive_same_named_heads_collapse_into_one_chunk() {
    const MULTI_ARM: &str = "fn f(0) -> Word { 10 }\n\
                             fn f(n) -> Word { n }\n\
                             fn main() -> Word { f(0) + f(1) }";

    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(MULTI_ARM).expect("lex"))
            .expect("parse"),
    )
    .expect("compile");
    let want: Vec<String> = module.chunks.iter().map(|c| c.name.clone()).collect();

    // The discriminating precondition, asserted rather than assumed: the source
    // must actually produce fewer chunks than it has `fn` heads, or the two rules
    // agree here too and this test proves nothing.
    assert_eq!(
        want.iter().filter(|n| n.as_str() == "f").count(),
        1,
        "the compiler no longer collapses two same-named heads into one chunk, so this \
         case cannot distinguish the grouping rule from a per-head one: {want:?}"
    );

    assert_eq!(
        keleusma::selfhost::chunk_names_from_pipeline(MULTI_ARM),
        want,
        "the derived numbering does not collapse consecutive same-named heads"
    );
}

/// **`wire.kel` COMPILES NOW.** This pin recorded that it could not, and is retired here.
///
/// # Why it is retired rather than followed to the letter
///
/// The pin's own instruction was to add `wire.kel` to `assert_stage_byte_identical`'s corpus
/// and delete this test. **That instruction assumed byte-identity, and byte-identity does not
/// hold**: two of 486 chunks still diverge. Following it literally would have put a
/// non-identical stage into the oracle and broken it, or forced the oracle to be relaxed —
/// which is how a corpus quietly stops meaning anything.
///
/// So the claim moved to `tests/wire_self_compile_status.rs`, which pins BOTH halves: that it
/// compiles, and that it is not yet byte-identical, with the two diverging chunks named.
///
/// # What the original recorded, kept because the lesson outlived the defect
///
/// `wire.kel` was never in the byte-identity corpus, so the largest stage at 486 chunks had
/// never been self-hosted and nothing said so. *Any construct the corpus does not contain is
/// unverified by construction* — the lesson that produced the boolean-literal and `Byte`-cast
/// miscompiles, and here the uncovered thing was an entire stage.
///
/// Three causes stood in the way and **two were first diagnosed wrongly**: a capacity bound
/// read off an index message (wrong), the lexer's missing radix literals (correct), a cap of
/// 256 on the declaration count (wrong), and a `Call` record whose chunk field overflowed at
/// index 256 (correct).
#[test]
fn the_self_hosted_compiler_can_now_compile_wire_kel() {
    const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");

    let module = keleusma::selfhost::self_host_compile(WIRE);
    assert_eq!(
        module.chunks.len(),
        486,
        "`wire.kel`'s chunk count moved; the figure this file has quoted for three sessions \
         is stale"
    );
}
