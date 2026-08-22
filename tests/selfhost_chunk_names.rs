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
/// # THIS DOC BLOCK ONCE SAID THE COMPILE PATH WAS UNAFFECTED. IT IS NOT.
///
/// The first revision asserted *"the COMPILE path is unaffected: `wire.kel`
/// self-compiles byte-identically, so the record stream is right"*. **That was
/// invented, not checked**, and it is false in both halves:
///
/// * `self_host_compile(wire.kel)` **panics** with ``no chunk named `acc` `` —
///   the mis-named declaration has no chunk to attach to. Pinned by
///   [`the_self_hosted_compiler_cannot_yet_compile_wire_kel`].
/// * **`wire.kel` is not in the byte-identity corpus at all.** That oracle covers
///   ten stages — `lexer`, `parse`, `reconstruct`, `codegen`, `analyze` and the
///   five `verify_*` — and `wire.kel` is not one of them, so nothing was
///   contradicting the claim either.
///
/// The correction matters more than the original finding. This is not a metadata
/// blemish beside a working compile; it is a **real limitation of the
/// self-hosted compiler**, on the one stage the differential oracle does not
/// cover. *Any construct the corpus does not contain is unverified by
/// construction* — here it is a whole stage.
///
/// # Why this is pinned rather than repaired
///
/// The trigger is reduced to four lines by
/// [`the_minimal_shape_that_misnames_the_following_declaration`]: a `for` loop
/// containing a data-field assignment, plus a trailing field read as the tail
/// expression. Repairing it means understanding how `parse.kel`'s record stream
/// nests, and this line's rule is to stop and record when the work widens rather
/// than to guess at a cause.
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

/// **THE MINIMAL SHAPE, FOUR LINES, FOUND BY DELTA-DEBUGGING RATHER THAN BY
/// READING.**
///
/// A `for` loop containing a data-field **assignment**, followed by a trailing
/// field **read** as the function's tail expression. The declaration that follows
/// is then named after the field.
///
/// # What was ruled out, because a hypothesis that survives by not being tested is
/// not a finding
///
/// | variant | mis-names? |
/// |---|---|
/// | `for` + assignment + trailing read | **yes** |
/// | `for` + `if`-valued assignment + trailing read | yes — the `if` is irrelevant |
/// | assignment + trailing read, **no `for`** | no |
/// | `for` + assignment, **no trailing read** | no |
/// | a bare field read as the whole body | no |
///
/// Both the loop and the trailing read are required. The first three hypotheses I
/// held — that the body's shape mattered, that the operator mattered, that a
/// single-field data block mattered — were each disproved by a variant.
///
/// # The delta-debug needed a precondition it did not start with
///
/// The first reduction produced three lines of a **malformed** program that
/// "diverged" because the pipeline could not parse it at all. A delta-debug whose
/// predicate does not require a WELL-FORMED input finds the nearest crash, not the
/// defect under study. The predicate now parses with the reference first.
#[test]
fn the_minimal_shape_that_misnames_the_following_declaration() {
    const REPRO: &str = "private data d { a: Word }\n\
                         fn y() -> Word { for j in 0..8 { d.a = 3; } d.a }\n\
                         fn z() -> Word { 9 }\n\
                         fn main() -> Word { y() + z() }\n";

    // The reference accepts it, so this is a valid program and not a syntax probe.
    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(REPRO).expect("lex")).expect("parse"),
    )
    .expect("the reference must accept the reproduction");
    assert!(
        module.chunks.iter().any(|c| c.name == "z"),
        "the reference lost `z`, so the divergence below is not the pipeline's"
    );

    let got = keleusma::selfhost::chunk_names_from_pipeline(REPRO);
    assert!(
        got.contains(&"a".to_string()),
        "the pipeline no longer names a declaration after the field `a`. If the defect \
         is fixed, delete this pin and re-check `wire.kel`: {got:?}"
    );
    assert!(
        !got.contains(&"z".to_string()),
        "the pipeline now carries `z` as well. The shape of the defect has changed and \
         needs re-diagnosing rather than re-pinning: {got:?}"
    );

    // THE CONTROL: removing the `for` removes the mis-naming, so the loop is
    // load-bearing rather than incidental.
    const NO_LOOP: &str = "private data d { a: Word }\n\
                           fn y() -> Word { d.a = 3; d.a }\n\
                           fn z() -> Word { 9 }\n\
                           fn main() -> Word { y() + z() }\n";
    let clean = keleusma::selfhost::chunk_names_from_pipeline(NO_LOOP);
    assert!(
        clean.contains(&"z".to_string()) && !clean.contains(&"a".to_string()),
        "the same program without the `for` loop also mis-names, so the loop is not the \
         trigger and this reduction is wrong: {clean:?}"
    );
}

/// **THE MIS-NAME IS THE TRAILING FIELD'S NAME, AND THAT IS NOW MEASURED RATHER
/// THAN INFERRED.**
///
/// The first record of this defect said only that a missing function "follows a
/// data block whose field turns up in its place", and called the pairing
/// *suggestive, not a diagnosis*. It is a diagnosis now, at the behavioural level,
/// with a control that discriminates:
///
/// | body of the preceding function | the following declaration is named |
/// |---|---|
/// | `for … { d.a = 3; }` then `d.a` | `a` |
/// | `for … { d.a = 3; }` then `d.b` | **`b`** |
/// | `for … { d.b = 3; }` then `d.a` | **`a`** |
/// | `for … { d.a = 3; }` then a literal | correct |
/// | `for … { d.a = 3; }` then a local | correct |
///
/// **Row three is the one that rules out the alternative.** If the mis-name came
/// from the ASSIGNED field it would read `b` there; it reads `a`. The name follows
/// the **trailing field access**, not the assignment, not the block's first field,
/// and not the block's name.
///
/// # What is still NOT established, and it is the part that matters for a fix
///
/// **Where in `parse.kel` this happens.** Two hypotheses have been eliminated:
/// `ps.emit_arg` cannot be the carrier, because `step()` resets it to its `-1`
/// sentinel at the start of every record; and the declaration COUNT matches the
/// source, which rules out a spurious extra declaration and points at a wrong
/// name PAYLOAD on the real header rather than a leaked body record.
///
/// Confirming the site needs the raw `(code, val)` stream, and `thread_local!` is
/// unavailable here (`no_std`), so tracing means threading a sink through
/// `parse_functions_impl` and its four call sites. **Budget for that rather than
/// assuming the fix is small.**
///
/// # Why the `for` loop is required and this test says so
///
/// The same body without the loop does not mis-name — asserted below, because a
/// reduction that keeps an irrelevant construct sends its next reader looking in
/// the wrong place.
#[test]
fn the_misname_follows_the_trailing_field_access() {
    let case = |body: &str| -> Vec<String> {
        let src = format!(
            "private data d {{ a: Word, b: Word }}\n\
             fn y() -> Word {{ {body} }}\n\
             fn z() -> Word {{ 9 }}\n\
             fn main() -> Word {{ y() + z() }}\n"
        );
        // Every probe must be a program the REFERENCE accepts, or it measures a
        // syntax error rather than this defect.
        keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(&src).expect("lex"))
                .expect("parse"),
        )
        .expect("the reference must accept every probe");
        keleusma::selfhost::chunk_names_from_pipeline(&src)
    };

    let trail_a = case("for j in 0..8 { d.a = 3; } d.a");
    let trail_b = case("for j in 0..8 { d.a = 3; } d.b");
    let assign_b_trail_a = case("for j in 0..8 { d.b = 3; } d.a");
    let trail_literal = case("for j in 0..8 { d.a = 3; } 7");
    let no_loop = case("d.a = 3; d.a");

    assert!(
        trail_a.contains(&"a".to_string()) && !trail_a.contains(&"z".to_string()),
        "the trailing `d.a` case no longer mis-names; the defect has moved: {trail_a:?}"
    );
    assert!(
        trail_b.contains(&"b".to_string()) && !trail_b.contains(&"a".to_string()),
        "the mis-name did not follow the trailing field to `b`. It is not the trailing \
         access that carries it, and this diagnosis is wrong: {trail_b:?}"
    );
    assert!(
        assign_b_trail_a.contains(&"a".to_string()),
        "assigning `d.b` and trailing `d.a` produced {assign_b_trail_a:?}. The mis-name \
         follows the ASSIGNED field after all, which is the alternative this case exists \
         to rule out"
    );

    // The two negative controls. Without them, "a trailing field read is required"
    // and "the loop is required" would both be unsupported assertions.
    assert!(
        trail_literal.contains(&"z".to_string()),
        "a trailing LITERAL now mis-names too, so the trailing field access is not the \
         trigger: {trail_literal:?}"
    );
    assert!(
        no_loop.contains(&"z".to_string()),
        "the same body without the `for` loop now mis-names, so the loop is not required \
         and the reduction recorded here is wrong: {no_loop:?}"
    );
}
