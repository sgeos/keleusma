//! **THE HOST DEMONSTRATOR RUNS, SCHEDULES ITS TASKS, AND SURVIVES A DELIBERATE FAULT.**
//!
//! `three-task-std` is given as the quick start in `CLAUDE.md`, `README.md` and
//! `MANUAL.md`. Measured 2026-09-27, it was built by **nothing**: continuous integration
//! cross-builds the two STM32N6 binaries and never touches the host target, and the release
//! gate does not mention this crate. Six Keleusma task scripts are reachable only through
//! these binaries, including `faulty.kel`, whose whole purpose is to exercise the kernel's
//! supervised-restart policy.
//!
//! **THIS CRATE'S `cargo test` DID NOT COMPILE.** Two causes, one behind the other. The
//! `bench_n6` binary was auto-discovered from `src/bin/` with no `required-features`, unlike
//! both of its siblings, so any host build tried to compile an embassy-and-defmt binary and
//! failed with eight unresolved imports. Behind that, the lib's own unit tests had been
//! broken since B28 replaced `Value::Enum { .. }` with `Enum(EnumBody)` — invisible, because
//! the test target never built.
//!
//! **WHAT IS ASSERTED.** What the demonstrator is FOR. A test checking only that the process
//! exited would pass against a binary that printed its banner and stopped, which is the
//! failure this repository keeps finding.
//!
//! **NO TIMESTAMP, CYCLE COUNT OR LINE COUNT IS PINNED.** The worst-case-execution-time
//! report prints measured cycles that depend on the host's calibration, and every log line
//! carries a millisecond stamp. This session removed two stale byte-count claims from the
//! guide for the same reason; an exact value here would be the same mistake.
//!
//! **THE BUDGET ARITHMETIC IS STATED RATHER THAN TUNED.** `faulty.kel` has a 1500ms period
//! and faults on every fifth iteration, so the first fault lands near 6000ms. The long
//! budget is 9000ms, comfortably past it; the short budget is 1200ms, comfortably before it.

use std::process::Command;
use std::time::Instant;

const BIN: &str = env!("CARGO_BIN_EXE_three-task-std");

/// Run the demonstrator with a wall-clock budget, returning (exit-ok, elapsed-ms, output).
fn run_for(ms: u64) -> (bool, u128, String) {
    let t0 = Instant::now();
    let out = Command::new(BIN)
        .arg("--run-for")
        .arg(ms.to_string())
        .output()
        .expect("run the host demonstrator");
    let elapsed = t0.elapsed().as_millis();
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), elapsed, text)
}

/// **THE SCHEDULER RUNS EVERY TASK AND THE SUPERVISED RESTART FIRES.**
#[test]
fn the_demonstrator_schedules_its_tasks_and_restarts_the_faulting_one() {
    let (ok, elapsed, text) = run_for(9000);

    assert!(
        ok,
        "the demonstrator did not exit successfully. Output:\n{text}"
    );
    // It must have actually run, not returned at once. Without this the assertions
    // below could be satisfied by a binary that printed everything and stopped.
    assert!(
        elapsed >= 8000,
        "the 9000ms budget returned after {elapsed}ms, so the budget is not bounding a real \
         run"
    );

    // Each periodic task's own observable. Named individually rather than counted, so a
    // task that stopped being scheduled is identified rather than hidden in a total.
    for (what, needle) in [
        ("the led task", "[gpio 13]"),
        ("the sensor task", "sensor ch0"),
        ("the heartbeat task", "heartbeat: system OK"),
        ("the event listener", "event_listener: woke"),
    ] {
        assert!(
            text.contains(needle),
            "{what} produced no output in a nine-second run. Output:\n{text}"
        );
    }

    // THE LOAD-BEARING PART. `faulty.kel` trips DivisionByZero on its fifth iteration and
    // the kernel is supposed to categorise it and restart the task. All three lines are
    // required: the trigger alone would pass against a kernel that then died, and the
    // restart alone would pass against one that restarted for another reason.
    for (what, needle) in [
        ("the deliberate fault", "faulty: deliberate fault"),
        (
            "the kernel's error categorisation",
            "task vm error (category=soft-script)",
        ),
        ("the supervised restart", "kernel: task restarted"),
    ] {
        assert!(
            text.contains(needle),
            "{what} is missing, so the supervised-restart policy was not exercised. \
             Output:\n{text}"
        );
    }

    // The kernel returned because its budget elapsed, not because it fell over.
    assert!(
        text.contains("Budget elapsed"),
        "the kernel stopped without reporting that its budget elapsed. Output:\n{text}"
    );
}

/// **THE BUDGET IS REAL, AND THE FAULT ASSERTION IS SENSITIVE TO IT.**
///
/// A short budget must stop before the first fault. This is the control on the test above:
/// it shows the fault lines come from elapsed runtime rather than from the banner, which
/// names every task and the fault policy in its own opening paragraph. Without this, a
/// binary that printed its banner and exited would satisfy the substring checks.
#[test]
fn a_short_budget_stops_before_the_first_deliberate_fault() {
    let (ok, elapsed, text) = run_for(1200);

    assert!(
        ok,
        "the short run did not exit successfully. Output:\n{text}"
    );
    assert!(
        elapsed < 6000,
        "a 1200ms budget took {elapsed}ms, which is past the first fault and makes this \
         control useless"
    );
    assert!(
        !text.contains("faulty: deliberate fault"),
        "the fault fired inside a 1200ms budget, so either the period changed or the \
         budget is not bounding the run. Output:\n{text}"
    );
    assert!(
        !text.contains("kernel: task restarted"),
        "a restart happened inside a 1200ms budget. Output:\n{text}"
    );
    // It still got far enough to schedule something, so this is a short run rather than a
    // failed start.
    assert!(
        text.contains("[gpio 13]") && text.contains("Budget elapsed"),
        "the short run produced no task activity at all, so it failed rather than being \
         cut short. Output:\n{text}"
    );
}

/// **AN UNPARSEABLE BUDGET IS REFUSED RATHER THAN IGNORED.**
///
/// A flag silently ignored would make every assertion above vacuous: the long run would
/// never stop and the test would hang instead of failing.
#[test]
fn a_malformed_budget_is_refused() {
    for bad in [vec!["--run-for"], vec!["--run-for", "soon"], vec!["--wat"]] {
        let out = Command::new(BIN)
            .args(&bad)
            .output()
            .expect("run the binary");
        assert!(
            !out.status.success(),
            "the argument set {bad:?} was accepted; a silently ignored budget would make the \
             bounded tests hang rather than fail"
        );
    }
}
