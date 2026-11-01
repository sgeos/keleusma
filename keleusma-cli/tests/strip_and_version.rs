//! The success paths of the subcommands a coverage census found unexercised.
//!
//! # Why these two
//!
//! The previous increment found `run-tasks` completely non-functional, and the
//! reason was that it had no end-to-end test at all. That is a confirmed
//! class, not a worry. Counting how often each subcommand is actually invoked
//! by a test that runs the binary:
//!
//! | subcommand | invocations |
//! |---|---|
//! | `run` | 24 |
//! | `compile` | 14 |
//! | `keygen` | 3 |
//! | `strip` | 1, and a REFUSAL case only |
//! | `version` | 0 |
//!
//! `strip` is the same shape as `run-tasks` was: a real transformation whose
//! success path nothing exercised. It works today, which is measured rather
//! than assumed, and is exactly when a guard is cheap to add.
//!
//! # What is asserted
//!
//! What the subcommand is FOR, not that the process exited. `bad_input.rs`
//! already owns the refusal paths.
//!
//! The strong property is **byte identity**: stripping a debug-built artifact
//! yields exactly the artifact a non-debug build produces. That pins the real
//! contract — debug metadata is strippable without residue — where "the
//! stripped artifact still runs" would pass against a strip that removed
//! nothing, or that removed something else as well.
//!
//! Bytes are compared rather than sizes. Two artifacts of equal length can
//! differ, and the bytes are right there.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_keleusma");

/// A program with a helper call, so a debug build has spans and names to carry
/// and stripping has something to remove.
const SRC: &str = "fn helper(a: Word) -> Word { a * 2 }\nfn main() -> Word { helper(21) }\n";

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("keleusma_strip_{}_{}", std::process::id(), tag));
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

/// Compiles `SRC`, with or without debug metadata, and returns the artifact.
///
/// Fails loudly when the fixture does not build. A test that silently skipped
/// a missing input would pass while exercising nothing, which is the failure
/// this suite keeps finding.
fn compile(dir: &TmpDir, name: &str, debug: bool) -> PathBuf {
    let src = dir.path("p.kel");
    std::fs::write(&src, SRC).expect("write source");
    let out = dir.path(name);
    let mut cmd = Command::new(BIN);
    cmd.arg("compile").arg(&src);
    if debug {
        cmd.arg("--debug");
    }
    cmd.arg("-o").arg(&out);
    let r = cmd.output().expect("spawn compile");
    assert!(
        r.status.success(),
        "the fixture did not compile, so this test would exercise nothing:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(
        out.is_file(),
        "compile reported success but wrote no artifact"
    );
    out
}

fn strip(dir: &TmpDir, input: &std::path::Path, name: &str) -> PathBuf {
    let out = dir.path(name);
    let r = Command::new(BIN)
        .arg("strip")
        .arg(input)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn strip");
    assert!(
        r.status.success(),
        "strip failed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(
        out.is_file(),
        "strip reported success but wrote no artifact"
    );
    out
}

fn run_artifact(path: &std::path::Path) -> String {
    let r = Command::new(BIN)
        .arg("run")
        .arg(path)
        .output()
        .expect("spawn run");
    assert!(
        r.status.success(),
        "the artifact did not run:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    String::from_utf8_lossy(&r.stdout).trim().to_string()
}

/// Stripping a debug build yields exactly the non-debug build.
///
/// The contract, stated as bytes. A strip that removed nothing would leave the
/// artifact larger; a strip that removed too much would leave it smaller or
/// different. Only an exact match says the debug metadata is separable without
/// residue.
#[test]
fn stripping_a_debug_artifact_reproduces_the_plain_one_byte_for_byte() {
    let dir = TmpDir::new("identity");
    let plain = compile(&dir, "plain.kbc", false);
    let debug = compile(&dir, "debug.kbc", true);

    let plain_bytes = std::fs::read(&plain).expect("read plain");
    let debug_bytes = std::fs::read(&debug).expect("read debug");
    assert!(
        debug_bytes.len() > plain_bytes.len(),
        "the debug build is not larger than the plain one ({} vs {}), so the fixture carries \
         no debug metadata and stripping it would prove nothing",
        debug_bytes.len(),
        plain_bytes.len()
    );

    let stripped = strip(&dir, &debug, "stripped.kbc");
    let stripped_bytes = std::fs::read(&stripped).expect("read stripped");
    assert_eq!(
        stripped_bytes,
        plain_bytes,
        "stripping the debug artifact did not reproduce the plain one. Lengths were {} \
         stripped against {} plain.",
        stripped_bytes.len(),
        plain_bytes.len()
    );
}

/// Stripping an already-stripped artifact changes nothing.
#[test]
fn stripping_is_idempotent() {
    let dir = TmpDir::new("idempotent");
    let debug = compile(&dir, "debug.kbc", true);
    let once = strip(&dir, &debug, "once.kbc");
    let twice = strip(&dir, &once, "twice.kbc");
    assert_eq!(
        std::fs::read(&once).expect("read once"),
        std::fs::read(&twice).expect("read twice"),
        "a second strip changed the artifact, so stripping is not idempotent"
    );
}

/// A stripped artifact still runs, and computes what the unstripped one did.
///
/// Byte identity above already implies this, since the plain artifact runs.
/// It is asserted separately because the identity property is about the
/// ENCODING and this is about the artifact remaining executable, and a future
/// change could preserve one while breaking the other.
#[test]
fn a_stripped_artifact_still_runs_and_agrees() {
    let dir = TmpDir::new("runs");
    let debug = compile(&dir, "debug.kbc", true);
    let stripped = strip(&dir, &debug, "stripped.kbc");
    let before = run_artifact(&debug);
    let after = run_artifact(&stripped);
    assert_eq!(
        before, after,
        "the stripped artifact computed a different result from the unstripped one"
    );
    assert_eq!(after, "42", "the fixture no longer computes what it did");
}

/// The reported version is the crate's version.
///
/// Guards a hardcoded string drifting from the manifest, which is invisible
/// until someone reads a release note against a binary.
#[test]
fn the_reported_version_matches_the_crate() {
    let r = Command::new(BIN)
        .arg("version")
        .output()
        .expect("spawn version");
    assert!(r.status.success(), "version exited non-zero");
    let out = String::from_utf8_lossy(&r.stdout);
    let want = env!("CARGO_PKG_VERSION");
    assert!(
        out.contains(want),
        "the binary reports {out:?} but the crate version is {want:?}"
    );
}
