//! The bare `for` form, from an unnamed five-layer failure to a byte-identical
//! self-compile.
//!
//! # What this file was
//!
//! `self_host_compile(wire.kel)` panicked with ``no chunk named `acc` ``. That
//! was traced through five layers: the wrong name is in `parse.kel`'s own record
//! stream; the cursor never rewinds; the tokens are correct; the function body
//! closes at the `for` loop's brace rather than the function's; and the
//! declaration path then reads the trailing field access as a declaration name.
//!
//! **All of that was mechanism. The cause was that `parse.kel`'s loop header
//! expected `for v in lo..hi limit CAP { … }`** — phase 4 waited for the
//! contextual `limit`, a bare `for v in lo..hi { … }` never supplied one, and
//! the braces were attributed to the wrong block.
//!
//! # What it is now
//!
//! **The bare form self-compiles byte-identically.** The gap closed in three
//! places, and the third was not in the estimate:
//!
//! 1. `parse.kel` accepts the header, reserves TWO frame slots against the
//!    counted form's five, and emits a short parts ladder.
//! 2. `reconstruct.kel` assembles the seven-word `for_parts` entry, SYNTHESISING
//!    `i >= limit` and `i + 1` — neither corresponds to any token, because both
//!    are properties of the lowering rather than of the source.
//! 3. **Neither the shipping driver nor this repository's copy of it ever read
//!    `for_parts` back from `reconstruct.kel`.** `codegen.kel` had the lowering
//!    all along and received seven zeros. The recorded cost said that stage was
//!    done; it was, and the wire to it was not.
//!
//! # Why it went unmeasured for so long, which is the durable part
//!
//! `codegen.kel` has had `push_forin` throughout, exercised by four cases in
//! `codegen_owns_its_constant_pool_and_matches_reference`. **That corpus drives
//! the REFERENCE parser**, so it fed `codegen.kel` nodes `parse.kel` has never
//! produced, and it passed while the pipeline was broken.
//!
//! *Any construct the corpus does not contain is unverified by construction* —
//! and here the construct WAS in a corpus, just not the one exercising the stage
//! that failed. The construct-support boundary, which drives the whole pipeline,
//! carried no bare-`for` case until one was added days later.
//!
//! # What the tests below now hold
//!
//! Each of them was a GAP PIN and each fired when the gap closed, which is what
//! a gap pin is for. They are converted rather than deleted, so a reader learns
//! what became of what they watched.

#![cfg(all(feature = "self-host", feature = "compile"))]

const BARE: &str = "private data acc { s: Word }\n\
                    fn f(n: Word) -> Word { acc.s = 0; for i in 0..n { acc.s = acc.s + i; } acc.s }\n\
                    fn main() -> Word { f(3) }";

const WITH_LIMIT: &str = "private data acc { s: Word }\n\
                          fn f(n: Word) -> Word { acc.s = 0; for i in 0..n limit 8 { acc.s = acc.s + i; } acc.s }\n\
                          fn main() -> Word { f(3) }";

/// **BOTH FORMS SELF-COMPILE BYTE-IDENTICALLY.**
///
/// This asserted that the counted form worked and the bare form did not. The
/// counted form is the CONTROL and still carries the weight: without it, "the
/// bare form compiles" would be a claim about the harness.
#[test]
fn both_for_forms_self_compile_byte_identically() {
    for (label, src) in [("counted", WITH_LIMIT), ("bare", BARE)] {
        let reference = keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
        )
        .expect("the reference accepts both forms");
        let mine = keleusma::selfhost::self_host_compile(src);
        assert_eq!(
            keleusma::wire_format::module_to_wire_bytes(&mine).expect("mine"),
            keleusma::wire_format::module_to_wire_bytes(&reference).expect("reference"),
            "the {label} `for` form no longer self-compiles byte-identically"
        );
    }
}

/// **THE BARE FORM YIELDS A MODULE**, which is what it never did.
///
/// It used to panic five layers downstream with a missing chunk name, then --
/// once `parse.kel` learned to say so -- refuse by name. Now it produces a
/// module, and the assertion is on its SHAPE rather than merely on its
/// existence: a loop with a `BreakIf` exit and two frame slots.
#[test]
fn the_bare_form_yields_a_module_with_the_plain_loop_shape() {
    use keleusma::bytecode::Op;
    let m = keleusma::selfhost::self_host_compile(BARE);
    let f = m
        .chunks
        .iter()
        .find(|c| c.name == "f")
        .expect("the bare form's function is present");
    assert!(
        f.ops.iter().any(|o| matches!(o, Op::Loop(_)))
            && f.ops.iter().any(|o| matches!(o, Op::BreakIf(_))),
        "the bare form no longer lowers to a plain Loop with a BreakIf exit: {:?}",
        f.ops
    );
    assert!(
        !f.ops.iter().any(|o| matches!(o, Op::Trap(_))),
        "the bare form emitted a Trap. That is the COUNTED form's outcome check, \
         which the bare lowering has no counter for: {:?}",
        f.ops
    );
}

/// **THE BOUNDARY MARKS THE BARE FORM SUPPORTED, AND ITS SUBJECT HAS MOVED
/// TWICE.**
///
/// It first asserted the boundary carried NO bare-`for` case, which was the
/// defect: a construct absent from that table is unverified by construction, and
/// a reader consulting it to learn whether loops work saw one `for` case marked
/// supported. A case was added marked `Refuses`, and the pin moved from ABSENCE
/// to VERDICT. The gap is now closed, so it moves again — to SUPPORTED.
///
/// **The name moved with the subject each time**, because a test whose name
/// asserts one thing and whose body checks another is how a test comes to
/// measure something other than what it says.
#[test]
fn the_boundary_marks_the_bare_for_case_supported() {
    const BOUNDARY: &str = include_str!("selfhost_codegen.rs");
    let table: String = BOUNDARY
        .lines()
        .skip_while(|l| !l.contains("fn boundary_cases()"))
        .take_while(|l| !l.starts_with('}'))
        .collect::<Vec<_>>()
        .join("\n");
    let case_start = table
        .find("\"ctrl/for_bare\"")
        .expect("the boundary carries a bare `for` case labelled `ctrl/for_bare`");
    let verdict = &table[case_start..case_start + 200];
    assert!(
        verdict.contains("SOk"),
        "the boundary's bare `for` case is no longer marked supported. If the \
         pipeline regressed, fix it; if the case was removed, the construct is \
         unverified by construction again -- which is the defect this pin has \
         been following since it recorded the case's ABSENCE."
    );
}

/// **THE TWO `for` FORMS ARE TWO CONSTRUCTS, NOT ONE WITH AN OPTIONAL CLAUSE.**
///
/// Measured before costing the work, and it changed the estimate. The bare form
/// compiles to a plain `Loop`/`EndLoop`; the `limit` form compiles to the
/// ForLimit machinery — counter slots, a cap, and an overflow check. For the same
/// body the reference emits **24 ops against 68**.
///
/// # Why that matters for whoever closes the gap
///
/// "Let phase 5 skip the missing cap" is the obvious reading of the parse-stage
/// failure and it is **wrong**. Supporting the bare form is not a relaxation of
/// the existing loop header; it is a second lowering that `parse.kel` does not
/// emit at all.
///
/// `codegen.kel` already handles the resulting nodes — four cases in the
/// codegen-only corpus drive them from reference-parsed input — so the missing
/// piece is confined to the front end. That is a smaller claim than "codegen
/// supports it, so only wiring remains", and it is the accurate one.
///
/// # Pinned because the ratio is the cost signal
///
/// If the two lowerings ever converge, the work shrinks to what the earlier
/// reading assumed and this test should fail so its author re-costs it.
#[test]
fn the_bare_and_limit_forms_have_different_lowerings() {
    let ops_of = |src: &str| -> Vec<String> {
        let m = keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
        )
        .expect("compile");
        m.chunks
            .iter()
            .find(|c| c.name == "main")
            .expect("main")
            .ops
            .iter()
            .map(|o| format!("{o:?}"))
            .collect()
    };

    const BODY: &str = "{ d.s = d.s + i; } d.s }";
    let bare = ops_of(&format!(
        "private data d {{ s: Word }}\nfn main() -> Word {{ for i in 0..4 {BODY}"
    ));
    let limit = ops_of(&format!(
        "private data d {{ s: Word }}\nfn main() -> Word {{ for i in 0..4 limit 8 {BODY}"
    ));

    assert!(
        !bare.is_empty() && !limit.is_empty(),
        "one of the forms produced no ops, so the comparison measures nothing"
    );
    assert!(
        limit.len() > bare.len() * 2,
        "the `limit` form is {} ops against the bare form's {}. They were 68 and 24 -- a \
         second lowering, not an optional clause. If they have converged, the parse-stage \
         gap is smaller than recorded and should be re-costed",
        limit.len(),
        bare.len()
    );

    // The bare form's shape, named: a plain loop, with none of the ForLimit
    // counter machinery. Asserted rather than inferred from the op count, because
    // a count can coincide.
    assert!(
        bare.iter().any(|o| o.starts_with("Loop")),
        "the bare form no longer lowers to a plain `Loop`: {bare:?}"
    );
}

/// **THE REFUSAL IS RETIRED, AND ITS DIAGNOSTIC CODE IS GONE FROM THE STAGE.**
///
/// `parse.kel` briefly refused the bare form by name at phase 4, which was a
/// large improvement on the missing-chunk-name panic it replaced and was always
/// meant to be temporary: the refusal sat exactly where the lowering now hooks
/// in.
///
/// This asserts the refusal is not merely unreachable but ABSENT. A dead
/// diagnostic that no input can produce is the same shape as a citation naming
/// a test that does not exist -- it reads as a capability and is not one.
#[test]
fn the_bare_form_refusal_is_gone_from_the_stage() {
    const PARSE: &str = include_str!("../src/selfhost/kel/parse.kel");
    assert!(
        !PARSE.contains("pe_bare_for"),
        "`parse.kel` still defines or raises the bare-`for` refusal. The form is \
         supported now, so a reachable refusal would be a contradiction and an \
         unreachable one would be a diagnostic no input can produce."
    );
    // AND THE CONSTRUCT ACTUALLY GOES THROUGH, so this is not satisfied by
    // deleting the code that used to say why it did not.
    let parsed = keleusma::selfhost::try_parse_functions(BARE)
        .expect("the bare form must parse now that the refusal is gone");
    assert_eq!(parsed.functions.len(), 2);
}

/// THE CONTROL, and it carries the weight for the test above.
///
/// "The bare form is refused" is satisfied by a front end that refuses
/// everything. This asserts the counted form still parses, so the refusal is a
/// property of the construct rather than of the compiler.
#[test]
fn the_counted_form_is_still_accepted_by_the_same_entry_point() {
    let parsed = keleusma::selfhost::try_parse_functions(WITH_LIMIT)
        .expect("the counted form must still be accepted");
    assert_eq!(
        parsed.functions.len(),
        2,
        "the counted form parsed, but not into the two functions it declares"
    );
}

/// **THE LOWERING IS REACHED BY EVERY STAGE NOW, AND THE ESTIMATE THAT SAID SO
/// WAS WRONG BY ONE SITE.**
///
/// This recorded a measured division of labour: `codegen.kel` DONE, the driver
/// DONE, `reconstruct.kel` declared-but-never-written, `parse.kel` absent. It
/// was written to fail when the work started, and it did.
///
/// **The row it got wrong was the driver's.** It read `for_parts` INTO
/// `codegen.kel` and never OUT of `reconstruct.kel`, in the shipping driver and
/// in this repository's copy of it — so `push_forin` received seven zeros and
/// produced a structurally correct loop whose every operand was slot 0. The
/// measurement checked that the plumbing existed and not that it ran in both
/// directions.
///
/// What it now pins is the completed wiring, in the direction that was missing.
#[test]
fn every_stage_reaches_the_bare_lowering() {
    const CODEGEN: &str = include_str!("../src/selfhost/kel/codegen.kel");
    const RECONSTRUCT: &str = include_str!("../src/selfhost/kel/reconstruct.kel");
    const PARSE: &str = include_str!("../src/selfhost/kel/parse.kel");
    const DRIVER: &str = include_str!("../src/selfhost/mod.rs");

    assert!(
        CODEGEN.contains("fn push_forin("),
        "`codegen.kel` no longer defines the bare lowering"
    );
    assert!(
        RECONSTRUCT.matches("for_parts[").count() > 0,
        "`reconstruct.kel` no longer writes `for_parts`, so the seven-word entry \
         `push_forin` reads is never assembled"
    );
    assert!(
        PARSE.contains("step_forin_emit"),
        "`parse.kel` no longer emits the bare form's parts"
    );
    // **THE ROW THE ESTIMATE GOT WRONG.** Reading it out of `reconstruct.kel` is
    // a different thing from writing it into `codegen.kel`, and only the second
    // existed.
    assert!(
        DRIVER.contains("RC_AST_FOR_PARTS"),
        "the driver no longer reads `for_parts` back from `reconstruct.kel`. \
         That omission produced a correct loop whose every operand was zero, and \
         it is the failure this assertion exists for."
    );
}
