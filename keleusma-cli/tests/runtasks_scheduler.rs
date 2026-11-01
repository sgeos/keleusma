//! The scheduler behaviours `run-tasks` documents and had never executed.
//!
//! # Why these had no tests
//!
//! Until the arena-move and yield-decode defects were repaired, `run-tasks`
//! could not run a single task. Every documented behaviour below therefore had
//! never run: the restart policy, the restart rate limit, multi-task
//! isolation, periodic scheduling. Each was probed before this file was
//! written and each is correct, which is exactly when a guard is cheapest.
//!
//! **The restart path matters most.** It re-allocates the task's arena and
//! re-instantiates its virtual machine, and that is the path whose defect was
//! just repaired. A regression there is that defect returning.
//!
//! # On asserting the scheduler's reported output
//!
//! A restart count has no externally observable state: the scheduler's report
//! IS the observable. That is a deliberate exception to this suite's usual
//! preference for asserting state over text, not an oversight. It is mitigated
//! two ways — a stable substring is matched rather than a whole formatted
//! line, and wherever a structural consequence exists it is asserted as well:
//! the runner surviving, the sibling task continuing.
//!
//! # One documented behaviour that is NOT tested here, and why
//!
//! The design document distinguishes `on_error` from `always` by what happens
//! on *voluntary* termination, hedging it as "unusual for `loop main`". It is
//! more than unusual: a `loop main` that never yields **cannot be compiled**,
//! because structural verification requires a Stream block to contain at least
//! one yield. The distinction rests on a case valid source does not readily
//! produce, so it is recorded rather than tested.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_keleusma");

/// Parks forever: yields a wait reason with a wake time an hour out.
const PARK: &str = "loop main(_r: Word) -> (Word, Word) {\n\
                    \x20 let _ = yield (0, 3600000);\n\
                    \x20 (0, 0)\n\
                    }";

/// Faults on every dispatch, by dividing by a runtime zero.
const BOOM: &str = "loop main(_r: Word) -> (Word, Word) {\n\
                    \x20 let z = 0;\n\
                    \x20 let _ = 1 / z;\n\
                    \x20 let _ = yield (0, 3600000);\n\
                    \x20 (0, 0)\n\
                    }";

/// Yields the periodic reason, so the scheduler reschedules it at its period.
const PERIODIC: &str = "loop main(_r: Word) -> (Word, Word) {\n\
                        \x20 let _ = yield (3, 0);\n\
                        \x20 (0, 0)\n\
                        }";

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("keleusma_sched_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TmpDir(dir)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Compiles a task, failing loudly rather than leaving a test to run against a
/// manifest whose bytecode is not there.
fn compile(dir: &TmpDir, name: &str, src: &str) -> PathBuf {
    let s = dir.path(&format!("{name}.kel"));
    std::fs::write(&s, src).expect("write source");
    let out = dir.path(&format!("{name}.kbc"));
    let r = Command::new(BIN)
        .args(["compile", s.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .output()
        .expect("spawn compile");
    assert!(
        r.status.success(),
        "fixture {name} did not compile, so the test would exercise nothing:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    out
}

/// Runs a manifest for `dwell`, then stops it and returns the scheduler's
/// output.
///
/// Asserts the runner was still alive when the dwell elapsed. A runner that
/// died early has not demonstrated the behaviour under test, and reporting
/// that as a pass is the failure mode this suite keeps finding.
fn run_and_collect(manifest: &std::path::Path, dwell: Duration, expect_alive: bool) -> String {
    let mut child: Child = Command::new(BIN)
        .args(["run-tasks", manifest.to_str().unwrap()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn run-tasks");
    std::thread::sleep(dwell);
    let alive = child.try_wait().expect("try_wait").is_none();
    if alive {
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status();
    }
    let out = child.wait_with_output().expect("collect output");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if expect_alive {
        assert!(
            alive,
            "the runner exited before the dwell elapsed, so nothing below was \
             demonstrated:\n{text}"
        );
    }
    text
}

fn manifest(entries: &str) -> String {
    format!("[scheduler]\ntick_interval = \"10ms\"\nshutdown_grace = \"200ms\"\n\n{entries}")
}

fn task(name: &str, path: &std::path::Path, extra: &str) -> String {
    format!(
        "[[task]]\nname = \"{name}\"\nbytecode = \"{}\"\n{extra}\n",
        path.display()
    )
}

/// A faulting task restarts, is disabled once its limit is exceeded, and takes
/// neither the runner nor its sibling down with it.
///
/// This is the restart path, which re-allocates the arena and re-instantiates
/// the machine. Three restarts exercise it three times.
#[test]
fn a_faulting_task_restarts_then_is_disabled_without_disturbing_the_others() {
    let dir = TmpDir::new("restart");
    let boom = compile(&dir, "boom", BOOM);
    let park = compile(&dir, "park", PARK);
    let m = dir.path("m.toml");
    std::fs::write(
        &m,
        manifest(&format!(
            "{}{}",
            task(
                "boom",
                &boom,
                "restart = \"always\"\nrestart_limit = 3\nrestart_window = \"1m\""
            ),
            task("park", &park, "restart = \"never\"")
        )),
    )
    .expect("write manifest");

    // The runner must still be alive: a faulting task must not end it.
    let text = run_and_collect(&m, Duration::from_secs(3), true);

    let restarts = text.matches("task boom restarted").count();
    assert!(
        restarts >= 3,
        "expected the task to restart up to its limit of three; saw {restarts}:\n{text}"
    );
    assert!(
        text.contains("task boom disabled"),
        "the restart limit did not disable the task:\n{text}"
    );
    // The sibling is never reported as terminated or errored, which is the
    // structural consequence: one task's failure is contained.
    assert!(
        !text.contains("task park terminated") && !text.contains("task park error"),
        "the faulting task disturbed its sibling:\n{text}"
    );
}

/// A task configured never to restart is not restarted.
#[test]
fn a_task_set_to_never_restart_is_not_restarted() {
    let dir = TmpDir::new("never");
    let boom = compile(&dir, "boom", BOOM);
    let park = compile(&dir, "park", PARK);
    let m = dir.path("m.toml");
    std::fs::write(
        &m,
        manifest(&format!(
            "{}{}",
            task("boom", &boom, "restart = \"never\""),
            task("park", &park, "restart = \"never\"")
        )),
    )
    .expect("write manifest");

    let text = run_and_collect(&m, Duration::from_secs(2), true);
    assert!(
        text.contains("task boom terminated"),
        "the faulting task was not reported terminated:\n{text}"
    );
    assert_eq!(
        text.matches("task boom restarted").count(),
        0,
        "a task set never to restart was restarted:\n{text}"
    );
}

/// A periodically-scheduled task keeps running, without error or termination.
#[test]
fn a_periodic_task_keeps_being_scheduled() {
    let dir = TmpDir::new("periodic");
    let per = compile(&dir, "per", PERIODIC);
    let m = dir.path("m.toml");
    std::fs::write(
        &m,
        manifest(&task("per", &per, "period = \"50ms\"\nrestart = \"never\"")),
    )
    .expect("write manifest");

    let text = run_and_collect(&m, Duration::from_secs(2), true);
    assert!(
        !text.contains("task per error"),
        "the periodic task faulted:\n{text}"
    );
    assert!(
        !text.contains("task per terminated"),
        "the periodic task stopped being scheduled:\n{text}"
    );
}
