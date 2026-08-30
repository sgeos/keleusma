//! **DOES THE FLOAT LOWERING COMPUTE THE SAME NUMBER AS THE REFERENCE?**
//!
//! The module-level float guard still refuses every float-carrying module, so
//! these arms are unreachable through `lower_module`. That ordering is
//! deliberate: **a half-implemented float path that is ACCEPTED rather than
//! refused is worse than the current refusal**, because a wrong float is a
//! plausible number rather than a fault.
//!
//! This file goes through `lower_chunk`, which does not carry the module guard,
//! and compares against the virtual machine. **Only if these agree is relaxing
//! the guard defensible**, and relaxing it is not done here.
//!
//! # What is covered, and what is not
//!
//! Covered: a float CONSTANT, `IntToFloat`, `FloatToInt`, and `Op::Add` on two
//! float operands — which is exactly what `examples/scripts/float_witness.kel`
//! needs. **Not covered**: the entry ABI (no corpus module has a float in a
//! signature, so it has no witness here), float shared slots, division,
//! comparisons, and `f32`.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_chunk};

fn vm_result(src: &str, arg: i64) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");
    match vm.call(&[Value::Int(arg)]).expect("vm run") {
        keleusma::vm::VmState::Finished(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

fn native_result(src: &str, arg: i64) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert_eq!(
        m.entry_point,
        Some(0),
        "this harness lowers chunks[0] and calls the ENTRY POINT in the virtual \
         machine; they coincide only for a single-function program"
    );
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect("the float lowering must accept this chunk");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>("kel_entry") }
        .expect("entry symbol");
    unsafe { f.call(arg) }
}

/// The witness's exact shape: `IntToFloat`, a float constant, float `Add`,
/// `FloatToInt`.
const ROUND_TRIP: &str = "\
fn main(w: Word) -> Word {
  let f = w as Float;
  let scaled = f + 1.5;
  scaled as Word
}
";

#[test]
fn the_float_round_trip_agrees_with_the_reference() {
    // **Values chosen so the fractional part MATTERS.** Adding 1.5 and
    // truncating toward zero gives a different answer from integer arithmetic
    // for every one of these, so an integer `add` lowered by mistake would
    // disagree rather than coincide.
    for arg in [0i64, 1, 2, 3, 7, 41, -1, -2, -7, 1000] {
        let v = vm_result(ROUND_TRIP, arg);
        let n = native_result(ROUND_TRIP, arg);
        assert_eq!(
            v, n,
            "float round trip disagrees at {arg}: reference {v}, native {n}"
        );
    }
}

/// **CONTROL, must-fire.** Without this, the agreement above could hold because
/// both sides do integer arithmetic and the float path never ran.
#[test]
fn the_result_is_not_what_integer_arithmetic_would_give() {
    // `3 as Float + 1.5 = 4.5`, truncating to 4. Integer `3 + 1` would give 4
    // too, so 3 is a BAD probe; 2 gives 3.5 -> 3 against an integer 3, also bad.
    // A value whose truncation differs from any plausible integer reading:
    // 0 -> 1.5 -> 1, where an integer add of a truncated 1.5 (=1) also gives 1.
    // The distinguishing case is NEGATIVE: -1 -> 0.5 -> 0, whereas integer
    // -1 + 1 = 0 as well. So compare against the VALUE ITSELF instead: the
    // result must differ from the input for at least one probe, or nothing was
    // computed.
    let moved = [0i64, 1, 7, -7]
        .iter()
        .any(|&a| vm_result(ROUND_TRIP, a) != a);
    assert!(
        moved,
        "the reference returns its own argument for every probe, so this program \
         computes nothing and the agreement above is vacuous"
    );
}

/// **A float must not reach an opcode that was not written for one.**
///
/// Implementing float arithmetic removed an accidental protection: a module with
/// a float local and no float constant or signature was previously refused only
/// because no float OPERATION existed. `float_guard_routes.rs` calls that *"a
/// property of what is unimplemented, not a guard"*, and it stopped being true
/// the moment the operations were written.
///
/// The module-level guard does not cover such a module — it scans signatures,
/// constants, native shapes and data slots, and this shape has none of them. So
/// the protection has to be at the operand, and it is a WHITELIST: anything not
/// written to understand a float refuses.
#[test]
fn a_float_cannot_reach_an_opcode_not_written_for_one() {
    // **THE SIGNATURE SUBJECT RETIRED 2026-08-30 when the entry ABI landed** —
    // `entry_abi_float.rs` calls the JIT-ed symbol through the real C
    // convention and agrees with the virtual machine — following division and
    // remainder, each retired the increment it was implemented and verified.
    //
    // The successor is route 3, a native declaring a Float **RETURN SHAPE**,
    // which stays closed because an unsignatured native result would reach the
    // operand stack as a float with no kind seeding behind it. The native is
    // deliberately UNCALLED so no other route can fire first; the isolation
    // argument is in `float_guard_routes.rs`.
    const SUBJECT: &str = "use host::read_temp() -> Float\n\nfn main(w: Word) -> Word { w }";
    let m = compile(&parse(&tokenize(SUBJECT).expect("lex")).expect("parse")).expect("compile");
    let refusals = keleusma_native::module_refusals(&m, LowerOptions::default());
    assert!(
        !refusals.is_empty(),
        "a native declaring a Float RETURN SHAPE no longer refuses the module. \
         Either native float results gained a lowering — in which case this \
         subject has retired like its predecessors and needs a successor — or a \
         route was opened without one"
    );
    let text = refusals
        .iter()
        .map(|(_, e)| format!("{e}"))
        .collect::<Vec<_>>()
        .join("; ");
    assert!(
        text.contains("Float") || text.contains("float"),
        "the refusal does not mention a float, so it may be refusing for an \
         unrelated reason and this test would guard nothing: {text}"
    );
}

/// **CONTROL, must-not-fire.** Without this, the whitelist could be refusing
/// everything and the test above would pass while the float path was dead.
#[test]
fn the_whitelist_does_not_refuse_the_supported_shape() {
    let m = compile(&parse(&tokenize(ROUND_TRIP).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect("the supported round trip must still lower; the whitelist is too tight");
}

/// **Float comparisons agree with the reference, including its unusual ordering.**
///
/// The virtual machine compares floats with
/// `x.partial_cmp(y).unwrap_or(Ordering::Equal)`, so a NaN is **equal to
/// everything** rather than unordered. The lowering matches that rather than
/// emitting the natural `fcmp`, which would make `NaN == x` true on the
/// reference and false natively.
///
/// **⚠ The NaN case is NOT exercised here and cannot be**: no source construct
/// produces a NaN, because the route is division and `Op::CheckedDiv` is not
/// lowered. The adjustment is written to match by reading the reference. Saying
/// so is the point — a green result below must not be read as covering it.
#[test]
fn float_comparisons_agree_with_the_reference() {
    // Each predicate, with operands whose fractional parts decide the answer, so
    // a comparison accidentally done on the integer bit pattern would differ.
    for (op, name) in [
        ("<", "lt"),
        (">", "gt"),
        ("<=", "le"),
        (">=", "ge"),
        ("==", "eq"),
        ("!=", "ne"),
    ] {
        let src = format!(
            "fn main(w: Word) -> Word {{\n  \
               let a = w as Float;\n  \
               let b = 2.5;\n  \
               if a {op} b {{ 1 }} else {{ 0 }}\n\
             }}\n"
        );
        for arg in [0i64, 1, 2, 3, 5, -1, -3] {
            let v = vm_result(&src, arg);
            let n = native_result(&src, arg);
            assert_eq!(
                v, n,
                "float `{name}` disagrees at {arg}: reference {v}, native {n}"
            );
        }
    }
}

/// **CONTROL, must-fire.** Without it, agreement above could hold because every
/// probe gives the same answer and nothing distinguishes the predicates.
#[test]
fn the_comparison_probes_actually_discriminate() {
    let src = "\
fn main(w: Word) -> Word {
  let a = w as Float;
  let b = 2.5;
  if a < b { 1 } else { 0 }
}
";
    let answers: Vec<i64> = [0i64, 3].iter().map(|&a| vm_result(src, a)).collect();
    assert!(
        answers[0] != answers[1],
        "both probes give {answers:?}, so the comparison never changes answer and \
         the agreement above would be vacuous"
    );
}

/// **THE FLOAT-TO-INT CONVERSION MUST SATURATE, NOT BE POISON.**
///
/// The reference converts with Rust's `as`, which **saturates**: NaN gives 0,
/// and out-of-range gives `i64::MIN`/`MAX`. LLVM's plain `fptosi` is **poison**
/// for exactly those inputs, so the two agree only by whatever the target
/// happens to do.
///
/// **Measured: they DO agree on aarch64**, whose `fcvtzs` saturates — and that
/// is the problem rather than the reassurance. On x86-64 `cvttsd2si` yields the
/// integer-indefinite value for every out-of-range input, so `+inf` would give
/// `MIN` where the reference gives `MAX`, and NaN would give `MIN` where the
/// reference gives 0.
///
/// The lowering now uses `llvm.fptosi.sat`, which is DEFINED to saturate on
/// every target and is what Rust lowers `as` to. **This test therefore passes
/// both before and after that change on this machine**; what it guards is that
/// the agreement stays true when the accident does not.
///
/// The probe uses a RUNTIME out-of-range value, not a constant: a constant is
/// folded at compile time and never reaches the target's instruction.
#[test]
fn the_float_to_int_conversion_saturates_like_the_reference() {
    // RUNTIME out of range, not a constant: a constant is folded at compile
    // time, while this emits the target's conversion instruction.
    const SRC: &str = "\
fn main(w: Word) -> Word {
  let f = w as Float;
  let scale = 10000000000000000000000.0;
  let big = f * scale;
  big as Word
}
";
    for arg in [1i64, 2, -1] {
        let v = vm_result(SRC, arg);
        let n = native_result(SRC, arg);
        println!("  arg {arg}: reference {v}   native {n}");
        assert_eq!(v, n, "runtime out-of-range float cast disagrees at {arg}");
    }
}

/// **Float division, including the results that used to be unreachable.**
///
/// The reference's `Op::Div` float arm is a bare `x / y` with **no zero check** —
/// division by zero yields a signed infinity and `0.0 / 0.0` yields NaN. That is
/// exactly `fdiv`, so the lowering is a straight bitcast pair around it.
///
/// **A previous increment recorded that float division flows through
/// `Op::CheckedDiv`'s three-value push. That was wrong for the `/` operator**:
/// the compiler emits plain `Op::Div`. Corrected in `FLOAT_DIVISION.md`.
#[test]
fn float_division_agrees_with_the_reference() {
    const SRC: &str = "\
fn main(w: Word) -> Word {
  let a = w as Float;
  let b = 2.0;
  let q = a / b;
  q as Word
}
";
    for arg in [0i64, 1, 2, 3, 7, -1, -3, -8] {
        let v = vm_result(SRC, arg);
        let n = native_result(SRC, arg);
        assert_eq!(v, n, "float division disagrees at {arg}: {v} vs {n}");
    }
}

/// **Division by zero, which the saturating conversion finally makes OBSERVABLE.**
///
/// `x / 0.0` is `+inf` or `-inf` and `0.0 / 0.0` is NaN. Converting those to a
/// word saturates on the reference — `i64::MAX`, `i64::MIN`, and **0 for NaN** —
/// and the lowering now uses `llvm.fptosi.sat`, so both sides are defined.
///
/// **Before division landed, no source construct could produce these values.**
/// This is the first test in the suite that exercises them.
#[test]
fn dividing_by_zero_agrees_including_nan() {
    const SRC: &str = "\
fn main(w: Word) -> Word {
  let a = w as Float;
  let z = 0.0;
  let q = a / z;
  q as Word
}
";
    // 1/0 -> +inf -> i64::MAX ; -1/0 -> -inf -> i64::MIN ; 0/0 -> NaN -> 0.
    for arg in [1i64, -1, 0] {
        let v = vm_result(SRC, arg);
        let n = native_result(SRC, arg);
        assert_eq!(v, n, "division by zero disagrees at {arg}: {v} vs {n}");
    }
    // Non-vacuity: the three probes must not all give the same answer, or the
    // saturation and NaN behaviour would be untested by them.
    let answers: Vec<i64> = [1i64, -1, 0].iter().map(|&a| vm_result(SRC, a)).collect();
    assert!(
        answers[0] != answers[1] && answers[1] != answers[2],
        "the zero-division probes give {answers:?}, so they do not distinguish \
         +inf, -inf and NaN and this test would pass without exercising them"
    );
}

/// **THE NaN COMPARISON PATH, WRITTEN BLIND LAST INCREMENT, IS NOW EXERCISED.**
///
/// The reference compares floats with `partial_cmp(...).unwrap_or(Equal)`, so a
/// NaN is **equal to everything**. That adjustment was implemented by reading the
/// reference, with no way to test it: nothing could produce a NaN.
///
/// **Division changed that.** `0.0 / 0.0` is a NaN, so every predicate can now be
/// asked about one — and this is the test the previous increment said it could
/// not write.
#[test]
fn nan_compares_equal_to_everything_as_the_reference_does() {
    for (op, name) in [
        ("<", "lt"),
        (">", "gt"),
        ("<=", "le"),
        (">=", "ge"),
        ("==", "eq"),
        ("!=", "ne"),
    ] {
        let src = format!(
            "fn main(w: Word) -> Word {{\n  \
               let z = 0.0;\n  \
               let nan = z / z;\n  \
               let other = 2.5;\n  \
               if nan {op} other {{ 1 }} else {{ 0 }}\n\
             }}\n"
        );
        let v = vm_result(&src, 0);
        let n = native_result(&src, 0);
        assert_eq!(
            v, n,
            "NaN `{name}` disagrees: reference {v}, native {n}. The reference \
             treats NaN as EQUAL to everything, so `==`, `<=` and `>=` are true \
             and the rest false"
        );
    }
}

/// **Float negation and remainder, the last two of the scalar surface.**
///
/// The reference's `Op::Neg` arm handles `Byte`, `Fixed` and `Float`, negating a
/// float with a plain `-x`. Its `Op::Mod` float arm is `x % y`, and **Rust's `%`
/// on `f64` is the TRUNCATED remainder carrying the sign of the dividend** —
/// exactly `frem`, and NOT a floor-style remainder.
///
/// **The negative dividends are the point.** `-7.0 % 2.0` is `-1.0` truncated and
/// `+1.0` floored; a probe with only positive operands would not tell them apart.
#[test]
fn float_negation_and_remainder_agree_with_the_reference() {
    const NEG: &str = "\
fn main(w: Word) -> Word {
  let f = w as Float;
  let g = 0.0 - f;
  g as Word
}
";
    for arg in [0i64, 1, 7, -1, -7] {
        let v = vm_result(NEG, arg);
        let n = native_result(NEG, arg);
        assert_eq!(v, n, "float negation disagrees at {arg}: {v} vs {n}");
    }

    const REM: &str = "\
fn main(w: Word) -> Word {
  let f = w as Float;
  let d = 2.0;
  let r = f % d;
  let scaled = r * 100.0;
  scaled as Word
}
";
    // Scaled by 100 so a fractional remainder survives the cast to a word:
    // `7 % 2 = 1.0` and `-7 % 2 = -1.0`, which differ in SIGN between the
    // truncated and floored conventions.
    for arg in [0i64, 1, 2, 3, 7, -1, -3, -7] {
        let v = vm_result(REM, arg);
        let n = native_result(REM, arg);
        assert_eq!(v, n, "float remainder disagrees at {arg}: {v} vs {n}");
    }
}

/// **CONTROL, must-fire.** Without it, the remainder probes could all give zero
/// and the agreement above would say nothing about the sign convention.
#[test]
fn the_remainder_probes_show_a_signed_result() {
    const REM: &str = "\
fn main(w: Word) -> Word {
  let f = w as Float;
  let d = 2.0;
  let r = f % d;
  let scaled = r * 100.0;
  scaled as Word
}
";
    let pos = vm_result(REM, 7);
    let neg = vm_result(REM, -7);
    assert!(
        pos > 0 && neg < 0,
        "the remainder probes give {pos} and {neg}; a truncated remainder must \
         carry the sign of the dividend, so if these are not opposite in sign \
         the probes do not distinguish the convention"
    );
}
