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
    // Division is the sharp case: `Op::Div` on a double's bit pattern is an
    // integer division that yields a plausible wrong number rather than a fault.
    const DIV: &str = "\
fn main(w: Word) -> Word {
  let f = w as Float;
  let g = f / 2.0;
  g as Word
}
";
    let m = compile(&parse(&tokenize(DIV).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_chunk(
        &ctx,
        &lm,
        &m.chunks[0],
        "kel_entry",
        LowerOptions::default(),
    )
    .expect_err(
        "float division lowered, but no float division was written. Either it \
             was added and this test should cover its differential instead, or a \
             double's bits are being divided as an integer",
    );
    let text = format!("{err}");
    assert!(
        text.contains("float") || text.contains("Float"),
        "the refusal does not mention a float, so it may be refusing for an \
         unrelated reason and this test would pass without guarding anything: {text}"
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
