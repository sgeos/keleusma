//! **Delegated suspension, EXECUTED against the virtual machine.**
//!
//! A `Stream` entry whose whole body is a tail call to a `Reentrant` chunk that
//! yields in tail position. `codegen.kel` is the one module in the shipped corpus
//! with this shape, and the design is in
//! `docs/decisions/NATIVE_DELEGATED_SUSPENSION.md`.
//!
//! # Why `codegen.kel` is not the subject here
//!
//! It cannot be execution-differentiated by this subproject. Its input is an
//! abstract-syntax-tree block whose 78 slot constants and two seeding helpers
//! (`analyze_class`, `verify_depth_kel_module`) are **private** to
//! `src/selfhost/mod.rs`, a file this line may read but must not edit, and no
//! function there hands out a seeded shared buffer. Verified by inspection, not
//! assumed.
//!
//! So the MECHANISM is verified on a synthetic module of the identical shape,
//! which can be driven on both sides, and `codegen.kel` stays refused under the
//! default options. The flag that would admit it is off by default, and that
//! default is the decision rather than an oversight: turning it on is an explicit
//! statement that unexecuted lowering is acceptable for the module in hand.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Module, SlotVisibility, Value};
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

mod common;

const TICKS: i64 = 40;

/// The qualifying shape. `emit` yields in tail position; `step` is a plain `fn`
/// and cannot suspend.
///
/// **The observable depends on the resume value**, which is the whole point: a
/// module that ignored its input would agree even if the transform dropped the
/// resume path entirely.
const SHAPE: &str = "\
private data st { n: Word }

fn step(r: Word) -> Word {
  st.n = st.n + 1;
  st.n * 100 + r
}

yield emit(resume: Word) -> Word {
  yield step(resume)
}

loop main(resume: Word) -> Word {
  emit(resume)
}
";

fn with_flag() -> LowerOptions {
    LowerOptions {
        delegated_suspension: true,
        ..LowerOptions::default()
    }
}

fn module_of(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn arena_for(m: &Module) -> keleusma_arena::Arena {
    const HOST_MARGIN: usize = 1 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

/// Drive the virtual machine, feeding each tick back as the next resume value.
fn run_vm(m: &Module) -> Vec<i64> {
    let arena = arena_for(m);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm loads");
    let mut shared = vec![0u8; shared_data_bytes_for(m)];
    let mut out = Vec::new();

    // **Push FIRST, then resume.** Pushing at the top of the loop yields
    // `TICKS - 1` values against the native side's `TICKS` calls, which presents
    // as a length mismatch rather than as a value difference and hides the fact
    // that every value agreed.
    let first = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("first call");
    out.push(scalar_of(&first));
    for t in 1..TICKS {
        let mut st = vm
            .resume_with_shared(&mut shared, Value::Int(t))
            .expect("resume");
        if matches!(st, VmState::Reset) {
            st = vm
                .resume_with_shared(&mut shared, Value::Int(t))
                .expect("resume after reset");
        }
        out.push(scalar_of(&st));
    }
    out
}

/// **SENTINEL CLASS: cannot receive one.** The only stage source this file loads
/// is `verify_depth.kel`, and the sentinel convention lives in `parse.kel` and
/// `reconstruct.kel` alone -- measured across all twelve sources on 2026-08-26.
/// Classified in the sentinel audit; see `is_stage_sentinel`.
fn scalar_of(st: &VmState) -> i64 {
    match st {
        VmState::Yielded(Value::Int(v)) | VmState::Finished(Value::Int(v)) => *v,
        other => panic!("unexpected VM state: {other:?}"),
    }
}

fn run_native(m: &Module) -> Vec<i64> {
    let entry = m.entry_point.expect("entry");
    let n_shared = shared_data_bytes_for(m);
    let n_priv = m
        .data_layout
        .as_ref()
        .map(|dl| {
            dl.slots
                .iter()
                .filter(|s| s.visibility == SlotVisibility::Private)
                .count()
        })
        .unwrap_or(0);
    let n_region = keleusma_native::region::region_total_bytes(m, entry, 0) as usize;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, with_flag()).expect("lower with delegated suspension");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let mut shared = vec![0u8; n_shared + 8];
    shared[n_shared..].copy_from_slice(&CANARY.to_le_bytes());
    let mut privs = vec![0u64; n_priv + 1];
    privs[n_priv] = CANARY;
    let mut region = vec![0u64; n_region.div_ceil(8) + 1];
    let canary_at = n_region.div_ceil(8);
    region[canary_at] = CANARY;

    let sym = format!("kel_chunk_{entry}");
    let fv = lm.get_function(&sym).expect("entry function");
    // **Assert the ABI before calling through it.** A wrong signature is
    // undefined behaviour that surfaces as SIGSEGV inside JIT code with no stack.
    assert_eq!(
        fv.count_params(),
        4,
        "entry `{sym}` takes {} parameters; this driver passes the resume value \
         plus the three trailing pointers",
        fv.count_params()
    );
    let f = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let mut out = Vec::new();
    for t in 0..TICKS {
        out.push(unsafe {
            f.call(
                t,
                shared.as_mut_ptr(),
                privs.as_mut_ptr() as *mut u8,
                region.as_mut_ptr() as *mut u8,
            )
        });
    }
    assert_eq!(privs[n_priv], CANARY, "wrote past the private region");
    assert_eq!(region[canary_at], CANARY, "wrote past the composite region");
    out
}

#[test]
fn the_delegated_shape_agrees_with_the_vm() {
    let m = module_of(SHAPE);
    assert!(
        keleusma_native::module_refusals(&m, with_flag()).is_empty(),
        "the shape must lower once the flag is on; refusals: {:?}",
        keleusma_native::module_refusals(&m, with_flag())
            .iter()
            .map(|(n, e)| format!("{n}: {e}"))
            .collect::<Vec<_>>()
    );

    // The VM runs FIRST: a trapping module reports an error there, while
    // natively the same trap is `llvm.trap` and kills the process with SIGTRAP.
    let vm = run_vm(&m);
    let nat = run_native(&m);

    // **Vacuity guard.** Each tick yields `st.n * 100 + resume`, so a run that
    // did nothing would repeat one value and agree for no reason.
    let mut d = vm.clone();
    d.sort_unstable();
    d.dedup();
    assert!(
        d.len() >= TICKS as usize - 1,
        "the VM produced only {} distinct values across {TICKS} ticks ({vm:?}); the \
         observable is meant to depend on both the persistent counter and the resume \
         value, so this run is not exercising the transform",
        d.len()
    );

    assert_eq!(
        vm, nat,
        "delegated suspension diverges. The resume value must travel entry slot 0 \
         -> callee argument on both sides; natively that is the next call's argument."
    );
}

/// **Must not fire.** With the flag off, the shape is still refused.
///
/// Without this the test above could pass because the module lowers by some other
/// route, and the transform would be unverified while looking verified.
#[test]
fn the_shape_is_refused_when_the_flag_is_off() {
    let m = module_of(SHAPE);
    let refusals = keleusma_native::module_refusals(&m, LowerOptions::default());
    assert!(
        refusals
            .iter()
            .any(|(_, e)| format!("{e}").contains("Stream")),
        "with `delegated_suspension` off the shape must still refuse on Stream. \
         Refusals: {refusals:?}"
    );
}

/// **Must not fire.** A callee whose `Yield` is NOT in tail position is refused
/// even with the flag on.
///
/// This is clause 2 of the predicate, and it is the clause that refuses the
/// general case. When something follows the yield, the resumed value is LIVE in
/// the callee and a return-based lowering loses it silently — the exact defect
/// the whole design turns on.
#[test]
fn a_non_tail_yield_in_the_callee_is_refused_even_with_the_flag() {
    let src = "\
private data st { n: Word }

fn step(r: Word) -> Word {
  st.n = st.n + 1;
  st.n * 100 + r
}

yield emit(resume: Word) -> Word {
  let got = yield step(resume);
  got + 1
}

loop main(resume: Word) -> Word {
  emit(resume)
}
";
    let m = module_of(src);
    let refusals = keleusma_native::module_refusals(&m, with_flag());
    assert!(
        !refusals.is_empty(),
        "a callee that USES its resumed value must be refused even with the flag \
         on: the value is live in the callee and a return-based lowering drops it. \
         This module lowered instead."
    );
}

/// **`codegen.kel` IS NO LONGER A DELEGATED-SUSPENSION CASE, and this records
/// what that cost as well as what it bought.**
///
/// This test used to assert two things: that `codegen.kel` refuses by DEFAULT,
/// and that it lowers WITH the flag. Both halves were about a module whose
/// `emit_next` was a `yield fn` called from `main`, which is exactly the nested
/// suspension the flag exists for.
///
/// **The `v0.2.3` line removed that shape** in `aaa87a01`, applying the nine-line
/// refactor this line had requested through the mailbox: `emit_next` became a
/// plain `fn` and `main` yields what it returns. The module now lowers with no
/// flag and no delegated suspension.
///
/// **What that bought**: the last refusal in the shipped corpus is gone.
///
/// **What it cost, and it is not nothing**: the predicate's only REAL-MODULE
/// witness. Every remaining subject in this file is synthetic — written to fit
/// the predicate — so "the predicate is not so narrow that it admits only the
/// case written for it" is no longer demonstrated by anything. That is a
/// coverage loss recorded as one, not a win to be quietly banked.
///
/// The standing decision it also carried is unchanged and still applies to any
/// future subject: a module whose input block is private to `src/selfhost/mod.rs`
/// cannot be execution-differentiated here, so `lower_module` returning `Ok` is a
/// fact about the compiler and never a substitute for running it.
#[test]
fn codegen_kel_no_longer_needs_delegated_suspension() {
    let src = std::fs::read_to_string("../src/selfhost/kel/codegen.kel").expect("read codegen.kel");
    let m = module_of(&src);

    let default_refusals = keleusma_native::module_refusals(&m, LowerOptions::default());
    assert!(
        default_refusals.is_empty(),
        "codegen.kel is expected to lower under DEFAULT options since `aaa87a01` \
         made `emit_next` a plain `fn`. If this fires, either that refactor was \
         reverted or a NEW refusal appeared, and the two need telling apart before \
         anything else. Refusals: {default_refusals:?}"
    );

    // Not a tautology: it pins that the flag does not INTRODUCE a refusal on a
    // module that lowers without it, which is the direction a widening can break.
    let flagged = keleusma_native::module_refusals(&m, with_flag());
    assert!(
        flagged.is_empty(),
        "the delegated-suspension flag introduced a refusal on a module that lowers \
         without it. Refusals: {:?}",
        flagged
            .iter()
            .map(|(n, e)| format!("{n}: {e}"))
            .collect::<Vec<_>>()
    );
}

/// **Does the non-tail-yield control refuse for the RIGHT reason?**
///
/// A must-not-fire control that fires for an unrelated reason asserts nothing.
/// `let got = yield ...` could plausibly be refused by some other rule, in which
/// case clause 2 would be untested while looking tested. This prints both
/// refusal sets so the reason is visible rather than assumed, and asserts the
/// one property that distinguishes them: the TAIL-yield shape lowers under the
/// flag and the non-tail one does not, so the difference is the tail position
/// and nothing else.
#[test]
fn the_non_tail_control_refuses_because_of_the_tail_position() {
    let non_tail = "\
private data st { n: Word }

fn step(r: Word) -> Word {
  st.n = st.n + 1;
  st.n * 100 + r
}

yield emit(resume: Word) -> Word {
  let got = yield step(resume);
  got + 1
}

loop main(resume: Word) -> Word {
  emit(resume)
}
";
    let tail_m = module_of(SHAPE);
    let non_tail_m = module_of(non_tail);

    let tail_r = keleusma_native::module_refusals(&tail_m, with_flag());
    let non_tail_r = keleusma_native::module_refusals(&non_tail_m, with_flag());
    println!("\n  TAIL yield, flag on     : {tail_r:?}");
    println!("  NON-TAIL yield, flag on : {non_tail_r:?}");

    // The two sources differ ONLY in what follows the yield. If the tail one
    // lowers and the non-tail one does not, the tail position is the cause.
    assert!(
        tail_r.is_empty(),
        "the tail-yield shape must lower under the flag, or the comparison below \
         has no baseline. Refusals: {tail_r:?}"
    );
    assert!(
        !non_tail_r.is_empty(),
        "the non-tail shape must be refused; it lowered instead"
    );
    assert!(
        non_tail_r
            .iter()
            .any(|(_, e)| format!("{e}").contains("Stream")),
        "the non-tail shape must be refused on Stream — that is clause 2 declining \
         to admit the delegated transform. A different refusal would mean clause 2 \
         is untested and this control is asserting something else. Refusals: {non_tail_r:?}"
    );
}
