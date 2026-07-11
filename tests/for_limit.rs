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

// A const parameter is an admissible cap; it is erased to a literal at
// monomorphization, in the cap position and inside outcome arms alike.
#[test]
fn a_const_parameter_is_an_admissible_cap() {
    let src = "private data d { s: Word } \
        fn go<const c: Word>(hi: Word) -> Word { for i in 0..hi limit c { d.s = d.s + i; } \
        on { ok => { }, limit => { }, } d.s } \
        fn main(hi: Word) -> Word { go::<4>(hi) }";
    // Capped at four: 0 + 1 + 2 + 3 = 6.
    assert_eq!(run(src, 100).unwrap(), 6);
}

// A const parameter used in the cap is also erased inside an arm guard.
#[test]
fn a_const_parameter_is_erased_in_an_arm_guard() {
    let src = "private data d { w: Word } \
        fn go<const c: Word>(hi: Word) -> Word { for i in 0..hi limit 9 { if i == c { break; } } \
        on { ok => { }, break(k) when k == c => { d.w = 9; }, } d.w } \
        fn main(hi: Word) -> Word { go::<3>(hi) }";
    // The body breaks at i == c == 3, and the guard k == c holds.
    assert_eq!(run(src, 100).unwrap(), 9);
}

// A body that allocates a composite each iteration still verifies: the cap
// bounds the worst-case memory usage, so strict `Vm::new` admits it (an
// unbounded per-iteration allocation would be rejected).
#[test]
fn an_allocating_body_is_wcmu_bounded_by_the_cap() {
    let src = "struct P { x: Word, y: Word } private data d { s: Word } \
        fn main(n: Word) -> Word { for i in 0..n limit 8 { let p = P { x: i, y: i }; \
        d.s = d.s + p.x + p.y; } on { ok => { }, limit => { }, } d.s }";
    // Capped at eight: sum of (i + i) for i in 0..8 = 2 * 28 = 56.
    assert_eq!(run(src, 100).unwrap(), 56);
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

// The `overflow` arm is inadmissible: the range bound and the cap keep the
// index below the type maximum, so the increment cannot overflow.
#[test]
fn overflow_arm_is_rejected_as_inadmissible() {
    let msg = compile_err(
        "fn main(hi: Word) -> Word { for i in 0..hi limit 4 { } \
        on { ok => { }, overflow => { }, } 0 }",
    );
    assert!(msg.contains("inadmissible"), "got: {msg}");
    assert!(msg.contains("overflow"), "got: {msg}");
}

// --- The count == cap boundary (the case the compiler refactor relies on) ---

// When the range length equals the cap exactly, the loop completes rather than
// reporting an overrun. The bare form must not trap.
#[test]
fn count_equal_to_cap_completes_and_does_not_trap() {
    let src = "private data d { s: Word } \
        fn main(n: Word) -> Word { for i in 0..n limit 8 { d.s = d.s + i; } d.s }";
    // n == cap == 8: all eight iterations run, 0 + ... + 7 = 28, no trap.
    assert_eq!(run(src, 8).unwrap(), 28);
    // n just over the cap traps.
    assert!(matches!(run(src, 9), Err(VmError::LoopLimitExceeded)));
}

// The same boundary through the `on` block reports `ok`, not `limit`.
#[test]
fn count_equal_to_cap_reports_ok_not_limit() {
    let src = "private data d { w: Word } \
        fn main(n: Word) -> Word { for i in 0..n limit 8 { } \
        on { ok => { d.w = 1; }, limit => { d.w = 2; }, } d.w }";
    assert_eq!(run(src, 8).unwrap(), 1); // exactly the cap: ok
    assert_eq!(run(src, 9).unwrap(), 2); // over the cap: limit
    assert_eq!(run(src, 5).unwrap(), 1); // under the cap: ok
}

// --- `when` guards on outcome arms ---

// A guard that holds runs the (guardable) arm; a break at index 2 with a guard
// that holds binds the index.
#[test]
fn a_true_guard_on_break_runs_the_arm() {
    let src = "private data d { w: Word } \
        fn main(n: Word) -> Word { for i in 0..n limit 9 { if i == 2 { break; } } \
        on { ok => { }, break(bi) when bi == 2 => { d.w = 7; }, } d.w }";
    assert_eq!(run(src, 5).unwrap(), 7);
}

// A guard that fails leaves an intended outcome (`break`) a noop; it does not
// fall through to `ok`, since a `break` arm is present.
#[test]
fn a_false_guard_on_break_is_a_noop() {
    let src = "private data d { w: Word } \
        fn main(n: Word) -> Word { for i in 0..n limit 9 { if i == 2 { break; } } \
        on { ok => { d.w = 1; }, break(bi) when bi == 5 => { d.w = 7; }, } d.w }";
    assert_eq!(run(src, 5).unwrap(), 0);
}

// A guard that fails leaves the defensive outcome (`limit`) unhandled, so it
// traps, exactly as an absent arm would.
#[test]
fn a_false_guard_on_limit_traps() {
    let src = "fn main(n: Word) -> Word { for i in 0..n limit 5 { } \
        on { ok => { }, limit(si) when si > 100 => { }, } 0 }";
    assert!(matches!(run(src, 100), Err(VmError::LoopLimitExceeded)));
}

// A guard that holds on `limit` handles the overrun.
#[test]
fn a_true_guard_on_limit_handles_the_overrun() {
    let src = "private data d { w: Word } \
        fn main(n: Word) -> Word { for i in 0..n limit 5 { } \
        on { ok => { }, limit(si) when si > 3 => { d.w = 1; }, } d.w }";
    assert_eq!(run(src, 100).unwrap(), 1);
}

// A non-Bool guard is rejected (on a guardable arm).
#[test]
fn a_non_bool_guard_is_rejected() {
    let msg = compile_err(
        "fn main(n: Word) -> Word { for i in 0..n limit 4 { } \
        on { ok => { }, limit when n => { }, } 0 }",
    );
    assert!(msg.contains("Bool"), "got: {msg}");
}

// --- The family discipline: an unguarded `ok` catch-all is mandatory ---

// An `on` block without an `ok` arm is rejected.
#[test]
fn on_block_requires_an_ok_catch_all() {
    let msg = compile_err(
        "fn main(n: Word) -> Word { for i in 0..n limit 4 { } on { limit => { }, } 0 }",
    );
    assert!(msg.contains("`ok` catch-all"), "got: {msg}");
}

// A guarded `ok` arm is rejected: the catch-all must be unguarded.
#[test]
fn a_guarded_ok_arm_is_rejected() {
    let msg = compile_err(
        "fn main(n: Word) -> Word { for i in 0..n limit 4 { } \
        on { ok(c) when c == 0 => { }, limit => { }, } 0 }",
    );
    assert!(msg.contains("unguarded"), "got: {msg}");
}

// A duplicate arm of the same outcome is rejected.
#[test]
fn a_duplicate_outcome_arm_is_rejected() {
    let msg = compile_err(
        "fn main(n: Word) -> Word { for i in 0..n limit 4 { } \
        on { ok => { }, limit => { }, limit => { }, } 0 }",
    );
    assert!(msg.contains("duplicate"), "got: {msg}");
}

// With no `break` arm, a `break` falls through to the `ok` catch-all.
#[test]
fn a_break_falls_through_to_ok_when_no_break_arm() {
    let src = "private data d { w: Word } \
        fn main(n: Word) -> Word { for i in 0..n limit 9 { if i == 2 { break; } } \
        on { ok => { d.w = 42; }, } d.w }";
    assert_eq!(run(src, 5).unwrap(), 42);
}
