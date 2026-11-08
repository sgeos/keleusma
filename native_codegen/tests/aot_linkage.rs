//! Ahead-of-time object emission and linkage against a C host.
//!
//! `V0_3_X_ROADMAP.md` success criterion 2 is that native artefacts "link as
//! static libraries against a host". Open decision 2 is whether V0.3.x is
//! ahead-of-time only or admits a just-in-time path. Everything else in this
//! package tests through the JIT, which answers neither: the JIT never produces
//! an object file, never goes through a linker, and never crosses a real C
//! calling convention.
//!
//! So this file runs the OTHER path end to end. It compiles Keleusma source,
//! lowers it, emits a genuine object file, links it against a C `main` with the
//! system linker, executes the result as a separate process, and compares the
//! answer to the VM.
//!
//! # Why this is not redundant with the JIT tests
//!
//! Three things only this path exercises:
//!
//! 1. **The System V / AAPCS calling convention at a real boundary.** The JIT
//!    calls through a Rust function pointer built by inkwell; a C caller uses
//!    the platform ABI. An `i64` return that happened to work in-process could
//!    still be wrong across a linked boundary.
//! 2. **Symbol emission and external linkage.** JIT symbol lookup is not
//!    linking. A function with the wrong linkage resolves fine in a JIT and
//!    fails at `ld`.
//! 3. **The optimisation pipeline that ships.** The JIT tests run at
//!    `OptimizationLevel::None`, so `mem2reg` never runs and the 30x stack-frame
//!    difference recorded in `NATIVE_LOWERING_INVENTORY.md` is never exercised
//!    by them. This path runs the middle end.
//!
//! # Skipping
//!
//! The test needs a C compiler to link. Where none is found it SKIPS LOUDLY,
//! for the reason `scripts/release-gate.sh` skips this whole package loudly: a
//! step that quietly does nothing reads as a step that passed.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::path::PathBuf;
use std::process::Command;

/// A unique scratch directory for one test, removed on success.
///
/// Deliberately not a fixed path. Two tests running concurrently under
/// `cargo test` would otherwise link over each other's object files and produce
/// a failure that reproduces only under parallelism.
fn scratch(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("keleusma-aot-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&p).expect("create scratch dir");
    p
}

fn c_compiler() -> Option<String> {
    for cc in ["cc", "clang", "gcc"] {
        if Command::new(cc)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Some(cc.to_string());
        }
    }
    None
}

fn vm_answer(src: &str, args: &[i64]) -> i64 {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    match vm.call(&vals).expect("vm run") {
        keleusma::vm::VmState::Finished(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Compile `src`, lower every chunk, optimise, and write a native object file.
/// Returns the object path and the symbol name of the chunk called `entry`.
fn emit_object(src: &str, entry: &str, dir: &std::path::Path) -> (PathBuf, String) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == entry)
        .expect("entry chunk");

    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    let machine = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            // PIC because a modern platform links position-independent
            // executables by default.
            //
            // I first wrote here that a non-PIC object "fails at `ld` with a
            // relocation error". **That was an unverified claim and it is
            // false on this platform**: switching to `RelocMode::Static` links
            // and runs fine on arm64 macOS. It is retained as PIC because it is
            // the right default for the committed target set, not because the
            // alternative was observed to break. Whether a non-PIC object fails
            // on the Linux and embedded targets is untested.
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");

    // The middle end, which the JIT tests never run. See the module comment.
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .expect("optimise");

    let obj = dir.join("kel.o");
    machine
        .write_to_file(&lm, FileType::Object, &obj)
        .expect("write object file");
    (obj, format!("kel_chunk_{idx}"))
}

#[test]
fn a_linked_native_object_agrees_with_the_vm() {
    let Some(cc) = c_compiler() else {
        eprintln!(
            "\n\x1b[1;33mSKIPPED: no C compiler found, so ahead-of-time linkage was NOT \
             verified by this run.\x1b[0m\n"
        );
        return;
    };

    // Exercised deliberately: arithmetic that wraps, a branch, a counted loop,
    // and a CALL, so the linked object contains more than one function and the
    // internal call must resolve within the object rather than through the JIT.
    //
    // **THE CALLEE SUBTRACTS, AND THAT IS NOT A STYLE CHOICE.** The first
    // version used `fn scale(x, k) -> x * k` called as `scale(a, 3)`. A
    // must-fire case that dropped the argument reversal in the lowering left
    // this test passing, because MULTIPLICATION IS COMMUTATIVE: `3 * a` and
    // `a * 3` are the same number, so the swap was undetectable. The test looked
    // like it covered the calling convention across a linked boundary and could
    // not have caught an argument swap. Subtraction is not commutative, and the
    // same mutation now fails it.
    //
    // This is the third vacuous test of this arc, all found by mutation and
    // none by reading. The pattern each time is a symmetry in the test data that
    // hides an asymmetry in the code.
    let src = "fn gap(x: Word, k: Word) -> Word { x - k }
               fn main(a: Word, b: Word) -> Word {
                   if a > b { gap(a, 3) - b } else { for i in 0..4 { } a + b }
               }";
    let args = [[7i64, 3], [3, 7], [i64::MAX, 1], [-5, 2], [0, 0]];

    let dir = scratch("linked");
    let (obj, sym) = emit_object(src, "main", &dir);
    assert!(obj.exists(), "no object file was written");

    // A C driver that calls the emitted symbol and prints the result. Generated
    // rather than checked in, because the symbol name depends on the chunk
    // index and a stale hard-coded name would fail at link time for a reason
    // unrelated to what is being tested.
    let cases: Vec<String> = args
        .iter()
        .map(|a| {
            format!(
                "    printf(\"%lld\\n\", (long long){sym}({}LL, {}LL));",
                a[0], a[1]
            )
        })
        .collect();
    let driver = format!(
        "#include <stdio.h>\nlong long {sym}(long long, long long);\nint main(void) {{\n{}\n    return 0;\n}}\n",
        cases.join("\n")
    );
    let cpath = dir.join("driver.c");
    std::fs::write(&cpath, driver).expect("write driver");

    let exe = dir.join("driver");
    let link = Command::new(&cc)
        .arg(&cpath)
        .arg(&obj)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run the C compiler");
    assert!(
        link.status.success(),
        "LINKING FAILED, which is itself the finding this test exists to \
         surface:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );

    let run = Command::new(&exe).output().expect("run the linked binary");
    assert!(
        run.status.success(),
        "the linked binary did not exit cleanly: {:?}",
        run.status
    );
    let got: Vec<i64> = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(|l| l.trim().parse().expect("numeric output"))
        .collect();

    let expected: Vec<i64> = args.iter().map(|a| vm_answer(src, a)).collect();
    assert_eq!(
        got.len(),
        expected.len(),
        "the linked binary printed {} lines, expected {}",
        got.len(),
        expected.len()
    );
    for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            g, e,
            "linked native and VM disagree for args {:?}: native={g}, vm={e}",
            args[i]
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_emitted_object_exports_the_entry_symbol() {
    // MUST-NOT-FIRE half of the linkage test above. If the symbol were internal
    // or absent, the link would fail there -- but that test SKIPS without a C
    // compiler, and a skipped test proves nothing. This one needs no linker, so
    // symbol emission stays covered on a machine that cannot link.
    let src = "fn helper(x: Word) -> Word { x + 1 }
               fn main(a: Word, b: Word) -> Word { helper(a) + b }";
    let dir = scratch("symbols");
    let (obj, sym) = emit_object(src, "main", &dir);

    let bytes = std::fs::read(&obj).expect("read object file");
    // A crude but binding check: the symbol name must appear literally in the
    // object's string table. Parsing Mach-O and ELF properly would be a real
    // dependency for a check whose whole purpose is to be independent of one.
    let needle = sym.as_bytes();
    assert!(
        bytes.windows(needle.len()).any(|w| w == needle),
        "the emitted object does not contain the symbol {sym}"
    );

    // MUST-FIRE CASE: a name that was never emitted must NOT be found, or the
    // search above is satisfied by any object at all.
    let absent = b"kel_chunk_9999";
    assert!(
        !bytes.windows(absent.len()).any(|w| w == absent),
        "the search found a symbol that was never emitted, so it does not \
         discriminate"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The ABI, across a real linker.** Composites, a data segment, and a native
/// call — the three things the existing linked test does not reach.
///
/// # Why this module and not an easier one
///
/// `a_linked_native_object_agrees_with_the_vm` links arithmetic, a branch, a
/// loop and a call. All of that lives in registers and the object's own text.
/// **None of it exercises the ABI**, which is where the interesting failures
/// are:
///
/// - the **three trailing pointers** (shared, private, region) that every
///   data-bearing or composite-building module carries;
/// - a **native symbol resolved by the linker** rather than by
///   `add_global_mapping`, which is how every JIT differential on this branch
///   bound them and which cannot fail the way a real link can;
/// - a **composite** built in the caller-provided region, so the region pointer
///   is actually dereferenced across the boundary.
///
/// # What it does NOT cover, stated rather than implied
///
/// **No string.** A string-taking native is the operator's open decision (see
/// below), and writing a C host for one would settle it by writing whichever
/// host compiles. **No composite RETURN across the boundary** — the `sret`
/// per-call-site block is exercised by `composite_return_aliasing.rs` through
/// the JIT and is not re-verified here. **No `Stream` entry**; the linked entry
/// is a `Func`.
///
/// # THE STRING ABI, WHICH THIS DELIBERATELY DOES NOT DECIDE
///
/// A string-taking native's C signature would have to be one of:
///
/// ```c
/// long long kel_native_host__name(const struct { long long len; char b[]; } *s);
/// long long kel_native_host__name(const char *s);   /* NUL-terminated only */
/// ```
///
/// The lowering emits the first; the virtual machine hands its native a
/// marshalled `String`. **The two embeddings are not source-compatible for such
/// a native**, and choosing between them is host-visible surface. Surfaced, not
/// settled.
#[test]
fn a_linked_object_with_natives_and_a_data_segment_agrees_with_the_vm() {
    let Some(cc) = c_compiler() else {
        eprintln!(
            "\n\x1b[1;33mSKIPPED: no C compiler found, so ahead-of-time linkage of the \
             ABI surface was NOT verified by this run.\x1b[0m\n"
        );
        return;
    };

    // `host::mix` is NOT commutative in its arguments, deliberately. The third
    // vacuous test of this arc was a commutative callee that could not detect an
    // argument swap; the same trap applies across a linked boundary and applies
    // to natives too.
    let src = "use host::mix(Word, Word) -> Word\n\
               data s { acc: Word }\n\
               fn main(a: Word, b: Word) -> Word {\n\
                   s.acc = host::mix(a, b);\n\
                   let p = (a, b);\n\
                   s.acc + p.0 - p.1\n\
               }";
    let args = [[7i64, 3], [3, 7], [-5, 2], [0, 0]];

    let dir = scratch("linked_abi");
    let (obj, sym) = emit_object(src, "main", &dir);
    assert!(obj.exists(), "no object file was written");

    // The C host DEFINES the native. A JIT would have bound it by address; a
    // linker must resolve the symbol by name, which is the stronger check and
    // the one that fails if `native_symbol`'s mangling is wrong.
    let cases: Vec<String> = args
        .iter()
        .map(|a| {
            format!(
                "    {{ char sh[64] = {{0}}; long long pv[8] = {{0}}; char rg[256] = {{0}};\n\
                 \x20     printf(\"%lld\\n\", (long long){sym}({}LL, {}LL, sh, (char*)pv, rg)); }}",
                a[0], a[1]
            )
        })
        .collect();
    let driver = format!(
        "#include <stdio.h>\n\
         long long kel_native_host__mix(long long x, long long y) {{ return x * 10 + y; }}\n\
         long long {sym}(long long, long long, char*, char*, char*);\n\
         int main(void) {{\n{}\n    return 0;\n}}\n",
        cases.join("\n")
    );
    let cpath = dir.join("driver.c");
    std::fs::write(&cpath, driver).expect("write driver");

    let exe = dir.join("driver");
    let link = Command::new(&cc)
        .arg(&cpath)
        .arg(&obj)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("run the C compiler");
    assert!(
        link.status.success(),
        "LINKING FAILED, which is itself the finding this test exists to \
         surface — a JIT resolves what a linker will not:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );

    let run = Command::new(&exe).output().expect("run the linked binary");
    assert!(
        run.status.success(),
        "the linked binary did not exit cleanly: {:?}",
        run.status
    );
    let got: Vec<i64> = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(|l| l.trim().parse().expect("numeric output"))
        .collect();

    // The VM side registers the SAME native, so both sides compute `x * 10 + y`.
    let expected: Vec<i64> = args
        .iter()
        .map(|a| {
            let m = keleusma::compiler::compile(
                &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex"))
                    .expect("parse"),
            )
            .expect("compile");
            let need = keleusma::vm::required_persistent_capacity_for(&m);
            let cap =
                keleusma::vm::auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
            let mut arena = keleusma_arena::Arena::with_capacity(cap);
            arena.resize_persistent(need).expect("persistent");
            let n_shared = keleusma::vm::shared_data_bytes_for(&m);
            let mut vm = keleusma::vm::Vm::new(m, &arena).expect("vm");
            vm.register_native_closure("host::mix", |v: &[keleusma::bytecode::Value]| {
                let g = |i: usize| match v.get(i) {
                    Some(keleusma::bytecode::Value::Int(x)) => *x,
                    other => panic!("host::mix got {other:?}"),
                };
                Ok(keleusma::bytecode::Value::Int(g(0) * 10 + g(1)))
            });
            let mut sh = vec![0u8; n_shared];
            match vm
                .call_with_shared(
                    &mut sh,
                    &[
                        keleusma::bytecode::Value::Int(a[0]),
                        keleusma::bytecode::Value::Int(a[1]),
                    ],
                )
                .expect("vm run")
            {
                keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(v))
                | keleusma::vm::VmState::Yielded(keleusma::bytecode::Value::Int(v)) => v,
                other => panic!("unexpected VM outcome: {other:?}"),
            }
        })
        .collect();

    assert_eq!(got.len(), expected.len(), "line count mismatch");
    for (i, (g, e)) in got.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            g, e,
            "LINKED native and VM disagree for args {:?}: native={g}, vm={e}",
            args[i]
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
