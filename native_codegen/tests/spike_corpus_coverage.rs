//! RESEARCH SPIKE, not a regression test.
//!
//! Question: over the shipped Keleusma corpus, which opcodes actually gate
//! native lowering, and in what proportion? The remaining-27 list in
//! `NATIVE_LOWERING_INVENTORY.md` is ordered by what each opcode COSTS to
//! implement. It says nothing about what each opcode BLOCKS, and those are
//! different orderings.
//!
//! Run with `cargo test --test spike_corpus_coverage -- --nocapture`.
//! It reports rather than asserts, except for one guard against measuring
//! nothing.

use keleusma::bytecode::Op;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

/// Every opcode the lowering handles today, by discriminant name.
fn is_lowered(op: &Op, chunk: &keleusma::bytecode::Chunk) -> bool {
    // `Const` is PARTIAL: the lowering accepts Int, Byte, Bool and Unit and
    // refuses a StaticStr or any composite. This copy listed `Op::Const(_)`
    // unconditionally until 2026-08-10, which OVERSTATED every figure below.
    // Caught by the drift control in `spike_stream_sufficiency.rs`, not here.
    if let Op::Const(idx) = op {
        return matches!(
            chunk.constants.get(*idx as usize),
            Some(
                keleusma::bytecode::ConstValue::Int(_)
                    | keleusma::bytecode::ConstValue::Byte(_)
                    | keleusma::bytecode::ConstValue::Bool(_)
                    | keleusma::bytecode::ConstValue::Unit
            )
        );
    }
    matches!(
        op,
        Op::GetLocal(_)
            | Op::SetLocal(_)
            | Op::PopN(_)
            | Op::Dup
            | Op::PushImmediate(_)
            | Op::CheckedAdd
            | Op::CheckedSub
            | Op::CheckedNeg
            | Op::CheckedMul(0)
            | Op::Div
            | Op::Mod
            | Op::CheckedDiv(0)
            | Op::CheckedMod
            | Op::CmpEq
            | Op::CmpNe
            | Op::CmpLt
            | Op::CmpGt
            | Op::CmpLe
            | Op::CmpGe
            | Op::Not
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Shr
            | Op::If(_)
            | Op::Else(_)
            | Op::EndIf
            | Op::Loop(_)
            | Op::EndLoop(_)
            | Op::Break(_)
            | Op::BreakIf(_)
            | Op::Return
            | Op::Trap(_)
            | Op::Call(_, _)
            | Op::WordToByte
            | Op::ByteToWord
            | Op::BoundsCheck(_)
            | Op::GetData(_)
            | Op::SetData(_)
            | Op::GetDataIndexed(..)
            | Op::SetDataIndexed(..)
    )
}

/// Which workstream owns an unsupported opcode, per the inventory.
fn workstream(op: &Op) -> &'static str {
    match op {
        Op::Add | Op::Sub | Op::Mul | Op::Neg => "A-typed (needs operand types)",
        Op::Stream | Op::Yield | Op::Reset => "B (sub-coroutines)",
        Op::NewComposite(..)
        | Op::GetField(..)
        | Op::GetIndex(..)
        | Op::GetTupleField(..)
        | Op::GetEnumField(..)
        | Op::Len
        | Op::IsEnum(..)
        | Op::IsStruct(..) => "C (composites)",
        Op::GetData(..) | Op::SetData(..) | Op::GetDataIndexed(..) | Op::SetDataIndexed(..) => {
            "D (data segment)"
        }
        Op::CallVerifiedNative(..) | Op::CallExternalNative(..) => "D (native ABI)",
        Op::IntToFloat
        | Op::FloatToInt
        | Op::WordToFixed(_)
        | Op::FixedToWord(_)
        | Op::FixedMul(_)
        | Op::FixedDiv(_) => "float / fixed-point",
        _ => "other",
    }
}

fn opcode_name(op: &Op) -> String {
    let d = format!("{op:?}");
    d.split(['(', ' ']).next().unwrap_or(&d).to_string()
}

/// GROUND TRUTH: how many corpus programs lower end to end through the real
/// entry point?
///
/// The classification above mirrors the lowering BY HAND, and a hand mirror
/// rots. This spike was written when 39 opcodes lowered; the set has since moved
/// twice, and each move required editing a list in a file the lowering never
/// reads. This test asks the lowering itself and therefore cannot drift. Where
/// the two disagree, this one is right.
///
/// Module granularity rather than chunk, because `lower_module` is the entry
/// point a consumer calls and it refuses a whole module on the first opcode it
/// cannot handle. It is also the number that matters to a consumer, who deploys
/// programs rather than compilation units.
#[test]
fn spike_report_modules_that_actually_lower() {
    let sources = corpus_sources();
    let (mut ok, mut refused, mut rejected) = (0usize, 0usize, 0usize);
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    for path in &sources {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else {
            rejected += 1;
            continue;
        };
        let Ok(ast) = parse(&toks) else {
            rejected += 1;
            continue;
        };
        let Ok(m) = compile(&ast) else {
            rejected += 1;
            continue;
        };
        let ctx = inkwell::context::Context::create();
        let lm = ctx.create_module("probe");
        match keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        {
            Ok(_) => ok += 1,
            Err(e) => {
                refused += 1;
                let msg = format!("{e}");
                let key: String = msg.split_whitespace().take(9).collect::<Vec<_>>().join(" ");
                *reasons.entry(key).or_default() += 1;
            }
        }
    }
    println!("\n================ GROUND TRUTH: whole modules through lower_module");
    println!("  compiled by the front end : {}", ok + refused);
    println!("  LOWER END TO END          : {ok}");
    println!("  refused by the backend    : {refused}");
    println!("  rejected by the front end : {rejected}");
    if ok + refused > 0 {
        println!(
            "  module-level coverage     : {:.1}%",
            100.0 * ok as f64 / (ok + refused) as f64
        );
    }
    println!("\n  refusal reasons:");
    let mut rs: Vec<_> = reasons.iter().collect();
    rs.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (why, n) in rs.iter().take(10) {
        println!("   {n:4}  {why}");
    }
    println!("================\n");
    assert!(
        ok + refused > 10,
        "measured almost nothing; corpus paths are probably wrong"
    );
}

/// Corpus discovery, shared by the spikes in this file.
fn corpus_sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("..");
    let mut sources: Vec<std::path::PathBuf> = Vec::new();
    for dir in [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ] {
        let d = root.join(dir);
        if let Ok(rd) = std::fs::read_dir(&d) {
            let mut stack: Vec<std::path::PathBuf> =
                rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            while let Some(p) = stack.pop() {
                if p.is_dir() {
                    if let Ok(rd2) = std::fs::read_dir(&p) {
                        stack.extend(rd2.filter_map(|e| e.ok()).map(|e| e.path()));
                    }
                } else if p.extension().is_some_and(|x| x == "kel") {
                    sources.push(p);
                }
            }
        }
    }
    sources.sort();
    sources
}

#[test]
fn spike_report_corpus_coverage() {
    let root = std::path::Path::new("..");
    let mut sources: Vec<std::path::PathBuf> = Vec::new();
    for dir in [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ] {
        let d = root.join(dir);
        if let Ok(rd) = std::fs::read_dir(&d) {
            let mut stack: Vec<std::path::PathBuf> =
                rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            while let Some(p) = stack.pop() {
                if p.is_dir() {
                    if let Ok(rd2) = std::fs::read_dir(&p) {
                        stack.extend(rd2.filter_map(|e| e.ok()).map(|e| e.path()));
                    }
                } else if p.extension().is_some_and(|x| x == "kel") {
                    sources.push(p);
                }
            }
        }
    }
    sources.sort();

    let mut compiled = 0usize;
    let mut rejected = 0usize;
    let mut total_ops = 0usize;
    let mut lowered_ops = 0usize;
    let mut blocking: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_workstream: BTreeMap<&'static str, usize> = BTreeMap::new();
    // A chunk is lowerable only if EVERY opcode in it lowers. That is the unit
    // that matters: a single unsupported opcode refuses the whole chunk.
    let mut chunks_total = 0usize;
    let mut chunks_lowerable = 0usize;
    let mut chunk_first_blocker: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut chunk_lengths: Vec<usize> = Vec::new();

    for path in &sources {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else {
            rejected += 1;
            continue;
        };
        let ast = match parse(&toks) {
            Ok(a) => a,
            Err(e) => {
                rejected += 1;
                println!(
                    "  REJECTED(parse) {} :: {}",
                    path.display(),
                    e.message.chars().take(60).collect::<String>()
                );
                continue;
            }
        };
        let m = match compile(&ast) {
            Ok(m) => m,
            Err(e) => {
                rejected += 1;
                println!(
                    "  REJECTED {} :: {}",
                    path.display(),
                    e.message.chars().take(70).collect::<String>()
                );
                continue;
            }
        };
        compiled += 1;
        for c in &m.chunks {
            chunks_total += 1;
            chunk_lengths.push(c.ops.len());
            let mut ok = true;
            let mut first: Option<&'static str> = None;
            for op in &c.ops {
                total_ops += 1;
                if is_lowered(op, c) {
                    lowered_ops += 1;
                } else {
                    *blocking.entry(opcode_name(op)).or_default() += 1;
                    *by_workstream.entry(workstream(op)).or_default() += 1;
                    if first.is_none() {
                        first = Some(workstream(op));
                    }
                    ok = false;
                }
            }
            if ok {
                chunks_lowerable += 1;
            } else if let Some(w) = first {
                *chunk_first_blocker.entry(w).or_default() += 1;
            }
        }
    }

    assert!(
        compiled > 10 && total_ops > 1000,
        "the spike measured almost nothing ({compiled} files, {total_ops} opcodes); \
         the corpus paths are probably wrong and every number below would be noise"
    );

    println!("\n================ SPIKE: native lowering coverage over the shipped corpus");
    println!(
        "sources found {}, compiled {}, rejected by the front end {}",
        sources.len(),
        compiled,
        rejected
    );
    println!(
        "\nOPCODE INSTANCES: {lowered_ops} of {total_ops} lower ({:.1}%)",
        100.0 * lowered_ops as f64 / total_ops as f64
    );
    println!(
        "CHUNKS FULLY LOWERABLE: {chunks_lowerable} of {chunks_total} ({:.1}%)",
        100.0 * chunks_lowerable as f64 / chunks_total as f64
    );

    println!("\nBLOCKING OPCODE INSTANCES BY WORKSTREAM");
    let mut ws: Vec<_> = by_workstream.iter().collect();
    ws.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (w, n) in ws {
        println!("  {n:6}  {w}");
    }

    println!("\nCHUNKS WHOSE FIRST BLOCKER IS");
    let mut cb: Vec<_> = chunk_first_blocker.iter().collect();
    cb.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (w, n) in cb {
        println!("  {n:6}  {w}");
    }

    println!("\nTOP BLOCKING OPCODES BY INSTANCE COUNT");
    let mut bl: Vec<_> = blocking.iter().collect();
    bl.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (name, n) in bl.iter().take(15) {
        println!("  {n:6}  {name}");
    }

    // Chunk-length distribution, needed to compute the independence null
    // properly rather than at the mean length. `p^L` is convex in `L`, so
    // Jensen's inequality makes the mean-length figure a LOWER bound on the
    // null; evaluating per chunk avoids relying on that direction.
    println!("\nINDEPENDENCE NULL");
    let p_inst = lowered_ops as f64 / total_ops as f64;
    let null_per_chunk: f64 = chunk_lengths
        .iter()
        .map(|&l| p_inst.powi(l as i32))
        .sum::<f64>()
        / chunks_total as f64;
    let null_at_mean = p_inst.powf(total_ops as f64 / chunks_total as f64);
    println!("  p_inst                      {p_inst:.6}");
    println!(
        "  mean chunk length           {:.2}",
        total_ops as f64 / chunks_total as f64
    );
    println!("  median chunk length         {}", {
        let mut v = chunk_lengths.clone();
        v.sort_unstable();
        v[v.len() / 2]
    });
    println!("  null at mean length         {null_at_mean:.3e}");
    println!("  null evaluated per chunk    {null_per_chunk:.3e}");
    println!(
        "  observed rho_unit           {:.6}",
        chunks_lowerable as f64 / chunks_total as f64
    );
    println!(
        "  clustering ratio Phi        {:.3e}",
        (chunks_lowerable as f64 / chunks_total as f64) / null_per_chunk
    );
    println!(
        "  rule-of-three upper bound on a zero-count instance rate: {:.3e}",
        3.0 / total_ops as f64
    );
    println!(
        "  rule-of-three upper bound on a zero-count chunk rate:    {:.3e}",
        3.0 / chunks_total as f64
    );
    println!("================\n");
}
