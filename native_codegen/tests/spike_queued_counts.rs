//! RESEARCH SPIKE, not a regression test.
//!
//! PREPARED WHILE ANOTHER SESSION'S GATE HELD THE MACHINE. Not yet compiled.
//! Install as `native_codegen/tests/spike_queued_counts.rs`.
//!
//! Four questions accumulated in `NATIVE_LOWERING_INVENTORY.md`, each recorded
//! as "queued, needs a corpus count" rather than guessed at. This answers all
//! four in one run so the machine is occupied once rather than four times.
//!
//! Every one of these was deliberately left unanswered rather than estimated,
//! because this document has already been wrong once by reasoning from a
//! plausible distribution instead of counting — it quoted a chunk-level coverage
//! figure where the module level was what mattered.
//!
//! Run with `cargo test --test spike_queued_counts -- --nocapture`.

use keleusma::bytecode::{ConstValue, Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::{BTreeMap, BTreeSet};

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

fn compiled_corpus() -> Vec<(std::path::PathBuf, Module)> {
    let mut out = Vec::new();
    for path in corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        out.push((path, m));
    }
    out
}

fn is_composite_const(c: &ConstValue) -> bool {
    // `Enum` belongs here. Omitting it was caught before this ever ran, and the
    // omission biased the result toward "composite constants cost nothing" --
    // an undercount in the direction that would have licensed skipping work.
    matches!(
        c,
        ConstValue::Tuple(_)
            | ConstValue::Array(_)
            | ConstValue::Struct { .. }
            | ConstValue::Enum { .. }
    )
}

/// QUEUED 1: how much coverage does refusing composite constants actually cost?
///
/// A composite constant materialises BOXED and `AbsVal::Top` cannot reconstruct
/// its shape, so the width stack must refuse any `NewComposite` that packs one.
/// The open question was whether the reference compiler folds literal aggregates
/// into composite constants at all, or emits `NewComposite` over scalar
/// constants instead.
///
/// PROXY, and labelled as one: this counts chunks holding BOTH a composite
/// constant and a construction, which over-approximates. Establishing that a
/// specific constant reaches a specific `NewComposite` needs the shape stack —
/// the very thing this measurement is meant to scope.
#[test]
fn queued_1_composite_constants() {
    let (mut composite_consts, mut chunks_with_any) = (0usize, 0usize);
    let mut chunks_with_both = 0usize;
    let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();

    for (_, m) in compiled_corpus() {
        for chunk in &m.chunks {
            let mut has_composite_const = false;
            for c in &chunk.constants {
                if is_composite_const(c) {
                    composite_consts += 1;
                    has_composite_const = true;
                    by_kind
                        .entry(match c {
                            ConstValue::Tuple(_) => "Tuple",
                            ConstValue::Array(_) => "Array",
                            ConstValue::Struct { .. } => "Struct",
                            _ => "Enum",
                        })
                        .and_modify(|n| *n += 1)
                        .or_insert(1);
                }
            }
            let constructs = chunk.ops.iter().any(|o| matches!(o, Op::NewComposite(..)));
            if has_composite_const {
                chunks_with_any += 1;
                if constructs {
                    chunks_with_both += 1;
                }
            }
        }
    }

    println!("\n================ QUEUED 1: composite constants");
    println!("  composite constants in the corpus : {composite_consts}");
    for (k, n) in &by_kind {
        println!("     {n:5}  {k}");
    }
    println!("  chunks holding one                : {chunks_with_any}");
    println!("  ... that also construct (PROXY)   : {chunks_with_both}");
    println!("  -> if this is 0, refusing composite constants costs NOTHING");
    println!("================\n");
}

/// QUEUED 2: coverage restricted to the self-hosted compiler's own stages.
///
/// Roadmap Order 1 is gated on "the self-hosted compiler's own bytecode runs
/// correctly as native code". Every coverage figure recorded so far is
/// corpus-wide, which is not an answer to that question. The prior was that
/// those stages are composite-heavy and therefore blocked, which would put
/// Order 1 behind the width stack — but a prior is not a measurement.
#[test]
fn queued_2_selfhost_stage_coverage() {
    let (mut ok, mut refused) = (0usize, 0usize);
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_file: Vec<(String, bool)> = Vec::new();

    for (path, m) in compiled_corpus() {
        if !path.to_string_lossy().contains("selfhost/kel") {
            continue;
        }
        let ctx = inkwell::context::Context::create();
        let lm = ctx.create_module("probe");
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        match keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        {
            Ok(_) => {
                ok += 1;
                per_file.push((name, true));
            }
            Err(e) => {
                refused += 1;
                per_file.push((name, false));
                let msg = format!("{e}");
                let key: String = msg.split_whitespace().take(8).collect::<Vec<_>>().join(" ");
                *reasons.entry(key).or_default() += 1;
            }
        }
    }

    println!("\n================ QUEUED 2: Order-1 scope (src/selfhost/kel/)");
    println!("  stage modules compiled : {}", ok + refused);
    println!("  LOWER END TO END       : {ok}");
    println!("  refused                : {refused}");
    for (n, good) in &per_file {
        println!("     {}  {n}", if *good { "OK  " } else { "----" });
    }
    println!("\n  refusal reasons:");
    let mut rs: Vec<_> = reasons.iter().collect();
    rs.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (why, n) in rs.iter().take(8) {
        println!("   {n:4}  {why}");
    }
    println!("================\n");
}

/// QUEUED 3: is `Chunk::name` unique enough to be a symbol?
///
/// The stability defect in `kel_chunk_<index>` is fixable by keying the symbol
/// on `Chunk::name` instead — but ONLY if names do not collide. A stable but
/// ambiguous symbol is worse than an unstable unique one: a link error at best,
/// a silently wrong call at worst.
///
/// Both scopes matter. Within a module, a collision is an immediate duplicate
/// symbol. Across modules it is a cross-module linkage hazard, which is exactly
/// what R4.2's module-path component exists to prevent.
#[test]
fn queued_3_chunk_name_uniqueness() {
    let (mut total_chunks, mut intra_collisions) = (0usize, 0usize);
    let mut global: BTreeMap<String, usize> = BTreeMap::new();
    let mut worst: Vec<(String, String, usize)> = Vec::new();

    for (path, m) in compiled_corpus() {
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for chunk in &m.chunks {
            total_chunks += 1;
            *seen.entry(chunk.name.as_str()).or_insert(0) += 1;
            *global.entry(chunk.name.clone()).or_insert(0) += 1;
        }
        for (name, n) in seen {
            if n > 1 {
                intra_collisions += n - 1;
                worst.push((
                    path.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    name.to_string(),
                    n,
                ));
            }
        }
    }

    let cross: usize = global.values().filter(|n| **n > 1).count();

    println!("\n================ QUEUED 3: Chunk::name uniqueness");
    println!("  chunks total                     : {total_chunks}");
    println!("  distinct names (corpus-wide)     : {}", global.len());
    println!("  WITHIN-module collisions         : {intra_collisions}");
    println!("  names shared ACROSS modules      : {cross}");
    println!("\n  -> within-module must be 0 to key symbols on the name at all.");
    println!("  -> across-module > 0 is the hazard R4.2's module path prevents.");
    if !worst.is_empty() {
        println!("\n  within-module collisions:");
        for (f, n, c) in worst.iter().take(12) {
            println!("     {c}x  {n}   in {f}");
        }
    }
    println!("================\n");
}

/// QUEUED 4: does the corpus use fixed-point at all?
///
/// The fixed-point family was scoped as cheap to implement, reusing the i128
/// machinery the checked ops already have. What was NOT established is whether
/// anything uses it. The coverage spike buckets "float / fixed-point" together,
/// so the payoff was unquantified. A scoped design for a class nobody uses is
/// wasted work.
#[test]
fn queued_4_fixed_point_usage() {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut chunks_touching: BTreeSet<String> = BTreeSet::new();

    for (path, m) in compiled_corpus() {
        for chunk in &m.chunks {
            for op in &chunk.ops {
                let key = match op {
                    Op::WordToFixed(_) => Some("WordToFixed"),
                    Op::FixedToWord(_) => Some("FixedToWord"),
                    Op::FixedMul(_) => Some("FixedMul"),
                    Op::FixedDiv(_) => Some("FixedDiv"),
                    // Only the non-zero forms are unlowered; fb == 0 is the
                    // integer form and already lowers.
                    Op::CheckedMul(fb) if *fb != 0 => Some("CheckedMul(fb>0)"),
                    Op::CheckedDiv(fb) if *fb != 0 => Some("CheckedDiv(fb>0)"),
                    _ => None,
                };
                if let Some(k) = key {
                    *counts.entry(k).or_insert(0) += 1;
                    chunks_touching.insert(format!("{}::{}", path.display(), chunk.name));
                }
            }
        }
    }

    println!("\n================ QUEUED 4: fixed-point usage");
    if counts.is_empty() {
        println!("  NO fixed-point opcodes in the corpus.");
        println!("  -> the scoped design is correct but currently worth ZERO coverage.");
    } else {
        for (k, n) in &counts {
            println!("   {n:5}  {k}");
        }
        println!("  chunks touching fixed-point : {}", chunks_touching.len());
    }
    println!("================\n");
}

/// Guard against measuring nothing, which is the failure mode that makes every
/// zero above look like a finding.
#[test]
fn the_corpus_is_actually_being_read() {
    let n = compiled_corpus().len();
    assert!(n > 10, "compiled only {n} modules; corpus paths are wrong");
}
