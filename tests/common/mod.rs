//! Shared support for the integration-test suite.
//!
//! Lives in a subdirectory of `tests/`, so Cargo does not compile it as its own
//! test binary; it is pulled into a test file with `mod common;`.
#![allow(dead_code)]

/// Fast-lane memoization for the expensive whole-stage self-compile byte-identity
/// tests (process-audit item 3, the "complete key" form).
///
/// These tests interpret a whole ~200 KB `.kel` stage on the VM and assert the
/// result is byte-identical to the Rust reference compiler. That is the dominant
/// cost of the inner loop, and re-running a stage whose inputs did not change is
/// wasted work.
///
/// # Blast radius
///
/// Active ONLY when the environment variable `KEL_SELFHOST_CACHE=1` is set, which
/// `scripts/fast-check.sh` does for the developer inner loop. The pre-push hook,
/// `scripts/release-gate.sh`, and CI leave it unset, so the full corpus always
/// runs for real. A cache defect can therefore at worst yield a false green in a
/// developer's inner loop, never in a gate.
///
/// # Why the key is complete
///
/// This is a DIFFERENTIAL ORACLE: it compares two independently produced results,
/// the `.kel`-on-VM output and the Rust reference output, and asserts they match.
/// A sound cache key must be a function of EVERY input that can move either result.
/// A key over the changed `.kel` stage alone (what the audit first sketched) is
/// incomplete: the reference compiler, the VM, and the wire format are hidden
/// inputs to both sides, so a change to `src/` with the `.kel` untouched could
/// silently mask a real divergence. This key closes that gap by combining:
///
/// 1. **the test binary identity** (`current_exe` size and mtime): any change to
///    `src/` rebuilds the binary and bumps its mtime, so this captures the entire
///    Rust reference compiler, the VM, the wire format, and `BYTECODE_VERSION` in
///    one component; and
/// 2. **the content of every `.kel` file the test reads**, by full content hash so
///    an editor that preserves mtime cannot hide an edit.
///
/// Callers must pass their COMPLETE `.kel` input set. Over-approximating it is
/// safe, since it only forces a redundant re-run; UNDER-approximating it is the one
/// unsound mistake, so when in doubt pass every stage file.
pub mod selfhost_cache {
    use std::hash::{Hash, Hasher};

    fn enabled() -> bool {
        std::env::var("KEL_SELFHOST_CACHE").ok().as_deref() == Some("1")
    }

    /// Cache directory, scoped to this build's target temp dir so `cargo clean`
    /// wipes it and stale markers never survive a rebuild.
    fn cache_dir() -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("selfhost-stage-cache");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// The complete key, or `None` when caching is disabled or any input `.kel`
    /// cannot be read. Returning `None` forces the real test to run, which will
    /// itself surface any read error.
    ///
    /// `DefaultHasher::new()` uses fixed keys, so the digest is stable across the
    /// separate processes that `record_pass` and `hit` run in under nextest.
    fn complete_key(cache_id: &str, kel_inputs: &[&str]) -> Option<u64> {
        if !enabled() {
            return None;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        cache_id.hash(&mut h);

        // (1) Binary identity — captures every Rust-side input at once.
        let exe = std::env::current_exe().ok()?;
        exe.to_string_lossy().hash(&mut h);
        let md = std::fs::metadata(&exe).ok()?;
        md.len().hash(&mut h);
        md.modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos()
            .hash(&mut h);

        // (2) Content of every .kel input, order-independent.
        let mut inputs = kel_inputs.to_vec();
        inputs.sort_unstable();
        for p in inputs {
            p.hash(&mut h);
            let bytes = std::fs::read(p).ok()?; // unreadable input -> None -> no cache
            bytes.hash(&mut h);
        }
        Some(h.finish())
    }

    fn marker(key: u64) -> std::path::PathBuf {
        cache_dir().join(format!("{key:016x}.pass"))
    }

    /// True when the caller may SKIP the expensive self-compile: caching is enabled
    /// and a PASS was recorded under the identical complete key.
    pub fn hit(cache_id: &str, kel_inputs: &[&str]) -> bool {
        match complete_key(cache_id, kel_inputs) {
            Some(k) => marker(k).exists(),
            None => false,
        }
    }

    /// Record that the caller's byte-identity assertion PASSED under the current
    /// complete key, so a future run with identical inputs may skip. No-op when
    /// caching is disabled.
    pub fn record_pass(cache_id: &str, kel_inputs: &[&str]) {
        if let Some(k) = complete_key(cache_id, kel_inputs) {
            let _ = std::fs::write(marker(k), b"");
        }
    }
}
