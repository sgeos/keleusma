//! **THE TWELFTH STAGE DOES NOT SELF-COMPILE, AND THIS IS EXACTLY WHY.**
//!
//! `verify_types.kel` is the only one of the twelve stage sources with no byte-identity test.
//! Nothing in the tree recorded a reason, so the absence read as an oversight. **It is not.**
//!
//! # The cause, bisected to four lines
//!
//! The self-hosted parser resolves a body's `block.field` against a layout table it accumulates
//! **as it encounters each `data` block**. A function that reads a block declared LATER in the
//! file therefore finds nothing, emits an unresolved operand, and `reconstruct.kel` refuses the
//! chunk because the record range does not reduce to a single root.
//!
//! `verify_types.kel` hits this at `ty_direct`, which reads `tyb.bres[b]` while `tyb` is declared
//! thirty lines further down.
//!
//! # Three hypotheses failed before the structural one succeeded
//!
//! `ty_direct` contains a doubly nested `if` EXPRESSION assigned to a data field, inside a
//! `for ... limit` loop, reading indexed arrays from two different blocks. Every one of those
//! looked like the culprit and **none of them is**: probes carrying each shape, and up to four
//! copies of the whole shape, all compile byte-identically. What separated the cases was
//! DECLARATION ORDER, which none of the shapes mention.
//!
//! **Tally consistent with the rest of this programme: guessing failed three times here and
//! prefix/structural bisection succeeded.** The lesson from the `wire.kel` chain applied
//! unchanged — reduce against the REAL file, not a simplified stand-in, and choose the predicate
//! deliberately.
//!
//! # What this file pins, and what it deliberately does not
//!
//! It pins the reproduction, the control, and the fact that the corpus is eleven of twelve. It
//! does **not** attempt the repair: resolving a forward reference needs the stage to collect data
//! declarations before parsing bodies, which is a two-pass restructuring of a single-pass
//! streaming parser, not a defect fix.
//!
//! **PROPORTIONALITY.** The reference compiler accepts forward references, and the self-hosted
//! compiler REFUSES loudly rather than mis-compiling. `self_hosted_compile` cross-checks against
//! the reference besides. No user receives a wrong module from this.

#![cfg(all(feature = "self-host", feature = "compile"))]

/// A function reading a `data` block declared after it. Four lines, no control flow, no nesting.
const FORWARD: &str = "private data t { hit: Word }\n\
                       fn g() -> Word { t.hit = u.z; t.hit }\n\
                       private data u { z: Word }\n\
                       fn main() -> Word { g() }";

/// The same program with the two declarations swapped. **The only difference is order.**
const BACKWARD: &str = "private data t { hit: Word }\n\
                        private data u { z: Word }\n\
                        fn g() -> Word { t.hit = u.z; t.hit }\n\
                        fn main() -> Word { g() }";

const VERIFY_TYPES: &str = include_str!("../src/selfhost/kel/verify_types.kel");

fn refusal(src: &str) -> Option<String> {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        keleusma::selfhost::self_host_compile(src)
    }));
    match caught {
        Ok(_) => None,
        Err(e) => Some(
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "non-string panic".into()),
        ),
    }
}

/// **THE REFERENCE ACCEPTS BOTH ORDERS.** Without this the pair below would be measuring the
/// language rather than the stage, which five probes on this line have done.
#[test]
fn the_reference_accepts_a_forward_data_reference() {
    for (label, src) in [("forward", FORWARD), ("backward", BACKWARD)] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex"))
            .unwrap_or_else(|e| panic!("{label}: the reference must parse it: {e:?}"));
        keleusma::compiler::compile(&ast)
            .unwrap_or_else(|e| panic!("{label}: the reference must compile it: {e:?}"));
    }
}

/// **DECLARATION ORDER IS THE WHOLE DIFFERENCE.**
///
/// The control is what makes this a finding rather than an anecdote: the identical program with
/// its two `data` declarations swapped compiles through the self-hosted pipeline without
/// complaint.
#[test]
fn a_data_block_referenced_before_its_declaration_is_refused() {
    assert!(
        refusal(BACKWARD).is_none(),
        "CONTROL FAILED: the backward-ordered program must self-compile, or the comparison \
         below is not about declaration order at all"
    );

    let msg = refusal(FORWARD).expect(
        "the forward-ordered program now self-compiles. THIS IS A GAP CLOSING, NOT A \
         REGRESSION: add `verify_types.kel` to the byte-identity corpus in \
         tests/selfhost_codegen.rs, take the corpus to twelve, and retire this file",
    );
    assert!(
        msg.contains("did not reduce to exactly one node"),
        "the forward reference is still refused but by a DIFFERENT cause, so the mechanism \
         recorded in this file's header may no longer be the one operating: {msg}"
    );
}

/// **`verify_types.kel` FAILS FOR THE SAME REASON, AT `ty_direct`.**
///
/// The link between the four-line witness and the real stage is asserted rather than assumed:
/// the refusal names the chunk, and `ty_direct` is the function that reads `tyb` before `tyb` is
/// declared.
#[test]
fn verify_types_kel_is_refused_at_the_chunk_that_reads_a_later_block() {
    let msg = refusal(VERIFY_TYPES).expect(
        "`verify_types.kel` now self-compiles. THIS IS A GAP CLOSING: check byte identity and \
         add it to the corpus as the twelfth stage",
    );
    assert!(
        msg.contains("ty_direct"),
        "the twelfth stage is still refused, but no longer at `ty_direct`. The cause this file \
         documents may have been replaced by another: {msg}"
    );

    // The structural claim the diagnosis rests on, checked against the source rather than
    // remembered: `ty_direct` reads `tyb`, and `tyb` is declared after it.
    // **THE ORDERING CLAIM BELOW IS THE REASON THIS STRIPS FIRST.** These positions feed an
    // `at_fn < at_blk` assertion, so a comment naming either anchor does not merely break an
    // extraction -- it moves a position and can change which declaration appears to come first.
    // That would make this test assert the wrong thing about the stage rather than fail loudly.
    let verify_types = code_only(VERIFY_TYPES);
    let at_fn = verify_types
        .find("fn ty_direct(")
        .expect("verify_types.kel declares ty_direct");
    let at_blk = verify_types
        .find("private data tyb {")
        .expect("verify_types.kel declares the tyb block");
    assert!(
        at_fn < at_blk,
        "`tyb` is no longer declared after `ty_direct`, so the mechanism recorded here cannot \
         be what refuses this file"
    );
    let body_end = verify_types[at_fn..]
        .find("\n}")
        .expect("ty_direct has a body");
    assert!(
        verify_types[at_fn..at_fn + body_end].contains("tyb."),
        "`ty_direct` no longer reads the `tyb` block"
    );
}

/// **THE BYTE-IDENTITY CORPUS IS ELEVEN OF THE TWELVE STAGES, AND THE MISSING ONE IS NAMED.**
///
/// Derived from the oracle's own test names rather than restated, so the two cannot part. The
/// absence was previously unexplained anywhere in the tree, which is how it read as an oversight.
#[test]
fn the_corpus_covers_every_stage_except_the_one_this_file_explains() {
    const ORACLE: &str = include_str!("selfhost_codegen.rs");
    let stages = [
        "analyze",
        "codegen",
        "lexer",
        "parse",
        "reconstruct",
        "verify_datalayout",
        "verify_depth",
        "verify_structural",
        "verify_typed",
        "verify_types",
        "verify_yield",
        "wire",
    ];
    let missing: Vec<&str> = stages
        .iter()
        .copied()
        .filter(|s| !ORACLE.contains(&format!("fn self_host_compiles_{s}_kel_byte_identically")))
        .collect();
    assert_eq!(
        missing,
        vec!["verify_types"],
        "the set of stages outside the byte-identity corpus has changed. If `verify_types` has \
         JOINED it, this whole file is retired; if another stage has LEFT it, that is a \
         regression and needs its own explanation"
    );
}

/// Source with `//` line comments removed, so an anchor search matches CODE.
///
/// **Locating on RAW source is the defect this removes.** A comment naming the anchor sends the
/// extraction to the comment; measured elsewhere in this repository, one such line failed four
/// tests in a file with nothing wrong in the code it read.
///
/// Truncating at the first `//` rather than tracking string literals is correct for an anchor
/// search: an early truncation can only make the anchor go missing, which fails loudly through the
/// `expect` below. `tests/call_chunk_index_limit.rs` needs a string-aware strip instead, because its
/// assertion is an ABSENCE one where the same truncation lets a real offender pass silently.
/// **THE ORDERING HAZARD ABOVE, MADE CHECKABLE INSTEAD OF DESCRIBED.**
///
/// The comment beside the ordering assertion states that a comment naming either anchor can move a
/// position and make the test assert the WRONG THING about the stage rather than fail loudly. That
/// was a claim about the strip, and nothing checked it. A guard census on 2026-09-10 found this
/// file among those documenting a hazard while recording no demonstration that its own assertion
/// still discriminates.
///
/// Both directions are pinned. The first input is the one that matters: a historical note naming
/// the LATER declaration placed BEFORE the earlier one, which on raw source inverts the order the
/// assertion reads. The second confirms the strip has not been tightened into finding nothing,
/// which is the way an extraction can be made comment-proof by being made useless.
#[test]
fn a_comment_naming_an_anchor_cannot_invert_the_declaration_order() {
    const DECOY: &str = "// historical: `private data tyb {` used to be declared first\n\
                         fn ty_direct( a ) {\n  tyb.x\n}\n\
                         private data tyb {\n  x: Word\n}\n";
    let stripped = code_only(DECOY);
    let at_fn = stripped
        .find("fn ty_direct(")
        .expect("the strip removed the function declaration itself");
    let at_blk = stripped
        .find("private data tyb {")
        .expect("the strip removed the data block itself");
    assert!(
        at_fn < at_blk,
        "a COMMENT naming the later declaration was located as if it were the declaration, \
         inverting the order; the ordering assertion in this file would then report a structure \
         the stage does not have, and would report it as a pass"
    );

    const PLAIN: &str = "fn ty_direct( a ) {\n  tyb.x\n}\nprivate data tyb {\n  x: Word\n}\n";
    let plain = code_only(PLAIN);
    assert!(
        plain.contains("fn ty_direct(") && plain.contains("private data tyb {"),
        "the strip removed real code, which would make every locate in this file fail to find \
         its anchor and the file comment-proof by finding nothing at all"
    );
}

fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
