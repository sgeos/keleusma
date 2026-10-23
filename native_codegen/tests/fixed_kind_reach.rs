//! **HOW FAR DOES THE `Fixed` REFUSAL ACTUALLY REACH?**
//!
//! # Why this file exists
//!
//! The previous increment refused `Op::Mod` and `Op::Div` on a `Fixed` operand,
//! closing a divergence where the reference traps and this backend returned a
//! value. The commit message said the fix was precise. **It was not.**
//!
//! `OperandKind::Fixed` was seeded at chunk parameters only, so
//! `(a + b) % (a + b)` still lowered the very `Op::Mod` that `a % b` refused:
//! neither operand was a direct parameter read, so neither carried the kind.
//! **A guard tested only on the operand shape it was written for is a guard whose
//! reach is unknown.** Driving sixteen routes by which a `Fixed` value can arrive
//! found **nine** of them open.
//!
//! > A passing check is evidence about the checker's reach before it is evidence
//! > about the tree.
//!
//! # What closed it
//!
//! `Fixed` is contagious through arithmetic, and every site that already knew a
//! scalar kind was discarding all of it but `Float`. The lattice was built for
//! floats and never widened when `Fixed` started mattering:
//!
//! - arithmetic results inherit `Fixed` from either operand,
//! - `FixedMul`, `FixedDiv` and `WordToFixed` produce it; `FixedToWord`
//!   deliberately does NOT, the scale being gone,
//! - `StructField::Flat` and `ArrayElem::Flat` carry a `ScalarKind` in the baked
//!   operand, now mapped rather than tested for floatness,
//! - a call result takes it from the callee signature, as the float half already
//!   did,
//! - a shared slot takes it from the slot layout's kind tag.
//!
//! # ⚠ ONE ROUTE REMAINS OPEN, AND IT CANNOT BE CLOSED FROM HERE
//!
//! **A `Fixed` value read back from a PRIVATE data slot.** `DataSlot` carries a
//! name and a visibility and **no scalar kind** — unlike `SharedSlotLayout`,
//! which carries a tag and is why the shared route closed. The declared type of a
//! private slot is not in the module, so the backend cannot know the slot holds a
//! scaled value.
//!
//! **The fail-closed alternative was considered and not taken.** Refusing `%` and
//! `/` on every value of unknown provenance would refuse ordinary `Word`
//! remainders on private-slot values, which is a real coverage loss for a
//! program the reference runs correctly. Recorded as the residual it is, reported
//! upstream as report 5, and pinned below so it cannot rot into an assumption
//! that it was closed.

//!
//! # ✅ REPORT 4 WAS ANSWERED FROM THE RUNTIME SIDE IN ABSORPTION 82
//!
//! Upstream made bare `Op::Div` and `Op::Mod` total. The reference now RUNS
//! `Fixed % Fixed`: the low slot is the raw remainder of the scaled bits, the
//! high slot is zero, and the flag is `0`, or `3` for a zero divisor. This
//! backend now lowers a matched Fixed pair to exactly that, so every route
//! below must AGREE rather than trap and refuse.
//!
//! **The private-slot route closed with it, and not because the kind arrived.**
//! An operand whose `Fixed` kind is unknown lowers as the integer remainder,
//! whose low slot is the same bits and whose flag is the same. Only the high
//! slot differs, a sign extension where Fixed keeps zero, and the compiled form
//! discards it with `PopN(1)`. So report 5's missing kind no longer reaches a
//! wrong value through `%`. The kind is still missing, which is why report 5
//! stays open.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

/// The raw bits of a scalar argument or result.
fn bits(v: &Value) -> i64 {
    match v {
        Value::Int(n) | Value::Fixed(n) => *n,
        other => panic!("this harness drives Word and Fixed scalars, not {other:?}"),
    }
}

/// The reference's result bits and the backend's, for a one or two argument
/// entry. Panics if either side fails to produce a value, because every route
/// here is now expected to run.
fn observe(src: &str, args: &[Value]) -> (i64, i64) {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let reference = match vm.call(args) {
        Ok(VmState::Finished(v)) => bits(&v),
        other => panic!("the reference no longer completes {src:?}: {other:?}"),
    };
    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(refusals.is_empty(), "{src:?} is refused: {refusals:?}");

    let ctx = Context::create();
    let lm = ctx.create_module("k");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let sym = format!("kel_chunk_{}", m.entry_point.expect("entry"));
    let np = lm.get_function(&sym).expect("entry fn").count_params() as usize;
    let a: Vec<i64> = args.iter().map(bits).collect();
    // Buffers sized from the published contract, as `common` sizes them.
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(&m).max(64) + 8];
    let n_priv =
        (need + keleusma_native::region::persistent_supplement_bytes(&m) as usize).div_ceil(8);
    let mut privs = vec![0u64; n_priv + 1];
    common::install_private_init(&m, &mut privs[..n_priv]);
    let n_region = keleusma_native::region::host_arena_supplement_bytes(&m) as usize;
    let mut region = vec![0u64; n_region.div_ceil(8) + 4];
    let (sp, pp, rp) = (
        shared.as_mut_ptr(),
        privs.as_mut_ptr() as *mut u8,
        region.as_mut_ptr() as *mut u8,
    );
    type P = *mut u8;
    let native = unsafe {
        match (a.as_slice(), np) {
            ([x], 1) => ee
                .get_function::<unsafe extern "C" fn(i64) -> i64>(&sym)
                .expect("symbol")
                .call(*x),
            ([x, y], 2) => ee
                .get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&sym)
                .expect("symbol")
                .call(*x, *y),
            ([x], 4) => ee
                .get_function::<unsafe extern "C" fn(i64, P, P, P) -> i64>(&sym)
                .expect("symbol")
                .call(*x, sp, pp, rp),
            ([x, y], 5) => ee
                .get_function::<unsafe extern "C" fn(i64, i64, P, P, P) -> i64>(&sym)
                .expect("symbol")
                .call(*x, *y, sp, pp, rp),
            (args, n) => panic!(
                "{} argument(s) with {n} parameters is not driven here",
                args.len()
            ),
        }
    };
    (reference, native)
}

/// `(label, source)` for every route by which a `Fixed` value can reach
/// `Op::Mod` with NEITHER operand being a bare parameter read where that is
/// possible.
///
/// **The pairs matter.** An early version put a parameter on one side, so the
/// refusal fired on that operand and the route under test was never exercised.
/// Every row below that can avoid a bare parameter does.
const ROUTES: &[(&str, &str)] = &[
    (
        "both parameters",
        "fn main(a: Fixed, b: Fixed) -> Fixed { a % b }",
    ),
    (
        "both sides an addition",
        "fn main(a: Fixed, b: Fixed) -> Fixed { (a + b) % (a + b) }",
    ),
    (
        "both sides a fixed multiply",
        "fn main(a: Fixed, b: Fixed) -> Fixed { (a * b) % (a * b) }",
    ),
    (
        "both sides a fixed divide",
        "fn main(a: Fixed, b: Fixed) -> Fixed { (a / b) % (a / b) }",
    ),
    (
        "both sides a negation",
        "fn main(a: Fixed) -> Fixed { (-a) % (-a) }",
    ),
    (
        "both sides a word-to-fixed cast",
        "fn main(w: Word) -> Fixed { (w as Fixed) % (w as Fixed) }",
    ),
    (
        "both sides a call result",
        "fn id(x: Fixed) -> Fixed { x }\nfn main(a: Fixed) -> Fixed { id(a) % id(a) }",
    ),
    (
        "both sides a struct field",
        "struct P { v: Fixed }\nfn main(a: Fixed) -> Fixed { let p: P = P { v: a }; p.v % p.v }",
    ),
    (
        "both sides an array element",
        "fn main(a: Fixed) -> Fixed { let xs: [Fixed; 2] = [a, a]; xs[0] % xs[1] }",
    ),
    (
        "through a local",
        "fn main(a: Fixed, b: Fixed) -> Fixed { let c: Fixed = a * b; c % b }",
    ),
];

#[test]
fn every_route_runs_on_the_reference_and_agrees_here() {
    assert!(
        ROUTES.len() >= 10,
        "the route list shrank; a short list passes this test while proving less"
    );
    let mut disagree = Vec::new();
    for (label, src) in ROUTES {
        let (reference, native) = observe(src, &args_for(src));
        if reference != native {
            disagree.push((*label, reference, native));
        }
    }
    assert!(
        disagree.is_empty(),
        "{} route(s) disagree with the reference: {disagree:?}",
        disagree.len()
    );
}

/// Arguments matching each route's parameter list.
fn args_for(src: &str) -> Vec<Value> {
    let fx = Value::Fixed;
    if src.contains("w: Word") {
        vec![Value::Int(200)]
    } else if src.contains("a: Fixed, b: Fixed") {
        vec![fx(200 << 16), fx(7 << 16)]
    } else {
        vec![fx(200 << 16)]
    }
}

/// **THE ROUTE THAT WAS OPEN, AND NOW AGREES WITHOUT THE KIND.**
///
/// A private data slot declares no scalar kind in the module, so a `Fixed` read
/// back from one is indistinguishable from a `Word`. Until absorption 82 the
/// backend lowered the remainder and returned a value where the reference
/// trapped. The reference now completes it, and the integer remainder this
/// backend lowers carries the same low bits, so the two agree. See the header.
///
/// **A negative operand is the case that would separate them**, if anything
/// did, since that is where the integer high slot is a sign extension.
#[test]
fn the_private_slot_route_agrees_without_the_kind() {
    const SRC: &str = "private data d { v: Fixed, w: Fixed }\n\
                       fn main(a: Fixed) -> Fixed { d.v = a + a + a + a; d.w = a + a + a; d.v % d.w }";
    for a in [200 << 16, -(200 << 16), 7, -7] {
        let (reference, native) = observe(SRC, &[Value::Fixed(a)]);
        assert_eq!(
            native, reference,
            "the private-slot route disagrees for {a}"
        );
    }
}
