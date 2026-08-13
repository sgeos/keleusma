//! RESEARCH SPIKE: what does the native frame actually cost?
//!
//! The bounds-transfer spike measured stack ALLOCATIONS in the intermediate
//! representation and found them all promoted away by the optimiser. It deferred
//! the question that matters — the size of the frame that actually ships —
//! because the section carrying per-function frame sizes is emitted only for
//! ELF and the development host produces Mach-O.
//!
//! **That deferral was unnecessary.** LLVM cross-targets, so an ELF object can be
//! produced from any host. This emits one, reads the `.stack_sizes` section, and
//! compares the real frame against the verifier's bound.
//!
//! # Skips rather than fails when the toolchain is absent
//!
//! Continuous integration runs on hosted runners with no MacPorts LLVM. A test
//! that required it would turn a missing optional tool into a red build, so this
//! reports and returns instead. **A skip that is silent is a test that quietly
//! stops testing**, so the skip prints its reason.

use inkwell::context::Context;
use keleusma::bytecode::{BlockType, Module};
use keleusma::verify::wcmu_stream_iteration;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::process::Command;

/// Locate the LLVM tools, preferring an explicit override.
fn llvm_bin() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KEL_LLVM_BIN") {
        let p = std::path::PathBuf::from(p);
        if p.join("llc").exists() {
            return Some(p);
        }
    }
    for c in [
        "/opt/local/libexec/llvm-22/bin",
        "/usr/lib/llvm-22/bin",
        "/usr/local/opt/llvm/bin",
    ] {
        let p = std::path::PathBuf::from(c);
        if p.join("llc").exists() && p.join("llvm-readobj").exists() {
            return Some(p);
        }
    }
    None
}

/// Emit `ir` as an ELF object and return each function's frame size in bytes.
fn frame_sizes(bin: &std::path::Path, ir: &str, opt: &str) -> Vec<(String, u64)> {
    let dir = std::env::temp_dir().join(format!("kel_frame_{opt}"));
    let _ = std::fs::create_dir_all(&dir);
    let ll = dir.join("m.ll");
    let obj = dir.join("m.o");
    std::fs::write(&ll, ir).expect("write ir");

    let status = Command::new(bin.join("llc"))
        .args([
            "-mtriple=x86_64-unknown-linux-gnu",
            "-stack-size-section",
            "-filetype=obj",
            opt,
            "-o",
        ])
        .arg(&obj)
        .arg(&ll)
        .status()
        .expect("run llc");
    assert!(status.success(), "llc failed at {opt}");

    let out = Command::new(bin.join("llvm-readobj"))
        .arg("--stack-sizes")
        .arg(&obj)
        .output()
        .expect("run llvm-readobj");
    let text = String::from_utf8_lossy(&out.stdout);

    // Entries are `Functions: [name]` followed by `Size: 0x...`.
    let mut sizes = Vec::new();
    let mut name: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Functions: [") {
            name = rest.strip_suffix(']').map(|s| s.to_string());
        } else if let Some(rest) = t.strip_prefix("Size: ")
            && let Some(n) = name.take()
        {
            let v = rest.trim_start_matches("0x");
            if let Ok(bytes) = u64::from_str_radix(v, 16) {
                sizes.push((n, bytes));
            }
        }
    }
    sizes
}

/// Lower `m` and run the real middle end over it, returning the promoted IR.
///
/// This is the step `llc` does NOT perform. `mem2reg` is a middle-end pass, so
/// raw IR handed to `llc` keeps every operand slot in the frame whatever `-O`
/// level is requested.
fn promoted_ir(m: &Module) -> Option<String> {
    use inkwell::OptimizationLevel;
    use inkwell::passes::PassBuilderOptions;
    use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
    let ctx = Context::create();
    let lm = ctx.create_module("promoted");
    lower_module(&ctx, &lm, m, LowerOptions::default()).ok()?;
    Target::initialize_native(&InitializationConfig::default()).ok()?;
    let triple = TargetMachine::get_default_triple();
    let machine = Target::from_triple(&triple).ok()?.create_target_machine(
        &triple,
        "generic",
        "",
        OptimizationLevel::Default,
        RelocMode::PIC,
        CodeModel::Default,
    )?;
    lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .ok()?;
    Some(lm.print_to_string().to_string())
}

fn corpus() -> Vec<(std::path::PathBuf, Module)> {
    let root = std::path::Path::new("..");
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = ["examples/scripts", "src/selfhost/kel"]
        .iter()
        .map(|d| root.join(d))
        .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel")
            && let Ok(src) = std::fs::read_to_string(&p)
            && let Ok(t) = tokenize(&src)
            && let Ok(a) = parse(&t)
            && let Ok(m) = compile(&a)
        {
            out.push((p, m));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// THE DEFERRED MEASUREMENT: the real frame, against the verifier's bound.
#[test]
fn spike_report_real_native_frame() {
    let Some(bin) = llvm_bin() else {
        println!(
            "\nSKIPPED: no llc/llvm-readobj found. Set KEL_LLVM_BIN to measure.\n\
             This is a reporting spike, not a regression test, so a missing\n\
             optional toolchain must not redden a build."
        );
        return;
    };
    println!("\n================ REAL NATIVE FRAME (ELF, cross-targeted)");
    println!("  toolchain: {}", bin.display());

    let (mut n, mut sum0, mut sum2, mut max0, mut max2) = (0usize, 0u64, 0u64, 0u64, 0u64);
    let (mut sump, mut maxp) = (0u64, 0u64);
    let mut shown = 0usize;
    for (path, m) in corpus() {
        let ctx = Context::create();
        let lm = ctx.create_module("frame");
        if lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err() {
            continue;
        }
        let ir = lm.print_to_string().to_string();
        // THREE measurements, not two. `llc` does not run `mem2reg`, which is a
        // middle-end pass, so raw IR through `llc -O0` and `llc -O2` differ only
        // by back-end choices and BOTH carry every operand slot in the frame.
        // Measuring only those two answers a question nobody asked.
        let f0 = frame_sizes(&bin, &ir, "-O0");
        let f2 = frame_sizes(&bin, &ir, "-O2");
        let promoted = promoted_ir(&m).unwrap_or_else(|| ir.clone());
        let fp = frame_sizes(&bin, &promoted, "-O2");
        if f0.is_empty() {
            continue;
        }
        n += 1;
        for (_, b) in &f0 {
            sum0 += *b;
            max0 = max0.max(*b);
        }
        for (_, b) in &f2 {
            sum2 += *b;
            max2 = max2.max(*b);
        }
        for (_, b) in &fp {
            sump += *b;
            maxp = maxp.max(*b);
        }
        // Print the verifier's bound beside the real frame for stream entries.
        for chunk in m
            .chunks
            .iter()
            .filter(|c| c.block_type == BlockType::Stream)
        {
            if shown >= 8 {
                break;
            }
            if let Ok((stack_bytes, _)) = wcmu_stream_iteration(chunk) {
                let biggest2 = f2.iter().map(|(_, b)| *b).max().unwrap_or(0);
                let biggestp = fp.iter().map(|(_, b)| *b).max().unwrap_or(0);
                let ratio = if stack_bytes > 0 {
                    biggestp as f64 / stack_bytes as f64
                } else {
                    0.0
                };
                println!(
                    "  {:24} verifier={stack_bytes:5}  raw@O2={biggest2:5}  PROMOTED={biggestp:5}  ratio={ratio:5.2}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                shown += 1;
            }
        }
    }
    println!("\n  modules measured        : {n}");
    println!("  total frame bytes at O0 : {sum0}");
    println!("  total frame bytes at O2 : {sum2}");
    println!("  largest single frame O0 : {max0}");
    println!("  largest single frame O2 : {max2}");
    println!("\n  PROMOTED then llc -O2 (the measurement that matters):");
    println!("  total frame bytes       : {sump}");
    println!("  largest single frame    : {maxp}");
    println!("\n  The verifier's number counts virtual-machine operand slots. The");
    println!("  frame counts machine bytes the register allocator could not keep");
    println!("  in registers. If O2 is far below O0, the frame is the optimiser's");
    println!("  decision and not the program's property.");
    println!("================\n");
}
