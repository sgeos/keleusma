//! **Two live composite returns from the same callee alias each other.**
//!
//! Found by `corpus_differential.rs` as `10_multbyte.kel` returning 1 on the
//! virtual machine and 0 natively, then narrowed to three lines.
//!
//! ```text
//! fn mk(x: Word, y: Word) -> [Word; 2] { [x, y] }
//! fn main(a: Word, b: Word) -> Word { let p = mk(a, b); let r = mk(b, a); p[0] + r[0] }
//! ```
//!
//! With `a = 3, b = 4` the answer is `3 + 4 = 7`. Natively it is **8**, because
//! `p[0]` reads `4`: the second call's body overwrote the first.
//!
//! # The mechanism
//!
//! `plan_chunk_region` gives every flat site in a chunk a distinct offset, and
//! (**Within-chunk non-reuse is enforced by `region_nonreuse.rs`.** The
//! cross-chunk collision this file documents is NOT covered by that guard and
//! remains open — the two are orthogonal.)
//! offsets are planned **per chunk from zero**. `mk` therefore writes its result
//! at the same region offset on every call, while the caller holds two of those
//! results live at once. One buffer, one offset, two live values.
//!
//! A single composite return is fine — `a_single_composite_return_is_correct` below passes —
//! which is why the corpus looked clean until a caller kept two alive.
//!
//! # This is the case `sret` exists for, and I said the corpus did not have one
//!
//! `docs/decisions/NATIVE_COMPOSITE_RETURN_ABI.md` records the operator-
//! authorised caller-allocated return slot and says no multi-chunk composite
//! return exists in the corpus, so the convention was not yet forced. **That was
//! wrong**, and `10_multbyte.kel` is the counterexample. It had lowered
//! "successfully" since composites landed and was never executed, so nothing
//! contradicted the claim.
//!
//! Under `sret` the defect cannot arise: the caller reserves a distinct slot per
//! CALL SITE, so two calls to one callee write to two places.
//!
//! # Reported, not repaired here
//!
//! Implementing `sret` is a lowering change with a region-cost measurement owed
//! first. These tests pin the defect and its boundary so the repair has a target
//! and cannot silently half-land.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

mod common;

/// `(vm, native)` for a two-argument entry, driving the trailing pointers when
/// the module builds composites.
fn both(src: &str, a: i64, b: i64) -> (i64, i64) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(
        keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default()).is_empty(),
        "the case must LOWER for the comparison to mean anything"
    );

    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    // **SENTINEL CLASS: cannot receive one.** This file compiles its own inline
    // source and loads no self-hosted stage module, so the `pe_tag_base()` /
    // `rc_fail_base()` convention cannot reach this value. Classified 2026-08-26.
    let vv = match vm.call(&[Value::Int(a), Value::Int(b)]).expect("vm run") {
        VmState::Finished(Value::Int(v)) | VmState::Yielded(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    };

    let ctx = Context::create();
    let lm = ctx.create_module("k");
    keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        .expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let entry = m.entry_point.expect("entry");
    let sym = format!("kel_chunk_{entry}");
    let np = lm.get_function(&sym).expect("entry fn").count_params();

    let n_region: usize = m
        .chunks
        .iter()
        .map(|c| keleusma_native::region::plan_chunk_region(c).bytes as usize)
        .sum();
    let mut region = vec![0u64; n_region.div_ceil(8) + 4];
    let mut shared = vec![0u8; 64];
    let mut privs = vec![0u64; 8];

    // Assert the ABI before calling through it; a wrong signature is UB that
    // surfaces as SIGSEGV inside JIT code with no stack.
    let nv = match np {
        2 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&sym) }
                .expect("symbol");
            unsafe { f.call(a, b) }
        }
        5 => {
            let f = unsafe {
                ee.get_function::<unsafe extern "C" fn(i64, i64, *mut u8, *mut u8, *mut u8) -> i64>(
                    &sym,
                )
            }
            .expect("symbol");
            unsafe {
                f.call(
                    a,
                    b,
                    shared.as_mut_ptr(),
                    privs.as_mut_ptr() as *mut u8,
                    region.as_mut_ptr() as *mut u8,
                )
            }
        }
        n => panic!("entry takes {n} parameters; this harness drives 2 or 5"),
    };
    (vv, nv)
}

const MK: &str = "fn mk(x: Word, y: Word) -> [Word; 2] { [x, y] }\n";

/// **The defect.** Two live results from one callee alias.
///
/// **REPAIRED 2026-08-14** and un-ignored. It was a pinned failing case; it is
/// now a regression guard. The repair gives each CALL SITE a disjoint block of
/// the caller's region, so two calls to one callee no longer share offsets.
#[test]
fn two_live_composite_returns_must_not_alias() {
    let src = format!(
        "{MK}fn main(a: Word, b: Word) -> Word {{ let p = mk(a, b); let r = mk(b, a); p[0] + r[0] }}"
    );
    let (vm, nat) = both(&src, 3, 4);
    assert_eq!(
        vm, nat,
        "two live composite returns alias: p[0] should be 3 and r[0] 4, summing \
         to 7, but natively p[0] reads r[0]'s value. plan_chunk_region gives `mk` \
         one offset and the caller holds two of its results at once."
    );
}

/// The boundary: **one** live composite return is correct.
///
/// This is why the corpus looked clean. Nothing was wrong until a caller kept
/// two alive, and no test kept two alive.
#[test]
fn a_single_composite_return_is_correct() {
    let src = format!("{MK}fn main(a: Word, b: Word) -> Word {{ let p = mk(a, b); p[0] + p[1] }}");
    let (vm, nat) = both(&src, 3, 4);
    assert_eq!(vm, 7, "the VM itself must compute 3 + 4");
    assert_eq!(vm, nat, "a single composite return must agree");
}

/// A caller building its OWN composite alongside one callee result is also
/// correct: the two live in different chunks' plans and do not collide.
#[test]
fn a_caller_composite_beside_one_callee_result_is_correct() {
    let src = format!(
        "{MK}fn main(a: Word, b: Word) -> Word {{ let q = [a, b]; let p = mk(a, b); q[0] + p[1] }}"
    );
    let (vm, nat) = both(&src, 3, 4);
    assert_eq!(
        vm, nat,
        "a caller composite beside one callee result must agree"
    );
}
