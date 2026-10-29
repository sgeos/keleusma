//! **Order 3: a native static library linked against a Rust host.**
//!
//! The roadmap's Order 3 gate is "a native static library links against a Rust
//! host and runs a real program". The program is the self-hosted lexer, the
//! largest stage frame. [`keleusma_native::linkage`] lowers it at its static
//! frame bound and exports it under a prefix with its contract as constants.
//! The test archives the object with the platform `ar`, compiles
//! `examples/lexer_host/host.rs` with `rustc` against the generated bindings,
//! runs the binary as a separate process, and compares every yield and every
//! shared byte with the virtual machine.
//!
//! The other tests check what Workstream D adds around that. The archive
//! exports its API and contract and nothing else. A host refuses bindings from
//! another build. A C host links the same archive. Two libraries link into one
//! Rust host only after export, and the host defines a native whose signature
//! the bindings check at compile time.
//!
//! # Tools
//!
//! `rustc`, `ar`, `cc` and `nm` must be on the path. Where one is missing the
//! test skips loudly, unless `CI` or `KEL_REQUIRE_LLVM_TOOLS` is set, in which
//! case it fails, since continuous integration provisions all four.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use keleusma::bytecode::{Module, SlotVisibility, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::coroutine::{self, HostContracts};
use keleusma_native::linkage::{self, LinkageError, TrapExport};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

const TEXTS: &[&[u8]] = &[
    b"fn add(x: Word, y: Word) -> Word { x + y }",
    b"loop main(t: Word) -> Word { if t > 10 { yield t * 2 } else { yield 0 } } // c",
    b"struct P { a: Word } fn f(p: P) -> Word { p.a >= 3 }",
];
const TURNS: i64 = 300;

fn machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).unwrap();
    machine_for(&TargetMachine::get_default_triple())
}

/// A machine for `triple`, which must be one of the targets LLVM was built for.
fn machine_for(triple: &TargetTriple) -> TargetMachine {
    Target::initialize_all(&InitializationConfig::default());
    Target::from_triple(triple)
        .unwrap()
        .create_target_machine(
            triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

/// The named tool, or `None` after a loud skip notice. Fails instead when the
/// environment says the tools are provisioned.
fn tool(name: &str, env: &str) -> Option<String> {
    let program = std::env::var(env).unwrap_or_else(|_| name.to_string());
    if Command::new(&program).arg("--version").output().is_ok() {
        return Some(program);
    }
    let required =
        std::env::var_os("CI").is_some() || std::env::var_os("KEL_REQUIRE_LLVM_TOOLS").is_some();
    assert!(
        !required,
        "`{program}` is required here and could not be run. Set {env} or install it."
    );
    eprintln!("\nSKIPPED: `{program}` was not found, so this linkage check did not run.\n");
    None
}

/// A directory unique to the process and tag, since tests run concurrently.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kel-static-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(cmd: &mut Command) -> Output {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("could not run {cmd:?}: {e}"));
    assert!(
        out.status.success(),
        "{cmd:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn archive(ar: &str, dir: &Path, prefix: &str) -> PathBuf {
    let lib = dir.join(format!("lib{prefix}.a"));
    let _ = std::fs::remove_file(&lib);
    run(Command::new(ar)
        .arg("rcs")
        .arg(&lib)
        .arg(dir.join(format!("{prefix}.o"))));
    lib
}

/// Write `prefix.o`, `prefix.h` and `prefix.rs` for `program` into `dir`.
fn emit(program: &Module, prefix: &str, dir: &Path) -> Emitted {
    emit_for(&machine(), program, prefix, dir)
}

/// [`emit`] for the target `tm` describes.
fn emit_for(tm: &TargetMachine, program: &Module, prefix: &str, dir: &Path) -> Emitted {
    let ctx = Context::create();
    let lib = linkage::build(&ctx, program, tm, prefix, &HostContracts::default())
        .unwrap_or_else(|e| panic!("{prefix}: {e:?}"));
    lib.write_object(tm, &dir.join(format!("{prefix}.o")))
        .unwrap();
    std::fs::write(dir.join(format!("{prefix}.h")), lib.c_header()).unwrap();
    std::fs::write(dir.join(format!("{prefix}.rs")), lib.rust_bindings()).unwrap();
    Emitted {
        exported: lib.exported_symbols(),
        natives: lib.natives.iter().map(|n| n.symbol.clone()).collect(),
        shared: lib
            .shared
            .iter()
            .map(|f| (f.name.clone(), f.offset, f.count))
            .collect(),
        trap: lib.trap.clone(),
        frame_bytes: lib.frame_bytes,
        stack_bytes: lib.stack_bytes,
    }
}

struct Emitted {
    exported: Vec<String>,
    natives: Vec<String>,
    shared: Vec<(String, u64, u64)>,
    trap: TrapExport,
    frame_bytes: u64,
    stack_bytes: u64,
}

fn lexer() -> Module {
    common::build(&std::fs::read_to_string("../src/selfhost/kel/lexer.kel").unwrap())
}

/// The lexer library, built and archived once per process, because the frame
/// bound is a bisection over complete lowerings and because tests running
/// concurrently must not rewrite an archive another is linking. `None` after a
/// skip notice when `ar` is absent.
fn lexer_library() -> Option<&'static (PathBuf, Emitted)> {
    static LIB: OnceLock<Option<(PathBuf, Emitted)>> = OnceLock::new();
    LIB.get_or_init(|| {
        let ar = tool("ar", "AR")?;
        let dir = scratch("lexer");
        let emitted = emit(&lexer(), "kel_lexer", &dir);
        archive(&ar, &dir, "kel_lexer");
        Some((dir, emitted))
    })
    .as_ref()
}

/// The offset of a shared slot read from the module's own layout, not from the
/// library, so the reference does not share the generator's arithmetic.
fn shared_offset(program: &Module, name: &str) -> usize {
    let layout = program.data_layout.as_ref().unwrap();
    let index = layout
        .slots
        .iter()
        .filter(|s| s.visibility == SlotVisibility::Shared)
        .position(|s| s.name == name)
        .unwrap_or_else(|| panic!("no shared slot `{name}`"));
    layout.shared_layout[index].offset as usize
}

/// The virtual machine's yields and final shared buffer for one source.
fn vm_lex(program: &Module, text: &[u8], turns: i64) -> (Vec<i64>, Vec<u8>) {
    let mut shared = vec![0u8; program.shared_data_bytes as usize];
    let (len, bytes) = (
        shared_offset(program, "src.len"),
        shared_offset(program, "src.bytes"),
    );
    shared[len..len + 8].copy_from_slice(&(text.len() as i64).to_le_bytes());
    shared[bytes..bytes + text.len()].copy_from_slice(text);
    let need = required_persistent_capacity_for(program);
    let mut arena = keleusma::Arena::with_capacity(
        auto_arena_capacity_for(program, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    let mut st = vm.call_with_shared(&mut shared, &[Value::Int(0)]).unwrap();
    let mut ys = Vec::new();
    for t in 1..=turns {
        let VmState::Yielded(Value::Int(v)) = st else {
            panic!("{st:?}")
        };
        ys.push(v);
        if t < turns {
            st = vm.resume_with_shared(&mut shared, Value::Int(t)).unwrap();
            if matches!(st, VmState::Reset) {
                st = vm.resume_with_shared(&mut shared, Value::Int(t)).unwrap();
            }
        }
    }
    (ys, shared)
}

fn build_lexer_host(rustc: &str, dir: &Path, bindings: &Path, out: &Path) -> Output {
    build_lexer_host_for(rustc, None, dir, bindings, out)
}

/// [`build_lexer_host`] for `target`, or for the host when it is `None`.
fn build_lexer_host_for(
    rustc: &str,
    target: Option<&str>,
    dir: &Path,
    bindings: &Path,
    out: &Path,
) -> Output {
    let host = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/lexer_host/host.rs");
    let mut cmd = Command::new(rustc);
    cmd.env("KEL_LIBRARY_BINDINGS", bindings)
        .args(["--edition", "2024", "-D", "warnings", "-L"])
        .arg(format!("native={}", dir.display()));
    if let Some(target) = target {
        cmd.args(["--target", target]);
    }
    cmd.arg("-o").arg(out).arg(host).output().unwrap()
}

/// **The gate.** Three lexer instances in one host-reserved pool lex three
/// sources in interleaved turns, against three runs of the virtual machine.
#[test]
fn a_rust_host_links_the_lexer_static_library_and_agrees_with_the_vm() {
    let (Some(rustc), Some((dir, emitted))) = (tool("rustc", "RUSTC"), lexer_library()) else {
        return;
    };
    let exe = dir.join("lexer_host");
    let built = build_lexer_host(&rustc, dir, &dir.join("kel_lexer.rs"), &exe);
    assert!(
        built.status.success(),
        "the Rust host did not build\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let out_dir = scratch("lexer-run");
    let sources: Vec<PathBuf> = TEXTS
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let p = out_dir.join(format!("source_{i}.kel"));
            std::fs::write(&p, t).unwrap();
            p
        })
        .collect();
    let ran = run(Command::new(&exe)
        .arg(&out_dir)
        .arg(TURNS.to_string())
        .args(&sources));

    let mut seen = vec![Vec::new(); TEXTS.len()];
    for line in String::from_utf8(ran.stdout).unwrap().lines() {
        let f: Vec<&str> = line.split(' ').collect();
        if f[0] == "S" {
            continue;
        }
        assert_eq!((f.len(), f[0]), (4, "Y"), "unexpected host line `{line}`");
        let (i, t, v): (usize, i64, i64) = (
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
        );
        assert_eq!(t, seen[i].len() as i64, "instance {i} turns out of order");
        seen[i].push(v);
    }
    let program = lexer();
    for (i, text) in TEXTS.iter().enumerate() {
        let (ys, shared) = vm_lex(&program, text, TURNS);
        assert_eq!(seen[i], ys, "instance {i}: token yields");
        let written = std::fs::read(out_dir.join(format!("shared_{i}.bin"))).unwrap();
        assert_eq!(written.len(), shared.len(), "instance {i}: shared length");
        assert!(written == shared, "instance {i}: shared writes differ");
        let codes: std::collections::BTreeSet<_> = ys.iter().map(|v| v & 255).collect();
        assert!(codes.len() >= 8, "instance {i} lexed vacuously: {codes:?}");
    }
    assert_ne!(seen[0], seen[1], "the sources must lex differently");
    println!(
        "Rust host: {} lexer instances x {TURNS} turns at a {}-byte frame bound agree with the VM",
        TEXTS.len(),
        emitted.frame_bytes
    );
}

/// **Order 4, Workstream E: the exported machine stack bound holds.**
///
/// The host paints the stack below its stack pointer before every start and
/// resume and reports the deepest byte any call changed. It runs twice with
/// different paint values, since a byte written with the paint value is
/// invisible, and the larger figure must stay within `STACK_BYTES`. The run
/// must also measure a real depth, or a probe that saw nothing would pass.
#[test]
fn the_measured_machine_stack_stays_within_the_exported_bound() {
    let (Some(rustc), Some((dir, emitted))) = (tool("rustc", "RUSTC"), lexer_library()) else {
        return;
    };
    let exe = dir.join("lexer_host_stack");
    let built = build_lexer_host(&rustc, dir, &dir.join("kel_lexer.rs"), &exe);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let out_dir = scratch("lexer-stack");
    let sources: Vec<PathBuf> = TEXTS
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let p = out_dir.join(format!("source_{i}.kel"));
            std::fs::write(&p, t).unwrap();
            p
        })
        .collect();
    let mut measured = Vec::new();
    for paint in ["90", "165"] {
        let ran = run(Command::new(&exe)
            .env("KEL_STACK_PAINT", paint)
            .arg(&out_dir)
            .arg(TURNS.to_string())
            .args(&sources));
        let text = String::from_utf8(ran.stdout).unwrap();
        let line = text
            .lines()
            .find_map(|l| l.strip_prefix("S "))
            .expect("the host reports its stack measurement");
        if line == "unmeasured" {
            eprintln!("SKIPPED: the host cannot measure its stack on this target");
            return;
        }
        measured.push(line.parse::<u64>().unwrap());
    }
    let deepest = *measured.iter().max().unwrap();
    println!(
        "machine stack: measured {measured:?} bytes, exported bound {} bytes",
        emitted.stack_bytes
    );
    assert!(
        deepest > 0,
        "the probe measured no stack use at all, so it measured nothing"
    );
    assert!(
        deepest <= emitted.stack_bytes,
        "the lexer used {deepest} stack bytes, above its exported bound of {}",
        emitted.stack_bytes
    );
}

/// **Operator question 15: the Rust host on `x86_64`, under Rosetta.**
///
/// The two tests above run the Rust host on the build machine's own
/// architecture. This one builds the lexer library and the Rust host for
/// `x86_64-apple-darwin` and runs them under Rosetta on an `arm64` Mac, so the
/// host's `x86_64` stack probe and the `x86_64` stack bound are exercised. The
/// yields and shared bytes must agree with the virtual machine, and the
/// measured stack must be real and within the exported bound.
///
/// It applies only on an `arm64` Mac. It skips loudly where Rosetta does not
/// run, since that is outside this repository. A missing `x86_64` Rust
/// standard library skips loudly too, and fails where the tools are required,
/// because continuous integration installs that target.
#[test]
fn the_rust_host_runs_on_x86_64_and_stays_within_its_stack_bound() {
    const TARGET: &str = "x86_64-apple-darwin";
    if !cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        eprintln!("SKIPPED: the x86_64 Rust host runs only on an arm64 Mac, under Rosetta");
        return;
    }
    let (Some(rustc), Some(ar)) = (tool("rustc", "RUSTC"), tool("ar", "AR")) else {
        return;
    };
    let rosetta = Command::new("arch")
        .args(["-x86_64", "/usr/bin/true"])
        .status()
        .is_ok_and(|s| s.success());
    if !rosetta {
        eprintln!("SKIPPED: Rosetta does not run here, so x86_64 code cannot be executed");
        return;
    }
    let libdir = Command::new(&rustc)
        .args(["--print", "target-libdir", "--target", TARGET])
        .output()
        .ok()
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    if !libdir.is_some_and(|d| has_std(&d)) {
        let required = std::env::var_os("CI").is_some()
            || std::env::var_os("KEL_REQUIRE_LLVM_TOOLS").is_some();
        assert!(
            !required,
            "the {TARGET} Rust standard library is required here. Install it with \
             `rustup target add {TARGET}`."
        );
        eprintln!("SKIPPED: the {TARGET} Rust standard library is not installed");
        return;
    }

    let dir = scratch("lexer-x86_64");
    let emitted = emit_for(
        &machine_for(&TargetTriple::create(TARGET)),
        &lexer(),
        "kel_lexer",
        &dir,
    );
    archive(&ar, &dir, "kel_lexer");
    let exe = dir.join("lexer_host_x86_64");
    let built = build_lexer_host_for(&rustc, Some(TARGET), &dir, &dir.join("kel_lexer.rs"), &exe);
    assert!(
        built.status.success(),
        "the x86_64 Rust host did not build\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let out_dir = scratch("lexer-x86_64-run");
    let sources: Vec<PathBuf> = TEXTS
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let p = out_dir.join(format!("source_{i}.kel"));
            std::fs::write(&p, t).unwrap();
            p
        })
        .collect();
    let program = lexer();
    let mut measured = Vec::new();
    for paint in ["90", "165"] {
        let ran = run(Command::new("arch")
            .arg("-x86_64")
            .arg(&exe)
            .env("KEL_STACK_PAINT", paint)
            .arg(&out_dir)
            .arg(TURNS.to_string())
            .args(&sources));
        let mut seen = vec![Vec::new(); TEXTS.len()];
        let mut stack = None;
        for line in String::from_utf8(ran.stdout).unwrap().lines() {
            if let Some(s) = line.strip_prefix("S ") {
                stack = Some(s.to_string());
                continue;
            }
            let f: Vec<&str> = line.split(' ').collect();
            assert_eq!((f.len(), f[0]), (4, "Y"), "unexpected host line `{line}`");
            let i: usize = f[1].parse().unwrap();
            seen[i].push(f[3].parse::<i64>().unwrap());
        }
        for (i, text) in TEXTS.iter().enumerate() {
            let (ys, shared) = vm_lex(&program, text, TURNS);
            assert_eq!(seen[i], ys, "x86_64 instance {i}: token yields");
            let written = std::fs::read(out_dir.join(format!("shared_{i}.bin"))).unwrap();
            assert!(
                written == shared,
                "x86_64 instance {i}: shared writes differ"
            );
        }
        let stack = stack.expect("the host reports its stack measurement");
        assert_ne!(stack, "unmeasured", "the x86_64 host has a stack probe");
        measured.push(stack.parse::<u64>().unwrap());
    }
    let deepest = *measured.iter().max().unwrap();
    println!(
        "x86_64 Rust host under Rosetta: agrees with the VM, stack measured {measured:?} \
         bytes, exported bound {} bytes",
        emitted.stack_bytes
    );
    assert!(deepest > 0, "the x86_64 probe measured no stack use at all");
    assert!(
        deepest <= emitted.stack_bytes,
        "the x86_64 lexer used {deepest} stack bytes, above its exported bound of {}",
        emitted.stack_bytes
    );
}

/// Whether `libdir` holds a standard library, whose file name carries a hash.
fn has_std(libdir: &Path) -> bool {
    std::fs::read_dir(libdir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libstd-") && name.ends_with(".rlib")
        })
    })
}

/// The generated layout names the fields the reference reads, at the offsets
/// the module declares, and the trap verdict is the analysis's own.
#[test]
fn the_generated_contract_agrees_with_the_module() {
    let Some((_, emitted)) = lexer_library() else {
        return;
    };
    let program = lexer();
    for name in ["src.len", "src.bytes"] {
        let field = emitted
            .shared
            .iter()
            .find(|(n, ..)| n == name)
            .unwrap_or_else(|| panic!("no generated field `{name}`"));
        assert_eq!(field.1 as usize, shared_offset(&program, name), "{name}");
    }
    let bytes = emitted
        .shared
        .iter()
        .find(|(n, ..)| n == "src.bytes")
        .unwrap();
    assert!(bytes.2 > 1, "the source array collapsed to one element");
    let expected = match keleusma::trap_analysis::classify_traps(&program) {
        Ok(keleusma::trap_analysis::TrapVerdict::TrapFree) => 1,
        Ok(keleusma::trap_analysis::TrapVerdict::MayTrap { .. }) => 2,
        Err(_) => 0,
    };
    assert_eq!(emitted.trap.code(), expected);
    let bound =
        coroutine::minimum_frame_bytes(&program, &machine(), &HostContracts::default()).unwrap();
    assert_eq!(emitted.frame_bytes, u64::from(bound));
}

fn symbols(nm: &str, path: &Path) -> (Vec<String>, Vec<String>) {
    let out = run(Command::new(nm).arg("-g").arg(path));
    let mach_o = TargetMachine::get_default_triple()
        .as_str()
        .to_string_lossy()
        .contains("apple");
    let (mut defined, mut undefined) = (Vec::new(), Vec::new());
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (kind, name) = match f.as_slice() {
            [kind, name] => (*kind, *name),
            [_, kind, name] => (*kind, *name),
            _ => continue,
        };
        let name = if mach_o {
            name.strip_prefix('_').unwrap_or(name)
        } else {
            name
        };
        if kind == "U" {
            undefined.push(name.to_string());
        } else {
            defined.push(name.to_string());
        }
    }
    defined.sort();
    undefined.sort();
    (defined, undefined)
}

/// Only the handles and contract are external. Undefined symbols are natives or
/// the C library routines `LINKAGE_SYMBOL_CENSUS.md` records.
#[test]
fn the_archive_exports_exactly_the_api_and_the_contract() {
    let (Some(nm), Some((dir, emitted))) = (tool("nm", "NM"), lexer_library()) else {
        return;
    };
    let lib = dir.join("libkel_lexer.a");
    let (defined, undefined) = symbols(&nm, &lib);
    assert_eq!(defined, emitted.exported, "external definitions");
    assert!(
        defined.iter().any(|s| s == "kel_lexer_start"),
        "the comparison is vacuous without the start handle"
    );
    const TOOLCHAIN: &[&str] = &["bzero", "memset", "memcpy", "memmove"];
    for s in &undefined {
        assert!(
            emitted.natives.contains(s) || TOOLCHAIN.contains(&s.as_str()),
            "unexpected import `{s}`"
        );
    }
}

/// **Must fire.** A host built against bindings whose frame figure differs
/// from the archive's refuses to run rather than reserve the wrong slot.
#[test]
fn a_host_refuses_bindings_from_another_build() {
    let (Some(rustc), Some((dir, emitted))) = (tool("rustc", "RUSTC"), lexer_library()) else {
        return;
    };
    let original = std::fs::read_to_string(dir.join("kel_lexer.rs")).unwrap();
    let line = format!("pub const FRAME_BYTES: u64 = {};", emitted.frame_bytes);
    assert!(original.contains(&line), "the bindings lack `{line}`");
    let stale = original.replace(
        &line,
        &format!("pub const FRAME_BYTES: u64 = {};", emitted.frame_bytes + 8),
    );
    let stale_dir = scratch("stale");
    let bindings = stale_dir.join("kel_lexer.rs");
    std::fs::write(&bindings, stale).unwrap();
    let exe = stale_dir.join("lexer_host");
    let built = build_lexer_host(&rustc, dir, &bindings, &exe);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let source = stale_dir.join("source.kel");
    std::fs::write(&source, TEXTS[0]).unwrap();
    let ran = Command::new(&exe)
        .arg(&stale_dir)
        .arg("4")
        .arg(&source)
        .output()
        .unwrap();
    assert!(!ran.status.success(), "the host ran with stale bindings");
    let err = String::from_utf8_lossy(&ran.stderr);
    assert!(
        err.contains("contract mismatch on frame_bytes"),
        "unexpected refusal: {err}"
    );
}

/// The same archive from C, through the generated header. The host checks the
/// header's macros against the linked constants before it runs.
#[test]
fn a_c_host_links_the_same_archive_and_agrees_with_the_vm() {
    let (Some(cc), Some((dir, _))) = (tool("cc", "CC"), lexer_library()) else {
        return;
    };
    let work = scratch("c-host");
    let host = work.join("host.c");
    std::fs::write(
        &host,
        r#"#include "kel_lexer.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc != 3) return 64;
    if (kel_lexer_frame_bytes != KEL_LEXER_FRAME_BYTES ||
        kel_lexer_slot_bytes != KEL_LEXER_SLOT_BYTES ||
        kel_lexer_shared_bytes != KEL_LEXER_SHARED_BYTES ||
        kel_lexer_private_bytes != KEL_LEXER_PRIVATE_BYTES ||
        kel_lexer_region_bytes != KEL_LEXER_REGION_BYTES ||
        kel_lexer_trap_verdict != KEL_LEXER_TRAP_VERDICT) {
        fputs("contract mismatch\n", stderr);
        return 2;
    }
    long turns = strtol(argv[2], NULL, 10);
    FILE *f = fopen(argv[1], "rb");
    if (!f) return 3;
    static unsigned char text[KEL_LEXER_SRC_BYTES_COUNT];
    size_t n = fread(text, 1, sizeof text, f);
    fclose(f);
    unsigned char *shared = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_SHARED_BYTES);
    unsigned char *priv = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_PRIVATE_BYTES);
    unsigned char *region = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_REGION_BYTES);
    unsigned char *slot = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_SLOT_BYTES);
    if (!shared || !priv || !region || !slot) return 4;
    memset(shared, 0, KEL_LEXER_SHARED_BYTES);
    memset(priv, 0, KEL_LEXER_PRIVATE_BYTES);
    memset(region, 0, KEL_LEXER_REGION_BYTES);
    int64_t len = (int64_t)n;
    memcpy(shared + KEL_LEXER_SRC_LEN_OFFSET, &len, sizeof len);
    memcpy(shared + KEL_LEXER_SRC_BYTES_OFFSET, text, n);
    memcpy(priv, kel_lexer_private_init, KEL_LEXER_PRIVATE_INIT_BYTES);
    struct kel_lexer_outcome o = kel_lexer_start(0, shared, priv, region, slot);
    for (long t = 0; t < turns; t++) {
        if (o.status != 1) return 5;
        printf("%lld\n", (long long)o.value);
        if (t + 1 < turns) o = kel_lexer_resume(slot, t + 1);
    }
    kel_lexer_release(slot);
    return 0;
}
"#,
    )
    .unwrap();
    let exe = work.join("c_host");
    run(Command::new(&cc)
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Werror")
        .arg("-I")
        .arg(dir)
        .arg("-o")
        .arg(&exe)
        .arg(&host)
        .arg(dir.join("libkel_lexer.a")));
    let source = work.join("source.kel");
    std::fs::write(&source, TEXTS[1]).unwrap();
    let ran = run(Command::new(&exe).arg(&source).arg(TURNS.to_string()));
    let seen: Vec<i64> = String::from_utf8(ran.stdout)
        .unwrap()
        .lines()
        .map(|l| l.parse().unwrap())
        .collect();
    let (ys, _) = vm_lex(&lexer(), TEXTS[1], TURNS);
    assert_eq!(seen, ys, "C host token yields");
}

const COUNT: &str =
    "loop main(a: Word) -> Word { let keep = a + 100; let r = 10 + (yield a); yield r + keep }";
// The declared signature gives the module the native's return shape, which
// the coroutine lowering needs to prove the yielded scalar's kind.
const SCALE: &str = "use host::scale(Word, Word) -> Word\nloop main(a: Word) -> Word { let r = yield host::scale(a, 3); yield r + a }";

fn scale(a: i64, k: i64) -> i64 {
    a.wrapping_mul(k).wrapping_add(1)
}

fn vm_stream(src: &str, first: i64, replies: &[i64]) -> Vec<i64> {
    let program = common::build(src);
    let need = required_persistent_capacity_for(&program);
    let mut arena = keleusma::Arena::with_capacity(
        auto_arena_capacity_for(&program, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(program, &arena).unwrap();
    vm.register_native_closure("host::scale", |args: &[Value]| {
        let (Value::Int(a), Value::Int(k)) = (&args[0], &args[1]) else {
            panic!("host::scale takes two Words");
        };
        Ok(Value::Int(scale(*a, *k)))
    });
    let mut st = vm.call(&[Value::Int(first)]).unwrap();
    let mut ys = Vec::new();
    for (t, reply) in replies.iter().enumerate() {
        let VmState::Yielded(Value::Int(v)) = st else {
            panic!("{st:?}")
        };
        ys.push(v);
        if t + 1 < replies.len() {
            st = vm.resume(Value::Int(*reply)).unwrap();
            if matches!(st, VmState::Reset) {
                st = vm.resume(Value::Int(*reply)).unwrap();
            }
        }
    }
    ys
}

/// **Two libraries, one host.** Both programs put their entry at chunk zero, so
/// the unexported objects define the same handle names and do not link
/// together. Exported under two prefixes they do, and the host's native is
/// checked against the bindings' type for it.
#[test]
fn two_libraries_link_into_one_rust_host_only_after_export() {
    let (Some(rustc), Some(ar), Some(cc)) =
        (tool("rustc", "RUSTC"), tool("ar", "AR"), tool("cc", "CC"))
    else {
        return;
    };
    let dir = scratch("pair");
    let programs = [("kel_count", COUNT), ("kel_scale", SCALE)];
    for (_, src) in programs {
        assert_eq!(common::build(src).entry_point, Some(0), "{src}");
    }

    // The control. Raw coroutine objects of both programs, linked directly.
    let tm = machine();
    let mut raw = Vec::new();
    for (prefix, src) in programs {
        let program = common::build(src);
        let bound =
            coroutine::minimum_frame_bytes(&program, &tm, &HostContracts::default()).unwrap();
        let ctx = Context::create();
        let m = coroutine::lower(&ctx, &program, &tm, bound).unwrap();
        let obj = dir.join(format!("raw_{prefix}.o"));
        tm.write_to_file(&m, FileType::Object, &obj).unwrap();
        raw.push(obj);
    }
    let main_c = dir.join("main.c");
    std::fs::write(
        &main_c,
        "long long kel_native_host__scale(long long a, long long k) { return a * k + 1; }\n\
         int main(void) { return 0; }\n",
    )
    .unwrap();
    let collided = Command::new(&cc)
        .arg("-o")
        .arg(dir.join("raw_pair"))
        .arg(&main_c)
        .args(&raw)
        .output()
        .unwrap();
    assert!(
        !collided.status.success(),
        "the unexported objects linked together, so the control shows nothing"
    );
    let err = String::from_utf8_lossy(&collided.stderr);
    assert!(
        err.contains("kel_coroutine_0_start"),
        "the raw link failed for another reason: {err}"
    );

    // The exported libraries.
    let mut natives = Vec::new();
    for (prefix, src) in programs {
        natives.extend(emit(&common::build(src), prefix, &dir).natives);
        archive(&ar, &dir, prefix);
    }
    assert_eq!(natives, ["kel_native_host__scale"], "imports");
    let host = dir.join("pair_host.rs");
    std::fs::write(
        &host,
        format!(
            r#"include!("{count}");
include!("{scale}");

#[unsafe(no_mangle)]
pub extern "C" fn kel_native_host__scale(a: i64, k: i64) -> i64 {{
    a.wrapping_mul(k).wrapping_add(1)
}}
// A definition whose signature differs from the bindings' does not compile.
const _: kel_scale::natives::kel_native_host__scale = kel_native_host__scale;

struct Buffer(Vec<u64>);
impl Buffer {{
    fn new(bytes: u64) -> Self {{ Self(vec![0; (bytes as usize).div_ceil(8).max(1)]) }}
    fn ptr(&mut self) -> *mut u8 {{ self.0.as_mut_ptr().cast() }}
}}

macro_rules! drive {{
    ($lib:ident, $start:ident, $resume:ident, $release:ident, $first:expr, $replies:expr) => {{{{
        let mut b = [
            Buffer::new($lib::SHARED_BYTES),
            Buffer::new($lib::PRIVATE_BYTES),
            Buffer::new($lib::REGION_BYTES),
            Buffer::new($lib::SLOT_BYTES),
        ];
        let p: Vec<*mut u8> = b.iter_mut().map(Buffer::ptr).collect();
        let mut out = Vec::new();
        let mut o = unsafe {{ $lib::$start($first, p[0], p[1], p[2], p[3]) }};
        for (t, r) in $replies.iter().enumerate() {{
            assert_eq!(o.status, 1);
            out.push(o.value);
            if t + 1 < $replies.len() {{
                o = unsafe {{ $lib::$resume(p[3], *r) }};
            }}
        }}
        unsafe {{ $lib::$release(p[3]) }};
        out
    }}}};
}}

fn main() {{
    let replies: Vec<i64> = (0..12).map(|t| t * 7 + 1).collect();
    let a = drive!(kel_count, kel_count_start, kel_count_resume, kel_count_release, 5, replies);
    let b = drive!(kel_scale, kel_scale_start, kel_scale_resume, kel_scale_release, 4, replies);
    println!("{{a:?}}");
    println!("{{b:?}}");
}}
"#,
            count = dir.join("kel_count.rs").display(),
            scale = dir.join("kel_scale.rs").display(),
        ),
    )
    .unwrap();
    let exe = dir.join("pair_host");
    run(Command::new(&rustc)
        .args(["--edition", "2024", "-D", "warnings", "-L"])
        .arg(format!("native={}", dir.display()))
        .arg("-o")
        .arg(&exe)
        .arg(&host));
    // **Must fire.** The same host with a one-argument native does not compile.
    let source = std::fs::read_to_string(&host).unwrap();
    let wrong = source.replace(
        "kel_native_host__scale(a: i64, k: i64) -> i64 {\n    a.wrapping_mul(k)",
        "kel_native_host__scale(a: i64) -> i64 {\n    let k = 3;\n    a.wrapping_mul(k)",
    );
    assert_ne!(wrong, source, "the wrong-arity host was not constructed");
    let wrong_host = dir.join("wrong_host.rs");
    std::fs::write(&wrong_host, wrong).unwrap();
    let refused = Command::new(&rustc)
        .args(["--edition", "2024", "-L"])
        .arg(format!("native={}", dir.display()))
        .arg("-o")
        .arg(dir.join("wrong_host"))
        .arg(&wrong_host)
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "a native with the wrong arity compiled against the bindings"
    );
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("mismatched types"),
        "the wrong-arity host failed for another reason: {}",
        String::from_utf8_lossy(&refused.stderr)
    );

    let ran = run(&mut Command::new(&exe));
    let text = String::from_utf8(ran.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let replies: Vec<i64> = (0..12).map(|t| t * 7 + 1).collect();
    let expect_count = vm_stream(COUNT, 5, &replies);
    let expect_scale = vm_stream(SCALE, 4, &replies);
    assert_ne!(expect_count, expect_scale, "the two programs must differ");
    assert_eq!(
        lines,
        [format!("{expect_count:?}"), format!("{expect_scale:?}")]
    );
}

/// A prefix is a C identifier outside the native namespace, or it is refused
/// before anything is lowered.
#[test]
fn malformed_prefixes_are_refused() {
    let tm = machine();
    let program = common::build(COUNT);
    let too_long = "p".repeat(linkage::PREFIX_LIMIT + 1);
    for prefix in [
        "",
        "1lib",
        "my-lib",
        "lib.x",
        "kel_native_lib",
        too_long.as_str(),
    ] {
        let ctx = Context::create();
        let refused = linkage::build(&ctx, &program, &tm, prefix, &HostContracts::default());
        assert!(
            matches!(refused, Err(LinkageError::Prefix(_))),
            "`{prefix}` was not refused as a prefix"
        );
    }
    let ctx = Context::create();
    let at_limit = "p".repeat(linkage::PREFIX_LIMIT);
    assert!(linkage::build(&ctx, &program, &tm, &at_limit, &HostContracts::default()).is_ok());
}
