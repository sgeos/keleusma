//! **A MEASUREMENT TAKEN WHILE ITS INPUTS MOVE IS NOT A MEASUREMENT, AND THIS
//! PROJECT KEEPS TAKING ONE.**
//!
//! Four times across three sessions, with the rule already written down each
//! time:
//!
//! | when | what moved | consequence |
//! |---|---|---|
//! | absorption 40 | increment edits landed mid-run | attribution had to be ARGUED, which is the one thing the discipline exists to avoid |
//! | 2026-09-02 | a documentation commit landed at 18:11 | a workspace run quotable only with a caveat |
//! | 2026-09-06 | documents edited under a running backend suite | the run was discarded rather than quoted |
//! | 2026-09-06 | a commit landed during a push | the push's own ref check printed a mismatch that was not one |
//!
//! **A resolution to be careful has failed four times.** This tree's own lesson
//! — from the twelve-tests-one-witness fault — is that a repair like this has to
//! be structural, not a rule someone remembers. `tools/frozen-run.sh` attaches
//! the verdict to the OUTPUT, where a reader meets it.
//!
//! # WHAT THE GUARD DOES NOT DO, STATED SO IT IS NOT OVERSOLD
//!
//! It hashes `HEAD` plus `git status --porcelain` before and after. It catches an
//! edit, a commit, a merge, a stash or a checkout landing mid-run.
//!
//! **It does not catch** untracked files a run reads, environment changes,
//! machine load, or writes outside the worktree — and it says nothing about
//! whether the measurement was correct. **A FROZEN VERDICT RULES OUT ONE WAY OF
//! BEING WRONG**, which is the way this project keeps being wrong. This line has
//! overclaimed an instrument's reach three times; that is why the limit is
//! written beside the guard rather than left to be assumed.
//!
//! **It labels; it does not block.** A legitimate edit to an unrelated file is
//! not an error, and failing a run for it would be a new failure mode.

use std::process::Command;

fn frozen_run(inner: &str) -> String {
    let out = Command::new("tools/frozen-run.sh")
        .args(["bash", "-c", inner])
        .output()
        .expect("run tools/frozen-run.sh");
    String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr)
}

/// **ALL THREE CASES IN ONE TEST, BECAUSE THE SHARED RESOURCE IS THE WHOLE TREE.**
///
/// # The first version of this file was itself the defect it guards against
///
/// It had three `#[test]` functions. The harness runs them concurrently, so the
/// probe that deliberately MOVES the tree ran while the case asserting a STILL
/// tree was measuring, and the still case failed. **The tests raced each other
/// through the very state the guard observes.**
///
/// That is the same class as `linkage_symbol_census`'s scratch-directory race —
/// except no per-test isolation can fix it, because the shared resource is not a
/// directory the test chose, it is the worktree. **A guard that observes global
/// state cannot be tested in parallel with anything that mutates global state.**
/// So the cases are sequential within one test, which is a property of what is
/// being checked rather than a workaround.
#[test]
fn the_frozen_tree_guard_reports_both_verdicts_and_preserves_exit_status() {
    // 1. A STILL RUN REPORTS FROZEN. If this failed, the guard would cry wolf on
    //    every measurement and be ignored within a day.
    let still = frozen_run("true");
    assert!(
        still.contains("VERDICT   : FROZEN"),
        "a run that touched nothing was not reported as frozen:\n{still}"
    );

    // 2. **THE MUST-FIRE CONTROL, AND THE WHOLE VALUE OF THE GUARD.** A guard
    //    that only ever says FROZEN is worse than none: it manufactures
    //    confidence. Three textual censuses on this line were falsified by
    //    exactly that, so the negative case is proven rather than assumed.
    //
    //    The probe writes an UNTRACKED file inside the worktree, which
    //    `git status --porcelain` sees while `HEAD` does not move -- so this also
    //    pins that a WORKING-TREE edit is caught, not merely a commit.
    let probe = "../.frozen_run_guard_probe";
    let moved = frozen_run(&format!("echo probe > {probe}"));
    let _ = std::fs::remove_file(probe);
    assert!(
        moved.contains("NOT FROZEN"),
        "THE GUARD DID NOT NOTICE THE TREE MOVING, so every FROZEN verdict it \
         prints is vacuous and must not be quoted:\n{moved}"
    );
    assert!(
        moved.contains("Do not quote this result"),
        "the guard reported a moved tree but did not say what to do about it; a \
         verdict a reader cannot act on is not much better than none:\n{moved}"
    );

    // 3. It must not swallow a failure. A wrapper that turned a red run green
    //    would be far worse than the problem it was built for.
    let out = std::process::Command::new("tools/frozen-run.sh")
        .args(["bash", "-c", "exit 3"])
        .output()
        .expect("run the guard");
    assert_eq!(
        out.status.code(),
        Some(3),
        "the guard changed the wrapped command's exit status; a wrapper that can \
         turn a failing measurement into a passing one is worse than no wrapper"
    );
}
