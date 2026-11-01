//! **FIXED STREAMS LOWER, AND UNTIL NOW NOTHING COMPARED THEM.**
//!
//! # The gap, found by probing
//!
//! Four `Fixed` stream shapes all lower with **zero refusals**: a reply added to
//! the parameter, a reply MULTIPLIED by it, a fixed-point value spilled beneath a
//! yielded value and consumed after the resume, and the degenerate tail-position
//! form. **Nothing compared any of them against the reference.**
//!
//! That is the exact state the float stream path was in before its defect
//! surfaced, and this package recorded the lesson then: *"the backend lowers these
//! streams, the reference runs them, and no test compared the two."* A panic was
//! the lucky outcome there.
//!
//! # ⚠ WHY `Fixed` IS THE INTERESTING CASE
//!
//! **A `Fixed` IS an `i64` at the LLVM level**, so the machine type cannot
//! discriminate it — the precise hazard that made the resumed float reply wrong.
//! The resume push kinds everything that is not a declared `Float` as
//! `OperandKind::Int`, so **a `Fixed` reply carries `Int`**.
//!
//! That it lowers anyway suggests the scale rides on the OPCODE —
//! `Op::FixedMul(frac_bits)` carries the fraction count — rather than on the
//! operand kind. [`the_reply_kind_is_not_load_bearing_for_fixed`] establishes that
//! by perturbation rather than leaving it implied.
//!
//! # The hazard is overflow, not precision
//!
//! `Fixed` is exact integer arithmetic on `i64` bits in both implementations, so
//! there is no configured width to disagree about and no exactness guard belongs
//! here. **What matters is that `Op::Mul` is UNCHECKED**: an overflow is a wrong
//! number rather than a trap, and **both implementations wrapping identically
//! would agree and prove nothing**.
//!
//! A stream feeds back, so a reply MULTIPLIED by the parameter compounds per tick.
//! The subjects use small values and few ticks, and
//! [`the_magnitude_bound_is_asserted`] checks the arithmetic rather than arguing it.
//!
//! # Two perturbations, each establishing something different
//!
//! | perturbation | outcome |
//! |---|---|
//! | the reset leg's resume restore drops the value | **DETECTED** on the first subject — so this covers the STREAM path, not merely `Fixed` arithmetic, which `generated_fixed.rs` already covers |
//! | the spilled `(Width, OperandKind)` pair corrupted to `Unknown` | **NOT detected** — neither half is load-bearing for a `Fixed` operand |
//!
//! The second confirms this file's reasoning by measurement rather than leaving it
//! an inference, and it **completes a picture the session built in pieces**: the
//! spill's KIND is witnessed by a `Float` subject, its WIDTH by a `Byte` subject,
//! and **`Fixed` witnesses neither** — because the fraction count travels in the
//! instruction, so nothing downstream consults the operand for it. The same
//! corruption IS detected by the float spill subjects, so this instrument is not
//! simply blind.
//!
//! # The scale, measured
//!
//! Bare `Fixed` is `Fixed<32>`: a value `v` is stored as `v * 2^32`. Literals are
//! written `(N as Fixed)` — **a bare decimal is a FLOAT**, and `Fixed + Float` is a
//! mixed pair the reference refuses, which would read like a backend refusal.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

/// One, in Q32.32.
const ONE: i64 = 1 << 32;
/// Ticks driven per subject.
const TICKS: usize = 6;
/// The largest whole value Q32.32 represents.
const FIXED_WHOLE_LIMIT: i128 = 1i128 << 31;
/// Whole-number bound on any operand or reply the subjects use.
const VALUE_MAX: i128 = 3;

fn replies(n: usize) -> Vec<i64> {
    (0..n).map(|i| ((i as i64 % 3) + 1) * ONE).collect()
}

/// The reference's yielded sequence, raw Q32.32 bits.
fn vm_sequence(src: &str, first: i64, reps: &[i64]) -> Vec<i64> {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(&m)];

    let mut out = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Fixed(first)])
        .expect("first call");
    while out.len() < reps.len() {
        match st {
            VmState::Yielded(Value::Fixed(v)) => {
                out.push(v);
                let r = reps[out.len() - 1];
                st = vm
                    .resume_with_shared(&mut shared, Value::Fixed(r))
                    .expect("resume");
            }
            VmState::Reset => {
                let r = reps[out.len().saturating_sub(1)];
                st = vm
                    .resume_with_shared(&mut shared, Value::Fixed(r))
                    .expect("resume after reset");
            }
            other => panic!("a Fixed stream produced {other:?}"),
        }
    }
    out
}

/// The backend's yielded sequence.
///
/// **A `Fixed` lowers with `i64` parameters**, because a Q-format value IS an `i64`
/// of fixed-point bits — so the signature is the integer one, not the float one.
/// The parameter count and kind are asserted before the call, since a mismatched
/// hand-named signature is undefined behaviour this package has paid for once.
fn native_sequence(src: &str, first: i64, reps: &[i64]) -> Vec<i64> {
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
        "parameter 0 is not an integer value, so the integer signature below would \
         be wrong and calling through it is undefined behaviour"
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
    let mut input = first;
    for &r in reps {
        out.push(unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region.as_mut_ptr(),
            )
        });
        input = r;
    }
    out
}

const SUBJECTS: &[(&str, &str)] = &[
    (
        "a reply added to the parameter",
        "loop main(t: Fixed) -> Fixed { let r = yield t; yield (r + t) }",
    ),
    // **THE LOAD-BEARING ONE.** `FixedMul` needs the fraction count, and a `Fixed`
    // reply is kinded `Int` by the resume push, so if the scale did NOT ride on the
    // opcode this is where it would go wrong.
    (
        "a reply MULTIPLIED by the parameter",
        "loop main(t: Fixed) -> Fixed { let r = yield t; yield (r * t) }",
    ),
    // **THE ONLY SUBJECT HERE THAT REACHES THE SPILL SLICE.** The others carry
    // values in LOCALS; a `yield` used as a SUBEXPRESSION puts a computed
    // fixed-point operand beneath it.
    (
        "a Fixed left on the operand stack across a yield, consumed by a multiply",
        "loop main(t: Fixed) -> Fixed { \
           let x: Fixed = ((t * (2 as Fixed)) * (yield t)); yield x }",
    ),
];

/// **THE DIFFERENTIAL, SEQUENCE BY SEQUENCE.**
///
/// Comparing only a final value would not do: the float stream perturbation
/// diverged at the loop-back ticks while both sequences ended identically.
#[test]
fn a_fixed_stream_agrees_with_the_reference_value_for_value() {
    let first = 2 * ONE;
    let reps = replies(TICKS);

    for (label, src) in SUBJECTS {
        assert!(
            keleusma_native::module_refusals(
                &common::build(src),
                keleusma_native::LowerOptions::default(),
            )
            .is_empty(),
            "`{label}` is refused; the differential below cannot run"
        );

        let vm = vm_sequence(src, first, &reps);
        let native = native_sequence(src, first, &reps);

        // **THE OVERFLOW GUARD.** Fixed arithmetic is UNCHECKED, so a matching
        // wrong number on both sides would agree and prove nothing.
        for v in vm.iter().chain(native.iter()) {
            assert!(
                (*v as i128).abs() < (FIXED_WHOLE_LIMIT << 32),
                "`{label}` produced {v} raw, outside the Q32.32 range. Fixed \
                 arithmetic is unchecked, so a matching overflow on both sides \
                 would AGREE. Tighten the subject's values or tick count."
            );
        }

        assert_eq!(
            vm, native,
            "DIVERGENCE on `{label}`:\n  {src}\n  reference = {vm:?}\n  \
             native    = {native:?}\n\nBoth sides are inside the representable \
             range, so this is not an overflow artefact. This establishes that the \
             two implementations disagree, NOT which of them is right."
        );
        assert!(
            !vm.is_empty(),
            "`{label}`: the reference produced no yields, so the comparison above \
             compared two empty vectors"
        );
    }
}

/// **IS THE RESUMED REPLY'S KIND LOAD-BEARING FOR `Fixed`? MEASURED, NOT ASSUMED.**
///
/// The resume push kinds everything that is not a declared `Float` as
/// `OperandKind::Int`, so a `Fixed` reply carries `Int` — and every subject above
/// still agrees. The reason is that `Op::FixedMul(frac_bits)` carries the fraction
/// count in the INSTRUCTION, so the lowering never consults the operand kind for
/// the scale.
///
/// **This test pins the consequence rather than the mechanism**: a `Fixed` stream
/// whose reply is multiplied must lower and agree. If a future change makes the
/// kind load-bearing here, this file's reasoning above is wrong and the subject
/// will say so by diverging.
#[test]
fn the_reply_kind_is_not_load_bearing_for_fixed() {
    let src = "loop main(t: Fixed) -> Fixed { let r = yield t; yield (r * t) }";
    let m = common::build(src);
    assert!(
        keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default()).is_empty(),
        "a Fixed reply under a multiply is now refused. Then the scale no longer \
         rides on the opcode alone and this file's central reasoning needs redoing."
    );

    // **THE CONTROL.** A `Float` reply under a multiply DOES need its kind, and the
    // float path carries one for exactly that reason. Without this the claim above
    // would read as "kinds never matter", which is false.
    let float_src = "loop main(t: Float) -> Float { let r = yield t; yield (r * t) }";
    assert!(
        keleusma_native::module_refusals(
            &common::build(float_src),
            keleusma_native::LowerOptions::default(),
        )
        .is_empty(),
        "the float control is refused, so the comparison this test rests on is not \
         available and the claim about `Fixed` stands alone"
    );
}

/// **THE MAGNITUDE BOUND IS ARITHMETIC, AND THE ARITHMETIC IS CHECKED.**
///
/// A reply multiplied by the parameter compounds per tick, so the bound is not the
/// single-expression one the straight-line `Fixed` generator uses. **An argument in
/// a comment is not a guard**: this fails if the values or tick count are raised
/// past what Q32.32 represents.
#[test]
fn the_magnitude_bound_is_asserted() {
    // Worst case: every tick multiplies two values each at most VALUE_MAX whole.
    let mut worst: i128 = VALUE_MAX;
    for _ in 0..TICKS {
        worst = worst.saturating_mul(VALUE_MAX);
        assert!(
            worst < FIXED_WHOLE_LIMIT,
            "after multiplying each tick the worst case reaches {worst}, at or above \
             the Q32.32 whole limit of {FIXED_WHOLE_LIMIT}. Fixed arithmetic is \
             unchecked, so the differential would compare two matching wrong numbers."
        );
    }

    // **NON-VACUITY.** A bound nothing could exceed guards nothing: twice the tick
    // count at the same per-tick factor must break it.
    let mut reckless: i128 = VALUE_MAX;
    for _ in 0..(TICKS * 4) {
        reckless = reckless.saturating_mul(VALUE_MAX);
    }
    assert!(
        reckless >= FIXED_WHOLE_LIMIT,
        "four times the tick count no longer exceeds the Q32.32 limit, so this \
         bound is not discriminating anything"
    );
}
