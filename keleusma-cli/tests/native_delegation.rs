//! `keleusma native` delegates to `kel-native` unchanged.
//!
//! The delegate is played by a shell script that records its arguments and
//! exits with a chosen status, so the test needs no native toolchain. It is
//! Unix-only for that reason.
//!
//! # What these tests do NOT establish
//!
//! They do not run the real `kel-native`, whose behaviour belongs to the
//! V0.3.X line's tests. The sibling lookup beside the `keleusma` executable is
//! not exercised, because a test cannot place a file beside the built binary
//! without affecting other tests.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("keleusma-native-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn keleusma() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_keleusma"));
    cmd.env_remove("KELEUSMA_NATIVE");
    cmd
}

#[test]
fn arguments_and_status_pass_through() {
    let dir = scratch("pass");
    let record = dir.join("args.txt");
    let fake = dir.join("kel-native");
    fs::write(
        &fake,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\nexit 7\n",
            record.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let status = keleusma()
        .env("KELEUSMA_NATIVE", &fake)
        .args(["native", "build", "a b.kel", "--prefix", "p", "--out", "o"])
        .status()
        .expect("runs");
    assert_eq!(status.code(), Some(7));
    assert_eq!(
        fs::read_to_string(&record).unwrap(),
        "build\na b.kel\n--prefix\np\n--out\no\n"
    );
}

#[test]
fn a_missing_delegate_is_reported_as_unavailable() {
    let dir = scratch("missing");
    let output = keleusma()
        .env("PATH", &dir)
        .args(["native", "inspect", "x.kel"])
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(69));
    assert!(String::from_utf8_lossy(&output.stderr).contains("kel-native"));
}

#[test]
fn an_explicit_path_that_cannot_run_is_an_error() {
    let dir = scratch("broken");
    let output = keleusma()
        .env("KELEUSMA_NATIVE", dir.join("absent"))
        .args(["native", "inspect", "x.kel"])
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(70));
}
