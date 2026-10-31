//! **CAN A MULTI-PARAMETER STREAM EVEN BE WRITTEN, AND WHAT REFUSES IT?**
//!
//! The backend refuses a resumable stream with more than one parameter, on the
//! ground that the runtime's `resume` writes slot 0 and nothing else. A handoff
//! then grouped that refusal with two others as *"one question, not three —
//! each is what survives the return"*. **That grouping was an inference stated
//! as a finding**, and this probe is the first step in checking it, because
//! reading the emitter suggests the multi-parameter case is about the parameter
//! PROLOGUE rather than about anything crossing a suspension.
//!
//! It asserts almost nothing on purpose. What it establishes is whether the
//! subject EXISTS — a refusal analysis about a program the reference will not
//! compile would be analysis about nothing.

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::{LowerOptions, module_refusals};

mod common;

fn try_compile(src: &str) -> Result<Module, String> {
    let toks = tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let ast = parse(&toks).map_err(|e| format!("parse: {e:?}"))?;
    compile(&ast).map_err(|e| format!("{e:?}"))
}

const SHAPES: &[(&str, &str)] = &[
    (
        "two parameters, tail yield",
        "loop main(a: Word, b: Word) -> Word { yield a + b }",
    ),
    (
        "two parameters, non-tail yield",
        "loop main(a: Word, b: Word) -> Word { let r = yield a + b; yield r + b }",
    ),
    (
        "one parameter, for comparison",
        "loop main(a: Word) -> Word { let r = yield a; yield r + 1 }",
    ),
];

#[test]
fn whether_a_multi_parameter_stream_exists_and_what_declines_it() {
    println!("\n================ MULTI-PARAMETER STREAM");
    let mut compiled = 0usize;
    for (label, src) in SHAPES {
        match try_compile(src) {
            Ok(m) => {
                compiled += 1;
                let r = module_refusals(&m, LowerOptions::default());
                let entry = m.entry_point.expect("entry");
                let text = if r.is_empty() {
                    "LOWERS".to_string()
                } else {
                    r.iter()
                        .map(|(c, e)| format!("{c}: {e}"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                };
                let short: String = text.chars().take(130).collect();
                println!(
                    "  {label:<32} params={} {short}",
                    m.chunks[entry].param_count
                );
            }
            Err(why) => {
                let short: String = why.chars().take(130).collect();
                println!("  {label:<32} REFERENCE REJECTED: {short}");
            }
        }
    }
    println!("================\n");

    // **NON-VACUITY ONLY.** If the reference rejects every shape here, the
    // backend's multi-parameter refusal is unreachable from source and any
    // analysis of it is analysis of a program that cannot exist.
    assert!(
        compiled > 0,
        "the reference compiles none of these, so this probe has no subject"
    );
}

/// **WHAT DOES THE RUNTIME DO WITH SLOT 1 ACROSS A REWIND? SINCE ABSORPTION
/// 122 THE QUESTION DOES NOT ARISE, BECAUSE THE REFERENCE REFUSES THE SHAPE.**
///
/// # The history
///
/// The backend refuses a resumable stream with more than one parameter because
/// `resume` writes slot 0 and nothing else. Until absorption 121 the reference
/// compiled the shape, and this probe measured what it did. `Op::Reset` set
/// every local to `Unit`, the resume wrote slot 0, and the next iteration's
/// `a + b` was `CheckedAdd` on `Int` and `Unit`, a `TypeError`. So the native
/// refusal was not what stood between a user and the shape, because the program
/// did not work on the reference either. That was recorded as this line's
/// report 2.
///
/// # The answer
///
/// V0.2.X's narrowed definite-initialization rule, its pull request 658,
/// resets the set of written locals to the stream parameter at `Stream`, so a
/// read of `b` after it reads a local that no path writes, and compilation
/// fails. The native refusal is now unreachable from source, and report 2 is
/// answered as `outstanding_reports.rs` records.
///
/// # What this probe therefore asserts
///
/// That the reference refuses the shape, and refuses it for that rule. If it
/// starts compiling the shape again, the run-time measurement this probe used
/// to make is the place to start.
#[test]
fn the_runtime_faults_on_a_second_parameter_after_the_rewind() {
    let src = "loop main(a: Word, b: Word) -> Word { let r = yield a + b; yield r + b }";
    let why = try_compile(src).expect_err(
        "the reference compiles a two-parameter stream again, so the run-time \
         measurement this probe made until absorption 121 needs restoring",
    );
    println!("\n================ THE RUNTIME'S SECOND PARAMETER");
    println!("  reference refused: {why}");
    println!("================\n");
    assert!(
        why.contains("definite-initialization"),
        "the reference refuses a two-parameter stream, but not by the \
         definite-initialization rule: {why}"
    );
}

/// **WHY IS A RESUME POINT ALSO A BRANCH TARGET, AND AT WHAT DEPTHS?**
///
/// The last refusal that is a gap in this backend. Printed rather than reasoned
/// about, because the previous two dispositions on this frontier were both
/// settled by reading what the ops actually are.
#[test]
fn what_the_branch_target_collision_looks_like() {
    use keleusma::bytecode::Op;
    let src =
        "loop main(t: Word) -> Word { if t > 0 { let a = yield t; yield a } else { yield 0 } }";
    let m = try_compile(src).expect("compiles");
    let entry = m.entry_point.expect("entry");
    println!("\n================ THE JOIN SHAPE");
    for (i, op) in m.chunks[entry].ops.iter().enumerate() {
        let mark = match op {
            Op::Yield => "  <- YIELD, so the next op is a RESUME point",
            Op::If(t) | Op::Else(t) | Op::EndLoop(t) | Op::Break(t) | Op::BreakIf(t) => {
                Box::leak(format!("  <- branches to op {t}").into_boxed_str())
            }
            _ => "",
        };
        println!("  {i:3} {op:?}{mark}");
    }
    println!("================\n");
}

/// **DOES THE CORPUS CONTAIN A GENERAL STREAM? MEASURED BY VISITS, AFTER A
/// SIGNATURE-BASED PROBE GOT IT WRONG.**
///
/// # The wrong probe, kept as the reason this one is shaped differently
///
/// The first attempt classified a stream chunk as general when its lowered
/// function carried the three arena pointers, and reported **26 of 28 corpus
/// modules taking the general path** — which would have made the ISA census's
/// `Reset` disposition stale and sent me to "correct" an accurate README.
///
/// **The signature does not say that.** `needs_region` is decided per MODULE: a
/// module where any chunk builds a composite gives EVERY chunk the pointers, so
/// a degenerate stream in such a module looks identical to a general one. The
/// emitter's own comment at `needs_region` records this exact hazard — *"adding
/// the pointers to every stream changed the signature of the degenerate chunks
/// too"* — and I had read it earlier the same day.
///
/// # What is measured instead
///
/// Whether the lowering VISITS the `Op::Reset` index, which is the property the
/// census reports and is not confounded by module-level decisions. A known
/// general stream is driven alongside as a positive control, because a probe
/// that reports "none" everywhere is indistinguishable from one that cannot
/// report anything.
#[test]
fn the_corpus_contains_no_general_stream_and_the_control_proves_the_probe_works() {
    use keleusma::bytecode::Op;
    use keleusma_native::module_lowered_op_indices;

    let corpus = common::corpus();
    assert!(!corpus.is_empty(), "the corpus loaded nothing");

    let mut with_reset = 0usize;
    let mut visited = 0usize;
    for (_name, m) in &corpus {
        let (_r, indices) = module_lowered_op_indices(m, LowerOptions::default());
        for (ci, chunk) in m.chunks.iter().enumerate() {
            let resets: Vec<usize> = chunk
                .ops
                .iter()
                .enumerate()
                .filter(|(_, o)| matches!(o, Op::Reset))
                .map(|(i, _)| i)
                .collect();
            if resets.is_empty() {
                continue;
            }
            with_reset += 1;
            if let Some(Some(seen)) = indices.get(ci)
                && resets.iter().any(|r| seen.contains(r))
            {
                visited += 1;
            }
        }
    }

    // **THE POSITIVE CONTROL.** A stream the general path certainly takes.
    let ctrl = try_compile("loop main(t: Word) -> Word { let a = yield t; yield a + 1 }")
        .expect("the control compiles");
    let centry = ctrl.entry_point.expect("entry");
    let (_cr, cix) = module_lowered_op_indices(&ctrl, LowerOptions::default());
    let creset = ctrl.chunks[centry]
        .ops
        .iter()
        .position(|o| matches!(o, Op::Reset))
        .expect("the control carries a Reset");
    let control_visited = matches!(cix.get(centry), Some(Some(seen)) if seen.contains(&creset));

    println!("\n================ DOES ANYTHING VISIT `Op::Reset`?");
    println!("  corpus chunks carrying a Reset : {with_reset}");
    println!("  ...where it was VISITED        : {visited}");
    println!("  control (a general stream)     : visited = {control_visited}");
    println!(
        "\n  The corpus figure being zero is a fact about the CORPUS. The control\n  \
         shows the backend visits `Reset` whenever a general stream appears, so\n  \
         the census's disposition is accurate and stays.\n================\n"
    );

    assert!(with_reset > 0, "no corpus chunk carries a Reset");
    assert!(
        control_visited,
        "the control's Reset was not visited, so this probe cannot report a \
         positive and its zero above means nothing"
    );
    assert_eq!(
        visited, 0,
        "a corpus chunk now visits `Op::Reset`, so the ISA lowering census's \
         disposition -- Reset accepted by a route it does not instrument -- has \
         become stale and must be re-derived rather than left standing"
    );
}
