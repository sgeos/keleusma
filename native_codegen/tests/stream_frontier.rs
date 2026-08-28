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
            println!(
                "  => the yield-escape refusal remains SHADOWED. It is a precaution,\n  \
                 not yet load-bearing.\n================\n"
            );
        }
        Err(why) => println!("  the reference rejects this program: {why}\n================\n"),
    }
}

/// The rule the matrix implies, pinned so a change announces itself.
///
/// **The discriminator is TAIL POSITION, not the yielded type.** A composite
/// yielded in tail position lowers; a `Word` yielded with code after it does
/// not. That single pair separates the two candidate explanations, and it rules
/// out the one this line would have guessed — that composites are the problem,
/// because composites are what `13_telemetry_stream.kel` yields.
///
/// It also rules out "exactly one yield": "yield then more code" has exactly one
/// and is refused.
#[test]
fn the_discriminator_is_tail_position_and_not_the_yielded_type() {
    let composite_tail =
        status("struct P { a: Word, b: Word }\nloop main(t: Word) -> P { yield P { a: t, b: t } }");
    let word_non_tail = status("loop main(t: Word) -> Word { yield t; t * 2 }");

    println!("\n================ WHAT SEPARATES THEM");
    println!("  composite yielded in TAIL position : {composite_tail:?}");
    println!("  Word yielded with code AFTER it    : {word_non_tail:?}");
    println!("================\n");

    assert!(
        matches!(composite_tail, Ok(None)),
        "a composite yielded in tail position no longer lowers, so the frontier is \
         not where this file says: {composite_tail:?}"
    );
    assert!(
        matches!(&word_non_tail, Ok(Some(why)) if why.contains("Stream")),
        "a single Word yield with code after it is no longer refused for Stream, so \
         tail position is no longer the discriminator: {word_non_tail:?}"
    );
}

/// **A SHAPE THAT LOWERS WITHOUT EXECUTION EVIDENCE, NAMED RATHER THAN
/// GLOSSED.**
///
/// `yield a composite, tail` lowers. The suspension differential's subjects all
/// yield `Word`, so **nothing in the tree executes a composite across the yield
/// boundary and compares it against the reference.** A composite handed to the
/// host from a tail yield is marshalled by code no test has run.
///
/// This is not the cross-iteration escape hazard — a tail-yielded composite is
/// built once and no later iteration overwrites it — but it is untested
/// lowering, which is the same class this line has been closing elsewhere.
///
/// Recorded as a gap rather than fixed: a witness needs a suspension harness
/// that drives composite yields, which the existing one does not.
#[test]
fn a_tail_yielded_composite_lowers_with_no_execution_witness() {
    let src = "struct P { a: Word, b: Word }\nloop main(t: Word) -> P { yield P { a: t, b: t } }";
    assert!(
        matches!(status(src), Ok(None)),
        "the subject must lower, or there is no untested lowering to report"
    );
    let harness = std::fs::read_to_string("tests/yield_sequence.rs")
        .expect("the suspension differential is a sibling of this file");
    // Its subjects are `loop main(a: Word) -> Word`; a composite-returning loop
    // would have to name a struct in the signature.
    let composite_subjects = harness.matches("loop main(a: Word) -> P").count();
    println!("\n================ UNTESTED: TAIL-YIELDED COMPOSITE");
    println!("  composite-yielding subjects in the suspension differential: {composite_subjects}");
    println!(
        "  => the shape LOWERS and nothing executes it. Named, not fixed: a witness\n  \
         needs a suspension harness that drives composite yields.\n================\n"
    );
    assert_eq!(
        composite_subjects, 0,
        "the suspension differential now drives a composite-yielding stream, so this \
         gap has closed and this test should be re-pointed rather than deleted"
    );
}
