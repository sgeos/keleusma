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

/// **THE SELF-HOSTED COMPILER CANNOT COMPILE `wire.kel`, AND NOTHING SAID SO.**
///
/// `self_host_compile(wire.kel)` panics with ``no chunk named `acc` ``: the
/// declaration that should be `crc_end` is named after a private-data field, and
/// no chunk carries that name.
///
/// # Why this was invisible
///
/// **`wire.kel` is not in the byte-identity corpus.** That oracle covers ten
/// stages — `lexer`, `parse`, `reconstruct`, `codegen`, `analyze`, and the five
/// `verify_*` — via `assert_stage_byte_identical` in
/// `tests/selfhost_codegen.rs`. `wire.kel` appears in the wire-format tests only
/// as a **reference-compiled input**, which never runs the self-hosted compiler
/// over it.
///
/// So the largest stage in the corpus, at 486 chunks, has never been self-hosted
/// and nothing recorded the gap. *Any construct the corpus does not contain is
/// unverified by construction* — the lesson that produced the boolean-literal and
/// `Byte`-cast miscompiles — and here the uncovered thing is an entire stage.
///
/// # This is a REPORT, not a regression
///
/// Nothing broke. `wire.kel` was never self-compiled, so no capability was lost;
/// what changed is that the tree now says so. A reader must not read this as
/// "self-hosting regressed".
///
/// Pinned in the firing direction, and the control is what makes it a statement
/// about `wire.kel` rather than about the compiler.
#[test]
fn the_self_hosted_compiler_cannot_yet_compile_wire_kel() {
    const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");
    const LEXER: &str = include_str!("../src/selfhost/kel/lexer.kel");

    // THE CONTROL FIRST. Without it a compiler that failed on everything would
    // satisfy the assertion below and look like a fact about `wire.kel`.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let control = std::panic::catch_unwind(|| keleusma::selfhost::self_host_compile(LEXER));
    let subject = std::panic::catch_unwind(|| keleusma::selfhost::self_host_compile(WIRE));
    std::panic::set_hook(prev);

    assert!(
        control.is_ok(),
        "`lexer.kel` no longer self-compiles, so the failure on `wire.kel` below says \
         nothing about `wire.kel` specifically"
    );
    assert!(
        subject.is_err(),
        "`wire.kel` now self-compiles. That closes a recorded gap and is worth saying \
         plainly: add it to `assert_stage_byte_identical`'s corpus in \
         `tests/selfhost_codegen.rs` and delete this pin rather than relaxing it"
    );
}
