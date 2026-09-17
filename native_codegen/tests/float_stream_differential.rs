//! **FLOAT STREAMS: THE REFERENCE RUNS THEM, THIS BACKEND REFUSES THEM.**
//!
//! # How that was established, not assumed
//!
//! The reference RUNS them — a `loop main(t: Float)` yields `Float(3.0)` when
//! called with `Value::Float(3.0)`. But it **rejects `Value::Int`** for that
//! parameter:
//!
//! > `TypeError("function `main` parameter 0 expected Float, got Int")`
//!
//! `common::general_vm_sequence` passes `Value::Int(first)`. **So the general
//! driver cannot carry a float parameter, and every stream differential in this
//! package goes through it.**
//!
//! # Why that mattered more than it looked
//!
//! `lower_module` PANICKED on `loop main(t: Float)` until 2026-09-17 — two sites
//! restoring the resume parameter disagreed about whether it could be a float.
//! **That defect surfaced as a crash, which is the lucky case.** A wrong NUMBER on
//! the same path would have had nothing watching it: the backend lowers these
//! streams, the reference runs them, and no test compared the two.
//!
//! # ⚠ AND THE DIFFERENTIAL IS IMPOSSIBLE, FOR A GOOD REASON
//!
//! This file was written to BE that differential. It cannot be one:
//!
//! > `Yield would consume a float operand, and this arm was not written for one.
//! > Interpreting a double's bit pattern as an integer is a plausible wrong
//! > number rather than a fault, so the operand kind fails closed here`
//!
//! **`Op::Yield` refuses a float operand deliberately**, which is the
//! conservative stance working exactly as designed — its second category, a case
//! that is provable in principle where the analysis is not yet written.
//!
//! So the asymmetry is the finding: **the reference RUNS a float stream, and this
//! backend declines to lower one.** That is a capability gap, recorded rather
//! than repaired, and the refusal is right until someone writes the arm.
//!
//! **My fix on 2026-09-17 is what makes that refusal reachable.** Before it,
//! `lower_module` PANICKED on `loop main(t: Float)` before the refusal could be
//! produced. A panic and a refusal look equally red from a distance; only one of
//! them is a decision.
//!
//! # The driver below is kept, and it is not dead weight
//!
//! It is the differential this path will need the day the `Yield` arm is written.
//! **Keeping it costs nothing and rebuilding it would cost the same care twice**
//! — the signature is configuration-dependent and hand-named, which is how a
//! probe here has already taken a SIGBUS. `the_float_stream_driver_still_builds`
//! keeps it compiling, so it cannot rot unnoticed while unused.
//!
//! # The signature is named by hand, which is how a probe here takes a SIGBUS
//!
//! A float-in, float-out stream lowers with a FLOATING-POINT parameter and
//! return, not an `i64` — so the usual `fn(i64, ptr, ptr, ptr) -> i64` shape is
//! wrong and calling through it is undefined behaviour. **The width also changes
//! with the configuration**: `f64` by default, `f32` under `narrow-float-32`.
//! Both are named below, and the parameter count is asserted before the call
//! rather than after it, because this package has already produced a SIGBUS from
//! a hand-named signature.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

/// The float type this configuration uses for `Float`.
#[cfg(feature = "narrow-float-32")]
type Flt = f32;
#[cfg(not(feature = "narrow-float-32"))]
type Flt = f64;

/// The reference's first yield, or `None` if it does not yield a float.
fn vm_first_yield(src: &str) -> Option<f64> {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut shared: Vec<u8> = Vec::new();
    match vm.call_with_shared(&mut shared, &[Value::Float(2.0)]) {
        Ok(VmState::Yielded(Value::Float(v))) => Some(v),
        _ => None,
    }
}

/// Drive the reference, passing and receiving `Value::Float`.
#[allow(dead_code)]
fn vm_sequence(src: &str, first: Flt, replies: &[Flt]) -> Vec<f64> {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut shared: Vec<u8> = Vec::new();

    let mut out = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Float(f64::from(first))])
        .expect("vm run");
    while out.len() < replies.len() {
        match st {
            VmState::Yielded(Value::Float(v)) => {
                out.push(v);
                let r = f64::from(replies[out.len() - 1]);
                st = vm
                    .resume_with_shared(&mut shared, Value::Float(r))
                    .expect("resume");
            }
            VmState::Reset => {
                let r = f64::from(replies[out.len().saturating_sub(1)]);
                st = vm
                    .resume_with_shared(&mut shared, Value::Float(r))
                    .expect("resume after reset");
            }
            other => panic!("a float stream produced {other:?}"),
        }
    }
    out
}

/// Drive the lowered module through a signature named for THIS configuration.
#[allow(dead_code)]
fn native_sequence(src: &str, first: Flt, replies: &[Flt]) -> Vec<f64> {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        .expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    // **ASSERTED BEFORE THE CALL.** A wrong signature here is undefined behaviour
    // that surfaces as a SIGBUS inside JIT code with no usable stack, which this
    // package has already paid for once.
    assert_eq!(
        f.count_params(),
        u32::from(m.chunks[entry].param_count) + 3,
        "a resumable stream must carry the three trailing pointers; the call below \
         names that signature by hand and cannot detect a change to it"
    );
    assert!(
        f.get_nth_param(0).expect("a parameter").is_float_value(),
        "parameter 0 is not a float value, so the hand-named floating-point \
         signature below would be wrong and calling through it is undefined \
         behaviour"
    );

    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(Flt, *mut u8, *mut u8, *mut u8) -> Flt>(&sym)
    }
    .expect("entry symbol");

    let persistent = required_persistent_capacity_for(&m)
        + keleusma_native::region::persistent_supplement_bytes(&m) as usize;
    let mut privs = vec![0u8; persistent + 64];
    common::install_private_init_bytes(&m, &mut privs);
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(&m).max(8)];
    let region_bytes = keleusma_native::region::host_arena_supplement_bytes(&m) as usize + 4096;
    let mut region = vec![0u8; region_bytes];

    let mut out = Vec::new();
    let mut input = first;
    for &r in replies {
        out.push(f64::from(unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region.as_mut_ptr(),
            )
        }));
        input = r;
    }
    out
}

const SUBJECTS: &[(&str, &str)] = &[
    (
        "float in, float out",
        "loop main(t: Float) -> Float { let r = yield t; yield (r + t) }",
    ),
    (
        "a negation carried across a yield",
        "loop main(t: Float) -> Float { let n: Float = -t; let r = yield n; yield (r + n) }",
    ),
    (
        "a product across a yield",
        "loop main(t: Float) -> Float { let p: Float = t * t; let r = yield p; yield (r - p) }",
    ),
];

fn replies(n: usize) -> Vec<Flt> {
    (0..n).map(|i| ((i % 5) as Flt) + 1.0).collect()
}

/// **THE ASYMMETRY, PINNED FROM BOTH SIDES.**
///
/// The reference runs a float stream; this backend refuses one. **Both halves are
/// asserted**, because a register stale in only one direction is a bias: if the
/// reference stopped running them the refusal would look justified for the wrong
/// reason, and if the backend started lowering them this test must fail so the
/// differential below can be switched on.
#[test]
fn the_reference_runs_a_float_stream_and_this_backend_refuses_one() {
    for (label, src) in SUBJECTS {
        // The reference side: it really does run these.
        let first = vm_first_yield(src);
        assert!(
            first.is_some(),
            "`{label}`: the reference no longer yields from a float stream. Then \
             the refusal below is no longer a capability gap and this file needs \
             re-reading from the top."
        );

        // This backend's side: a refusal, naming the operand kind.
        let refusals = keleusma_native::module_refusals(
            &common::build(src),
            keleusma_native::LowerOptions::default(),
        );
        let Some((_, e)) = refusals.first() else {
            panic!(
                "`{label}` now LOWERS. That is the gap closing, and it is good \
                 news — but the differential in this file must then be switched \
                 on, because a lowered float stream with nothing comparing it is \
                 exactly the state that let a panic sit on this path unnoticed."
            );
        };
        let msg = format!("{e:?}");
        assert!(
            msg.contains("Yield") && msg.contains("float"),
            "`{label}` is refused for a different reason now: {msg}. The recorded \
             gap is the `Yield` arm not being written for a float operand; a \
             different refusal is a different fact."
        );
    }
}

/// **The driver must keep compiling while it waits.** An unused differential that
/// stops building is one that will be rewritten rather than switched on.
#[test]
fn the_float_stream_driver_still_builds() {
    let _ = (
        vm_sequence as fn(&str, Flt, &[Flt]) -> Vec<f64>,
        native_sequence as fn(&str, Flt, &[Flt]) -> Vec<f64>,
    );
    assert_eq!(replies(3).len(), 3, "the reply generator is intact");
}
