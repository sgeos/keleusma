//! **FLOAT STREAMS: BOTH IMPLEMENTATIONS RUN THEM, AND THIS COMPARES THEM.**
//!
//! # What this file used to say, and why the change is the point
//!
//! Until 2026-09-18 this file recorded an ASYMMETRY — the reference ran float
//! streams and this backend refused them — and carried two drivers unused,
//! waiting for the day the arm was written. **That day is this one.** `Op::Yield`
//! is float-aware for a general stream now, and the differential is switched on.
//!
//! The history is kept because the refusal was RIGHT while it stood, and because
//! two defects on this path were found only by writing the instrument before the
//! capability:
//!
//! * `lower_module` PANICKED on `loop main(t: Float)` until 2026-09-17 — two
//!   sites restoring the resume parameter disagreed about whether it could be a
//!   float. **A panic is the lucky case.** A wrong NUMBER on the same path had
//!   nothing watching it.
//! * The reply was pushed with a marking that made it an integer-kinded operand,
//!   so every float operation on it refused. Admitting the yield without fixing
//!   that would have delivered a capability that does not work.
//!
//! # ⚠ WHAT THE MISSING KIND ACTUALLY DID, MEASURED RATHER THAN ASSUMED
//!
//! `FLOAT_YIELD_SCOPE.md` predicted it would produce *"a module that lowers and
//! computes on a float's bit pattern as an integer — precisely the silent wrong
//! number the refusal prevents."* **That was measured after the work and it is
//! false.** With the kind reverted, a subject that operates on the reply REFUSES
//! loudly, and a subject that yields it straight back lowers cleanly and still
//! agrees value for value. **No subject produced a wrong number.**
//!
//! The scope document is corrected in place. The distinction matters because a
//! hazard overstated in the direction that makes the work sound necessary is the
//! direction to distrust.
//!
//! # The degenerate form is deliberately still refused
//!
//! A yield in TAIL POSITION lowers to the host callback `kel_yield(i64) -> i64`.
//! Carrying a float through that is a HOST-FACING ABI change, so the float-aware
//! admission is conditional on a general stream, and a test below pins the
//! consequence with a `Word` control beside it.
//!
//! # The f64-versus-f32 constraint, which is a harness fact and not a backend one
//!
//! `keleusma::vm::Vm` is `GenericVm<.., f64>` — **the reference runs at eight
//! bytes in BOTH configurations** — while this backend lowers at the configured
//! width. Every subject value, reply and result is therefore checked to be exact
//! in four bytes, OPERANDS INCLUDED. A result-only version of that check let a
//! phantom divergence through in `scalar_operator_matrix.rs` earlier the same day.
//!
//! # The signature is named by hand, which is how a probe here takes a SIGBUS
//!
//! A float-in, float-out stream lowers with a FLOATING-POINT parameter and
//! return, not an `i64` — so the usual `fn(i64, ptr, ptr, ptr) -> i64` shape is
//! wrong and calling through it is undefined behaviour. **The width also changes
//! with the configuration**: `f64` by default, `f32` under `narrow-float-32`.
//! Both are named below, and the parameter count and kind are asserted before the
//! call rather than after it.

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

/// Widen this configuration's float to `f64` for comparison.
///
/// **Split by configuration rather than written once**, because `f64::from` is
/// required at four bytes and is a `useless_conversion` lint error at eight. A
/// single spelling cannot be clean in both, and the gate runs both — which is
/// how this was found: clippy failed under default features while the narrow
/// configuration passed, with all three suite phases green in each.
#[cfg(feature = "narrow-float-32")]
fn wide(x: Flt) -> f64 {
    f64::from(x)
}
#[cfg(not(feature = "narrow-float-32"))]
fn wide(x: Flt) -> f64 {
    x
}

/// Drive the reference, passing and receiving `Value::Float`.
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
        .call_with_shared(&mut shared, &[Value::Float(wide(first))])
        .expect("vm run");
    while out.len() < replies.len() {
        match st {
            VmState::Yielded(Value::Float(v)) => {
                out.push(v);
                let r = wide(replies[out.len() - 1]);
                st = vm
                    .resume_with_shared(&mut shared, Value::Float(r))
                    .expect("resume");
            }
            VmState::Reset => {
                let r = wide(replies[out.len().saturating_sub(1)]);
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
        out.push(wide(unsafe {
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
    // **THE PASS-THROUGH REPLY — A DISTINCT SHAPE, AND IT DISCRIMINATES NOTHING.**
    //
    // It was added believing it was the case where a wrong reply kind would
    // produce a silent wrong NUMBER, since the other three operate on the reply
    // and would hit the mixed-kind guard instead. **Both halves of that belief
    // were then measured and the second is false.**
    //
    // With the reply's kind reverted to `Int`:
    //   * the other three REFUSE, loudly — *"Add with operand kinds Int and
    //     Float"* — so they discriminate the fix, by refusal and not by value;
    //   * this one LOWERS CLEANLY **and still agrees value for value**, because a
    //     reply that is only moved is a pure bit copy and no arm consults its kind.
    //
    // **So no subject here produces a wrong number without the fix.** The kind is
    // required for CAPABILITY, not for correctness: without it every float
    // operation on a reply refuses and the arm delivers nothing. That is a weaker
    // and more accurate claim than the one `FLOAT_YIELD_SCOPE.md` recorded, and
    // the scope document is corrected rather than left standing.
    //
    // The subject is KEPT because it covers a shape the others do not — a value
    // crossing the suspension untouched, exercising the spill and restore alone —
    // not because it guards the kind.
    (
        "a reply yielded straight back, never operated on",
        "loop main(t: Float) -> Float { let r = yield t; yield r }",
    ),
    // **THE ONLY SUBJECTS HERE THAT REACH THE SPILL SLICE, AND THAT IS THE POINT.**
    //
    // Every subject above — and all 240 generated streams in
    // `generated_float_streams.rs` — carries its values in LOCALS. At each
    // `Op::Yield` the operand stack holds only the yielded value, so `deep` is
    // zero and the spill loop never runs. **`local_widths` and `spilled` are
    // different tables and different mechanisms**, and `stream_width_survival.rs`
    // measured that distinction the hard way: corrupting the restored spill
    // widths left five subjects passing.
    //
    // A `yield` used as a SUBEXPRESSION puts a computed operand underneath it.
    // `(t * 2.0)` is pushed, then `yield t` suspends with it on the stack, so the
    // pair `(Width, OperandKind)` must survive the suspension and come back.
    //
    // **Before these, the package's ONLY spill witness was a `Byte`.**
    // `OperandKind::Float` crossing a suspension had none at all, and
    // `Op::Yield` only became float-aware on 2026-09-18.
    //
    // # ⚠ WHAT THESE WITNESS, MEASURED IN BOTH DIRECTIONS
    //
    // The spill carries a `(Width, OperandKind)` PAIR, and it would be easy to
    // write that these subjects prove the pair survives. **They do not.** Both
    // halves were corrupted separately:
    //
    // | corruption | outcome |
    // |---|---|
    // | kind dropped to `Unknown` | **DETECTED** — refused, *"Add with operand kinds Unknown and Float"* |
    // | width dropped to `Unknown`, kind kept | **NOT DETECTED** — both configurations still agree |
    //
    // So these witness the **KIND** crossing the suspension. The `Byte` subject
    // in `stream_width_survival.rs` witnesses the **WIDTH**, because a byte add
    // needs a matched one-byte pair and a float add does not. The two are
    // complementary and neither covers the other.
    //
    // The detection is by REFUSAL rather than by a wrong value, which is the
    // same shape as the reply-kind case above: the mixed-kind guard fails closed
    // instead of reinterpreting bits. That is worth stating rather than
    // glossing, because a differential that only ever reports refusals is not
    // testing the numbers it appears to be testing.
    (
        "a float product left on the operand stack across a yield",
        "loop main(t: Float) -> Float { let x: Float = ((t * 2.0) + (yield t)); yield x }",
    ),
    (
        "a float sum left on the operand stack, consumed by subtraction",
        "loop main(t: Float) -> Float { let x: Float = ((t + 1.0) - (yield t)); yield x }",
    ),
    // **THREE SUBJECTS AIMED AT THE SPILLED WIDTH — AND THEY MISS IT TOO.**
    //
    // They were written to catch a lost WIDTH, the half the two subjects above are
    // blind to, on the reasoning that the composite arm packs each operand at the
    // operand's own width. **Measured: they do not.** With the spilled width
    // dropped to `Unknown` and the kind kept, all three still agree with the
    // reference.
    //
    // The chain breaks before the composite. The spilled operand is consumed by
    // the add at the resume point, its result goes into a LOCAL, and the composite
    // is built from that local — whose width comes from `local_widths`, not from
    // the spill. So the spilled width is never load-bearing here.
    //
    // **They are kept as coverage and labelled for what they are.** A spilled
    // float reaching a composite, a mixed composite and an array across a
    // suspension is worth driving. It is not a width witness, and calling it one
    // would be the failure this file has already corrected once.
    //
    // **No expressible subject found so far makes the float spill width
    // observable.** That is a real result rather than a shortfall: it bounds the
    // hazard — a width lost on this path has no consumer that would notice — but
    // it does not close it. See `the_float_reply_cannot_be_packed_into_a_composite`
    // below for what was found while looking.
    (
        "a spilled float packed into an all-float struct",
        "struct P { a: Float, b: Float }\nloop main(t: Float) -> Float { \
           let x: Float = ((t * 2.0) + (yield t)); let p = P { a: x, b: 1.0 }; yield p.a }",
    ),
    (
        "a spilled float packed beside a Word",
        "struct Q { a: Float, b: Word }\nloop main(t: Float) -> Float { \
           let x: Float = ((t * 2.0) + (yield t)); let q = Q { a: x, b: 1 }; yield q.a }",
    ),
    (
        "a spilled float placed in an array",
        "loop main(t: Float) -> Float { \
           let x: Float = ((t * 2.0) + (yield t)); let arr: [Float; 2] = [x, 1.0]; yield arr[0] }",
    ),
];

fn replies(n: usize) -> Vec<Flt> {
    (0..n).map(|i| ((i % 5) as Flt) + 1.0).collect()
}

/// **EVERY SUBJECT AND REPLY IS EXACT IN FOUR BYTES, AND THAT IS CHECKED.**
///
/// `keleusma::vm::Vm` is `GenericVm<.., f64>` — the reference runs at EIGHT bytes
/// in BOTH configurations, while this backend lowers at the configured width.
/// Under `narrow-float-32` the comparison is therefore f64-against-f32, and a
/// value needing more than a four-byte mantissa would differ **legitimately**,
/// reporting rounding as a divergence.
///
/// **The operands are checked, not only the results**, and that distinction was
/// paid for: in `scalar_operator_matrix.rs` a result-only version of this check
/// let a phantom `Disagree` through, because an inexact OPERAND means the two
/// implementations are computing different problems even when both results
/// happen to be representable.
fn assert_four_byte_exact(label: &str, side: &str, values: &[f64]) {
    for v in values {
        assert!(
            f64::from(*v as f32) == *v,
            "`{label}`: the {side} value {v} is not exact in four bytes. Under \
             `narrow-float-32` this differential would report rounding as a \
             divergence. Choose subjects whose every intermediate is four-byte \
             exact; do not add a tolerance."
        );
    }
}

/// **THE DIFFERENTIAL, SWITCHED ON.**
///
/// This file previously asserted an ASYMMETRY — the reference ran float streams
/// and this backend refused them — and carried these two drivers unused, waiting.
/// `Op::Yield` is now float-aware for a general stream, so the asymmetry is gone
/// and the assertion that recorded it would be false.
///
/// **The refusal was sound while it stood**, and it is worth saying why it is
/// safe to drop rather than merely possible: the reply used to be pushed with a
/// marking that made it an INTEGER-kinded operand, so admitting the yield without
/// kinding the reply would have computed on a double's bit pattern as an integer
/// — the silent wrong number the refusal existed to prevent. Both halves landed
/// together.
#[test]
fn a_float_stream_agrees_with_the_reference_value_for_value() {
    for (label, src) in SUBJECTS {
        let first: Flt = 2.0;
        let reps = replies(6);

        assert_four_byte_exact(label, "first argument", &[wide(first)]);
        let widened: Vec<f64> = reps.iter().map(|r| wide(*r)).collect();
        assert_four_byte_exact(label, "reply", &widened);

        let vm = vm_sequence(src, first, &reps);
        let native = native_sequence(src, first, &reps);

        assert_four_byte_exact(label, "reference result", &vm);
        assert_four_byte_exact(label, "backend result", &native);

        assert_eq!(
            vm, native,
            "`{label}`: the reference and the backend disagree on a float stream. \
             This is the oracle for the float yield arm; a disagreement means the \
             arm is wrong, not that the comparison needs loosening."
        );
        assert!(
            !vm.is_empty(),
            "`{label}`: the reference produced no yields, so the comparison above \
             compared two empty vectors and established nothing"
        );
    }
}

/// **THE BACKEND REALLY DOES LOWER THESE NOW**, asserted apart from the
/// differential.
///
/// The comparison above would pass vacuously if `native_sequence` were never
/// reached, and it would panic rather than report if lowering failed. This says
/// the refusal is gone, in the terms the refusal itself used.
#[test]
fn a_general_float_stream_is_no_longer_refused() {
    for (label, src) in SUBJECTS {
        let refusals = keleusma_native::module_refusals(
            &common::build(src),
            keleusma_native::LowerOptions::default(),
        );
        assert!(
            refusals.is_empty(),
            "`{label}` is still refused: {refusals:?}. The differential above \
             cannot run, and the float yield arm is not in place."
        );
    }
}

/// **THE DEGENERATE YIELD IS UNTOUCHED, AND THAT IS A HOST ABI PROMISE.**
///
/// A degenerate yield is one in TAIL POSITION: the emitter lowers it to the host
/// callback `kel_yield(i64) -> i64` rather than to a return. Admitting `Op::Yield`
/// to the float-aware set WITHOUT the `general_stream` condition would carry a
/// double's bit pattern through that signature, where the host reads an integer —
/// a host-facing ABI change made by accident.
///
/// # ⚠ THE FIRST VERSION OF THIS TEST WAS VACUOUS
///
/// It used `fn ping(t: Float) -> Float {{ let r = yield t; r }}` and returned
/// early, because **the reference refuses that shape outright** — so it
/// established nothing about the backend while passing. Found by probing what the
/// reference actually does with the subject, not by the test failing.
///
/// The shape below is a real degenerate yield that the reference DOES compile,
/// so the refusal asserted here is the backend's own.
#[test]
fn a_float_degenerate_yield_is_still_refused() {
    let src = "loop main(t: Float) -> Float { yield t }";
    let m = common::build(src);
    let refusals = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
    let Some((_, e)) = refusals.first() else {
        panic!(
            "a float-carrying DEGENERATE yield now lowers. `kel_yield` takes an \
             `i64`, so this is a host-facing ABI change, and it must not happen \
             as a side effect of admitting the general-stream arm."
        );
    };
    let msg = format!("{e:?}");
    assert!(
        msg.contains("Yield") && msg.contains("float"),
        "the degenerate float yield is refused for a different reason now: \
         {msg}. The recorded cause is the operand kind failing closed."
    );

    // **NON-VACUITY.** The same shape at `Word` must LOWER, so the refusal above
    // is about the float and not about degenerate yields in general.
    let control = common::build("loop main(t: Word) -> Word { yield t }");
    assert!(
        keleusma_native::module_refusals(&control, keleusma_native::LowerOptions::default())
            .is_empty(),
        "the Word control is refused too, so the assertion above says nothing \
         about floats"
    );
}

/// **A FLOAT REPLY CANNOT BE PACKED DIRECTLY INTO A COMPOSITE. A BYTE REPLY CAN.**
///
/// # Found while hunting the spill width, which is why it is here
///
/// Making the spilled width load-bearing needs a composite that consumes the
/// spilled operand DIRECTLY rather than through a local. Writing that subject
/// produced a refusal instead:
///
/// > `NewComposite ... has an operand of unknown packed width: operand 2 of 2`
///
/// The offending operand is **the yield's own result** — the resumed reply — not
/// the spilled value. A reply is pushed with the width its declared tag gives,
/// and `width_of_tag` returns `Unknown` for `Float` deliberately: a float's packed
/// width inside a body would be a guess, and that function exists not to guess.
///
/// # Why the Byte control matters
///
/// The identical shape at `Byte` LOWERS, because `width_of_tag` gives `Byte` a
/// one-byte width. **So this is specific to `Float`, not a general rule about
/// replies in composites**, and without the control this test would support a
/// claim much broader than the fact.
///
/// # This is a capability gap, recorded and NOT repaired here
///
/// The refusal is sound — it fails closed on a width it does not know rather than
/// packing at a guessed size. Changing `width_of_tag` for `Float` is an emitter
/// change with consequences for every composite carrying a float field, and the
/// increment that found this committed in advance to adding no emitter change.
/// **A float inside a composite already works** when the operand's width is known,
/// which is why the subjects above lower; only a value whose width comes from the
/// TAG is affected.
///
/// The test pins both halves so a movement in either direction is noticed.
#[test]
fn the_float_reply_cannot_be_packed_into_a_composite() {
    let float_src = "struct P { a: Float, b: Float }\nloop main(t: Float) -> Float { \
                     let p = P { a: (t * 2.0), b: (yield t) }; yield p.a }";
    let refusals = keleusma_native::module_refusals(
        &common::build(float_src),
        keleusma_native::LowerOptions::default(),
    );
    let Some((_, e)) = refusals.first() else {
        panic!(
            "a float reply now packs into a composite. That is the gap closing and \
             it is good news -- but `width_of_tag` must then give `Float` a width, \
             and every composite carrying a float field is affected, so this test \
             must be replaced by one that checks the PACKING rather than the \
             refusal."
        );
    };
    let msg = format!("{e:?}");
    assert!(
        msg.contains("NewComposite") && msg.contains("unknown packed width"),
        "the float reply in a composite is refused for a different reason now: \
         {msg}. The recorded cause is an operand whose packed width is unknown."
    );

    // **THE CONTROL, AND WITHOUT IT THIS TEST OVERCLAIMS.** The same shape at
    // `Byte` must lower, or the refusal above is about replies in composites
    // generally rather than about `Float`.
    let byte_src = "struct R { a: Byte, b: Byte }\nloop main(t: Word) -> Word { \
                    let r = R { a: ((t as Byte) / (2 as Byte)), b: ((yield t) as Byte) }; \
                    yield (r.a as Word) }";
    assert!(
        keleusma_native::module_refusals(
            &common::build(byte_src),
            keleusma_native::LowerOptions::default(),
        )
        .is_empty(),
        "the Byte control is refused too, so the refusal above says nothing \
         specific about `Float` and this test supports a broader claim than the \
         facts do"
    );
}
