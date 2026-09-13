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

use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, module_refusals};

mod common;

/// Does the reference trap, and does the backend refuse?
fn observe(src: &str, args: &[Value]) -> (bool, bool) {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let traps = !matches!(vm.call(args), Ok(VmState::Finished(_)));
    let refuses = !module_refusals(&m, LowerOptions::default()).is_empty();
    (traps, refuses)
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
fn every_closed_route_traps_on_the_reference_and_is_refused_here() {
    assert!(
        ROUTES.len() >= 10,
        "the route list shrank; a short list passes this test while proving less"
    );
    let mut open = Vec::new();
    for (label, src) in ROUTES {
        let (traps, refuses) = observe(src, &args_for(src));
        assert!(
            traps,
            "`{label}` no longer traps on the reference. If `Fixed % Fixed` became \
             runnable upstream, report 4 is answered and this whole file should be \
             re-read rather than patched."
        );
        if !refuses {
            open.push(*label);
        }
    }
    assert!(
        open.is_empty(),
        "{} route(s) reach `Op::Mod` with a Fixed operand the backend does not \
         recognise: {open:?}. Each one lowers an integer remainder on a scaled \
         value and returns a number the reference never produces. Find where that \
         operand's kind is dropped; do not remove the row.",
        open.len()
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

/// **THE ROUTE THAT IS STILL OPEN, PINNED SO IT CANNOT BE FORGOTTEN.**
///
/// A private data slot declares no scalar kind in the module, so a `Fixed` read
/// back from one is indistinguishable from a `Word`. The backend lowers the
/// remainder and returns a value; the reference traps.
///
/// **This test asserts the DEFECT still exists.** It fails when the route closes,
/// which is the signal to delete it, retract report 5, and move the route into
/// `ROUTES` above.
#[test]
fn the_private_slot_route_is_still_open_and_that_is_recorded() {
    const SRC: &str = "private data d { v: Fixed }\n\
                       fn main(a: Fixed) -> Fixed { d.v = a; d.v % d.v }";
    let (traps, refuses) = observe(SRC, &[Value::Fixed(200 << 16)]);
    assert!(
        traps,
        "the reference now RUNS a private-slot `Fixed % Fixed`. That answers report \
         4 from the runtime side and makes this residual moot; delete this test."
    );
    assert!(
        !refuses,
        "the backend now REFUSES the private-slot route. **Good** — delete this \
         test, move the route into `ROUTES`, and retract report 5. Whatever \
         supplied the slot's scalar kind is the thing to record."
    );
}
