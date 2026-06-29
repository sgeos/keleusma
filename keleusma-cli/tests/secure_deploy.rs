//! Black-box integration tests for the secure signed + encrypted
//! deployment flow: keygen, signed/encrypted compile, run, and the
//! strict-mode policy gates.
//!
//! Each test is hermetic. It points the trust-store and decryption-key
//! store at its own temp directories and clears any ambient `KELEUSMA_*`
//! policy variables, so it never depends on (or touches) the host's key
//! stores and is unaffected by the developer's environment. Strict mode is
//! activated by enrolling a key into the trust directory, matching how a
//! deployment would be configured.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_keleusma");

/// A unique temp directory, removed on drop.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("keleusma_secdeploy_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TmpDir(dir)
    }
    /// Path of `name` under this dir.
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    /// Path of `name` as an owned `String` for use as a CLI argument.
    fn s(&self, name: &str) -> String {
        self.path(name).to_str().expect("utf8 path").to_string()
    }
    /// Create a subdirectory `name` and return its path as a `String`.
    fn mkdir(&self, name: &str) -> String {
        let p = self.path(name);
        std::fs::create_dir_all(&p).expect("mkdir");
        p.to_str().expect("utf8 path").to_string()
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct RunResult {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Run the keleusma binary with `args` and `env`, clearing any ambient
/// `KELEUSMA_*` policy variables first so the runner's environment cannot
/// influence the result.
fn keleusma(args: &[&str], env: &[(&str, &str)]) -> RunResult {
    let mut cmd = Command::new(BIN);
    cmd.args(args)
        .env_remove("KELEUSMA_TRUSTED_KEYS_DIR")
        .env_remove("KELEUSMA_REQUIRE_SIGNED")
        .env_remove("KELEUSMA_DECRYPTION_KEYS_DIR")
        .env_remove("KELEUSMA_REQUIRE_ENCRYPTED");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn keleusma");
    RunResult {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Write a trivial source program and generate an Ed25519 signing keypair
/// and an X25519 encryption keypair in `dir` (`sign.seed`/`sign.pub`,
/// `dest.seed`/`dest.pub`, and `hello.kel`).
fn setup(dir: &TmpDir) {
    std::fs::write(dir.path("hello.kel"), "fn main() -> Word { 42 }\n").expect("write source");
    let r = keleusma(
        &[
            "keygen",
            "--seed",
            dir.s("sign.seed").as_str(),
            "--public",
            dir.s("sign.pub").as_str(),
        ],
        &[],
    );
    assert_eq!(r.code, 0, "keygen signing failed: {}", r.stderr);
    let r = keleusma(
        &[
            "keygen",
            "--kind",
            "encryption",
            "--seed",
            dir.s("dest.seed").as_str(),
            "--public",
            dir.s("dest.pub").as_str(),
        ],
        &[],
    );
    assert_eq!(r.code, 0, "keygen encryption failed: {}", r.stderr);
}

#[test]
fn happy_path_sign_encrypt_run_with_supplied_keys() {
    let dir = TmpDir::new("happy");
    setup(&dir);
    let empty_trust = dir.mkdir("trust_empty");
    let empty_dec = dir.mkdir("dec_empty");
    let r = keleusma(
        &[
            "compile",
            dir.s("hello.kel").as_str(),
            "--signing-key",
            dir.s("sign.seed").as_str(),
            "--encryption-key",
            dir.s("dest.pub").as_str(),
            "-o",
            dir.s("hello.bin").as_str(),
        ],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", empty_trust.as_str())],
    );
    assert_eq!(r.code, 0, "compile failed: {}", r.stderr);
    // Empty key stores => strict off; the supplied keys verify and decrypt.
    let r = keleusma(
        &[
            "run",
            dir.s("hello.bin").as_str(),
            "--verifying-key",
            dir.s("sign.pub").as_str(),
            "--decryption-key",
            dir.s("dest.seed").as_str(),
        ],
        &[
            ("KELEUSMA_TRUSTED_KEYS_DIR", empty_trust.as_str()),
            ("KELEUSMA_DECRYPTION_KEYS_DIR", empty_dec.as_str()),
        ],
    );
    assert_eq!(r.code, 0, "run failed: {}\n{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("42"), "stdout: {}", r.stdout);
}

#[test]
fn strict_mode_runs_enrolled_signed_encrypted_artifact() {
    let dir = TmpDir::new("strict_ok");
    setup(&dir);
    let trust = dir.mkdir("trust");
    let dec = dir.mkdir("dec");
    std::fs::copy(dir.path("sign.pub"), dir.path("trust").join("sign.pub")).expect("enroll signer");
    std::fs::copy(dir.path("dest.seed"), dir.path("dec").join("dest.seed")).expect("enroll dec");
    let r = keleusma(
        &[
            "compile",
            dir.s("hello.kel").as_str(),
            "--signing-key",
            dir.s("sign.seed").as_str(),
            "--encryption-key",
            dir.s("dest.pub").as_str(),
            "-o",
            dir.s("hello.bin").as_str(),
        ],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str())],
    );
    assert_eq!(r.code, 0, "compile failed: {}", r.stderr);
    // Non-empty stores => strict on; no command-line keys supplied.
    let r = keleusma(
        &["run", dir.s("hello.bin").as_str()],
        &[
            ("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str()),
            ("KELEUSMA_DECRYPTION_KEYS_DIR", dec.as_str()),
        ],
    );
    assert_eq!(r.code, 0, "strict run failed: {}\n{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("42"), "stdout: {}", r.stdout);
}

#[test]
fn strict_mode_rejects_source_execution() {
    let dir = TmpDir::new("reject_src");
    setup(&dir);
    let trust = dir.mkdir("trust");
    std::fs::copy(dir.path("sign.pub"), dir.path("trust").join("sign.pub")).expect("enroll");
    let r = keleusma(
        &["run", dir.s("hello.kel").as_str()],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str())],
    );
    assert_ne!(
        r.code, 0,
        "source run should be rejected; stdout: {}",
        r.stdout
    );
    assert!(
        r.stderr.contains("source execution disabled"),
        "stderr: {}",
        r.stderr
    );
}

#[test]
fn strict_mode_rejects_unsigned_bytecode() {
    let dir = TmpDir::new("reject_unsigned");
    setup(&dir);
    let trust = dir.mkdir("trust");
    std::fs::copy(dir.path("sign.pub"), dir.path("trust").join("sign.pub")).expect("enroll");
    let r = keleusma(
        &[
            "compile",
            dir.s("hello.kel").as_str(),
            "-o",
            dir.s("plain.bin").as_str(),
        ],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str())],
    );
    assert_eq!(r.code, 0, "compile failed: {}", r.stderr);
    let r = keleusma(
        &["run", dir.s("plain.bin").as_str()],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str())],
    );
    assert_ne!(
        r.code, 0,
        "unsigned run should be rejected; stdout: {}",
        r.stdout
    );
    assert!(
        r.stderr.contains("unsigned bytecode disabled"),
        "stderr: {}",
        r.stderr
    );
}

#[test]
fn strict_mode_rejects_command_line_verifying_key() {
    let dir = TmpDir::new("reject_cli_key");
    setup(&dir);
    let trust = dir.mkdir("trust");
    std::fs::copy(dir.path("sign.pub"), dir.path("trust").join("sign.pub")).expect("enroll");
    let r = keleusma(
        &[
            "compile",
            dir.s("hello.kel").as_str(),
            "--signing-key",
            dir.s("sign.seed").as_str(),
            "-o",
            dir.s("signed.bin").as_str(),
        ],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str())],
    );
    assert_eq!(r.code, 0, "compile failed: {}", r.stderr);
    // An operator cannot relax the system-managed trust list at the CLI.
    let r = keleusma(
        &[
            "run",
            dir.s("signed.bin").as_str(),
            "--verifying-key",
            dir.s("sign.pub").as_str(),
        ],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str())],
    );
    assert_ne!(
        r.code, 0,
        "command-line verifying-key should be rejected; stdout: {}",
        r.stdout
    );
    assert!(
        r.stderr.contains("--verifying-key is rejected"),
        "stderr: {}",
        r.stderr
    );
}

#[test]
fn strict_mode_rejects_artifact_signed_by_non_enrolled_key() {
    let dir = TmpDir::new("reject_attacker");
    setup(&dir);
    // An attacker signing key that is never enrolled.
    let r = keleusma(
        &[
            "keygen",
            "--seed",
            dir.s("attacker.seed").as_str(),
            "--public",
            dir.s("attacker.pub").as_str(),
        ],
        &[],
    );
    assert_eq!(r.code, 0, "attacker keygen failed: {}", r.stderr);
    let trust = dir.mkdir("trust");
    let dec = dir.mkdir("dec");
    // Enroll only the legitimate signer and the decryption key.
    std::fs::copy(dir.path("sign.pub"), dir.path("trust").join("sign.pub")).expect("enroll signer");
    std::fs::copy(dir.path("dest.seed"), dir.path("dec").join("dest.seed")).expect("enroll dec");
    // Sign with the attacker key but encrypt to the enrolled recipient, so
    // decryption succeeds and only the signature check should refuse it.
    let r = keleusma(
        &[
            "compile",
            dir.s("hello.kel").as_str(),
            "--signing-key",
            dir.s("attacker.seed").as_str(),
            "--encryption-key",
            dir.s("dest.pub").as_str(),
            "-o",
            dir.s("evil.bin").as_str(),
        ],
        &[("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str())],
    );
    assert_eq!(r.code, 0, "compile failed: {}", r.stderr);
    let r = keleusma(
        &["run", dir.s("evil.bin").as_str()],
        &[
            ("KELEUSMA_TRUSTED_KEYS_DIR", trust.as_str()),
            ("KELEUSMA_DECRYPTION_KEYS_DIR", dec.as_str()),
        ],
    );
    assert_ne!(
        r.code, 0,
        "non-enrolled-signer artifact should be rejected; stdout: {}",
        r.stdout
    );
    // The refusal is a strict-mode policy error. The precise wording
    // (signature failure versus decryption-key mismatch) is tightened by the
    // diagnostic-clarity fix; this pins that the artifact is refused.
    assert!(r.stderr.contains("strict mode"), "stderr: {}", r.stderr);
}
