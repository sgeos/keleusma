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
//! **The construct-support boundary contained exactly one `for` case, and it was
//! the `limit` form.** There was no bare-`for` case, so its support was never
//! measured. That is the same shape as the boolean-literal and `Byte`-cast
//! miscompiles: *any construct the corpus does not contain is unverified by
//! construction*.
//!
//! **CLOSED 2026-08-25.** `ctrl/for_bare` is in the table, marked `Refuses`,
//! taking it to 96 cases at 90 SOk / 2 Refuses / 3 Diverges / 1 RefRejects. The
//! gap is now MEASURED rather than only diagnosed here.
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
/// | `boundary_cases()` | the **whole** self-hosted pipeline, `parse.kel` included | **none until 2026-08-25; now one, marked `Refuses`** |
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
fn the_boundary_marks_the_bare_for_case_refused_rather_than_supported() {
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
    // **THIS PIN'S OWN INSTRUCTION, FOLLOWED.** It used to require that every
    // `for` case carry `limit`, and its failure message said: if the bare form
    // is not supported, that case's verdict should say so rather than the table
    // implying coverage it does not have. A `ctrl/for_bare` case marked
    // `Refuses` was added on 2026-08-25 and does exactly that, so the pin's
    // subject moves from ABSENCE to VERDICT.
    //
    // The table now implies no coverage it does not have, which is the property
    // the original was protecting. What must not happen is the bare case
    // appearing as supported while `parse.kel` refuses it.
    let bare: Vec<&str> = for_cases
        .iter()
        .filter(|l| !l.contains("limit"))
        .copied()
        .collect();
    assert_eq!(
        bare.len(),
        1,
        "expected exactly one bare `for` case in the boundary table, found \
         {}: {bare:?}. If the bare form became supported, this file's subject \
         is gone and it should be retired rather than adjusted.",
        bare.len()
    );
    let case_start = table
        .find("\"ctrl/for_bare\"")
        .expect("the bare case is labelled `ctrl/for_bare`");
    let verdict = &table[case_start..case_start + 200];
    assert!(
        verdict.contains("Refuses"),
        "the boundary's bare `for` case is no longer marked as refused. If \
         `parse.kel` now lowers the bare form, retire this file; if it does \
         not, the table is claiming coverage it does not have -- which is the \
         defect this test was written for."
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

/// **THE REFUSAL NAMES THE CONSTRUCT AND THE REMEDY.**
///
/// Before this, `parse.kel` never left phase 4 of the loop header, the opening
/// brace was attributed to the enclosing block, and the failure surfaced five
/// layers downstream as ``no chunk named `acc` `` — naming neither the
/// construct nor the file. Seven iterations of diagnosis were spent on that
/// message once.
///
/// The refusal is now raised where the fact is known: phase 4 ends at the
/// contextual `limit` identifier, so a `{` there is unambiguously the bare
/// form.
///
/// To see this fail, delete the `Tok::LBrace()` arm from phase 4 of
/// `step_forheader` in `src/selfhost/kel/parse.kel`. The stage then returns to
/// the downstream panic and this test reports a message naming neither thing.
#[test]
fn the_bare_form_is_refused_by_a_message_naming_the_construct_and_the_remedy() {
    let err = keleusma::selfhost::try_parse_functions(BARE)
        .expect_err("the bare form must be refused, not accepted");
    let message = err.to_string();

    // ITEM 1: the construct, not an unrelated symbol.
    assert!(
        message.contains("bare `for") && message.contains("not implemented"),
        "the refusal does not name the construct as unimplemented: {message}"
    );
    assert!(
        !message.contains("no chunk named"),
        "the refusal is still the downstream symptom rather than the cause: \
         {message}"
    );

    // ITEM 2: the remedy, because that is what a reader needs next.
    assert!(
        message.contains("limit"),
        "the refusal does not name the counted form as the supported \
         alternative: {message}"
    );

    // A capacity diagnostic tells a reader to split the function, and that
    // advice is wrong here. The message says so.
    assert!(
        message.contains("UNSUPPORTED CONSTRUCT"),
        "the refusal does not distinguish itself from a capacity limit: \
         {message}"
    );
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

/// **WHERE THE BARE FORM'S SUPPORT ACTUALLY STANDS, MEASURED ACROSS THE STAGES.**
///
/// The recorded cost was "a second lowering across three stage sources". That
/// was written from a correct observation — the two `for` forms are different
/// lowerings, not one with an optional clause — and it inferred the WORK from
/// the DIFFERENCE. Two lowerings means two must be written, unless one already
/// is.
///
/// **`codegen.kel` already has it.** `push_forin` emits the whole bare lowering
/// from a seven-word `for_parts` entry, and four bare-`for` cases exercise it in
/// `codegen_owns_its_constant_pool_and_matches_reference`. It got written
/// because that corpus drives the REFERENCE parser, so it has always received
/// nodes `parse.kel` has never produced. **The same corpus split that hid the
/// gap is why the lowering exists and was never connected.**
///
/// So the remaining work is two stages, not three, and the part that had to
/// reproduce the reference byte for byte is done. **A better estimate is not a
/// small estimate**: emitting the records in `parse.kel` and populating the
/// parts in `reconstruct.kel` is real work in a phase machine.
///
/// # This is a GAP PIN
///
/// It fails when the work starts. That is not a regression — it means the state
/// it records has changed, and its successor should say what became of it, as
/// three sibling pins did when the refusal landed and one did when the boundary
/// case did.
#[test]
fn the_bare_lowering_exists_in_codegen_and_is_unreached_by_the_earlier_stages() {
    const CODEGEN: &str = include_str!("../src/selfhost/kel/codegen.kel");
    const RECONSTRUCT: &str = include_str!("../src/selfhost/kel/reconstruct.kel");
    const PARSE: &str = include_str!("../src/selfhost/kel/parse.kel");

    // DONE: the lowering itself.
    assert!(
        CODEGEN.contains("fn push_forin("),
        "`codegen.kel` no longer defines `push_forin`. If the bare lowering was \
         renamed the cost recorded above needs re-deriving; if it was removed, \
         the estimate is three stages again."
    );

    // NOT DONE: `reconstruct.kel` DECLARES the parts and never writes them.
    // Declared-versus-written is the whole distinction — a test satisfied by the
    // declaration would report the stage as done.
    assert!(
        RECONSTRUCT.contains("for_parts: [Word;"),
        "`reconstruct.kel` no longer declares `for_parts`, so the measurement \
         below is about a different structure than the one recorded"
    );
    let writes = RECONSTRUCT.matches("for_parts[").count();
    assert_eq!(
        writes, 0,
        "`reconstruct.kel` now indexes `for_parts` {writes} time(s). **THE WORK \
         HAS STARTED**, which is what this pin watches for. Retire it and record \
         what the stage now does, the way the refusal and boundary pins were \
         moved rather than deleted."
    );

    // NOT DONE: `parse.kel` does not know the node at all.
    assert!(
        !PARSE.contains("for_parts") && !PARSE.contains("forin"),
        "`parse.kel` now mentions the bare form's node or its parts. **THE WORK \
         HAS STARTED** — see the note above."
    );

    // THE CONTRAST THAT MAKES THE ZERO MEAN SOMETHING. The counted form's
    // equivalent is populated, so "declared but unwritten" is a real state of
    // this stage rather than an artefact of how the file is written.
    assert!(
        RECONSTRUCT.matches("limit_parts").count() > 10,
        "`limit_parts` is barely referenced in `reconstruct.kel`, so the zero \
         above no longer contrasts with anything and says little"
    );
}
