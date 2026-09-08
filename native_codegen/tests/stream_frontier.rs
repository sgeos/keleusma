//! Which suspending shapes the backend lowers, and which it refuses.
//!
//! # Why this exists
//!
//! A minimal `loop main(t: Word) -> Word { yield t }` lowers with no refusal,
//! while `13_telemetry_stream.kel` is refused for `Stream`. Both are streams.
//! **So "the backend does not support `Stream`" is false as stated, and "the
//! backend supports `Stream`" is equally false.** Nothing in the tree said where
//! the boundary lies, and this line has described it wrongly twice — calling
//! `Stream` unsupported outright, and predicting `Reset` unreachable — with both
//! corrected by measuring a single program.
//!
//! # What the columns mean, because the distinction has bitten before
//!
//! **REFERENCE REJECTED** is the compiler declining the program. It never
//! reaches the backend and says nothing about it. **REFUSED** is the backend
//! declining bytecode it was given. Reporting the first as the second would
//! attribute a language rule to the lowering.
//!
//! **LOWERS is not "works".** Lowering without refusal says an arm ran, not that
//! the result is right. Execution evidence for suspension lives in
//! `yield_sequence.rs`, which compares whole yielded sequences.

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::{LowerOptions, module_refusals};

fn try_compile(src: &str) -> Result<Module, String> {
    let toks = tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let ast = parse(&toks).map_err(|e| format!("parse: {e:?}"))?;
    compile(&ast).map_err(|e| format!("{e:?}"))
}

/// `Ok(None)` lowers; `Ok(Some(text))` is refused by the backend; `Err` is the
/// reference declining the program.
fn status(src: &str) -> Result<Option<String>, String> {
    let m = try_compile(src)?;
    let refusals = module_refusals(&m, LowerOptions::default());
    if refusals.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            refusals
                .iter()
                .map(|(s, e)| format!("{s}: {e}"))
                .collect::<Vec<_>>()
                .join(" | "),
        ))
    }
}

const SHAPES: &[(&str, &str)] = &[
    (
        "yield in tail position",
        "loop main(t: Word) -> Word { yield t }",
    ),
    (
        "yield then more code",
        "loop main(t: Word) -> Word { yield t; t * 2 }",
    ),
    (
        "two yields in sequence",
        "loop main(t: Word) -> Word { let a = yield t; yield a + 1 }",
    ),
    (
        "yield inside an if",
        "loop main(t: Word) -> Word { if t > 0 { let _ = yield t; } yield 0 }",
    ),
    (
        "yield inside a for",
        "loop main(t: Word) -> Word { let xs = [1, 2]; for x in xs { let _ = yield x; } yield 0 }",
    ),
    (
        "yield a composite, tail",
        "struct P { a: Word, b: Word }\nloop main(t: Word) -> P { yield P { a: t, b: t } }",
    ),
    (
        "yield a composite inside a for",
        "struct P { a: Word, b: Word }\n\
         loop main(t: Word) -> P {\n\
           let xs = [1, 2];\n\
           for x in xs { let _ = yield P { a: x, b: x }; }\n\
           yield P { a: 0, b: 0 }\n\
         }",
    ),
    (
        "yield calling a function",
        "fn f(x: Word) -> Word { x * 3 }\nloop main(t: Word) -> Word { yield f(t) }",
    ),
    // **ADDED 2026-09-08**, when general `Op::Stream` lowering left the original
    // eight shapes with a single refusal between them. A matrix whose refusals
    // all come from one cause locates one edge of the frontier and implies the
    // rest is open, which is not what the backend does.
    (
        "yield a composite, non-tail",
        "struct P { a: Word, b: Word }\n\
         loop main(t: Word) -> P { let a = yield P { a: t, b: t }; yield P { a: a, b: a } }",
    ),
    (
        "yield inside an if with a tail",
        "loop main(t: Word) -> Word { if t > 0 { let a = yield t; yield a } else { yield 0 } }",
    ),
    (
        "yield calling a suspending callee",
        "yield helper(x: Word) -> Word { let r = yield x; r }\n\
         loop main(t: Word) -> Word { yield helper(t) }",
    ),
];

#[test]
fn where_the_stream_frontier_lies() {
    let mut lowers: Vec<&str> = Vec::new();
    let mut refused: Vec<(&str, String)> = Vec::new();
    let mut rejected: Vec<(&str, String)> = Vec::new();

    println!("\n================ STREAM FRONTIER");
    for (label, src) in SHAPES {
        match status(src) {
            Ok(None) => {
                println!("  {label:<32} LOWERS");
                lowers.push(label);
            }
            Ok(Some(why)) => {
                let short: String = why.chars().take(110).collect();
                println!("  {label:<32} REFUSED: {short}");
                refused.push((label, why));
            }
            Err(why) => {
                let short: String = why.chars().take(110).collect();
                println!("  {label:<32} REFERENCE REJECTED: {short}");
                rejected.push((label, why));
            }
        }
    }
    println!("  ------------------------------------------------");
    println!(
        "  lowers {}, backend-refused {}, reference-rejected {}",
        lowers.len(),
        refused.len(),
        rejected.len()
    );
    println!(
        "\n  LOWERS IS NOT \"WORKS\". Execution evidence for suspension is in\n  \
         `yield_sequence.rs`, which compares whole yielded sequences.\n================\n"
    );

    // **NON-VACUITY IN BOTH DIRECTIONS.** A matrix where everything lowers, or
    // where everything is refused, locates no boundary at all and would satisfy
    // any claim about one.
    assert!(
        !lowers.is_empty(),
        "no shape lowers, so this matrix locates no frontier: refused={refused:?} \
         rejected={rejected:?}"
    );
    assert!(
        !refused.is_empty(),
        "no shape is refused by the BACKEND, so this matrix locates no frontier. \
         If the backend now lowers every shape tried, that is a much larger result \
         than this test is written for."
    );
}

/// The yield-escape refusal is shadowed by whatever refuses composite-yielding
/// streams first. This asks whether that is still true.
///
/// **A tripwire on the tripwire.** `yield_escape_gate.rs` already fails when the
/// shadowing refusal changes; this states the consequence in the terms that
/// matter — whether a program capable of the silent wrong value can reach the
/// backend's placement at all.
#[test]
fn whether_a_composite_yielding_stream_can_reach_the_placement() {
    let src = "struct P { a: Word, b: Word }\n\
               loop main(t: Word) -> P {\n\
                 let xs = [1, 2];\n\
                 for x in xs { let _ = yield P { a: x, b: x }; }\n\
                 yield P { a: 0, b: 0 }\n\
               }";
    println!("\n================ CAN THE ESCAPING SHAPE REACH THE PLACEMENT?");
    match status(src) {
        Ok(None) => panic!(
            "a composite-yielding stream now LOWERS. The yield-escape refusal is no \
             longer shadowed, so it is the only thing between this corpus and a \
             silently wrong value. Confirm it fires."
        ),
        Ok(Some(why)) => {
            println!("  still refused, by: {why}");
            // **CORRECTED 2026-09-08.** This line used to conclude that the
            // escape refusal "remains SHADOWED ... not yet load-bearing". That
            // was true only while `Op::Stream` was refused. It lowers now, so
            // the refusal printed above IS the escape check, and it is the only
            // thing between this shape and a silently wrong value.
            println!(
                "  => the refusal above is now the ONE that stands between this shape\n  \
                 and the placement. It is load-bearing, not a precaution.\n================\n"
            );
        }
        Err(why) => println!("  the reference rejects this program: {why}\n================\n"),
    }
}

/// The rule the matrix implies, pinned so a change announces itself.
///
/// # **RE-DERIVED 2026-09-08. TAIL POSITION IS NO LONGER THE DISCRIMINATOR.**
///
/// This test asserted that tail position separated what lowers from what does
/// not, and it named the pair that established it: a composite yielded in tail
/// position lowered, a `Word` yielded with code after it did not. **The second
/// half stopped being true** when general `Op::Stream` lowering landed. A yield
/// with code after it now lowers, and so does a yield inside an `if` and a yield
/// inside a `for`.
///
/// The expectation is not edited to match. The discriminator is re-measured, and
/// it is a different property:
///
/// **A COMPOSITE THAT ESCAPES THE ITERATION THAT BUILT IT.** Neither half alone
/// does it — a composite yielded in tail position lowers, and a `Word` yielded
/// from inside a `for` lowers. It is the CONJUNCTION that is refused, because
/// every construction site has a fixed offset, so the next iteration would
/// overwrite bytes the host still holds.
///
/// Three shapes, not two, because two candidate explanations survive a pair
/// here: "composites are the problem" and "loops are the problem". The third
/// case rules out whichever the pair leaves standing.
#[test]
fn the_discriminator_is_a_composite_escaping_its_iteration() {
    let composite_tail =
        status("struct P { a: Word, b: Word }\nloop main(t: Word) -> P { yield P { a: t, b: t } }");
    let word_in_a_loop = status(
        "loop main(t: Word) -> Word { let xs = [1, 2]; for x in xs { let _ = yield x; } yield 0 }",
    );
    let composite_in_a_loop = status(
        "struct P { a: Word, b: Word }\n\
         loop main(t: Word) -> P {\n\
           let xs = [1, 2];\n\
           for x in xs { let _ = yield P { a: x, b: x }; }\n\
           yield P { a: 0, b: 0 }\n\
         }",
    );

    println!("\n================ WHAT SEPARATES THEM");
    println!("  composite, NOT in a loop  : {composite_tail:?}");
    println!("  Word, IN a loop           : {word_in_a_loop:?}");
    println!("  composite, IN a loop      : {composite_in_a_loop:?}");
    println!("================\n");

    assert!(
        matches!(composite_tail, Ok(None)),
        "a composite yielded outside a loop no longer lowers, so the composite \
         type alone would explain the refusal and this pair no longer separates \
         the candidates: {composite_tail:?}"
    );
    assert!(
        matches!(word_in_a_loop, Ok(None)),
        "a Word yielded from inside a loop no longer lowers, so the loop alone \
         would explain the refusal: {word_in_a_loop:?}"
    );
    assert!(
        matches!(&composite_in_a_loop, Ok(Some(why)) if why.contains("yielded at op")),
        "a composite yielded from inside a loop is no longer refused for the \
         escape hazard. Either the guard stopped firing -- which is the silent \
         wrong value case -- or the frontier moved again: {composite_in_a_loop:?}"
    );
}

/// **CORRECTED: THE SHAPE NOW HAS A WITNESS, AND THE ORIGINAL FRAMING WAS
/// WRONG.**
///
/// This test previously claimed that `yield a composite, tail` lowers with
/// nothing executing it, and described the untested code as "a composite
/// crossing the yield boundary". **There is no yield boundary in that
/// lowering.** Measured in `what_the_native_side_yields_for_a_composite`: the
/// module declares no host yield hook at all and the entry RETURNS a pointer
/// into the caller's region. A single yield in tail position, with nothing after
/// it, is lowered as a return.
///
/// So the marshalling is the composite-RETURN ABI, and the shape is now
/// witnessed byte-for-byte against the reference.
///
/// What remains true, and is what this test still checks: **the suspension
/// differential drives no composite-yielding subject.** That matters for
/// SEQUENCE semantics — the order and count of yields for a composite-yielding
/// stream — which is a different property from the value comparison now covered.
#[test]
fn the_suspension_differential_drives_no_composite_yielding_subject() {
    let harness = std::fs::read_to_string("tests/yield_sequence.rs")
        .expect("the suspension differential is a sibling of this file");
    let composite_subjects = harness.matches("loop main(a: Word) -> P").count();
    let word_subjects = harness.matches("loop main(a: Word) -> Word").count();
    println!("\n================ SUSPENSION DIFFERENTIAL SUBJECTS");
    println!("  Word-yielding      : {word_subjects}");
    println!("  composite-yielding : {composite_subjects}");
    // **THE REASON HERE WAS CORRECTED 2026-09-08.** It used to say the gap
    // "cannot be [closed] until such a stream lowers at all -- it needs a yield
    // that is not in tail position, which is refused". A non-tail yield lowers
    // now, so that blocker is gone and the gap is still open, for a DIFFERENT
    // and newly visible reason: the value delivered at a resume point has no
    // declared width, so a composite built from it is refused by the
    // `NewComposite` width check. Measured as the "yield a composite, non-tail"
    // row of the matrix above.
    println!(
        "  => value marshalling for a tail composite IS witnessed, in\n  \
         `what_the_native_side_yields_for_a_composite`. SEQUENCE semantics for a\n  \
         composite-yielding stream are still not, and the blocker is now the\n  \
         RESUME VALUE'S UNKNOWN WIDTH rather than the absence of Stream lowering:\n  \
         a composite built from a resumed value is refused by the NewComposite\n  \
         width check.\n================\n"
    );
    assert!(
        word_subjects > 0,
        "the differential drives no Word subject either, so this count says \
         nothing about a composite gap specifically"
    );
    assert_eq!(
        composite_subjects, 0,
        "the suspension differential now drives a composite-yielding stream, so \
         re-point this rather than delete it"
    );
}
