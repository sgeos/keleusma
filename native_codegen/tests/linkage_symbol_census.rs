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
    let format = object_format(obj);
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
        // **`nm -u` DOES NOT PRINT THE SAME SHAPE ON BOTH FORMATS.** On Mach-O
        // it emits a bare name; on ELF it emits `U <name>`. Taking the line
        // verbatim put every ELF symbol into the UNCLASSIFIED bucket, because
        // `"U __divdi3"` matches neither the host-contract prefix nor a
        // toolchain one. Take the
        // last field, which is the name under both.
        .filter_map(|l| l.split_whitespace().next_back())
        // **Decoration is a property of the OBJECT FORMAT, not of a habit.**
        // Mach-O prefixes every symbol with an underscore and ELF does not, so
        // stripping unconditionally would turn a genuine `__aeabi_ldivmod` on an
        // ELF object into `_aeabi_ldivmod` — a name no linker would resolve, and
        // a silent corruption of the very thing the census reports.
        .map(|l| match format {
            ObjFormat::MachO => l.strip_prefix('_').unwrap_or(l).to_string(),
            ObjFormat::Elf => l.to_string(),
        })
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

// ── The target the value proposition is actually written for ──────────────────
//
// Everything above measures `aarch64-apple-darwin`: an operating system, a C
// library, hardware double-precision floating point, and a hardware 64-bit
// divide. **It is the least representative target this project has.**
//
// `examples/rtos/` targets `thumbv8m.main-none-eabihf` — bare metal, no
// operating system, no C library unless one is linked, and a single-precision
// floating-point unit. If linking a Keleusma object there needs symbols nobody
// has named, that is a defect in a shipped example rather than a hypothesis.

/// The bare-metal target measured, chosen because `examples/rtos/` builds for it
/// rather than because it is convenient.
const NARROW_TRIPLE: &str = "thumbv8m.main-none-eabihf";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ObjFormat {
    MachO,
    Elf,
}

/// Reads the object's magic rather than assuming a format from the target.
///
/// **Decoration differs by FORMAT, not by habit.** Mach-O prefixes every symbol
/// with an underscore and ELF does not, so stripping one unconditionally — which
/// the host-only version did — would corrupt a genuine `__aeabi_ldivmod` into
/// `_aeabi_ldivmod` and report a name no linker would resolve.
fn object_format(obj: &Path) -> ObjFormat {
    let bytes = std::fs::read(obj).unwrap_or_else(|e| panic!("read {obj:?}: {e}"));
    assert!(bytes.len() >= 4, "object {obj:?} is too short to identify");
    match &bytes[..4] {
        [0x7f, b'E', b'L', b'F'] => ObjFormat::Elf,
        // Mach-O, 32- and 64-bit, little-endian.
        [0xce, 0xfa, 0xed, 0xfe] | [0xcf, 0xfa, 0xed, 0xfe] => ObjFormat::MachO,
        other => panic!(
            "object {obj:?} is in an unrecognised format (magic {other:02x?}); \
             refusing to guess its symbol decoration"
        ),
    }
}

fn emit_object_for_target(
    src: &str,
    dir: &Path,
    name: &str,
    triple_str: Option<&str>,
) -> Option<std::path::PathBuf> {
    use inkwell::targets::TargetTriple;
    let m = compile(&parse(&tokenize(src).ok()?).ok()?).ok()?;
    // `initialize_native` registers only the host. Registering everything is
    // what makes a cross-target object possible at all.
    Target::initialize_all(&InitializationConfig::default());
    // **DO NOT ROUND-TRIP THE HOST TRIPLE THROUGH A STRING.** A first version
    // passed `get_default_triple().to_string()` and emitted ZERO host objects,
    // silently, so the comparison ran between an empty set and a full one and
    // reported the prediction refuted. `None` means "the machine's own default",
    // which is the thing that actually works.
    let triple = match triple_str {
        Some(t) => TargetTriple::create(t),
        None => TargetMachine::get_default_triple(),
    };
    let target = Target::from_triple(&triple).ok()?;
    let machine = target.create_target_machine(
        &triple,
        "generic",
        "",
        OptimizationLevel::Default,
        RelocMode::PIC,
        CodeModel::Default,
    )?;
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lm.set_triple(&triple);
    lower_module(&ctx, &lm, &m, LowerOptions::default()).ok()?;
    lm.verify().ok()?;
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .ok()?;
    let obj = dir.join(format!("{name}.o"));
    machine.write_to_file(&lm, FileType::Object, &obj).ok()?;
    Some(obj)
}

/// Sweeps the corpus at a chosen target, returning the classified census.
fn sweep_target(triple_str: Option<&str>, tag: &str) -> Census {
    let dir = scratch(tag);
    let mut census = Census::default();
    for (i, path) in common::corpus_sources().into_iter().enumerate() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(obj) = emit_object_for_target(&src, &dir, &format!("m{i}"), triple_str) else {
            continue;
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

/// **NON-VACUITY FOR THE NEW FORMAT, and it must come first.**
///
/// Every comparison below is about set membership, and all of it holds trivially
/// if no narrow-target object was produced or if the reader cannot read ELF.
///
/// **A target absent from the linked LLVM is a FINDING, not an error to route
/// around.** Reporting a host measurement under a narrow-sounding name would be
/// worse than reporting that the census cannot be taken here.
#[test]
fn a_bare_metal_object_is_produced_and_its_symbols_are_readable() {
    let dir = scratch("narrow_probe");
    let src = "fn main(w: Word) -> Word {\n  let d = w + 1;\n  w / d\n}\n";
    let obj =
        emit_object_for_target(src, &dir, "probe", Some(NARROW_TRIPLE)).unwrap_or_else(|| {
            panic!(
                "no object could be emitted for {NARROW_TRIPLE}. If the linked LLVM \
             lacks that target, THAT is the result to record — the census cannot \
             be taken on this machine — and it must not be replaced by a host \
             measurement under a narrow-sounding name."
            )
        });

    assert_eq!(
        object_format(&obj),
        ObjFormat::Elf,
        "the bare-metal object is not ELF, so either the triple was ignored and \
         a host object was produced, or the format detection is wrong. Either \
         way the comparison below would be between two host measurements."
    );

    let syms = undefined_symbols(&obj);
    println!("\n  {NARROW_TRIPLE} Word division -> {syms:?}");
    assert!(
        !syms.is_empty(),
        "the reader returned no symbol from an ELF object. Every absence this \
         file reports about the narrow target would be a property of the reader \
         rather than of the object."
    );
}

/// **THE COMPARISON, WHICH IS THE POINT.** Two unrelated lists answer nothing.
#[test]
fn the_narrow_target_needs_more_of_the_toolchain_than_the_host_does() {
    let host = sweep_target(None, "host_cmp");
    let narrow = sweep_target(Some(NARROW_TRIPLE), "narrow_cmp");

    let only_narrow: BTreeSet<&String> = narrow.toolchain.difference(&host.toolchain).collect();
    let only_host: BTreeSet<&String> = host.toolchain.difference(&narrow.toolchain).collect();
    let shared: BTreeSet<&String> = host.toolchain.intersection(&narrow.toolchain).collect();

    println!("\n  host objects   : {}", host.objects);
    println!("  narrow objects : {}", narrow.objects);
    println!(
        "  host toolchain   ({:2}) : {:?}",
        host.toolchain.len(),
        host.toolchain
    );
    println!(
        "  narrow toolchain ({:2}) : {:?}",
        narrow.toolchain.len(),
        narrow.toolchain
    );
    println!("  shared           ({:2}) : {shared:?}", shared.len());
    println!(
        "  NARROW ONLY      ({:2}) : {only_narrow:?}",
        only_narrow.len()
    );
    println!("  host only        ({:2}) : {only_host:?}", only_host.len());
    println!("  narrow unclassified   : {:?}", narrow.unclassified);

    assert!(
        narrow.objects > 0,
        "no narrow-target objects were emitted; the comparison is vacuous"
    );
    assert!(
        !only_narrow.is_empty(),
        "the bare-metal target requires NO toolchain symbol the host does not. \
         That refutes the recorded prediction — say so and re-derive the record \
         rather than adjusting it. host={:?} narrow={:?}",
        host.toolchain,
        narrow.toolchain
    );
}

/// **WHICH CONSTRUCTS COST WHAT, ON THE TARGET THAT MATTERS.**
///
/// The corpus sweep says the narrow target needs eleven toolchain symbols. It
/// does not say which language construct reaches for which, and an embedder
/// cannot act on a list alone.
#[test]
fn the_narrow_target_attributes_its_runtime_calls_to_constructs() {
    let dir = scratch("narrow_construct");
    let cases: [(&str, &str); 6] = [
        (
            "Word division",
            "fn main(w: Word) -> Word {\n  let d = w + 1;\n  w / d\n}\n",
        ),
        (
            "Word multiplication",
            "fn main(w: Word) -> Word {\n  let d = w + 1;\n  w * d\n}\n",
        ),
        (
            "Float addition",
            "fn main(w: Word) -> Word {\n  let a = w as Float;\n  let s = a + 1.5;\n  s as Word\n}\n",
        ),
        (
            "Float comparison",
            "fn main(w: Word) -> Word {\n  let a = w as Float;\n  if a > 1.5 { 1 } else { 0 }\n}\n",
        ),
        (
            "Fixed division",
            "fn main(w: Word) -> Word {\n  let a = w as Fixed<8>;\n  let b = (w + 1) as Fixed<8>;\n  let q = a / b;\n  q as Word\n}\n",
        ),
        (
            "Fixed multiplication",
            "fn main(w: Word) -> Word {\n  let a = w as Fixed<8>;\n  let b = (w + 1) as Fixed<8>;\n  let q = a * b;\n  q as Word\n}\n",
        ),
    ];

    let mut float_needs_runtime = false;
    let mut word_div_needs_runtime = false;
    for (label, src) in cases {
        let o = emit_object_for_target(src, &dir, "c", Some(NARROW_TRIPLE))
            .unwrap_or_else(|| panic!("{label} must lower for {NARROW_TRIPLE}"));
        let u: Vec<String> = undefined_symbols(&o)
            .into_iter()
            .filter(|s| s.starts_with("__"))
            .collect();
        println!("  {label:24} -> {u:?}");
        if label.starts_with("Float") && !u.is_empty() {
            float_needs_runtime = true;
        }
        if label == "Word division" && !u.is_empty() {
            word_div_needs_runtime = true;
        }
    }

    // These two are falsifiers 2 and 3 of the recorded prediction, checked
    // explicitly rather than read off the corpus aggregate.
    assert!(
        word_div_needs_runtime,
        "Word division is clean on the bare-metal target. That is falsifier 2 of \
         the recorded prediction and it has fired; say so and re-derive the \
         record rather than adjusting it."
    );
    assert!(
        float_needs_runtime,
        "float arithmetic is clean on the bare-metal target. That is falsifier 3 \
         of the recorded prediction and it has fired; say so and re-derive the \
         record rather than adjusting it."
    );
}
