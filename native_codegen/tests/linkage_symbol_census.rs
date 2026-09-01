//! **WHAT MUST A HOST SUPPLY TO LINK A KELEUSMA NATIVE OBJECT?**
//!
//! `V0_3_X_ROADMAP.md` success criterion 2 is that native artefacts "link as
//! static libraries against a host". **One example linking does not meet a
//! criterion phrased that way.** Meeting it means knowing what linking requires,
//! which is a property of every object the backend emits rather than of the one
//! host that happened to satisfy it.
//!
//! `aot_linkage.rs` proves the path works. This file says what the path COSTS.
//!
//! # ⚠ THE OBVIOUS INSTRUMENT MISSES THE ONE CATEGORY THIS EXISTS FOR
//!
//! The natural census is over the LLVM module: list every `declare` with no
//! body. **That answer would be confidently wrong.** Compiler-runtime calls —
//! `memcpy` for a composite copy, `__truncdfsf2` for a float narrowing on a
//! target without hardware support — are **synthesised during code generation**
//! and appear nowhere in the IR. An IR-level census returns a clean, plausible,
//! incomplete list.
//!
//! This line made the identical mistake earlier the same day, in
//! `float_no_contraction.rs`, where an IR-level search for a fused multiply-add
//! passed while measuring nothing. **So this file reads the emitted OBJECT.**
//!
//! # Why now rather than when `f16` lands
//!
//! `FLOAT_LADDER.md` precondition 3 records this question and defers it to the
//! arrival of `f16`. **That trigger is wrong and this file overrules it.** If a
//! compiler-runtime dependency already exists at the shipped rung, the packaging
//! question is live now and `f16` merely widens it; discovering that at an `f16`
//! link failure would mean discovering two things at once and attributing both
//! to the new rung.
//!
//! # The limit of this measurement, stated rather than left to inference
//!
//! **One target, one machine: the host default triple.** It says nothing about
//! `thumbv8m` or any target without hardware floating point, which is the case
//! precondition 3 actually cares about. A narrow-target census is separate work.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// How the backend names the host's contract. `kel_native_<mangled>` for a
/// registered native, plus the coroutine hook.
const HOST_CONTRACT_PREFIXES: [&str; 1] = ["kel_native_"];
const HOST_CONTRACT_EXACT: [&str; 1] = ["kel_yield"];

/// Symbols a toolchain supplies rather than an embedder. Matched on shape, not
/// on an enumerated list, because the point of the census is to find ones that
/// were not anticipated.
fn is_toolchain_symbol(s: &str) -> bool {
    s.starts_with("__")
        || matches!(
            s,
            // `bzero` was UNCLASSIFIED on the first run and is added here
            // **because it is a C-library zero fill**, which is the class this
            // arm already covers — it is Darwin's spelling of what other
            // platforms emit as `memset`. It is not added to empty the
            // unclassified bucket, and the distinction matters: the first run's
            // output is quoted in the record so the addition is auditable.
            "memcpy" | "memset" | "memmove" | "memcmp" | "bzero" | "abort" | "trap"
        )
}

#[derive(Default)]
struct Census {
    host: BTreeSet<String>,
    toolchain: BTreeSet<String>,
    unclassified: BTreeSet<String>,
    /// Which modules require each non-host symbol. **A symbol without the module
    /// that needs it is a fact; with it, it is actionable**, because the module
    /// names the language feature that costs the dependency.
    blame: BTreeMap<String, BTreeSet<String>>,
    objects: usize,
}

impl Census {
    fn classify(&mut self, sym: &str, module: &str) {
        if HOST_CONTRACT_PREFIXES.iter().any(|p| sym.starts_with(p))
            || HOST_CONTRACT_EXACT.contains(&sym)
        {
            self.host.insert(sym.to_string());
        } else if is_toolchain_symbol(sym) {
            self.toolchain.insert(sym.to_string());
            self.blame
                .entry(sym.to_string())
                .or_default()
                .insert(module.to_string());
        } else {
            // **Unclassified is reported as unclassified.** Assigning a symbol
            // to whichever bucket empties this one would defeat the census.
            self.unclassified.insert(sym.to_string());
            self.blame
                .entry(sym.to_string())
                .or_default()
                .insert(module.to_string());
        }
    }

    fn total(&self) -> usize {
        self.host.len() + self.toolchain.len() + self.unclassified.len()
    }
}

/// Reads UNDEFINED symbols out of an object file.
///
/// **Fails loudly rather than skipping when `nm` is absent.** A census that
/// silently reports nothing on a machine without the tool is exactly the shape
/// of a test that passes without testing.
fn undefined_symbols(obj: &Path) -> Vec<String> {
    let out = Command::new("nm")
        .arg("-u")
        .arg(obj)
        .output()
        .unwrap_or_else(|e| panic!("`nm` is required to read {obj:?} and could not be run: {e}"));
    assert!(
        out.status.success(),
        "`nm -u {obj:?}` failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        // Mach-O prefixes every symbol with an underscore; ELF does not. Strip
        // ONE leading underscore so both platforms yield the same name, and note
        // that a genuine `__`-prefixed toolchain symbol keeps its second.
        .map(|l| l.strip_prefix('_').unwrap_or(l).to_string())
        .collect()
}

/// A scratch directory unique to each call.
///
/// # ⚠ THIS FILE PASSED ALONE AND FAILED IN THE SUITE, AND THE REASON IS HERE
///
/// The first version shared one directory and named its objects `m0.o`,
/// `m1.o`, … The three tests that sweep the corpus run **concurrently** under
/// the ordinary test harness, so they wrote the same paths and deleted each
/// other's objects between the write and the read. `nm` then reported the file
/// missing.
///
/// **`--test-threads=1` hid it completely**, which is exactly how it came to be
/// committed to a working tree: the file was verified alone, five tests green,
/// and the full suite found two failures immediately. **A green run of one
/// binary is not evidence about that binary in the suite.**
fn scratch(tag: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("kel_linkage_{tag}_{n}"));
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn emit_object_for(src: &str, dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    let m = compile(&parse(&tokenize(src).ok()?).ok()?).ok()?;
    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    let machine = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).ok()?;
    lm.verify().ok()?;
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .ok()?;

    let obj = dir.join(format!("{name}.o"));
    machine.write_to_file(&lm, FileType::Object, &obj).ok()?;
    Some(obj)
}

fn sweep() -> Census {
    let dir = scratch("census");
    let mut census = Census::default();
    for (i, path) in common::corpus_sources().into_iter().enumerate() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(obj) = emit_object_for(&src, &dir, &format!("m{i}")) else {
            continue; // refused or not standalone; the refusal set is measured elsewhere
        };
        census.objects += 1;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("m{i}"));
        for sym in undefined_symbols(&obj) {
            census.classify(&sym, &name);
        }
        let _ = std::fs::remove_file(&obj);
    }
    census
}

/// **NON-VACUITY, and it comes first.** Every claim below is about what is or is
/// not in a set. All of them hold trivially if no object was emitted or if the
/// reader returned nothing.
#[test]
fn the_sweep_emits_objects_and_the_reader_finds_symbols_in_them() {
    let census = sweep();
    assert!(
        census.objects >= 10,
        "only {} corpus objects were emitted; the census below would be \
         measuring almost nothing",
        census.objects
    );
    assert!(
        census.total() > 0,
        "{} objects were emitted and the reader found no undefined symbol in \
         any of them. Either every object is self-contained — which would be a \
         finding worth stating rather than asserting past — or the reader is \
         broken.",
        census.objects
    );
}

/// **THE INSTRUMENT, DEMONSTRATED ON INPUT THAT DOES NOT COME FROM THIS
/// BACKEND.**
///
/// Demonstrating the reader by compiling a different `.kel` file would perturb
/// the SUBJECT rather than the reader, which this line has recorded proving
/// nothing. So the probe is a C translation unit with one deliberately
/// unresolvable call, built by the system compiler.
#[test]
fn the_symbol_reader_finds_a_symbol_known_to_be_undefined() {
    let dir = scratch("probe");
    let c = dir.join("probe.c");
    std::fs::write(
        &c,
        "extern void zzz_probe_symbol(void);\nvoid f(void){ zzz_probe_symbol(); }\n",
    )
    .expect("write probe");
    let obj = dir.join("probe.o");
    let out = Command::new("cc")
        .args(["-c", "-o"])
        .arg(&obj)
        .arg(&c)
        .output()
        .expect("a C compiler is required to demonstrate the symbol reader");
    assert!(out.status.success(), "probe did not compile");

    let syms = undefined_symbols(&obj);
    assert!(
        syms.iter().any(|s| s == "zzz_probe_symbol"),
        "the reader did not find a symbol that is certainly undefined in the \
         probe object, so every absence it reports elsewhere is worthless. It \
         found: {syms:?}"
    );
}

/// The deliverable: the symbols, named and partitioned.
#[test]
fn the_external_symbol_set_is_named_and_partitioned() {
    let census = sweep();
    println!("\n  objects emitted        : {}", census.objects);
    println!(
        "  host contract ({:2})     : {:?}",
        census.host.len(),
        census.host
    );
    println!(
        "  toolchain     ({:2})     : {:?}",
        census.toolchain.len(),
        census.toolchain
    );
    println!(
        "  UNCLASSIFIED  ({:2})     : {:?}",
        census.unclassified.len(),
        census.unclassified
    );

    for (sym, mods) in &census.blame {
        let shown: Vec<&str> = mods.iter().take(4).map(String::as_str).collect();
        println!(
            "    {sym} <- {} module(s): {}{}",
            mods.len(),
            shown.join(", "),
            if mods.len() > 4 { ", ..." } else { "" }
        );
    }

    assert!(
        census.unclassified.is_empty(),
        "the emitted objects require symbols that are neither the host's \
         registered natives nor recognisable toolchain symbols: {:?}. Each one \
         is something an embedder must supply without having been told to.",
        census.unclassified
    );
}

/// **THE FINDING, PINNED SO IT CANNOT QUIETLY STOP BEING TRUE.**
///
/// `FLOAT_LADDER.md` precondition 3 deferred the linkage question to the arrival
/// of `f16`, on the reasoning that narrow float operations would be the first
/// thing to need a compiler runtime. **Measured, that reasoning is wrong on its
/// facts and not merely on its timing.**
///
/// A compiler-runtime dependency **already exists at the shipped `f64` rung**.
/// It is not a floating-point helper, which is why the deferral looked safe: it
/// is `__divti3`, signed 128-bit integer division, reached through the widened
/// intermediate that fixed-point division uses to avoid overflow.
///
/// **The consequence for the deployment shape is concrete.** An embedder linking
/// a Keleusma object today needs a compiler runtime — `compiler-rt` or `libgcc`
/// — and not merely the natives they registered. On a bare-metal target that
/// links neither, this is a link failure, and it is present now rather than
/// arriving with `f16`.
///
/// If this test ever fails, the dependency has gone away and the record above is
/// stale. **Say so and re-derive it**, rather than deleting the test.
#[test]
fn a_compiler_runtime_dependency_already_exists_at_the_shipped_rung() {
    let census = sweep();
    assert!(
        !census.toolchain.is_empty(),
        "no toolchain symbol was found across {} objects. If that is now true,          the recorded finding that a compiler runtime is required at the f64          rung has expired and FLOAT_LADDER.md must be re-derived rather than          left standing.",
        census.objects
    );
    assert!(
        census.toolchain.iter().any(|s| s.starts_with("__")),
        "the toolchain category holds only C-library symbols {:?}. The recorded          finding is specifically that a COMPILER-RUNTIME symbol is required,          which is the stronger claim, and it no longer holds.",
        census.toolchain
    );
}

/// **WHICH CONSTRUCT COSTS THE DEPENDENCY?**
///
/// A symbol attributed to a module is a fact. A symbol attributed to a LANGUAGE
/// CONSTRUCT is actionable, because an embedder can then be told what it costs.
///
/// # ⚠ TWO WRONG ANSWERS WERE REACHED BY READING RATHER THAN MEASURING
///
/// The backend comment beside the 128-bit widening says the domain exists for
/// **checked arithmetic**, so the obvious inference is that ordinary `Word`
/// division reaches for `__divti3`. **It does not.** A probe divided by the
/// literal 3 and found nothing, which is only because LLVM strength-reduces a
/// constant divisor. A second probe used a runtime divisor and STILL found
/// nothing, because the target has a 64-bit divide instruction.
///
/// **Either wrong answer, written up, would have told an embedder to avoid the
/// wrong operation.** The cause was found by sweeping candidate constructs
/// rather than by a third guess.
///
/// # The answer, and why it matters more than a Word division would have
///
/// **`Fixed` division.** It is the only construct in the sweep that reaches for
/// a compiler-runtime symbol: not `Word` division, not `Word` modulo, not
/// `Fixed` multiplication, not `Byte` division, not `Float` division. Fixed
/// division scales the numerator before dividing, which does not fit in 64 bits,
/// and no target has a 128-bit divide instruction.
///
/// **`FLOAT_LADDER.md` recommends `Fixed` as the default for control work**, and
/// a derate curve, a ratio or a normalisation is a division. So the single
/// operation that costs a runtime dependency is one of the likeliest to appear
/// in the domain this language targets.
#[test]
fn fixed_division_is_the_construct_that_reaches_for_the_compiler_runtime() {
    let dir = scratch("construct");

    const FIXED_DIV: &str = "fn main(w: Word) -> Word {\n  let a = w as Fixed<8>;\n  let b = (w + 1) as Fixed<8>;\n  let q = a / b;\n  q as Word\n}\n";

    // **THE CONTRASTS ARE THE CLAIM.** Without them, "division needs a runtime
    // symbol" could be true of every arithmetic operation and would identify
    // nothing. Each of these was measured clean.
    const CLEAN: [(&str, &str); 5] = [
        (
            "Word division by a runtime divisor",
            "fn main(w: Word) -> Word {\n  let d = w + 1;\n  w / d\n}\n",
        ),
        (
            "Word modulo by a runtime divisor",
            "fn main(w: Word) -> Word {\n  let d = w + 1;\n  w % d\n}\n",
        ),
        (
            "Fixed multiplication",
            "fn main(w: Word) -> Word {\n  let a = w as Fixed<8>;\n  let b = (w + 1) as Fixed<8>;\n  let q = a * b;\n  q as Word\n}\n",
        ),
        (
            "Byte division",
            "fn main(w: Word) -> Word {\n  let a = w as Byte;\n  let b = (w + 1) as Byte;\n  let q = a / b;\n  q as Word\n}\n",
        ),
        (
            "Float division",
            "fn main(w: Word) -> Word {\n  let a = w as Float;\n  let q = a / 3.0;\n  q as Word\n}\n",
        ),
    ];

    let obj = emit_object_for(FIXED_DIV, &dir, "fixed_div").expect("Fixed division must lower");
    let syms = undefined_symbols(&obj);
    println!("\n  Fixed division  -> {syms:?}");
    assert!(
        syms.iter().any(|s| s == "__divti3"),
        "Fixed division no longer reaches for __divti3. The recorded finding is \
         stale and must be re-derived rather than left standing. Found: {syms:?}"
    );

    for (label, src) in CLEAN {
        let o = emit_object_for(src, &dir, "clean").expect("probe must lower");
        let u = undefined_symbols(&o);
        println!("  {label:36} -> {u:?}");
        assert!(
            !u.iter().any(|s| s.starts_with("__")),
            "{label} now reaches for a compiler-runtime symbol {u:?}. The \
             dependency is therefore no longer specific to Fixed division, and \
             the record naming it as the single cause is wrong."
        );
    }
}
