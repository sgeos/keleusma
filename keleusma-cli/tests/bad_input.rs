//! How the shipping binary behaves on input a user supplies by mistake.
//!
//! # Why this exists
//!
//! Everything the runtime hardens — the verifier, the hostile-bytecode corpus,
//! the fault census — sits behind this interface, and a panic here is a defect
//! a user hits on their first typo. Unlike hostile bytecode it needs no
//! attacker: a missing file, a half-written script, a truncated artifact.
//!
//! # The contract each case asserts
//!
//! A bad input exits non-zero and does not panic. The **exit status** is the
//! signal, not the message: classifying an outcome by searching output text
//! when a status is available is the crude instrument this repository has
//! repeatedly found to miscount.
//!
//! **A case that is expected to fail and instead succeeds is a failure of this
//! file**, not a pass. An unfilled cell reported as a clean result is the
//! failure mode the suite keeps paying for, so the expectation is asserted in
//! both directions and the controls below must succeed.
//!
//! # One trap, paid for once
//!
//! The panic detector must NOT look for the free text "stack overflow". The
//! parser's own recursion guard reports "deeply nested expressions are
//! rejected to prevent stack overflow", which is a **correct refusal**, and a
//! detector matching that phrase reports the healthiest possible outcome as a
//! crash. Measured while writing this file.
//!
//! # What this found
//!
//! Nothing. Across the inputs below the binary exits non-zero with an
//! actionable message and never panics. That is a result about these inputs,
//! which is why they are enumerated rather than summarised.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_keleusma");

/// Markers of an abnormal exit.
///
/// Deliberately narrow. See the module note: the obvious extra marker matches
/// a legitimate error message.
const PANIC_MARKERS: &[&str] = &["panicked at", "fatal runtime error", "RUST_BACKTRACE=1"];

fn scratch() -> PathBuf {
    let d = std::env::temp_dir().join("keleusma-cli-bad-input");
    fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn write(name: &str, bytes: &[u8]) -> PathBuf {
    let p = scratch().join(name);
    fs::write(&p, bytes).expect("write fixture");
    p
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("spawn keleusma")
}

/// Asserts the invocation failed cleanly: non-zero status, no panic marker.
fn refused(what: &str, args: &[&str]) {
    let out = run(args);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for m in PANIC_MARKERS {
        assert!(
            !text.contains(m),
            "{what}: the binary panicked (marker {m:?}):\n{text}"
        );
    }
    assert!(
        !out.status.success(),
        "{what}: expected a non-zero exit and got success. This is an UNFILLED \
         CELL, not a pass: the input was supposed to be refused.\n{text}"
    );
}

/// Asserts the invocation succeeded, so the file has controls and a universal
/// refusal could not satisfy it.
fn accepted(what: &str, args: &[&str]) {
    let out = run(args);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.success(),
        "{what}: expected success, got {:?}\n{text}",
        out.status.code()
    );
}

fn good_script() -> PathBuf {
    write("good.kel", b"fn main() -> Word { 42 }")
}

/// Compiles the good script and returns the artifact path.
fn good_artifact() -> PathBuf {
    let src = good_script();
    let out = scratch().join("good.kbc");
    let r = run(&[
        "compile",
        src.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(r.status.success(), "fixture compile failed");
    out
}

// ---------------------------------------------------------------------------

/// Source the compiler cannot accept.
#[test]
fn malformed_and_unreadable_source_is_refused() {
    let truncated = write("malformed.kel", b"fn main() -> Word { 1 +");
    let garbage = write("garbage.kel", b"not keleusma at all @@@ \x00\x01\x02");
    let empty = write("empty.kel", b"");
    let missing = scratch().join("does-not-exist.kel");
    let dir = scratch().join("a-directory");
    fs::create_dir_all(&dir).expect("dir fixture");

    refused("half-written script", &["run", truncated.to_str().unwrap()]);
    refused("not a script at all", &["run", garbage.to_str().unwrap()]);
    refused("empty file", &["run", empty.to_str().unwrap()]);
    refused("missing file", &["run", missing.to_str().unwrap()]);
    refused(
        "a directory where a file belongs",
        &["run", dir.to_str().unwrap()],
    );
    refused(
        "compile a missing file",
        &["compile", missing.to_str().unwrap()],
    );
}

/// Bytes that are not a valid compiled artifact.
#[test]
fn a_damaged_artifact_is_refused_rather_than_run() {
    let good = good_artifact();
    let bytes = fs::read(&good).expect("read artifact");
    assert!(bytes.len() > 64, "fixture artifact is implausibly small");

    let truncated = write("truncated.kbc", &bytes[..40]);
    let mut flipped_bytes = bytes.clone();
    let mid = flipped_bytes.len() / 2;
    flipped_bytes[mid] ^= 0xFF;
    let flipped = write("flipped.kbc", &flipped_bytes);
    let random = write("random.bin", &[0xA5u8; 4096]);

    refused("truncated artifact", &["run", truncated.to_str().unwrap()]);
    refused("single flipped byte", &["run", flipped.to_str().unwrap()]);
    refused("random bytes", &["run", random.to_str().unwrap()]);
    refused(
        "strip a truncated artifact",
        &["strip", truncated.to_str().unwrap()],
    );
}

/// Flags given no argument, or an argument that is not valid.
///
/// A flag consuming the next argument is the classic place for an
/// out-of-bounds index, which is why every flag that takes one is exercised
/// with nothing after it.
#[test]
fn flags_missing_or_malformed_arguments_are_refused() {
    let src = good_script();
    let s = src.to_str().unwrap();
    let missing_key = scratch().join("no-such-key.bin");
    let short_key = write("short-key.bin", b"short");

    refused(
        "--verifying-key with no path",
        &["run", s, "--verifying-key"],
    );
    refused(
        "--tick-interval with no duration",
        &["run", s, "--tick-interval"],
    );
    refused("--output with no path", &["compile", s, "--output"]);
    refused(
        "--tick-interval with nonsense",
        &["run", s, "--tick-interval", "zzz"],
    );
    refused(
        "--tick-interval beyond the maximum",
        &["run", s, "--tick-interval", "99999w"],
    );
    refused(
        "--target unknown",
        &["compile", s, "--target", "zzz", "-o", "/dev/null"],
    );
    refused(
        "a key file of the wrong length",
        &["run", s, "--verifying-key", short_key.to_str().unwrap()],
    );
    refused(
        "a key file that is not there",
        &["run", s, "--verifying-key", missing_key.to_str().unwrap()],
    );
    refused("an unknown subcommand", &["frobnicate"]);
}

/// An output path that cannot be written.
#[test]
fn an_unwritable_output_path_is_refused() {
    let src = good_script();
    let dir = scratch().join("out-is-a-directory");
    fs::create_dir_all(&dir).expect("dir fixture");
    refused(
        "output under a directory that does not exist",
        &[
            "compile",
            src.to_str().unwrap(),
            "-o",
            "/nonexistent-directory-for-keleusma/x.kbc",
        ],
    );
    refused(
        "output onto a directory",
        &[
            "compile",
            src.to_str().unwrap(),
            "-o",
            dir.to_str().unwrap(),
        ],
    );
}

/// Deeply nested source is refused by the parser's own guard, not by a crash.
///
/// This is the one case whose correct behaviour is easy to misread. The
/// binary parses on the process's main thread, whose stack is far larger than
/// a spawned thread's, so the recursion guard is reached long before the
/// stack is, and the refusal message itself mentions a stack overflow it
/// prevented. Both facts are why the panic detector above is narrow.
#[test]
fn deeply_nested_source_is_refused_by_the_guard_not_by_a_crash() {
    for depth in [60usize, 400] {
        let mut body = String::from("1");
        for _ in 0..depth {
            body = format!("if a > 0 {{ {body} }} else {{ 0 }}");
        }
        let src = write(
            &format!("deep-{depth}.kel"),
            format!("fn main() -> Word {{ let a = 7; {body} }}").as_bytes(),
        );
        refused(
            &format!("source nested {depth} deep"),
            &["run", src.to_str().unwrap()],
        );
    }
}

/// Controls. Without these a binary that refused everything would pass.
#[test]
fn valid_input_still_works() {
    let src = good_script();
    accepted("a valid script", &["run", src.to_str().unwrap()]);
    let art = good_artifact();
    accepted("a compiled artifact", &["run", art.to_str().unwrap()]);
    accepted("help", &["help"]);
}

/// The scratch fixtures are real files, so a path typo cannot make the whole
/// file vacuously green by refusing everything for the wrong reason.
#[test]
fn the_fixtures_exist() {
    let good = good_script();
    assert!(Path::new(&good).is_file(), "fixture script was not written");
    assert!(
        fs::read(&good).expect("read fixture").len() > 10,
        "fixture script is empty"
    );
}
