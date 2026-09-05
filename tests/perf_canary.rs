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
//! # THE CONTENDED FIGURE IMPERSONATES THE REGRESSION, SO THIS TABLE CANNOT TRIAGE IT
//!
//! The `v0.3.0` line measured this canary at **69.04 s under concurrent load and
//! 1.20 s alone**, a 57x spread, on a run whose only fault was another suite
//! compiling at the same time.
//!
//! Put that beside the table above. The regression row is **67.3 s**. The
//! contended figure is **69.04 s**. Three per cent apart, on the same side of
//! the ceiling, by a similar multiple.
//!
//! **So the table does not merely fail to discriminate the two causes, it
//! corroborates the wrong one.** A reader with a red canary, doing exactly what
//! this file tells them to do, compares against the reference points, lands on
//! the wire-format-v2 read-path regression, and starts bisecting something that
//! did not happen. Every signal available to them agrees.
//!
//! Nothing in the number distinguishes them. **Re-run alone. That is the only
//! discriminator**, and it costs 1.2 seconds.
//!
//! # What the load averages printed on failure are worth
//!
//! Less than they look, and they under-report BY CONSTRUCTION.
//!
//! A load average is an exponentially weighted moving average, not a window. A
//! contention event lasting as long as a failing run here is only partly
//! captured at the moment of sampling: about 68 per cent by the one-minute
//! figure for a 69-second event, about 39 per cent for a 30-second one. **A low
//! one-minute figure is therefore EXPECTED even when contention is exactly what
//! happened**, and does not clear it.
//!
//! The three figures answer different questions rather than corroborating each
//! other. For a 69-second event the one-minute average captures roughly 68 per
//! cent, the five-minute about 21, the fifteen-minute about 7. Only the first
//! describes the run; the others describe the period around it.
//!
//! The one-minute figure is also volatile rather than merely lagging. Readings
//! on one machine within a single hour: 5.56, 4.53, 7.75, 27.83, against
//! fifteen-minute values that stayed between 25 and 35 throughout. It read a
//! quarter of the long-run value during a decay and above the five-minute value
//! during a spike.
//!
//! Sampled AFTER the timed section, so its window overlaps the section it
//! describes rather than a period the run did not occur in.
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

/// **Hard bound on the canary's own runtime, in seconds.**
///
/// # Why a second bound exists when there is already a ceiling
///
/// [`CEILING_SECS`] is asserted AFTER the timed call returns, so it can only
/// fire once the thing it guards has finished. **A tripwire whose alarm depends
/// on the mine not exploding is not a tripwire.** Measured 2026-09-05: under
/// `--features narrow-word-16` this test ran 57 minutes at 99% of a core and
/// never returned, because its own parameters -- `1234567` and the iteration
/// count -- cannot be represented at that width. The suite could not proceed.
///
/// This bound is enforced by waiting on a channel rather than by inspecting a
/// duration, so it does not depend on the work completing.
///
/// # Why this value
///
/// Four times [`CEILING_SECS`], so a run that is merely slow trips the ceiling
/// first and gets that assertion's far better diagnostic. A healthy runtime
/// finishes this loop in well under a second, so the margin against ordinary
/// contention is three orders of magnitude.
const HARD_TIMEOUT_SECS: u64 = 120;

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
/// [`time_run`], abandoned if it does not finish within `HARD_TIMEOUT_SECS`.
///
/// # What this does and does not do
///
/// It bounds how long the TEST waits. **It does not stop the work.** A Rust
/// thread cannot be killed from outside, so a non-terminating program keeps
/// running until the test binary exits. What is gained is that the suite reaches
/// a verdict and proceeds, instead of stopping for as long as anyone lets it.
///
/// The thread is given a large stack because the virtual machine recurses
/// through composite construction and a spawned thread's default is smaller than
/// the main thread's.
fn time_run_bounded(src: &str, arg: i64) -> Result<(i64, f64), String> {
    let owned = src.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            // A send failure means the receiver already timed out and gave up,
            // which is not this thread's problem to report.
            let _ = tx.send(time_run(&owned, arg));
        })
        .expect("spawn the timed run");
    rx.recv_timeout(std::time::Duration::from_secs(HARD_TIMEOUT_SECS))
        .map_err(|_| {
            alloc_format(
                HARD_TIMEOUT_SECS,
                read_load_average().unwrap_or_else(|| "unavailable".to_string()),
            )
        })
}

/// The message for a run that never returned. Separated so the failure path
/// reads as a sentence rather than as formatting.
fn alloc_format(secs: u64, load: String) -> String {
    format!(
        "the timed program did not finish within {secs}s, so the wall-clock ceiling \
         below could never be reached and the suite would otherwise wait \
         indefinitely.\n\
         \n\
         Load average at the timeout (1/5/15 min): {load}\n\
         \n\
         This is USUALLY NOT a performance regression. The likeliest cause is that \
         the program's own constants cannot be represented at the configured word \
         width -- measured under `narrow-word-16`, where `1234567` exceeds the \
         32767 maximum. A genuine slowdown trips the ceiling instead, with a \
         better diagnostic."
    )
}

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
    let (result, elapsed) = match time_run_bounded(src, ITERATIONS) {
        Ok(pair) => pair,
        Err(message) => panic!("{message}"),
    };
    // Sampled AFTER the timing, so the one-minute window overlaps the section it
    // describes. Best-effort and never fatal: this is a diagnostic printed on
    // the red path, not a verdict, and a machine that cannot report it should
    // still run the test.
    let load = read_load_average().unwrap_or_else(|| "unavailable".to_string());

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
         Load average at the end of the timed section (1/5/15 min): {load}\n\
         A LOW ONE-MINUTE FIGURE DOES NOT CLEAR CONTENTION. It is an \
         exponentially weighted average and under-reports a contention event \
         this short by roughly a third; only the one-minute figure describes \
         this run at all, the longer two describe the period around it.\n\
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
         AND CHECK `uptime` FIRST: \"ALONE\" MEANS THE MACHINE IS IDLE, NOT THAT \
         YOU STOPPED YOUR OWN WORK. On 2026-08-30 an agent satisfied every \
         instruction above -- no cargo processes of its own, no sibling worktree \
         binaries -- and still measured under a load average of 15, because a \
         game and a browser were consuming more processor than everything else \
         combined. It then reported the red as \"confirmed false\" on that \
         measurement, and separately reported a \"thin 1.8x margin\" that was \
         pure artifact. Measured on the same tree minutes apart: 1.196s idle, \
         16.51s, 35.23s and 36.79s under load -- a 31x spread saying nothing \
         about the code. A load average above about 4 makes this number \
         meaningless whoever owns the processes.\n\
         \n\
         THE HEALTHY FIGURE IS ABOUT 1.2s, a 25x margin under the ceiling, and \
         the `v0.3.0` line independently recorded 1.20s for its equivalent. If \
         you measure single-digit seconds you are still under load; if you \
         measure ~1.2s and it still fails, that is real.\n\
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

/// Best-effort system load average, as a display string, or `None`.
///
/// Diagnostic only, printed on the failure path. `libc` is not a dependency
/// here, so this reads `/proc/loadavg` where it exists and falls back to the
/// `uptime` command. A machine offering neither still runs the test.
fn read_load_average() -> Option<String> {
    if let Ok(s) = std::fs::read_to_string("/proc/loadavg") {
        let mut it = s.split_whitespace();
        let (a, b, c) = (it.next()?, it.next()?, it.next()?);
        return Some(format!("{a} {b} {c}"));
    }
    let out = std::process::Command::new("uptime").output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    let tail = s.rsplit_once("average")?.1;
    Some(tail.trim_start_matches([':', 's', ' ']).trim().to_string())
}

/// The diagnostic must actually work, or it silently reports "unavailable" on
/// exactly the failure it exists to explain.
///
/// This is the must-fire control for `read_load_average`. Its value is printed
/// only on the red path, so nothing else in this file would ever notice it
/// returning `None` or garbage. It asserts three parseable numbers rather than
/// merely `Some`, because a parser that returned the wrong slice of the
/// `uptime` line would still be `Some`.
#[test]
fn the_load_average_diagnostic_reports_three_numbers() {
    let s = read_load_average().expect(
        "no load average available on this platform -- the failure message would \
         say 'unavailable' at exactly the moment it is needed",
    );
    let nums: Vec<f64> = s
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse::<f64>().ok())
        .collect();
    assert_eq!(
        nums.len(),
        3,
        "expected three load figures, parsed {} from {s:?}",
        nums.len()
    );
    assert!(
        nums.iter().all(|n| *n >= 0.0),
        "negative load average parsed from {s:?}, so the slice is wrong"
    );
}
