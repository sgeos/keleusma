//! RESEARCH SPIKE: do the bytecode resource bounds survive translation?
//!
//! The project's premise is that a program's worst-case execution time and
//! worst-case memory are statically bounded. Those bounds are proven on the
//! BYTECODE. This asks whether they say anything about the native code.
//!
//! Run with `cargo test --test spike_bounds_transfer -- --nocapture --test-threads=1`.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::{BlockType, Module};
use keleusma::verify::{wcet_stream_iteration, wcmu_stream_iteration};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

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

/// Emit a module's IR at O0 and, optionally, after the shipped pipeline.
fn ir_for(m: &Module, optimise: bool) -> Option<String> {
    let ctx = Context::create();
    let lm = ctx.create_module("bounds");
    lower_module(&ctx, &lm, m, LowerOptions::default()).ok()?;
    if optimise {
        Target::initialize_native(&InitializationConfig::default()).ok()?;
        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).ok()?;
        let machine = target.create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )?;
        lm.run_passes("default<O2>", &machine, PassBuilderOptions::create())
            .ok()?;
    }
    Some(lm.print_to_string().to_string())
}

/// Q1: is the native frame sized by the PROVEN operand depth, or by a constant?
#[test]
fn q1_does_the_native_frame_reflect_the_proven_bound() {
    println!("\n================ Q1: what sizes the native frame?");
    let (mut n, mut allocas_o0, mut allocas_o2) = (0usize, 0usize, 0usize);
    for (path, m) in corpus() {
        let Some(o0) = ir_for(&m, false) else {
            continue;
        };
        let Some(o2) = ir_for(&m, true) else { continue };
        let a0 = o0.matches(" = alloca ").count();
        let a2 = o2.matches(" = alloca ").count();
        let chunks = m.chunks.len();
        if n < 6 {
            println!(
                "  {:32} chunks={chunks:3}  alloca O0={a0:5}  O2={a2:4}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        n += 1;
        allocas_o0 += a0;
        allocas_o2 += a2;
    }
    println!("\n  modules measured : {n}");
    println!("  allocas at O0    : {allocas_o0}");
    println!("  allocas at O2    : {allocas_o2}");
    println!("\n  The lowering emits MAX_STACK = 64 operand slots per function");
    println!("  UNCONDITIONALLY, plus one per local. If O2 removes them the frame");
    println!("  is decided by the optimiser, not by the verifier's bound.");
    println!("================\n");
}

/// Q2: is the bytecode cost bound monotone in the native code size?
///
/// The domination argument for timing assumes native execution is faster than
/// the virtual machine, so the bytecode bound covers it. That assumption needs
/// the two to be ordered the same way. This checks whether a chunk with a larger
/// bytecode bound reliably produces more native instructions.
#[test]
fn q2_is_the_bytecode_bound_monotone_in_native_size() {
    println!("\n================ Q2: bytecode bound against native size");
    let mut pts: Vec<(u32, usize, String)> = Vec::new();
    for (path, m) in corpus() {
        for (i, chunk) in m.chunks.iter().enumerate() {
            if chunk.block_type != BlockType::Stream {
                continue;
            }
            let Ok(w) = wcet_stream_iteration(chunk) else {
                continue;
            };
            let ctx = Context::create();
            let lm = ctx.create_module("one");
            if lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err() {
                continue;
            }
            let sym = format!("kel_chunk_{i}");
            let Some(f) = lm.get_function(&sym) else {
                continue;
            };
            let insts: usize = f
                .get_basic_blocks()
                .iter()
                .map(|b| b.get_instructions().count())
                .sum();
            pts.push((
                w,
                insts,
                format!(
                    "{}::{}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    chunk.name
                ),
            ));
        }
    }
    pts.sort();
    println!("  stream chunks with both figures : {}", pts.len());
    for (w, i, name) in pts.iter().take(12) {
        println!("   wcet={w:6}  native_insts={i:5}  {name}");
    }
    // Count inversions: pairs ordered one way by bound and the other by size.
    let mut inv = 0usize;
    let mut tot = 0usize;
    for a in 0..pts.len() {
        for b in (a + 1)..pts.len() {
            tot += 1;
            if pts[a].0 < pts[b].0 && pts[a].1 > pts[b].1 {
                inv += 1;
            }
        }
    }
    println!("\n  comparable pairs : {tot}");
    println!("  INVERSIONS       : {inv}");
    if tot > 0 {
        println!(
            "  inversion rate   : {:.1}%",
            100.0 * inv as f64 / tot as f64
        );
    }
    println!("\n  An inversion is a pair where the bytecode bound says A < B and");
    println!("  the native code says A > B. Any inversion falsifies the claim that");
    println!("  the bytecode ordering is a proxy for the native ordering.");
    println!("================\n");
}

/// Q3: does the memory bound have a native counterpart at all?
#[test]
fn q3_what_does_the_memory_bound_measure() {
    println!("\n================ Q3: the memory bound's units");
    let mut shown = 0usize;
    for (path, m) in corpus() {
        for chunk in &m.chunks {
            if chunk.block_type != BlockType::Stream || shown >= 6 {
                continue;
            }
            if let Ok((stack, heap)) = wcmu_stream_iteration(chunk) {
                println!(
                    "  {:28} stack_bytes={stack:6}  heap_bytes={heap:6}  locals={}",
                    chunk.name, chunk.local_count
                );
                let _ = path;
                shown += 1;
            }
        }
    }
    println!("\n  `stack_bytes` counts VIRTUAL MACHINE operand slots at the module's");
    println!("  value-slot width. The native frame holds 64 i64 operand slots plus");
    println!("  locals plus whatever the register allocator spills. The two count");
    println!("  different things in different units.");
    println!("================\n");
}
