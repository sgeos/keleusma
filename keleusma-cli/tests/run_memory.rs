//! Integration tests for the run path's memory behavior: the
//! `--print-memory` worst-case footprint report and the guarantee that it
//! reports without executing the program. The fallible arena allocation
//! itself (out-of-memory becoming a clean error rather than an abort) is
//! unit-tested in `keleusma-arena`, since forcing a real allocation failure
//! through the auto-sized CLI is not practical.
//!
//! The tests are hermetic: they point the key-store env vars at empty temp
//! directories and clear the force-strict vars, so strict mode is off and a
//! source `.kel` script runs regardless of the host's key stores.

use std::path::PathBuf;
use std::process::Command;

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("keleusma_mem_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TmpDir(dir)
    }
    fn s(&self, name: &str) -> String {
        self.0.join(name).to_str().expect("utf8 path").to_string()
    }
    fn mkdir(&self, name: &str) -> String {
        let p = self.0.join(name);
        std::fs::create_dir_all(&p).expect("mkdir");
        p.to_str().expect("utf8 path").to_string()
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run `keleusma run <script> [extra]` with strict mode disabled via empty
/// key stores. Returns `(success, stdout)`.
fn run(script: &str, extra: &[&str], trust: &str, dec: &str) -> (bool, String) {
    let mut args = vec!["run", script];
    args.extend_from_slice(extra);
    let out = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .args(&args)
        .env_remove("KELEUSMA_REQUIRE_SIGNED")
        .env_remove("KELEUSMA_REQUIRE_ENCRYPTED")
        .env("KELEUSMA_TRUSTED_KEYS_DIR", trust)
        .env("KELEUSMA_DECRYPTION_KEYS_DIR", dec)
        .output()
        .expect("spawn keleusma");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn print_memory_reports_footprint_and_does_not_execute() {
    let dir = TmpDir::new("print");
    let trust = dir.mkdir("trust_empty");
    let dec = dir.mkdir("dec_empty");
    let script = dir.s("h.kel");
    // `main` returns 42; a normal run prints it, so its absence proves the
    // program did not execute under --print-memory.
    std::fs::write(&script, "fn main() -> Word { 42 }\n").expect("write script");

    let (ok, out) = run(&script, &["--print-memory"], &trust, &dec);
    assert!(ok, "--print-memory should exit 0; stdout: {out}");
    assert!(
        out.contains("arena:") && out.contains("bytes total"),
        "expected a footprint report; stdout: {out}"
    );
    assert!(
        !out.contains("42"),
        "the program must not execute under --print-memory; stdout: {out}"
    );
}

#[test]
fn normal_run_executes_and_prints_the_result() {
    let dir = TmpDir::new("normal");
    let trust = dir.mkdir("trust_empty");
    let dec = dir.mkdir("dec_empty");
    let script = dir.s("h.kel");
    std::fs::write(&script, "fn main() -> Word { 42 }\n").expect("write script");

    let (ok, out) = run(&script, &[], &trust, &dec);
    assert!(ok, "normal run should exit 0; stdout: {out}");
    assert!(out.contains("42"), "expected the result; stdout: {out}");
}
