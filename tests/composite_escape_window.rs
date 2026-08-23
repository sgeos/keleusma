#![cfg(all(feature = "compile", feature = "verify"))]
//! How long a yielded composite stays readable by the host, MEASURED.
//!
//! # Why this exists
//!
//! `docs/proofs/COMPOSITE_REGION_REUSE.md` on the `v0.3.0` line asks whether a
//! composite built inside a loop body may be given one reused slot. Its §4.0.1
//! rests on a claim this line supplied — that the host may hold a yielded
//! composite, resume, and still read it, so the escape set is bounded by `RESET`
//! rather than by the yield. **That claim was made from reading the code.** A
//! proof resting on an unverified claim is worth less than no proof, so it is
//! measured here.
//!
//! # What is measured, and the bound is tighter than the claim
//!
//! `Op::Reset` is emitted **once per stream cycle**, at the end of the `loop main`
//! body — NOT once per iteration of a `for` inside it. So a `for` loop containing
//! a `yield` runs every one of its iterations inside a single epoch, and a handle
//! taken at the first iteration outlives all of them.
//!
//! That is the useful form of "bounded by `RESET`": the window is **one stream
//! cycle**, which may contain arbitrarily many loop-body iterations.
//!
//! # What this says about slot reuse
//!
//! Under the flat no-reuse model each iteration's composite has its own address,
//! so the held handle keeps reading the iteration it came from. **A backend that
//! gave the site one reused slot would have the same handle read the LATEST
//! iteration's bytes**, successfully, because the address and the epoch are both
//! unchanged. The `Stale` guard fires on `RESET` and cannot see an overwrite in
//! place.
//!
//! # What a passing run does NOT establish
//!
//! Only that `yield` is an escape route, not that it is the only one. The proof's
//! §6.3 asks for an exhaustive enumeration; this test settles one member of it.

use keleusma::bytecode::{GenericValue as Value, Op, StructBody};
use keleusma::flat_value::FlatComposite;
use keleusma::vm::{Vm, VmState};

/// The proof document's §4.1 counterexample, verbatim in shape: a composite built
/// in a `for` body and yielded out of it.
const ESCAPING: &str = "\
struct P { a: Word, b: Word, c: Word }
loop main(t: Word) -> P {
    let xs = [1, 2];
    for x in xs { let _ = yield P { a: x, b: x, c: x }; }
    let _ = yield P { a: 0, b: 0, c: 0 };
    P { a: 9, b: 9, c: 9 }
}
";

fn compile(src: &str) -> keleusma::bytecode::Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile")
}

/// First field of the flat body, which the source sets to the iteration value.
fn first_field(bytes: &[u8]) -> i64 {
    i64::from_le_bytes(bytes[..8].try_into().expect("at least one word"))
}

#[test]
fn reset_is_once_per_stream_cycle_not_once_per_loop_iteration() {
    let module = compile(ESCAPING);
    let chunk = &module.chunks[0];

    let resets = chunk.ops.iter().filter(|o| matches!(o, Op::Reset)).count();
    let yields = chunk.ops.iter().filter(|o| matches!(o, Op::Yield)).count();
    let loops = chunk
        .ops
        .iter()
        .filter(|o| matches!(o, Op::Loop(_)))
        .count();

    assert_eq!(
        resets, 1,
        "the epoch window is one stream cycle only while there is ONE Reset in \
         the body. If this becomes per-iteration the escape window collapses and \
         the proof's §4.0.1 must be restated."
    );
    assert_eq!(loops, 1, "the `for` must still be a real loop in the body");
    assert!(
        yields >= 2,
        "the counterexample needs a yield inside the loop AND one after it, or it \
         does not exercise an escape that crosses iterations"
    );
}

/// The crux for the proof: two iterations' composites are simultaneously
/// readable and DIFFERENT.
///
/// Asserting only that a held handle still reads `1` does not establish this. A
/// runtime that gave the site one reused slot would have both handles read the
/// LATER iteration and `resolve` would succeed for both — same address, same
/// epoch — so the held handle would read `2` while the caller believed it held
/// iteration one. This test is the one that separates those two worlds.
#[test]
fn two_iterations_composites_are_live_together_and_distinct() {
    let module = compile(ESCAPING);
    let arena = keleusma_arena::Arena::with_capacity(1 << 16);
    let mut vm = Vm::new(module, &arena).expect("verify");

    let mut held: Option<FlatComposite> = None;
    let mut state = vm.call(&[Value::Int(0)]).expect("call");

    // First yield: iteration one. Hold it.
    if let VmState::Yielded(Value::Struct(StructBody::Flat(body))) = &state {
        held = Some(body.clone());
    }
    let held = held.expect("the first yield must carry a flat composite");
    assert_eq!(held.resolve(vm.arena()).map(first_field), Ok(1));

    // Second yield: iteration two, while iteration one is still held.
    state = vm.resume(Value::Int(0)).expect("resume");
    let VmState::Yielded(Value::Struct(StructBody::Flat(fresh))) = &state else {
        panic!("the second step must yield the loop's second composite, got {state:?}");
    };

    let fresh_value = fresh.resolve(vm.arena()).map(first_field);
    let held_value = held.resolve(vm.arena()).map(first_field);

    assert_eq!(
        fresh_value,
        Ok(2),
        "the second iteration must build its own value"
    );
    assert_eq!(
        held_value,
        Ok(1),
        "the handle held from iteration one now reads the second iteration. That \
         is the reuse hazard the proof is about, observed in this runtime rather \
         than in the backend's."
    );
    assert_ne!(
        held_value, fresh_value,
        "both handles resolve to the same value, so the two iterations are not \
         separately readable and the no-reuse property does not hold here"
    );
}

#[test]
fn a_yielded_composite_outlives_its_iteration_and_dies_at_reset() {
    let module = compile(ESCAPING);
    let arena = keleusma_arena::Arena::with_capacity(1 << 16);
    let mut vm = Vm::new(module, &arena).expect("verify");

    let mut held: Option<FlatComposite> = None;
    let mut held_epoch_reads = Vec::new();
    let mut saw_reset_at = None;

    let mut state = vm.call(&[Value::Int(0)]).expect("call");
    for step in 0..4 {
        if let VmState::Reset = state {
            saw_reset_at = Some(step);
        }
        if let VmState::Yielded(Value::Struct(StructBody::Flat(body))) = &state
            && held.is_none()
        {
            held = Some(body.clone());
        }
        if let Some(handle) = &held {
            held_epoch_reads.push(handle.resolve(vm.arena()).map(first_field).ok());
        }
        state = vm.resume(Value::Int(0)).expect("resume");
    }

    // The handle is taken at the FIRST loop iteration, whose composite carries 1.
    assert_eq!(
        held_epoch_reads.first().copied().flatten(),
        Some(1),
        "the held handle must start out reading its own iteration"
    );

    let reset_step = saw_reset_at.expect("the stream must reach Reset within four steps");
    assert!(
        reset_step >= 2,
        "Reset arrived at step {reset_step}, before the loop's second iteration \
         could yield; the escape this test is about would not have been possible"
    );

    // THE ESCAPE. Every read before the Reset still yields 1, across an
    // intervening resume and a further loop iteration that built its own
    // composite. This is the property §4.0.1 asserts.
    for (step, read) in held_epoch_reads.iter().enumerate().take(reset_step) {
        assert_eq!(
            *read,
            Some(1),
            "at step {step}, before Reset, the handle taken at iteration 1 no \
             longer reads iteration 1. Either the window closed early or the \
             address was reused."
        );
    }

    // And the window CLOSES. Without this the test would pass on a runtime that
    // never invalidates anything, which is not the claim.
    assert_eq!(
        held_epoch_reads.get(reset_step).copied().flatten(),
        None,
        "the handle must be Stale once Reset has advanced the epoch; a window \
         that never closes is a different and worse property than the one claimed"
    );
}
