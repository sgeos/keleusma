//! **ORDER 6: THE NATIVE SHAPE AS A COMMAND.**
//!
//! The roadmap's Order 6 gate names native code as a first-class deployment
//! shape beside bytecode. Until Order 6 the only way to produce a native
//! library was an example program. `kel-native` is a real binary of this
//! package, and this file drives it as a user would. A library built through
//! the command, archive included, is linked into a C host and agrees with the
//! virtual machine, its shared object is produced, `inspect` reports the
//! contract, and every documented exit status is reached. Since question 19
//! the command also selects the compiler, and the three compilers build the
//! same library bytes.
//!
//! # What this does not establish
//!
//! The command is not part of the `keleusma-cli` front end, because this
//! package is detached so that LLVM is not a build dependency of the whole
//! repository. Under question 16, `keleusma native` delegates to this command
//! and passes every argument through, which V0.2.X tests on its side. The
//! selector's byte identity is shown here for one program, and for the corpus
//! by `native_stage_factory.rs`.
mod common;

use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CLI: &str = env!("CARGO_BIN_EXE_kel-native");

const STREAM: &str = "shared data io { total: Word } \
    loop main(t: Word) -> Word { io.total = io.total + t; let r = yield io.total * 3; yield r - t }";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kel-cli-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cli(args: &[&str]) -> Output {
    Command::new(CLI).args(args).output().unwrap()
}

fn reference(replies: &[i64]) -> Vec<i64> {
    let m = common::build(STREAM);
    let need = required_persistent_capacity_for(&m);
    let mut arena = keleusma_arena::Arena::with_capacity(
        auto_arena_capacity_for(&m, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut shared = vec![0u8; m.shared_data_bytes as usize];
    let mut vm = Vm::new(m, &arena).unwrap();
    let mut out = Vec::new();
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(replies[0])])
        .unwrap();
    let mut rest = replies[1..].iter();
    let mut last = replies[0];
    loop {
        match state {
            VmState::Yielded(Value::Int(v)) => {
                out.push(v);
                let Some(&r) = rest.next() else {
                    return out;
                };
                last = r;
                state = vm.resume_with_shared(&mut shared, Value::Int(r)).unwrap();
            }
            VmState::Reset => {
                state = vm
                    .resume_with_shared(&mut shared, Value::Int(last))
                    .unwrap()
            }
            other => panic!("{other:?}"),
        }
    }
}

const HOST: &str = r#"#include "kel_cli.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    static _Alignas(8) unsigned char slot[KEL_CLI_SLOT_BYTES];
    static _Alignas(8) unsigned char shared[KEL_CLI_SHARED_BYTES + 8];
    static _Alignas(8) unsigned char priv[KEL_CLI_PRIVATE_BYTES + 8];
    static _Alignas(8) unsigned char region[KEL_CLI_REGION_BYTES + 8];
    if (argc < 2) return 64;
    struct kel_cli_outcome o = kel_cli_start(atoll(argv[1]), shared, priv, region, slot);
    for (int i = 2;; i++) {
        if (o.status != 1) return 5;
        printf("%lld\n", (long long)o.value);
        if (i == argc) break;
        o = kel_cli_resume(slot, atoll(argv[i]));
    }
    kel_cli_release(slot);
    return 0;
}
"#;

fn write_source(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.to_string_lossy().into_owned()
}

#[test]
fn a_library_built_by_the_command_links_and_agrees_with_the_vm() {
    let dir = scratch("build");
    let src = write_source(&dir, "stream.kel", STREAM);
    let out_dir = dir.join("out");
    let out = out_dir.to_string_lossy().into_owned();
    let built = cli(&[
        "build",
        &src,
        "--prefix",
        "kel_cli",
        "--out",
        &out,
        "--archive",
        "--shared",
    ]);
    assert!(built.status.success(), "{built:?}");
    let shared_object = if cfg!(target_os = "macos") {
        "libkel_cli.dylib"
    } else {
        "libkel_cli.so"
    };
    for f in [
        "kel_cli.o",
        "kel_cli.h",
        "kel_cli.rs",
        "libkel_cli.a",
        shared_object,
    ] {
        assert!(out_dir.join(f).is_file(), "the command did not write {f}");
    }
    let host = dir.join("host.c");
    std::fs::write(&host, HOST).unwrap();
    let exe = dir.join("host");
    let linked = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Werror", "-I"])
        .arg(&out_dir)
        .arg(&host)
        .arg(out_dir.join("libkel_cli.a"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let replies: Vec<i64> = vec![4, 9, 2, 7, 1, 8];
    let args: Vec<String> = replies.iter().map(i64::to_string).collect();
    let ran = Command::new(&exe).args(&args).output().unwrap();
    assert!(ran.status.success(), "{ran:?}");
    let seen: Vec<i64> = String::from_utf8(ran.stdout)
        .unwrap()
        .lines()
        .map(|l| l.parse().unwrap())
        .collect();
    assert_eq!(seen, reference(&replies));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inspect_reports_the_contract_without_writing_anything() {
    let dir = scratch("inspect");
    let src = write_source(&dir, "stream.kel", STREAM);
    let out = cli(&["inspect", &src]);
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    for line in [
        "frame bytes",
        "stack bytes",
        "schema hash",
        "cost claim",
        "run-time kinds",
    ] {
        assert!(text.contains(line), "inspect omitted `{line}`:\n{text}");
    }
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        1,
        "inspect must write nothing beside the source"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// MUST-NOT-FIRE for the success paths above. Each documented failure reaches
/// its own status, so a command that exited zero on everything would fail here.
#[test]
fn every_documented_failure_reaches_its_exit_status() {
    let dir = scratch("fail");
    let out = dir.join("o").to_string_lossy().into_owned();
    let good = write_source(&dir, "good.kel", STREAM);
    let bad = write_source(&dir, "bad.kel", "fn main( -> Word {");
    let atomic = write_source(&dir, "atomic.kel", "fn main(a: Word) -> Word { a + 1 }");
    let missing = dir.join("absent.kel").to_string_lossy().into_owned();
    let undefined = write_source(
        &dir,
        "undefined.kel",
        "fn main() -> Word { undefined_symbol }",
    );
    let outside = write_source(&dir, "outside.kel", OUTSIDE_SUBSET);
    let cases: &[(&[&str], u8)] = &[
        (&["frobnicate", &good], 64),
        (&["build", &good, "--out", &out], 64),
        (&["inspect", &good, "--fallible", "host::nothing"], 64),
        (&["build", &missing, "--prefix", "p", "--out", &out], 66),
        (&["build", &bad, "--prefix", "p", "--out", &out], 65),
        (&["build", &atomic, "--prefix", "p", "--out", &out], 70),
        (&["build", &good, "--prefix", "9bad", "--out", &out], 70),
        (&["inspect", &good, "--compiler", "bogus"], 64),
        (&["inspect", &good, "--compiler"], 64),
        (&["inspect", &undefined, "--compiler", "self-hosted"], 65),
        (&["inspect", &outside, "--compiler", "self-hosted"], 72),
        (
            &["inspect", &outside, "--compiler", "self-hosted-native"],
            72,
        ),
    ];
    for (args, code) in cases {
        let ran = cli(args);
        assert_eq!(
            ran.status.code(),
            Some(i32::from(*code)),
            "{args:?}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        assert!(!ran.stderr.is_empty(), "{args:?} failed silently");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A nested destructuring `let`. The reference compiler accepts it, and the
/// self-hosted destructuring filter refuses it as outside the subset, which is
/// the witness `tests/self_hosted_backend.rs` uses for
/// `SelfHostError::Unsupported`. It replaced a pipe into a generic template at
/// absorption 129, when upstream pull request 694 brought pipes into the
/// self-hosted subset.
const OUTSIDE_SUBSET: &str = "loop main(t: Word) -> Word { \
    let (a, (b, c)) = (t, (2, 3)); let r = yield a + b + c; yield r }";

/// **QUESTION 19, THE SELECTOR.** Each of the three compilers produces the
/// module the native route lowers, and the libraries they yield are the same
/// bytes. Equal object files are the end-to-end form of the corpus check in
/// `native_stage_factory.rs`, here through the command a user runs. The native
/// self-hosted route must report every stage native and none declined, so a
/// factory that silently fell back to the virtual machine fails here.
#[test]
fn the_three_compilers_build_byte_identical_libraries() {
    let dir = scratch("compilers");
    let src = write_source(&dir, "stream.kel", STREAM);
    let mut files: Vec<(String, Vec<Vec<u8>>)> = Vec::new();
    for compiler in ["rust", "self-hosted", "self-hosted-native"] {
        let out = dir.join(compiler).to_string_lossy().into_owned();
        let built = cli(&[
            "build",
            &src,
            "--prefix",
            "kel_cli",
            "--out",
            &out,
            "--compiler",
            compiler,
        ]);
        assert!(built.status.success(), "{compiler}: {built:?}");
        let stderr = String::from_utf8_lossy(&built.stderr);
        if compiler == "self-hosted-native" {
            assert!(
                stderr.contains("native stages, 0 declined"),
                "the native route did not report a fully native compile: {stderr}"
            );
        } else {
            assert!(
                stderr.is_empty(),
                "{compiler} wrote to standard error: {stderr}"
            );
        }
        let bytes = ["kel_cli.o", "kel_cli.h", "kel_cli.rs"]
            .iter()
            .map(|f| std::fs::read(dir.join(compiler).join(f)).unwrap())
            .collect();
        files.push((compiler.to_string(), bytes));
    }
    for (compiler, bytes) in &files[1..] {
        assert!(
            bytes == &files[0].1,
            "{compiler} built a library that differs from the reference compiler's"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The out-of-subset failure names the way forward, and the reference route
/// does not stop at the compiler for the same source. The native route then
/// refuses it for its own reason, which this test does not depend on.
#[test]
fn an_out_of_subset_source_suggests_the_reference_compiler() {
    let dir = scratch("outside");
    let src = write_source(&dir, "outside.kel", OUTSIDE_SUBSET);
    let selfhosted = cli(&["inspect", &src, "--compiler", "self-hosted"]);
    assert_eq!(selfhosted.status.code(), Some(72), "{selfhosted:?}");
    assert!(
        String::from_utf8_lossy(&selfhosted.stderr).contains("retry with --compiler rust"),
        "{selfhosted:?}"
    );
    let reference = cli(&["inspect", &src]);
    assert_ne!(reference.status.code(), Some(65), "{reference:?}");
    assert_ne!(reference.status.code(), Some(72), "{reference:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
