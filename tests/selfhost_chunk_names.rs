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
    // `wire.kel` is DELIBERATELY ABSENT and has its own test below. The mapping
    // diverges there, the divergence is not understood, and folding it in would
    // mean either a failing suite or a weakened assertion. It is pinned instead.
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
    // THE BOUND MOVED WHEN `wire` LEFT THE CORPUS, and it is recorded rather than
    // just lowered: it was 500, which `wire.kel`'s 486 chunks carried almost
    // single-handedly. A vacuity guard that only the excluded case satisfied was
    // guarding the wrong thing.
    assert!(
        total_chunks > 200,
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

/// **THE MAPPING DIVERGES ON `wire.kel`, AND THE DIVERGENCE IS NOT UNDERSTOOD.**
///
/// Found 2026-08-22 while checking the derivation rather than assuming it, which
/// is the only reason it is known at all. The shape is specific and reproducible:
///
/// | | |
/// |---|---|
/// | chunk COUNT | agrees exactly, 486 against 486 |
/// | in the compiler, absent from the pipeline | `crc_end`, `parse_prologue` |
/// | in the pipeline, absent from the compiler | `acc`, `dis` |
///
/// **`acc` and `dis` are FIELDS of private data blocks**, declared at `wire.kel`
/// lines 157 and 163. `crc_end` is at line 215 and `parse_prologue` at 403 — each
/// missing function follows a data block whose field turns up in its place. That
/// pairing is suggestive and it is not a diagnosis.
///
/// # Why this is pinned rather than repaired
///
/// The COMPILE path is unaffected: `wire.kel` self-compiles byte-identically, so
/// the record stream `reconstruct.kel` consumes is right. What diverges is the
/// per-function METADATA the driver exposes beside that stream. Repairing it means
/// understanding `parse.kel`'s declaration state machine on this shape, and this
/// line's own rule is to stop and record when the work widens rather than to guess
/// at a cause.
///
/// # What a reader must not conclude
///
/// **Not that the pipeline drops two functions from the compiler.** It does not —
/// byte identity forbids it. The claim is narrower and is the one asserted here:
/// the derived chunk NAME LIST disagrees for this stage. Anything built on
/// `chunk_names_from_pipeline` must treat `wire.kel` as unvalidated until this is
/// closed.
///
/// Pinned in the firing direction: when the divergence goes away, this fails and
/// its author folds `wire` back into the corpus test above.
#[test]
fn the_chunk_name_mapping_is_not_yet_established_for_wire() {
    const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");

    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(WIRE).expect("lex")).expect("parse"),
    )
    .expect("compile");
    let want: Vec<String> = module.chunks.iter().map(|c| c.name.clone()).collect();
    let got = keleusma::selfhost::chunk_names_from_pipeline(WIRE);

    assert!(
        !want.is_empty() && !got.is_empty(),
        "one side produced no names, so this pin measures nothing"
    );
    assert_eq!(
        got.len(),
        want.len(),
        "the chunk COUNTS have parted as well ({} against {}). The recorded divergence was \
         in the name set only; a count difference is a different finding and needs its own \
         diagnosis",
        got.len(),
        want.len()
    );
    assert_ne!(
        got, want,
        "the derived chunk names now AGREE for `wire.kel`. That closes a recorded gap: fold \
         `wire` back into `the_derived_chunk_names_match_the_reference_compiler` and delete \
         this pin rather than relaxing it"
    );

    // The exact shape, so a CHANGE in the divergence is not mistaken for the
    // divergence being unchanged. A different set here means something else moved.
    let missing: Vec<&String> = want.iter().filter(|n| !got.contains(n)).collect();
    let extra: Vec<&String> = got.iter().filter(|n| !want.contains(n)).collect();
    assert_eq!(
        missing.len(),
        2,
        "the recorded divergence was two names missing from the pipeline; it is now \
         {missing:?}"
    );
    assert_eq!(
        extra.len(),
        2,
        "the recorded divergence was two names extra in the pipeline; it is now {extra:?}"
    );
}
