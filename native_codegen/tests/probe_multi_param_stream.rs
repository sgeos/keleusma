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

use keleusma::bytecode::{Module, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
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

/// **WHAT DOES THE RUNTIME DO WITH SLOT 1 ACROSS A SUSPENSION AND ACROSS A
/// REWIND? IT FAULTS, AND THAT CHANGES WHAT THE NATIVE REFUSAL IS FOR.**
///
/// # The question, and the answer that was not anticipated
///
/// The backend refuses a resumable stream with more than one parameter because
/// `resume` writes slot 0 and nothing else. Reading the emitter suggested the
/// refusal was really about the parameter PROLOGUE — which stores every
/// parameter into its local unconditionally, so a resume call would clobber
/// live locals with whatever the calling convention left in those registers —
/// and that clearing slots 1..n at `Op::Reset` would AGREE with the runtime,
/// since the runtime clears them too.
///
/// **The runtime does clear them, and then it FAULTS.** `Op::Reset` sets every
/// local to `Unit`, the resume writes slot 0, and the next iteration's `a + b`
/// is `CheckedAdd` on `Int` and `Unit`:
///
/// ```text
/// TypeError("Op::CheckedAdd expects Word, Byte, Float, or Fixed operands, got Int and Unit")
/// ```
///
/// # Why this matters more than the prologue question
///
/// **"Agrees with the runtime" was the wrong frame.** A native lowering that
/// cleared slot 1 and carried on would not agree with the reference — the
/// reference stops. And a fault is OBSERVABLE, so producing a value where the
/// reference faults is a silently wrong answer, which is the class this backend
/// exists to refuse.
///
/// So the native refusal is not what stands between a user and this shape. **A
/// multi-parameter stream whose body reads a second parameter after its first
/// rewind is not a working program on the reference either.** Whether that is a
/// language defect, an intended consequence of `Reset` semantics, or a case the
/// verifier should reject outright is a question for the reference, not for this
/// backend, and it is recorded here rather than answered.
///
/// # What this probe therefore asserts
///
/// The measurement, not a verdict: that the reference compiles the shape, that
/// it yields on the first iteration, and that it FAULTS after the rewind. If any
/// of those changes, the reasoning above needs redoing.
#[test]
fn the_runtime_faults_on_a_second_parameter_after_the_rewind() {
    let src = "loop main(a: Word, b: Word) -> Word { let r = yield a + b; yield r + b }";
    let m = try_compile(src).expect("the reference compiles a two-parameter stream");

    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent region");
    let mut vm = Vm::new(m, &arena).expect("vm");

    let replies = [100i64, 200, 300, 400];
    let mut yielded: Vec<i64> = Vec::new();
    let mut legs: Vec<String> = Vec::new();
    let mut fault: Option<String> = None;

    let mut st = vm.call(&[Value::Int(3), Value::Int(10)]).expect("vm run");
    while yielded.len() < replies.len() {
        let reply = match st {
            VmState::Yielded(Value::Int(v)) => {
                yielded.push(v);
                legs.push(format!("Yielded({v})"));
                replies[yielded.len() - 1]
            }
            VmState::Reset => {
                legs.push("Reset".to_string());
                replies[yielded.len().saturating_sub(1)]
            }
            ref other => panic!("a stream produced {other:?}"),
        };
        match vm.resume(Value::Int(reply)) {
            Ok(next) => st = next,
            Err(e) => {
                fault = Some(format!("{e:?}"));
                break;
            }
        }
    }

    println!("\n================ THE RUNTIME'S SECOND PARAMETER");
    println!("  call(a=3, b=10), replies {replies:?}");
    println!("  legs    : {legs:?}");
    println!("  yielded : {yielded:?}");
    println!("  fault   : {fault:?}");
    println!(
        "\n  The first yield is a + b. The second is reply + b, so it reports what\n  \
         slot 1 held after the suspension. After the REWIND, slot 1 is `Unit` and\n  \
         the arithmetic is a TYPE ERROR -- the reference does not merely lose the\n  \
         parameter, it STOPS.\n================\n"
    );

    assert_eq!(
        yielded.first().copied(),
        Some(13),
        "the first yield is a + b with a=3, b=10 and must be 13; if it is not, \
         the arguments are not reaching the slots this probe assumes"
    );
    assert!(
        legs.iter().any(|l| l == "Reset"),
        "the subject never rewound, so this says nothing about what `Op::Reset` \
         leaves in slot 1: {legs:?}"
    );
    let f = fault.expect(
        "the reference no longer faults after the rewind. A multi-parameter \
         stream may have become a working program, which would make the native \
         refusal the only thing standing in the way -- the opposite of what this \
         probe records.",
    );
    assert!(
        f.contains("Unit"),
        "the reference faults after the rewind, but not on a `Unit` operand, so \
         the cause is not slot 1 being cleared: {f}"
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
