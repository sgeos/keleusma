//! The bare `for` form, which the self-hosted pipeline does not support — and
//! which fails without saying so.
//!
//! # What seven iterations of diagnosis actually found
//!
//! `self_host_compile(wire.kel)` panics with ``no chunk named `acc` ``. That was
//! traced through five layers: the wrong name is in `parse.kel`'s own record
//! stream; the cursor never rewinds; the tokens are correct; the function body
//! closes at the `for` loop's brace rather than the function's; and the
//! declaration path then reads the trailing field access as a declaration name.
//!
//! **All of that is the mechanism. This file is the cause.** `parse.kel`'s loop
//! header expects `for v in lo..hi limit CAP { … }` — phase 5 waits for the cap's
//! integer literal. A bare `for v in lo..hi { … }` never supplies one, so the
//! header machine does not reach its body phase and the braces are attributed to
//! the wrong block.
//!
//! # Why this went unmeasured
//!
//! **The 95-case construct-support boundary contains exactly one `for` case, and
//! it is the `limit` form.** There is no bare-`for` case, so its support was never
//! measured. That is the same shape as the boolean-literal and `Byte`-cast
//! miscompiles: *any construct the corpus does not contain is unverified by
//! construction*.
//!
//! # The failure is loud, which is the one piece of good news
//!
//! It panics rather than emitting a wrong module, so no caller receives a silent
//! miscompile. **A panic is still a poor refusal**: this project's parser names
//! thirteen failure modes with their own causes precisely so a user learns what
//! happened. ``no chunk named `acc` `` names neither the construct nor the file.

#![cfg(all(feature = "self-host", feature = "compile"))]

const BARE: &str = "private data acc { s: Word }\n\
                    fn f(n: Word) -> Word { acc.s = 0; for i in 0..n { acc.s = acc.s + i; } acc.s }\n\
                    fn main() -> Word { f(3) }";

const WITH_LIMIT: &str = "private data acc { s: Word }\n\
                          fn f(n: Word) -> Word { acc.s = 0; for i in 0..n limit 8 { acc.s = acc.s + i; } acc.s }\n\
                          fn main() -> Word { f(3) }";

/// **THE `limit` FORM IS SUPPORTED AND BYTE-IDENTICAL. THE BARE FORM IS NOT
/// SUPPORTED AT ALL.**
///
/// The control comes first and carries the weight: without it, "the bare form
/// fails" would be a claim about the compiler rather than about the construct.
#[test]
fn the_limit_form_self_compiles_and_the_bare_form_does_not() {
    // THE CONTROL. Byte-identical, which is the boundary's own verdict for
    // `ctrl/for_limit` and the reason that case is marked supported.
    let reference = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(WITH_LIMIT).expect("lex"))
            .expect("parse"),
    )
    .expect("the reference accepts the limit form");
    let mine = keleusma::selfhost::self_host_compile(WITH_LIMIT);
    assert_eq!(
        keleusma::wire_format::module_to_wire_bytes(&mine).expect("mine"),
        keleusma::wire_format::module_to_wire_bytes(&reference).expect("reference"),
        "the `limit` form is no longer byte-identical, so the bare form's failure below \
         says nothing about the bare form specifically"
    );

    // THE REFERENCE ACCEPTS THE BARE FORM, so this is a pipeline gap and not an
    // ill-formed program.
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(BARE).expect("lex")).expect("parse"),
    )
    .expect("the reference accepts the bare form; if it stopped, this file is obsolete");

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let attempt = std::panic::catch_unwind(|| keleusma::selfhost::self_host_compile(BARE));
    std::panic::set_hook(prev);

    assert!(
        attempt.is_err(),
        "the bare `for` form now self-compiles. That closes a real gap: add a bare-`for` \
         case to the construct-support boundary, re-check `self_host_compile(wire.kel)`, \
         and delete this pin rather than relaxing it"
    );
}

/// **IT FAILS LOUDLY, NOT SILENTLY — AND THAT IS THE ONLY REASON THIS IS NOT A
/// MISCOMPILE.**
///
/// Pinned separately because the two can come apart, and the direction matters
/// more than the failure. A future change that made the bare form *emit* a module
/// instead of panicking would satisfy the test above — `is_err` would go false
/// and it would read as the gap being closed — while actually producing a wrong
/// artifact.
///
/// So this asserts the shape of the failure, not merely that there is one.
#[test]
fn the_bare_form_never_yields_a_module() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let attempt = std::panic::catch_unwind(|| {
        let m = keleusma::selfhost::self_host_compile(BARE);
        // Reached only if the pipeline returns. Render it, so a module that is
        // wrong rather than absent is still caught here.
        keleusma::wire_format::module_to_wire_bytes(&m).map(|b| b.len())
    });
    std::panic::set_hook(prev);

    assert!(
        attempt.is_err(),
        "the bare `for` form produced a module. If it is CORRECT, this gap is closed and \
         the boundary needs the case; if it is WRONG, this is a silent miscompile and is \
         the most serious class of defect this tree tracks"
    );
}

/// **THE FULL-PIPELINE BOUNDARY HAS NO BARE-`for` CASE, AND THE CODEGEN-ONLY
/// CORPUS HAS FOUR.**
///
/// That difference is the whole explanation, and the first version of this test
/// got it wrong by scanning the file instead of the table.
///
/// | corpus | drives | bare `for` cases |
/// |---|---|---|
/// | `boundary_cases()` | the **whole** self-hosted pipeline, `parse.kel` included | **none** |
/// | `codegen_owns_its_constant_pool_and_matches_reference` | the REFERENCE parser, then `codegen.kel` | four |
///
/// So **`codegen.kel` handles the bare `for` perfectly well** — those four cases
/// pass — and `parse.kel` does not. Only a corpus that runs the whole pipeline
/// could have caught that, and the one that does has no case for it.
///
/// *Any construct the corpus does not contain is unverified by construction* —
/// and here the construct IS in a corpus, just not in the one that exercises the
/// stage that fails.
///
/// # The reader is scoped to the table, and it was not at first
///
/// The first version searched the whole file for `for … in 0..`, which matched
/// the four codegen-only cases and a Rust `for _ in 0..65536` in a helper. It
/// failed, and the failure is what surfaced the distinction above — so the
/// scoping is recorded rather than quietly corrected.
#[test]
fn the_full_pipeline_boundary_carries_no_bare_for_case() {
    const BOUNDARY: &str = include_str!("selfhost_codegen.rs");

    // THE TABLE ONLY. `boundary_cases()` is what drives the whole pipeline; the
    // rest of the file drives individual stages against reference-parsed input.
    let table: String = BOUNDARY
        .lines()
        .skip_while(|l| !l.contains("fn boundary_cases()"))
        .take_while(|l| !l.starts_with('}'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        table.contains("ctrl/for_limit"),
        "the boundary table was not located, so the absence below means nothing"
    );

    let for_cases: Vec<&str> = table
        .lines()
        .filter(|l| l.contains("for ") && l.contains("in 0.."))
        .collect();
    assert!(
        !for_cases.is_empty(),
        "no `for` case at all in the boundary table; the reader is broken"
    );
    for case in &for_cases {
        assert!(
            case.contains("limit"),
            "the boundary table now carries a `for` case without `limit`: {case}\n\
             If the bare form is supported by `parse.kel`, delete this file. If it is \
             not, that case's verdict should say so rather than the table implying \
             coverage it does not have."
        );
    }
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
