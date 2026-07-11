//! Integration tests for the `for i in min..max limit L { } on { ... }`
//! bounded-runtime-range loop.
//!
//! The `limit` clause caps the iteration count with a compile-time constant, so
//! a runtime range that strict verification would reject on its own is admitted:
//! the worst-case iteration count is the cap. An optional `on` block captures the
//! outcome (`ok`, `break`, `limit`); an unhandled `limit` overrun traps loud as
//! `LoopLimitExceeded`, the fail-loud default consistent with the other capture
//! constructs. Overflow capture and `when` guards on outcome arms are not yet
//! implemented and are rejected explicitly.

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};

/// Compile, verify, and run `main(arg)`, returning the `Word` result or the
/// error. A runtime range under a cap must both verify and run.
fn run(src: &str, arg: i64) -> Result<i64, VmError> {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("verify");
    match vm.call(&[Value::Int(arg)])? {
        VmState::Finished(Value::Int(n)) => Ok(n),
        other => panic!("unexpected VM state {other:?}"),
    }
}

/// Compile only, for the negative (rejection) cases.
fn compile_err(src: &str) -> String {
    match compile(&parse(&tokenize(src).expect("lex")).expect("parse")) {
        Ok(_) => panic!("expected a compile error, but compilation succeeded"),
        Err(e) => e.message,
    }
}

// A runtime range `0..hi` is normally rejected by strict verification; under a
// `limit` it verifies and runs, completing when the range is exhausted.
#[test]
fn runtime_range_under_a_limit_verifies_and_runs_to_completion() {
    let src = "private data d { s: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit 8 { d.s = d.s + i; } d.s }";
    // hi = 3 exhausts the range before the cap: 0 + 1 + 2 = 3.
    assert_eq!(run(src, 3).unwrap(), 3);
}

// The same loop capped: with hi beyond the cap, exactly `limit` iterations run.
#[test]
fn the_cap_bounds_the_iteration_count() {
    let src = "private data d { s: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit 8 { d.s = d.s + i; } \
        on { ok => { }, limit => { }, } d.s }";
    // 0 + 1 + ... + 7 = 28, the cap of eight iterations.
    assert_eq!(run(src, 100).unwrap(), 28);
}

// The bare form (no `on` block) traps loud when the cap is hit before the range
// is exhausted.
#[test]
fn a_bare_limit_loop_traps_on_overrun() {
    let src = "private data d { s: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit 4 { d.s = d.s + i; } d.s }";
    // hi = 2 completes within the cap.
    assert_eq!(run(src, 2).unwrap(), 1);
    // hi = 100 overruns the cap of four and traps.
    assert!(matches!(run(src, 100), Err(VmError::LoopLimitExceeded)));
}

// A `limit` outcome arm handles the overrun instead of trapping.
#[test]
fn a_limit_arm_handles_the_overrun() {
    let src = "private data d { s: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit 4 { } \
        on { ok => { }, limit => { d.s = 99; }, } d.s }";
    // Overrun runs the limit arm, setting the sentinel.
    assert_eq!(run(src, 100).unwrap(), 99);
    // Completing within the cap does not run the limit arm.
    assert_eq!(run(src, 2).unwrap(), 0);
}

// The `break` outcome arm fires when the body breaks, and binds the index.
#[test]
fn a_break_arm_binds_the_index() {
    let src = "private data d { hit: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit 16 { if i == 3 { break; } } \
        on { ok => { }, break(bi) => { d.hit = bi; }, limit => { }, } d.hit }";
    assert_eq!(run(src, 10).unwrap(), 3);
}

// The `limit` arm binds the loop variable at the stop.
#[test]
fn a_limit_arm_binds_the_stopping_index() {
    let src = "private data d { hit: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit 5 { } \
        on { ok => { }, limit(si) => { d.hit = si; }, } d.hit }";
    assert_eq!(run(src, 100).unwrap(), 5);
}

// The `ok` arm binds the completed iteration count.
#[test]
fn an_ok_arm_binds_the_count() {
    let src = "private data d { hit: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit 16 { } \
        on { ok(c) => { d.hit = c; }, limit => { }, } d.hit }";
    assert_eq!(run(src, 7).unwrap(), 7);
}

// A const-data field is an admissible cap and inlines to its literal.
#[test]
fn a_const_data_field_is_an_admissible_cap() {
    let src = "const data cp { cap: Word = 4 } private data d { s: Word } \
        fn main(hi: Word) -> Word { for i in 0..hi limit cp.cap { d.s = d.s + i; } \
        on { ok => { }, limit => { }, } d.s }";
    // Capped at four: 0 + 1 + 2 + 3 = 6.
    assert_eq!(run(src, 100).unwrap(), 6);
}

// --- Rejections ---

// An `on` block without a `limit` clause is rejected.
#[test]
fn on_block_requires_a_limit() {
    let msg = compile_err("fn main(hi: Word) -> Word { for i in 0..hi { } on { ok => { }, } 0 }");
    assert!(msg.contains("requires a `limit`"), "got: {msg}");
}

// A non-constant cap is rejected.
#[test]
fn a_runtime_cap_is_rejected() {
    let msg =
        compile_err("fn main(hi: Word, cap: Word) -> Word { for i in 0..hi limit cap { } 0 }");
    assert!(msg.contains("compile-time"), "got: {msg}");
}

// A non-positive cap is rejected.
#[test]
fn a_non_positive_cap_is_rejected() {
    let msg = compile_err("fn main(hi: Word) -> Word { for i in 0..hi limit 0 { } 0 }");
    assert!(msg.contains("positive"), "got: {msg}");
}

// The `overflow` arm is not yet implemented and is rejected.
#[test]
fn overflow_arm_is_rejected_as_unimplemented() {
    let msg = compile_err(
        "fn main(hi: Word) -> Word { for i in 0..hi limit 4 { } \
        on { ok => { }, overflow => { }, } 0 }",
    );
    assert!(msg.contains("overflow"), "got: {msg}");
}

// A `when` guard on an outcome arm is not yet implemented and is rejected.
#[test]
fn guard_on_outcome_arm_is_rejected_as_unimplemented() {
    let msg = compile_err(
        "fn main(hi: Word) -> Word { for i in 0..hi limit 4 { } \
        on { ok => { }, limit when hi > 0 => { }, } 0 }",
    );
    assert!(msg.contains("guard"), "got: {msg}");
}
