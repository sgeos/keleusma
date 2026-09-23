//! Manifest reload on the hangup signal (backlog B31 item 4, first slice).
//!
//! # What is asserted, and why it is timing rather than text
//!
//! The reload's effect is observed through **behaviour**: `shutdown_grace` is
//! consulted when the runner is asked to stop, so changing it changes how long
//! the process takes to exit. Asserting on a log line would pass equally well
//! against a runner that printed the message and applied nothing.
//!
//! The control test is what makes the other one mean anything. Without it, a
//! runner that ignored `shutdown_grace` entirely and exited immediately would
//! satisfy the "reload applied" assertion.
//!
//! # The property an operator depends on
//!
//! A manifest that cannot be parsed leaves the runner **running**, with its
//! previous configuration intact. That is the behaviour worth having, not a
//! fallback: a bad edit must not take down a live runner.
//!
//! # Platform
//!
//! Unix only. `signals.rs` records that the hangup signal has no Windows
//! equivalent and that its flag is never set there, so nothing here claims
//! anything about Windows.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_keleusma");

/// A task that parks: it yields a wait reason with a wake time an hour out, so
/// the runner stays alive without spinning and a shutdown must wait out the
/// grace period rather than ending early because the task finished.
const TASK_SRC: &str = "loop main(_reason: Word) -> (Word, Word) {\n\
                        \x20 let _ = yield (0, 3600000);\n\
                        \x20 (0, 0)\n\
                        }";

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("keleusma_reload_{}_{}", std::process::id(), tag));
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

/// Compiles the parking task and returns the artifact path.
fn build_task(dir: &TmpDir) -> PathBuf {
    let src = dir.path("task.kel");
    let out = dir.path("task.kbc");
    std::fs::write(&src, TASK_SRC).expect("write task source");
    let r = Command::new(BIN)
        .args([
            "compile",
            src.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("spawn compile");
    assert!(
        r.status.success(),
        "the fixture task did not compile, so every test here would be testing nothing:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    out
}

fn manifest_text(task: &std::path::Path, grace: &str) -> String {
    format!(
        "[scheduler]\ntick_interval = \"10ms\"\nshutdown_grace = \"{grace}\"\n\n\
         [[task]]\nname = \"parker\"\nbytecode = \"{}\"\nrestart = \"never\"\n",
        task.display()
    )
}

fn spawn(manifest: &std::path::Path) -> Child {
    Command::new(BIN)
        .args(["run-tasks", manifest.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn run-tasks")
}

fn signal(child: &Child, sig: &str) {
    let r = Command::new("kill")
        .args([sig, &child.id().to_string()])
        .status()
        .expect("spawn kill");
    assert!(r.success(), "failed to deliver {sig}");
}

/// Waits for the runner to come up, failing rather than proceeding blind.
fn wait_until_running(child: &mut Child) {
    for _ in 0..100 {
        if child.try_wait().expect("try_wait").is_none() {
            std::thread::sleep(Duration::from_millis(150));
            if child.try_wait().expect("try_wait").is_none() {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the runner exited before the test could signal it; nothing was exercised");
}

/// Stops the runner and returns how long it took to exit.
fn terminate_and_time(child: &mut Child) -> Duration {
    let t0 = Instant::now();
    signal(child, "-TERM");
    let _ = child.wait().expect("wait");
    t0.elapsed()
}

/// A task loads, runs and stays alive.
///
/// This is the regression guard for two defects that each made `run-tasks`
/// unable to run ANY task, and that survived because nothing exercised the
/// subcommand end to end.
///
/// The first: `load_task` took a `&'static Arena` from a by-value local and
/// then MOVED the arena into the returned task, so the virtual machine read a
/// stale address. It presented as an arena reporting a few dozen bytes of
/// capacity and every task failing on its first composite allocation. The
/// safety comment reasoned about the arena staying alive, which is true and
/// insufficient: moving it invalidates the reference.
///
/// The second: the yielded `(reason, payload)` pair was decoded without the
/// arena. Since B28 a yielded tuple's body is flat and arena-resident, so the
/// context-free decode saw a value it could not read and the scheduler treated
/// every task as having "yielded a non-tuple value" and finished it.
///
/// Either defect alone ends the process within a second of start, so a test
/// that merely spawns the runner and asserts it is still alive catches both.
#[test]
fn a_task_loads_and_keeps_running() {
    let dir = TmpDir::new("runs");
    let task = build_task(&dir);
    let manifest = dir.path("tasks.toml");
    std::fs::write(&manifest, manifest_text(&task, "1s")).expect("write manifest");

    let mut child = spawn(&manifest);
    std::thread::sleep(Duration::from_millis(1200));
    let alive = child.try_wait().expect("try_wait").is_none();
    if !alive {
        panic!(
            "the runner exited on its own within 1.2 seconds. A parked task should keep it              alive indefinitely, so this is the arena-move or the yield-decode defect              returning."
        );
    }
    let _ = terminate_and_time(&mut child);
}

/// A reload applies a changed scheduler setting, observed by how long a
/// shutdown then takes.
#[test]
fn a_reload_applies_a_changed_shutdown_grace() {
    let dir = TmpDir::new("apply");
    let task = build_task(&dir);
    let manifest = dir.path("tasks.toml");
    std::fs::write(&manifest, manifest_text(&task, "6s")).expect("write manifest");

    let mut child = spawn(&manifest);
    wait_until_running(&mut child);

    std::fs::write(&manifest, manifest_text(&task, "200ms")).expect("rewrite manifest");
    signal(&child, "-HUP");
    std::thread::sleep(Duration::from_millis(400));

    let elapsed = terminate_and_time(&mut child);
    assert!(
        elapsed < Duration::from_secs(3),
        "the shortened grace was not applied: shutdown took {elapsed:?}, which is the old \
         six-second grace rather than the reloaded two hundred milliseconds"
    );
}

/// The control, without which the test above proves nothing.
///
/// A runner that ignored `shutdown_grace` altogether and exited at once would
/// satisfy that assertion. This establishes that the long grace really is
/// observed when no reload happens, so the short exit above is attributable to
/// the reload.
#[test]
fn without_a_reload_the_original_grace_still_governs() {
    let dir = TmpDir::new("control");
    let task = build_task(&dir);
    let manifest = dir.path("tasks.toml");
    std::fs::write(&manifest, manifest_text(&task, "6s")).expect("write manifest");

    let mut child = spawn(&manifest);
    wait_until_running(&mut child);

    // The manifest is rewritten exactly as in the test above, and no hangup is
    // sent. If the runner picked the change up anyway, the reload would not be
    // what caused the short exit there.
    std::fs::write(&manifest, manifest_text(&task, "200ms")).expect("rewrite manifest");
    std::thread::sleep(Duration::from_millis(400));

    let elapsed = terminate_and_time(&mut child);
    assert!(
        elapsed > Duration::from_secs(3),
        "the runner stopped in {elapsed:?} without being told to reload, so the other test's \
         short shutdown is not evidence that the reload applied anything"
    );
}

/// A manifest that cannot be parsed leaves the runner running.
#[test]
fn an_unparseable_manifest_does_not_take_the_runner_down() {
    let dir = TmpDir::new("refuse");
    let task = build_task(&dir);
    let manifest = dir.path("tasks.toml");
    std::fs::write(&manifest, manifest_text(&task, "6s")).expect("write manifest");

    let mut child = spawn(&manifest);
    wait_until_running(&mut child);

    std::fs::write(&manifest, "this is not valid TOML [[[\n").expect("write bad manifest");
    signal(&child, "-HUP");
    std::thread::sleep(Duration::from_millis(500));

    assert!(
        child.try_wait().expect("try_wait").is_none(),
        "a manifest that does not parse took the runner down; the running configuration must \
         be kept instead"
    );

    // And the configuration it kept is the old one: the shutdown still waits
    // out the original grace rather than some partially-applied value.
    let elapsed = terminate_and_time(&mut child);
    assert!(
        elapsed > Duration::from_secs(3),
        "after a refused reload the shutdown took {elapsed:?}, so something was applied from a \
         manifest that did not parse"
    );
}

/// A manifest file that has been removed is refused the same way.
#[test]
fn a_missing_manifest_does_not_take_the_runner_down() {
    let dir = TmpDir::new("missing");
    let task = build_task(&dir);
    let manifest = dir.path("tasks.toml");
    std::fs::write(&manifest, manifest_text(&task, "6s")).expect("write manifest");

    let mut child = spawn(&manifest);
    wait_until_running(&mut child);

    std::fs::remove_file(&manifest).expect("remove manifest");
    signal(&child, "-HUP");
    std::thread::sleep(Duration::from_millis(500));

    assert!(
        child.try_wait().expect("try_wait").is_none(),
        "removing the manifest took the runner down; it must keep running on the configuration \
         it already has"
    );
    let _ = terminate_and_time(&mut child);
}
