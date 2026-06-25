#![cfg(all(feature = "compile", feature = "verify"))]
//! Host-attested native body time folded into the WCET (#50).
//!
//! The module's compile-time `wcet_cycles` header is the script-only bound,
//! since natives are not known at compile time. After registering natives and
//! attesting their per-call WCET through `Vm::set_native_bounds`, the host calls
//! `Vm::wcet_per_iteration` to obtain the per-iteration WCET with native body
//! time folded in -- the symmetric counterpart of `Vm::auto_arena_capacity` for
//! WCMU.

extern crate alloc;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, DEFAULT_NATIVE_WCET, Vm};
use keleusma::{Arena, Value};

#[test]
fn verified_native_body_wcet_is_folded_into_per_iteration_wcet() {
    // A Stream that calls a verified native once per iteration. The default
    // per-call native WCET is folded into the baseline; attesting a large body
    // time raises the per-iteration WCET by the difference, since the native is
    // called once per iteration.
    let src = "use slow() -> Word\n\
               loop main(seed: Word) -> Word { yield slow() + seed }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    vm.register_native("slow", |_args| Ok(Value::Int(0)));

    let baseline = vm.wcet_per_iteration().expect("wcet");
    assert!(
        baseline >= DEFAULT_NATIVE_WCET,
        "the baseline folds in the default native body time"
    );

    vm.set_native_bounds("slow", 1000, 0).expect("set bounds");
    let attested = vm.wcet_per_iteration().expect("wcet");

    assert_eq!(
        attested - baseline,
        1000 - DEFAULT_NATIVE_WCET,
        "attesting a 1000-cycle native body replaces the default {DEFAULT_NATIVE_WCET} \
         (baseline={baseline}, attested={attested})"
    );
    // A native called inside a loop scales by the loop bound: the verified-
    // native per-call cost rides the same `wcet_region` loop-multiplicity walk
    // as the script ops (covered by the verify-crate unit tests and the #49
    // loop tests), so it is not re-exercised end to end here.
}
