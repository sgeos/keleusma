//! A performance TRIPWIRE for the VM's constant-load and module-scalar paths.
//!
//! # Why this file exists
//!
//! On 2026-08-08 the wire-format v2 cutover shipped a runtime that was
//! functionally perfect and roughly forty times slower. Every test passed. One
//! stage self-compile went from 54 seconds to over 37 minutes, and the only
//! reason anyone noticed was a human watching a clock.
//!
//! Two hot reads had quietly become whole-module operations: `Vm::aux()`
//! rebuilding fifteen sub-tables to fetch one scalar inside the interpreter
//! loop, and `chunk_const` materialising every constant in the module to return
//! one. Neither is expressible as a correctness assertion. A test suite that
//! cannot fail on "the right answer, eventually" will report green through an
//! unshippable regression, so this file adds the one signal the others cannot
//! carry.
//!
//! # What this is NOT
//!
//! It is not a benchmark and the number it prints is not a measurement. It is a
//! tripwire with a deliberately loose ceiling, and it should be read only as
//! "nothing catastrophic happened". Do not tighten it toward the observed
//! runtime chasing precision: a canary that fails on a loaded laptop gets
//! disabled, and a disabled canary is worse than none. Against the regression it
//! exists for -- a factor of forty -- an order of magnitude of headroom still
//! fires on the first run.
//!
//! If this fails, do not raise the ceiling as the first move. Profile it. The
//! defect it is built to catch is a hot path doing work proportional to the
//! whole module, and `cargo test` will keep saying the answers are correct.
//!
//! # It reads wall-clock time, so concurrent load is a real false-positive source
//!
//! Raised by the parallel `v0.3.0` session, and correct: the same reasoning that
//! makes an orphaned test binary corrupt this signal applies to *legitimate*
//! concurrent work. A heavy build in another worktree can push this over without
//! anything being wrong with the branch under test.
//!
//! The headroom absorbs a lot — a healthy run is around an order of magnitude
//! under the ceiling — but it is not unlimited. **Under parallel development,
//! suspect concurrent load before suspecting a regression**, and re-run alone
//! before investigating. This is a second, independent argument for serialising
//! full gates between sessions rather than only for throughput.
//!
//! # Reference points
//!
//! Measured on one machine, uncontended, debug build, at 200k iterations. These
//! are for ORIENTATION when triaging a failure, not thresholds — absolute
//! numbers travel badly between machines, but the ratios do not.
//!
//! | Runtime | Elapsed |
//! |---|---|
//! | pre-cutover, rkyv-archived aux body | 6.4 s |
//! | wire format v2, as first committed | 67.3 s |
//! | wire format v2, repaired | 1.2 s |
//!
//! The middle row is the regression this guards against, and it is the reason
//! the ceiling sits where it does. The bottom row being five times faster than
//! the top is the actual payoff of the v2 read path; a future change that gives
//! that back will show up here long before anyone notices it by hand.

// Drives the compile front end and constructs a `Vm` (strict verification), so
// it needs both `compile` and `verify`. Gate the whole file so the
// `--no-default-features` build does not attempt it.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
use std::time::Instant;

/// Iterations of the constant-loading loop.
///
/// Large enough that per-load overhead dominates process startup, small enough
/// that a healthy runtime finishes in well under a second.
const ITERATIONS: i64 = 200_000;

/// Wall-clock ceiling, in seconds.
///
/// Chosen from the failure mode rather than from the observed runtime: the
/// regression this guards against was a factor of forty, so anything within an
/// order of magnitude of healthy still trips it immediately. See the module docs
/// before changing this.
const CEILING_SECS: f64 = 30.0;

/// Compile, verify, and run `main(arg)`, returning the result and the elapsed
/// execution time. Compilation is deliberately excluded from the timing: the
/// paths under guard are VM reads, and including the front end would dilute the
/// signal with unrelated work.
fn time_run(src: &str, arg: i64) -> (i64, f64) {
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("verify");
    let start = Instant::now();
    let state = vm.call(&[Value::Int(arg)]).expect("run");
    let elapsed = start.elapsed().as_secs_f64();
    match state {
        VmState::Finished(Value::Int(n)) => (n, elapsed),
        other => panic!("unexpected VM state {other:?}"),
    }
}

/// A loop dominated by constant loads and module-scalar reads must stay fast.
///
/// Every iteration loads immediate constants and performs word-width arithmetic,
/// so it exercises `chunk_const`, `module_word_bytes` and `chunk_local_count` --
/// the three that regressed. The result is asserted as well as the time, because
/// a canary that only checks the clock would pass if the loop were optimised
/// into doing nothing.
#[test]
fn constant_loads_in_a_loop_stay_fast() {
    let src = "private data d { s: Word } \
               fn main(hi: Word) -> Word { \
               for i in 0..hi limit 200000 { d.s = d.s + 1234567 + i; } d.s }";
    let (result, elapsed) = time_run(src, ITERATIONS);

    // The arithmetic must be right. Without this the timing proves nothing: a
    // VM that skipped the body entirely would be extremely fast.
    let expected = 1234567 * ITERATIONS + (ITERATIONS - 1) * ITERATIONS / 2;
    assert_eq!(
        result, expected,
        "the loop did not compute what it should have, so its timing is meaningless"
    );

    assert!(
        elapsed < CEILING_SECS,
        "VM executed {ITERATIONS} constant-loading iterations in {elapsed:.2}s, \
         over the {CEILING_SECS}s tripwire.\n\
         \n\
         FIRST, RULE OUT CONCURRENT LOAD. This reads wall-clock time, so a heavy \
         build in another session or worktree on the same machine can push it \
         over without anything being wrong with your branch. Under parallel \
         development that is a likelier explanation than a regression. Re-run it \
         alone before investigating, and reap any orphaned test binaries left by \
         an interrupted gate (pkill -f \"$PWD/target/debug/deps\" -- SCOPE IT to \
         your own worktree, an unscoped pattern kills a sibling session's live \
         run).\n\
         \n\
         If it still fails alone: do NOT raise the ceiling as a first response. \
         This guard exists because the wire-format v2 cutover shipped a runtime \
         forty times slower with every correctness test passing. Profile the VM's \
         inner loop and look for a per-access read that has become proportional \
         to the whole module -- a rebuilt view, a re-parsed table, or a whole-pool \
         decode behind what should be a single-record fetch. See the \
         tests/perf_canary.rs module docs."
    );

    // Reported so a gradual drift is visible in the log even while passing. A
    // tripwire only ever says pass or fail; the number is what shows a slow
    // slide toward it.
    println!("perf canary: {ITERATIONS} iterations in {elapsed:.3}s (ceiling {CEILING_SECS}s)");
}
