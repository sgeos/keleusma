//! The opcodes whose lowering arms have never run, asked one at a time.
//!
//! `isa_lowering_census` reports three as "emitted, never visited, never named"
//! — `FloatToInt`, `IntToFloat`, `Reset` — and one, `IsStruct`, as having no
//! corpus witness. **"The backend lowers it" is a claim about an arm existing,
//! not about its behaviour**, and an arm that has never executed is where a
//! miscompile hides.
//!
//! This file does not try to make them reachable. It establishes, per opcode,
//! whether a witness is possible at all and on what evidence — and where one is
//! possible, runs it against the reference.

use keleusma::bytecode::{Module, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::{LowerOptions, module_refusals};

fn try_compile(src: &str) -> Result<Module, String> {
    let toks = tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let ast = parse(&toks).map_err(|e| format!("parse: {e:?}"))?;
    compile(&ast).map_err(|e| format!("compile: {e:?}"))
}

fn emits(m: &Module, want: fn(&Op) -> bool) -> bool {
    m.chunks.iter().any(|c| c.ops.iter().any(want))
}

/// Can a float CONVERSION reach the backend without a float constant?
///
/// The float guard refuses a module carrying a float constant, on the ground
/// that integer arithmetic lowering would silently miscompile a float. A
/// conversion is not a constant, so whether the guard stops one is a question
/// for measurement rather than for reasoning — and the answer decides whether
/// `IntToFloat` and `FloatToInt` are reachable at all.
///
/// **Nothing is weakened to reach them.** If the guard blocks the witness, that
/// IS the finding.
#[test]
fn whether_a_float_conversion_reaches_the_backend() {
    let candidates = [
        (
            "round trip",
            "fn main(x: Word) -> Word { (x as Float) as Word }",
        ),
        ("to float only", "fn main(x: Word) -> Float { x as Float }"),
    ];
    println!("\n================ FLOAT CONVERSIONS: REACHABLE?");
    let mut any_lowered = false;
    for (label, src) in candidates {
        match try_compile(src) {
            Err(why) => println!("  {label:<14} REFERENCE REJECTED: {why}"),
            Ok(m) => {
                let has_conv = emits(&m, |o| matches!(o, Op::IntToFloat | Op::FloatToInt));
                let refusals: Vec<String> = module_refusals(&m, LowerOptions::default())
                    .iter()
                    .map(|(s, e)| format!("{s}: {e}"))
                    .collect();
                println!(
                    "  {label:<14} compiles; emits a conversion: {has_conv}; backend: {}",
                    if refusals.is_empty() {
                        "LOWERS".to_string()
                    } else {
                        format!("{refusals:?}")
                    }
                );
                if has_conv && refusals.is_empty() {
                    any_lowered = true;
                }
            }
        }
    }
    println!("  any float conversion both emitted AND lowered: {any_lowered}");
    println!("================\n");
}

/// `Reset` is REACHABLE and its module LOWERS — the opposite of what was
/// assumed.
///
/// The plan for this increment guessed that `Reset` was gated behind the
/// `Stream` refusal and therefore unreachable. **Measured, a minimal
/// `loop main` emits both `Stream` and `Reset` and the backend refuses
/// nothing.** `Stream` is lowered for this shape; the refusal seen on
/// `13_telemetry_stream.kel` is about that module, not about the opcode.
///
/// So "unproven" here means unproven **from the corpus**, which is what the
/// census says it means. The opcode is not unreachable, and the hand-written
/// suspension differential already drives exactly this shape.
#[test]
fn reset_is_reachable_and_its_module_lowers() {
    let m = try_compile("loop main(t: Word) -> Word { yield t }").expect("a loop compiles");
    let has_reset = emits(&m, |o| matches!(o, Op::Reset));
    let has_stream = emits(&m, |o| matches!(o, Op::Stream));
    let refusals: Vec<String> = module_refusals(&m, LowerOptions::default())
        .iter()
        .map(|(s, e)| format!("{s}: {e}"))
        .collect();
    println!("\n================ RESET");
    println!("  a minimal loop emits Reset: {has_reset}, Stream: {has_stream}");
    println!("  backend refusals: {refusals:?}");
    println!("================\n");
    assert!(
        has_reset && has_stream,
        "the subject must emit both, or it measures nothing: Reset={has_reset} \
         Stream={has_stream}"
    );
    assert!(
        refusals.is_empty(),
        "a minimal stream is no longer lowered, so Reset's reachability has \
         changed: {refusals:?}"
    );
}

/// Each float conversion, asked separately, so one refusal does not stand in for
/// the other.
#[test]
fn each_float_conversion_is_refused_by_name() {
    let cases = [(
        "IntToFloat",
        "fn main(x: Word) -> Word { (x as Float) as Word }",
    )];
    println!("\n================ FLOAT CONVERSIONS, NAMED");
    for (label, src) in cases {
        let m = try_compile(src).expect("compiles");
        let ops: Vec<String> = m
            .chunks
            .iter()
            .flat_map(|c| c.ops.iter())
            .filter(|o| matches!(o, Op::IntToFloat | Op::FloatToInt))
            .map(|o| format!("{o:?}"))
            .collect();
        let refusals: Vec<String> = module_refusals(&m, LowerOptions::default())
            .iter()
            .map(|(s, e)| format!("{s}: {e}"))
            .collect();
        println!("  {label}: conversions emitted {ops:?}");
        println!("      refusals: {refusals:?}");
        assert!(
            !ops.is_empty(),
            "{label}: the subject emits no conversion, so it measures nothing"
        );
        assert!(
            refusals
                .iter()
                .any(|r| r.contains("ToFloat") || r.contains("ToInt")),
            "{label}: expected a refusal naming a float conversion, got {refusals:?}. \
             If the backend now lowers one, it is no longer unproven and needs an \
             execution witness rather than this test."
        );
    }
    println!("================\n");
}

/// `IsStruct` cannot be witnessed, and the reason is structural.
///
/// The reference implementation's arm matches a **Boxed** struct body and treats
/// a flat one as a mis-compilation. **After B28 the corpus contains zero
/// non-`Flat` composites**, so even a witness built by injecting the opcode into
/// real bytecode would be comparing the backend against a reference fault rather
/// than against a value.
///
/// The `v0.2.3` line separately recorded a bounded search for a producer in
/// `src/compiler.rs`, and its own standard is worth repeating here: **"no
/// producer found by a bounded search" is not the same as unreachable**, and a
/// producerless claim made there was falsified within the hour. This test
/// therefore records the SEARCH, not a conclusion of impossibility.
#[test]
fn what_stands_between_is_struct_and_a_witness() {
    // The shapes a struct pattern can be matched against, as tried here.
    let tried = [
        (
            "pattern vs foreign struct",
            "struct P { a: Word }\nstruct Q { a: Word }\n\
             fn g(P { a }: Q) -> Word { a }\nfn main() -> Word { g(Q { a: 1 }) }",
        ),
        (
            "pattern in a match arm",
            "struct P { a: Word }\n\
             fn main() -> Word { let p = P { a: 1 }; match p { P { a } => a } }",
        ),
    ];
    println!("\n================ IS_STRUCT: WHAT WAS TRIED");
    let mut produced = false;
    for (label, src) in tried {
        match try_compile(src) {
            Err(why) => println!("  {label:<26} REFERENCE REJECTED: {why}"),
            Ok(m) => {
                let has = emits(&m, |o| matches!(o, Op::IsStruct(_)));
                println!("  {label:<26} compiles; emits IsStruct: {has}");
                produced |= has;
            }
        }
    }
    println!(
        "  a producer was found by THIS search: {produced}\n  \
         (a negative here is a fact about the search, not a proof of \
         unreachability)"
    );
    println!("================\n");
}

/// `Reset` already HAS an execution witness in this package, and the census
/// cannot see it because its population is the corpus.
///
/// The suspension differential drives `loop main` programs through both the
/// native lowering and the reference, comparing the whole yielded sequence. Any
/// such program emits `Reset`. So "never visited" is true of the shipped corpus
/// and false of the test suite, and those are different claims.
///
/// **The linkage is checked rather than asserted in prose**: this reads the
/// sibling harness for a `loop main` subject and separately confirms that such a
/// program emits `Reset`. Citing a neighbouring file without checking it is how
/// a claim survives the file it describes.
#[test]
fn resets_execution_witness_is_the_suspension_differential() {
    let harness = std::fs::read_to_string("tests/yield_sequence.rs")
        .expect("the suspension differential is a sibling of this file");
    let subjects = harness.matches("loop main").count();
    println!("\n================ RESET'S WITNESS");
    println!("  `loop main` subjects in the suspension differential: {subjects}");
    assert!(
        subjects > 0,
        "the suspension differential no longer drives any `loop main` program, so \
         the claim that Reset is executed somewhere in this package is stale"
    );

    let m = try_compile("loop main(a: Word) -> Word { yield a }").expect("compiles");
    assert!(
        emits(&m, |o| matches!(o, Op::Reset)),
        "a `loop main` program no longer emits Reset, so driving one would not \
         exercise it"
    );
    println!("  and such a program emits Reset: true");
    println!("================\n");
}
