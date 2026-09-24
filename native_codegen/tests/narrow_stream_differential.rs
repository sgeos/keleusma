//! **A `Byte` STREAM AND A `bool` STREAM LOWER, AND UNTIL NOW NOTHING DROVE ONE.**
//!
//! # The gap, found by asking what lowers with nothing comparing it
//!
//! `loop main(t: Byte) -> Byte` and `loop main(t: bool) -> bool` both lower with
//! **zero refusals**, and before this file **no test in the package declared
//! either shape.** Every stream subject and driver used `Word`, `Float` or `Fixed`.
//!
//! Two neighbouring shapes bound the surface and are **sound refusals, not gaps**:
//! a stream with two parameters and one with zero are both refused, because
//! *"resume defines slot 0 only, so any further parameter would have no defined
//! value on re-entry."*
//!
//! # Why the one-byte types are the interesting ones
//!
//! Every driven stream so far carried an **eight-byte** value. `width_of_tag` gives
//! `Byte` and `bool` a `Scalar(1)`, and `Byte` arithmetic is
//! promote-operate-truncate MASKED — so a yielded `Byte` must come back masked to
//! eight bits. **A wrong mask on the resume path is a wrong NUMBER, not a fault.**
//!
//! **The ABI is already compatible, measured rather than assumed**: `Byte`, `bool`
//! and `Word` all lower to `(i64, ptr, ptr, ptr) -> i64`. That was the SIGBUS
//! hazard this package has paid for once, and it is absent here — which is what
//! makes this differential cheap.
//!
//! # ⚠ THE REFERENCE RUNS FIRST, AND THAT IS A SAFETY PROPERTY
//!
//! **`Byte` arithmetic is CHECKED.** The reference errors on overflow and the
//! native side executes `llvm.trap`, which **kills the process with SIGTRAP** — not
//! a failed comparison, a dead test binary. `generated_expressions.rs` bounds its
//! byte leaves at 3 rather than 9 for exactly this reason.
//!
//! So every subject is driven through the reference first, and the backend only if
//! the reference completed the sequence. [`the_byte_bound_is_asserted`] checks the
//! arithmetic that keeps every value inside a byte across all ticks, because a
//! stream feeds back: the reply becomes the parameter after a reset.
//!
//! # Two perturbations, and the second completes a three-way picture
//!
//! | perturbation | outcome |
//! |---|---|
//! | the reset leg's resume restore drops the value | **DETECTED** on the first byte subject — so this covers the STREAM path, not byte arithmetic, which the operator matrix already covers |
//! | the resumed value's declared WIDTH dropped to `Unknown` | **REFUSED** — so for a `Byte` the width IS load-bearing |
//!
//! The second is the interesting one, because **the same corruption is harmless
//! for a `Fixed`**. Across the session the resumed value's two attributes divide up
//! by type:
//!
//! | type | what the resumed value needs |
//! |---|---|
//! | `Byte` | its **WIDTH** — byte arithmetic needs a matched one-byte pair |
//! | `Float` | its **KIND** — the machine type cannot distinguish a float's bits |
//! | `Fixed` | **neither** — the fraction count travels in `Op::FixedMul` |
//!
//! Each of those three was measured by corrupting the attribute and observing what
//! happened, not inferred from the other two.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

/// Ticks driven per subject.
const TICKS: usize = 6;
/// The largest value any operand or reply takes.
const VALUE_MAX: u32 = 3;
/// What a byte holds.
const BYTE_LIMIT: u32 = 256;

/// Which one-byte type a subject uses, because the `Value` constructor differs.
#[derive(Clone, Copy)]
enum Narrow {
    Byte,
    Bool,
}

impl Narrow {
    fn value(self, n: i64) -> Value {
        match self {
            // **`Value::Byte`, NOT `Value::Int`.** The runtime type-checks the
            // entry parameter; the float stream file records the same rejection for
            // an `Int` passed to a `Float` parameter.
            Narrow::Byte => Value::Byte(n as u8),
            Narrow::Bool => Value::Bool(n != 0),
        }
    }
    /// The raw `i64` the NATIVE side must receive for the same logical input.
    ///
    /// # ⚠ THIS EXISTS BECAUSE ITS ABSENCE PRODUCED A FALSE DIVERGENCE
    ///
    /// The first run of this file reported the backend yielding **2 and 3 from a
    /// `bool` stream**, where a bool holds only 0 or 1. **It was this harness.**
    /// The reference received `Value::Bool(r != 0)` — normalised to `true` — while
    /// the native side received the raw reply, 2 and 3. **The two sides were given
    /// different inputs**, so the comparison said nothing about the emitter.
    ///
    /// That is the failure class this line keeps finding in its own instruments,
    /// and the differential caught it on the first run rather than after it had
    /// been believed. A `bool` must reach both sides normalised.
    fn raw(self, n: i64) -> i64 {
        match self {
            Narrow::Byte => n,
            Narrow::Bool => i64::from(n != 0),
        }
    }
    fn of(self, v: &Value) -> Option<i64> {
        match (self, v) {
            (Narrow::Byte, Value::Byte(b)) => Some(i64::from(*b)),
            (Narrow::Bool, Value::Bool(b)) => Some(i64::from(*b)),
            _ => None,
        }
    }
}

/// The reference's yielded sequence, or `None` if it declines the sequence.
///
/// **Fallible on purpose**: a trapping input must be reported rather than reaching
/// the backend, where the same overflow is a `SIGTRAP` that kills the binary.
fn vm_sequence(src: &str, kind: Narrow, first: i64, reps: &[i64]) -> Option<Vec<i64>> {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).ok()? + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).ok()?;
    let mut vm = Vm::new(m.clone(), &arena).ok()?;
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(&m)];

    let mut out = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[kind.value(first)])
        .ok()?;
    while out.len() < reps.len() {
        match st {
            VmState::Yielded(ref v) => {
                out.push(kind.of(v)?);
                let r = reps[out.len() - 1];
                st = vm.resume_with_shared(&mut shared, kind.value(r)).ok()?;
            }
            VmState::Reset => {
                let r = reps[out.len().saturating_sub(1)];
                st = vm.resume_with_shared(&mut shared, kind.value(r)).ok()?;
            }
            _ => return None,
        }
    }
    Some(out)
}

/// The backend's yielded sequence.
///
/// **The signature is the same one every other stream driver uses**, because a
/// `Byte`, a `bool` and a `Word` stream all lower to `(i64, ptr, ptr, ptr) -> i64`.
/// The parameter count and kind are asserted before the call regardless.
fn native_sequence(src: &str, kind: Narrow, first: i64, reps: &[i64]) -> Vec<i64> {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        .expect("lower module");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    assert_eq!(
        f.count_params(),
        u32::from(m.chunks[entry].param_count) + 3,
        "a resumable stream must carry the three trailing pointers"
    );
    assert!(
        f.get_nth_param(0).expect("a parameter").is_int_value(),
        "parameter 0 is not an integer value, so the signature below is wrong and \
         calling through it is undefined behaviour"
    );
    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let persistent = required_persistent_capacity_for(&m)
        + keleusma_native::region::persistent_supplement_bytes(&m) as usize;
    let mut privs = vec![0u8; persistent + 64];
    common::install_private_init_bytes(&m, &mut privs);
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(&m)];
    let region_bytes = keleusma_native::region::host_arena_supplement_bytes(&m) as usize + 4096;
    let mut region = vec![0u8; region_bytes];

    let mut out = Vec::new();
    // **NORMALISED THROUGH `Narrow::raw`**, so this side receives the same logical
    // value the reference does. See that method for why.
    let mut input = kind.raw(first);
    for &r in reps {
        out.push(unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region.as_mut_ptr(),
            )
        });
        input = kind.raw(r);
    }
    out
}

const SUBJECTS: &[(&str, Narrow, &str)] = &[
    (
        "a byte reply added to the parameter",
        Narrow::Byte,
        "loop main(t: Byte) -> Byte { let r = yield t; yield (r + t) }",
    ),
    (
        "a byte carried across a yield in a local",
        Narrow::Byte,
        "loop main(t: Byte) -> Byte { let q: Byte = (t + (1 as Byte)); let r = yield t; yield (q + r) }",
    ),
    (
        "a byte left on the operand stack across a yield",
        Narrow::Byte,
        "loop main(t: Byte) -> Byte { let x: Byte = ((t + (1 as Byte)) + (yield t)); yield x }",
    ),
    (
        "a bool reply conjoined with the parameter",
        Narrow::Bool,
        "loop main(t: bool) -> bool { let r = yield t; yield (r and t) }",
    ),
    (
        "a bool reply disjoined, so both polarities occur",
        Narrow::Bool,
        "loop main(t: bool) -> bool { let r = yield t; yield (r or t) }",
    ),
];

fn replies(n: usize) -> Vec<i64> {
    (0..n)
        .map(|i| ((i as i64) % (VALUE_MAX as i64)) + 1)
        .collect()
}

/// **THE DIFFERENTIAL, SEQUENCE BY SEQUENCE.**
#[test]
fn a_narrow_stream_agrees_with_the_reference_value_for_value() {
    let reps = replies(TICKS);

    for (label, kind, src) in SUBJECTS {
        assert!(
            keleusma_native::module_refusals(
                &common::build(src),
                keleusma_native::LowerOptions::default(),
            )
            .is_empty(),
            "`{label}` is refused; the differential cannot run"
        );

        // **THE REFERENCE FIRST.** A trapping input would reach `llvm.trap` on the
        // native side and kill this binary with no usable result.
        let Some(vm) = vm_sequence(src, *kind, 1, &reps) else {
            panic!(
                "`{label}`: the reference declined the tick sequence. Driving the \
                 backend now would risk a SIGTRAP, so this is reported rather than \
                 pressed on. Tighten the subject's values."
            );
        };
        let native = native_sequence(src, *kind, 1, &reps);

        for v in vm.iter().chain(native.iter()) {
            assert!(
                (0..BYTE_LIMIT as i64).contains(v),
                "`{label}` produced {v}, outside a byte. Byte arithmetic is CHECKED, \
                 so the reference would have errored; a value out of range here means \
                 the comparison is not measuring what it claims."
            );
        }

        assert_eq!(
            vm, native,
            "DIVERGENCE on `{label}`:\n  {src}\n  reference = {vm:?}\n  \
             native    = {native:?}\n\nThis establishes that the two \
             implementations disagree, NOT which of them is right."
        );
        assert_eq!(
            vm.len(),
            TICKS,
            "`{label}` yielded {} values, not {TICKS}; a stream that did not \
             suspend is not exercising the resume path",
            vm.len()
        );
    }
}

/// **THE NEIGHBOURING SHAPES ARE REFUSED, AND THAT BOUNDS THE SURFACE.**
///
/// A stream with two parameters and one with zero are both refused for a stated
/// reason. **Pinned so a change announces itself**: if either began to lower, the
/// resume path would define slot 0 only and the other parameters would hold
/// whatever the previous iteration left — a wrong number rather than a fault.
#[test]
fn a_stream_with_other_than_one_parameter_is_refused() {
    for (what, src) in [
        (
            "two parameters",
            "loop main(a: Word, b: Word) -> Word { let r = yield a; yield (r + b) }",
        ),
        (
            "zero parameters",
            "loop main() -> Word { let r = yield 1; yield (r + 1) }",
        ),
    ] {
        let m = common::build(src);
        let refusals =
            keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
        let Some((_, e)) = refusals.first() else {
            panic!(
                "a stream with {what} now LOWERS. Resume defines slot 0 only, so any \
                 further parameter would hold whatever the previous iteration left — \
                 a wrong number rather than a fault. If this is deliberate, the \
                 resume path must define every parameter and this test must be \
                 replaced by one that checks it does."
            );
        };
        let msg = format!("{e:?}");
        assert!(
            msg.contains("Stream") && msg.contains("slot 0 only"),
            "a stream with {what} is refused for a different reason now: {msg}"
        );
    }
}

/// **THE BYTE BOUND IS ARITHMETIC, AND THE ARITHMETIC IS CHECKED.**
///
/// `Byte` arithmetic is CHECKED, so an overflow is a trap that kills the binary
/// rather than a wrong number a comparison would catch. A stream feeds back — the
/// reply becomes the parameter after a reset — so the bound must hold across every
/// tick, not just within one expression.
#[test]
fn the_byte_bound_is_asserted() {
    // Worst case per tick: the parameter, plus a literal, plus a reply.
    let worst = VALUE_MAX + 1 + VALUE_MAX;
    assert!(
        worst < BYTE_LIMIT,
        "the worst case per tick is {worst}, at or above {BYTE_LIMIT}. Byte \
         arithmetic is CHECKED, so this would trap and kill the test binary rather \
         than fail a comparison."
    );

    // **NON-VACUITY.** A bound nothing could exceed guards nothing: the word
    // generator's own byte leaf bound of 9, compounded, must break it.
    let reckless = 9u32.pow(3);
    assert!(
        reckless >= BYTE_LIMIT,
        "cubing the word generator's byte leaf bound no longer exceeds a byte, so \
         this bound is not discriminating anything"
    );
}
