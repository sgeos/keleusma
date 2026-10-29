//! **ORDER 6: THE COMMITTED TARGET SET, TIER 1.**
//!
//! `V0_4_0_NATIVE_CODEGEN.md` fixes the targets V0.4.0 ships on as x86-64
//! Linux, AArch64 Linux and macOS on both architectures. Until Order 6 this
//! line executed one of them, `aarch64-apple-darwin`.
//!
//! This file builds the self-hosted lexer as a library for all four and reads
//! each object back. Every object must define exactly the handles and the
//! contract and import nothing. On both macOS architectures the archive is then
//! linked into a C host and run, `x86_64-apple-darwin` under Rosetta when the
//! machine is Apple silicon, and every yield must equal the virtual machine's.
//! The x86-64 run also calls a fallible native, so the System V return of a
//! two-word structure is executed and not only argued.
//!
//! # What this does not establish
//!
//! The two Linux objects are emitted and read but not linked or run, because
//! this machine has no Linux toolchain and native continuous integration runs
//! on macOS. Their execution is an open question for the operator.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use keleusma::bytecode::{Module, SlotVisibility, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::coroutine::HostContracts;
use keleusma_native::linkage;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TIER_ONE: &[&str] = &[
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
];

const TURNS: i64 = 120;
const TEXT: &[u8] =
    b"loop main(t: Word) -> Word { if t > 10 { yield t * 2 } else { yield 0 } } // c";

fn machine_for(triple: &str) -> TargetMachine {
    Target::initialize_all(&InitializationConfig::default());
    let triple = TargetTriple::create(triple);
    Target::from_triple(&triple)
        .unwrap()
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kel-tier1-{tag}-{}", std::process::id()));
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

/// The tool, or `None` with a loud notice, failing where the tools are required.
fn tool(name: &str) -> Option<String> {
    if Command::new(name).arg("--version").output().is_ok() {
        return Some(name.to_string());
    }
    assert!(
        std::env::var_os("KEL_REQUIRE_LLVM_TOOLS").is_none() && std::env::var_os("CI").is_none(),
        "`{name}` is required here and could not be run"
    );
    eprintln!("\nSKIPPED: `{name}` was not found, so the target-set check did not run.\n");
    None
}

/// Defined external and undefined symbols of an object, undecorated.
fn symbols(nm: &str, path: &Path, mach_o: bool) -> (BTreeSet<String>, BTreeSet<String>) {
    let out = run(Command::new(nm).arg("-g").arg(path));
    let (mut defined, mut undefined) = (BTreeSet::new(), BTreeSet::new());
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
            undefined.insert(name.to_string());
        } else {
            defined.insert(name.to_string());
        }
    }
    (defined, undefined)
}

fn lexer() -> Module {
    common::build(&std::fs::read_to_string("../src/selfhost/kel/lexer.kel").unwrap())
}

fn shared_offset(program: &Module, name: &str) -> usize {
    let layout = program.data_layout.as_ref().unwrap();
    let index = layout
        .slots
        .iter()
        .filter(|s| s.visibility == SlotVisibility::Shared)
        .position(|s| s.name == name)
        .unwrap();
    layout.shared_layout[index].offset as usize
}

fn vm_lex(program: &Module, text: &[u8], turns: i64) -> Vec<i64> {
    let mut shared = vec![0u8; program.shared_data_bytes as usize];
    let (len, bytes) = (
        shared_offset(program, "src.len"),
        shared_offset(program, "src.bytes"),
    );
    shared[len..len + 8].copy_from_slice(&(text.len() as i64).to_le_bytes());
    shared[bytes..bytes + text.len()].copy_from_slice(text);
    let need = required_persistent_capacity_for(program);
    let mut arena = keleusma_arena::Arena::with_capacity(
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
    ys
}

const LEXER_HOST: &str = r#"#include "kel_lexer.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    if (argc != 3) return 64;
    long turns = strtol(argv[2], NULL, 10);
    FILE *f = fopen(argv[1], "rb");
    if (!f) return 3;
    static unsigned char text[KEL_LEXER_SRC_BYTES_COUNT];
    size_t n = fread(text, 1, sizeof text, f);
    fclose(f);
    unsigned char *shared = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_SHARED_BYTES);
    unsigned char *priv = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_PRIVATE_BYTES);
    unsigned char *region = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_REGION_BYTES + 8);
    unsigned char *slot = aligned_alloc(KEL_LEXER_BUFFER_ALIGN, KEL_LEXER_SLOT_BYTES);
    if (!shared || !priv || !region || !slot) return 4;
    memset(shared, 0, KEL_LEXER_SHARED_BYTES);
    memset(priv, 0, KEL_LEXER_PRIVATE_BYTES);
    memset(region, 0, KEL_LEXER_REGION_BYTES + 8);
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
"#;

/// The macOS architecture name `cc -arch` takes for a Darwin triple.
fn darwin_arch(triple: &str) -> Option<&'static str> {
    match triple {
        "aarch64-apple-darwin" => Some("arm64"),
        "x86_64-apple-darwin" => Some("x86_64"),
        _ => None,
    }
}

/// Whether this machine runs `arch` binaries, natively or through Rosetta.
fn runs(arch: &str, dir: &Path) -> bool {
    let src = dir.join("probe.c");
    std::fs::write(&src, "int main(void) { return 0; }\n").unwrap();
    let exe = dir.join(format!("probe-{arch}"));
    Command::new("cc")
        .args(["-arch", arch])
        .arg(&src)
        .arg("-o")
        .arg(&exe)
        .status()
        .is_ok_and(|s| s.success())
        && Command::new(&exe).status().is_ok_and(|s| s.success())
}

#[test]
fn the_lexer_builds_for_every_tier_one_target_and_runs_on_both_macos_architectures() {
    let (Some(nm), Some(ar)) = (tool("nm"), tool("ar")) else {
        return;
    };
    let program = lexer();
    let expected = vm_lex(&program, TEXT, TURNS);
    let mut executed = Vec::new();
    for &triple in TIER_ONE {
        let dir = scratch(triple);
        let tm = machine_for(triple);
        let ctx = Context::create();
        let lib = linkage::build(&ctx, &program, &tm, "kel_lexer", &HostContracts::default())
            .unwrap_or_else(|e| panic!("{triple}: {e:?}"));
        let object = dir.join("kel_lexer.o");
        lib.write_object(&tm, &object).unwrap();
        std::fs::write(dir.join("kel_lexer.h"), lib.c_header()).unwrap();
        let (defined, undefined) = symbols(&nm, &object, triple.contains("apple"));
        let exported: BTreeSet<String> = lib.exported_symbols().into_iter().collect();
        assert_eq!(defined, exported, "{triple}: external definitions");
        assert!(undefined.is_empty(), "{triple}: imports {undefined:?}");
        println!(
            "  {triple:<28} frame {:>5} B, stack {:>5} B, {} symbols, no imports",
            lib.frame_bytes,
            lib.stack_bytes,
            defined.len()
        );
        let Some(arch) = darwin_arch(triple) else {
            continue;
        };
        if !runs(arch, &dir) {
            eprintln!("  {triple}: this machine cannot run {arch} code, so it was not executed");
            continue;
        }
        let archive = dir.join("libkel_lexer.a");
        let _ = std::fs::remove_file(&archive);
        run(Command::new(&ar).arg("rcs").arg(&archive).arg(&object));
        let host = dir.join("host.c");
        std::fs::write(&host, LEXER_HOST).unwrap();
        let exe = dir.join("host");
        run(Command::new("cc")
            .args(["-arch", arch, "-std=c11", "-Wall", "-Werror", "-I"])
            .arg(&dir)
            .arg("-o")
            .arg(&exe)
            .arg(&host)
            .arg(&archive));
        let source = dir.join("source.kel");
        std::fs::write(&source, TEXT).unwrap();
        let ran = run(Command::new(&exe).arg(&source).arg(TURNS.to_string()));
        let seen: Vec<i64> = String::from_utf8(ran.stdout)
            .unwrap()
            .lines()
            .map(|l| l.parse().unwrap())
            .collect();
        assert_eq!(seen, expected, "{triple}: token yields");
        executed.push(triple);
        let _ = std::fs::remove_dir_all(&dir);
    }
    println!("  executed and agreeing with the VM: {executed:?}");
    assert!(
        executed.contains(&"aarch64-apple-darwin") || executed.contains(&"x86_64-apple-darwin"),
        "no tier one target was executed"
    );
}

/// The fallible-native return is a two-word structure. AAPCS64 and System V
/// x86-64 both return it in two registers, and this runs it on whichever of
/// the two macOS architectures this machine can execute, against the VM.
#[test]
fn a_fallible_native_returns_correctly_on_each_executable_macos_architecture() {
    let src = "use host::risky(Word) -> Word\n\
               yield main(x: Word) -> Word { let a = yield x; host::risky(a) + 1 }";
    let program = common::build(src);
    let contracts = HostContracts {
        fallible_natives: BTreeSet::from([0]),
        ..HostContracts::default()
    };
    let symbol = keleusma_native::native_symbol("host::risky");
    let mut executed = 0;
    for triple in ["aarch64-apple-darwin", "x86_64-apple-darwin"] {
        let arch = darwin_arch(triple).unwrap();
        let dir = scratch(&format!("risky-{arch}"));
        if !runs(arch, &dir) {
            eprintln!("  {triple}: not executable here");
            continue;
        }
        let tm = machine_for(triple);
        let ctx = Context::create();
        let lib = linkage::build(&ctx, &program, &tm, "kel_risky", &contracts).unwrap();
        lib.write_object(&tm, &dir.join("kel_risky.o")).unwrap();
        std::fs::write(dir.join("kel_risky.h"), lib.c_header()).unwrap();
        let host = format!(
            r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "kel_risky.h"
kel_native_result {symbol}(int64_t a) {{
    kel_native_result r;
    if (a == 0) {{ r.value = 42; r.failed = 1; }} else {{ r.value = a * 2; r.failed = 0; }}
    return r;
}}
static _Alignas(8) unsigned char slot[KEL_RISKY_SLOT_BYTES];
static _Alignas(8) unsigned char shared[KEL_RISKY_SHARED_BYTES + 8];
static _Alignas(8) unsigned char priv[KEL_RISKY_PRIVATE_BYTES + 8];
static _Alignas(8) unsigned char region[KEL_RISKY_REGION_BYTES + 8];
int main(int argc, char **argv) {{
    for (int i = 1; i < argc; i++) {{
        memset(priv, 0, sizeof priv);
        struct kel_risky_outcome o = kel_risky_start(7, shared, priv, region, slot);
        if (o.status != 1) return 2;
        o = kel_risky_resume(slot, atoll(argv[i]));
        printf("%llu %lld\n", (unsigned long long)o.status, (long long)o.value);
        kel_risky_release(slot);
    }}
    return 0;
}}
"#
        );
        std::fs::write(dir.join("host.c"), host).unwrap();
        let exe = dir.join("host");
        run(Command::new("cc")
            .args(["-arch", arch, "-std=c11", "-Wall", "-Werror", "-I"])
            .arg(&dir)
            .arg(dir.join("host.c"))
            .arg(dir.join("kel_risky.o"))
            .arg("-o")
            .arg(&exe));
        let out = run(Command::new(&exe).args(["0", "5"]));
        let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(String::from)
            .collect();
        // The VM: a failure reaches the default HostError trap, code 9, and a
        // success completes with the doubled argument plus one.
        let host_error = keleusma::bytecode::TrapKind::HostError.code();
        assert_eq!(
            lines,
            [format!("3 {host_error}"), String::from("2 11")],
            "{triple}"
        );
        executed += 1;
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert!(executed > 0, "neither macOS architecture could be executed");
}
