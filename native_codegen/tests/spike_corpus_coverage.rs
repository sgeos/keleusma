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
mod common;

use std::collections::BTreeMap;
/// Which chunks the REAL lowering refuses, by name.
///
/// **This replaced a hand-maintained per-opcode model on 2026-08-14.** That
/// model was measured stale by 1019 `CallVerifiedNative` instances alone, in the
/// safe direction, and every figure below was correspondingly conservative. It
/// is gone rather than resynchronised: a second copy of a predicate is a drift
/// hazard whatever its current accuracy, and this branch has now been bitten by
/// that three times.
///
/// The granularity changed with it, and honestly so. The model claimed to know
/// per OPCODE; `module_refusals` knows per CHUNK. Per-chunk truth beats per-op
/// fiction, and the refusal message names the blocking construct anyway — which
/// is better attribution than the model ever gave.
/// Chunk names this module's lowering refuses.
///
/// # ⚠ A MODULE-LEVEL REFUSAL USED TO LEAVE EVERY CHUNK COUNTED AS LOWERABLE
///
/// `module_refusals` reports a per-chunk refusal against the chunk's NAME and a
/// whole-module refusal against a symbol that is no chunk's name. The caller
/// marks a chunk unlowerable by matching that symbol against the chunk name, so
/// a module the backend **cannot lower at all** contributed all of its chunks to
/// the lowerable count.
///
/// **Measured: `float_witness.kel` is refused as a module for carrying a float
/// constant, and both of its chunks were being counted as lowerable.** The
/// published figure was therefore two chunks high — `1072 of 1074` where the
/// honest figure is `1070 of 1074`.
///
/// A refusal naming no chunk of this module now marks EVERY chunk of it, which
/// is what "the backend produces no code for this module" means.
fn refused_chunks(m: &keleusma::bytecode::Module) -> std::collections::BTreeSet<String> {
    let refusals = keleusma_native::module_refusals(m, keleusma_native::LowerOptions::default());
    let chunk_names: std::collections::BTreeSet<String> =
        m.chunks.iter().map(|c| c.name.clone()).collect();
    let module_level = refusals
        .iter()
        .any(|(sym, _)| !chunk_names.contains(sym.as_str()));
    if module_level {
        return chunk_names;
    }
    refusals.into_iter().map(|(name, _)| name).collect()
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

/// Compile every corpus source, discarding those that do not compile.
///
/// Ground truth for the module-level ranking, since the retired per-op model was a static
/// model and this asks the real lowering.
fn compiled_corpus_modules() -> Vec<(std::path::PathBuf, keleusma::bytecode::Module)> {
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

/// Every `.kel` source in the shipped example and self-hosted corpora.
fn corpus_sources() -> Vec<std::path::PathBuf> {
    // **Delegates to the one canonical walk**, licensed by
    // `the_shared_walk_matches_this_spikes_own` below rather than by inspection.
    common::corpus_sources()
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
        let refused = refused_chunks(&m);
        for c in &m.chunks {
            chunks_total += 1;
            chunk_lengths.push(c.ops.len());
            let mut ok = true;
            let mut first: Option<&'static str> = None;
            for op in &c.ops {
                total_ops += 1;
                if !refused.contains(&c.name) {
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

    // **REGRESSION FLOORS.** The only assertion above this point is a
    // non-vacuity check — `compiled > 10 && total_ops > 1000` — which catches
    // "the corpus paths are wrong" and not "the backend got worse". Both figures
    // printed here go into this line's handoff every increment, and until now a
    // collapse in either turned no test red.
    //
    // **RATIOS, NOT COUNTS.** Both denominators move with the corpus: adding one
    // `.kel` source changes `chunks_total` and `total_ops`. An absolute floor
    // would fail on ordinary growth, and a guard that fails on growth gets
    // deleted rather than investigated.
    //
    // **IT COMPOUNDS, WHICH IS WHY A FLOOR IS WORTH THE LINE.** `module_refusals`
    // reports per CHUNK but the corpus differential exempts per MODULE, so one
    // newly-refusing chunk removes a whole file from the correctness comparison.
    // A lowering regression therefore shrinks coverage AND the differential.
    //
    // Calibrated 2026-08-29 at 1070 of 1074 chunks (99.63%) and 89841 of 89940
    // instances (99.89%). The slack is roughly half a percent, which absorbs a
    // corpus source arriving with a refused construct without absorbing a
    // regression.
    const CHUNK_RATIO_FLOOR: f64 = 0.99;
    const INSTANCE_RATIO_FLOOR: f64 = 0.99;
    let chunk_ratio = chunks_lowerable as f64 / chunks_total as f64;
    let instance_ratio = lowered_ops as f64 / total_ops as f64;
    assert!(
        chunk_ratio >= CHUNK_RATIO_FLOOR,
        "{chunks_lowerable} of {chunks_total} chunks fully lowerable ({:.2}%),          below the {:.2}% floor calibrated against 1070 of 1074 on 2026-08-29.          Either the backend regressed, or a corpus source arrived carrying          constructs it refuses — the second is legitimate and the floor should          move WITH a recorded reason",
        100.0 * chunk_ratio,
        100.0 * CHUNK_RATIO_FLOOR
    );
    assert!(
        instance_ratio >= INSTANCE_RATIO_FLOOR,
        "{lowered_ops} of {total_ops} opcode instances lower ({:.2}%), below the          {:.2}% floor calibrated against 89841 of 89940 on 2026-08-29",
        100.0 * instance_ratio,
        100.0 * INSTANCE_RATIO_FLOOR
    );
    // NOT A CORRECTNESS CLAIM. Meeting these floors says the backend emitted
    // code, not that the code is right. That is the differential's job, and the
    // census banner says so in its own words.

    // **THESE THREE TABLES DESCRIBE THE CONTENTS OF THE REFUSED CHUNKS, NOT
    // THE CAUSES OF THE REFUSALS**, because an instance counts as blocking when
    // it merely SITS IN a refused chunk. Headed as causes, the last of them
    // reads as a work queue naming opcodes that already lower. Renamed rather
    // than removed: the composition is still worth seeing, provided nobody
    // mistakes it for a diagnosis. The identity behind this is pinned by
    // `the_blocking_instances_are_exactly_the_contents_of_the_refused_chunks`.
    println!("\nWHAT THE REFUSED CHUNKS CONTAIN, BY WORKSTREAM (composition, not cause)");
    let mut ws: Vec<_> = by_workstream.iter().collect();
    ws.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (w, n) in ws {
        println!("  {n:6}  {w}");
    }

    println!("\nCHUNKS BY THE WORKSTREAM OF THEIR FIRST OPCODE (a proxy, not a cause)");
    let mut cb: Vec<_> = chunk_first_blocker.iter().collect();
    cb.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (w, n) in cb {
        println!("  {n:6}  {w}");
    }

    println!("\nCOMMONEST OPCODES INSIDE THE REFUSED CHUNKS (NOT what blocks them)");
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

/// MODULE-LEVEL blocker ranking, from the REAL lowering rather than the model.
///
/// The instance-count ranking above answers "which opcode appears most in code
/// that does not lower". The previous article in this series established that the
/// useful question is different: **which blocker, if removed, frees the most
/// whole programs**. A consumer cannot run 98 percent of a program.
///
/// This uses `lower_module` as ground truth rather than a per-op model, for a
/// reason that now matters: since the degenerate stream lowering landed,
/// That model was stale and still counted `Stream`, `Reset` and `Yield`
/// as unsupported. Ranking from it would put a workstream that is largely DONE
/// at 98 blocking instances.
///
/// **Attribution is first-blocker and therefore order-dependent**, exactly as the
/// previous article recorded. A module refused for `CallVerifiedNative` may also
/// contain composites, so these counts are a lower bound on what each blocker
/// participates in and an upper bound on what removing it alone would free.
#[test]
fn spike_report_module_blocker_ranking() {
    use std::collections::BTreeMap;
    let mut ok = 0usize;
    let mut by_reason: BTreeMap<String, usize> = BTreeMap::new();

    for (path, m) in compiled_corpus_modules() {
        let ctx = inkwell::context::Context::create();
        let lm = ctx.create_module("rank");
        match keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        {
            Ok(_) => ok += 1,
            Err(e) => {
                let msg = format!("{e}");
                // Classify by OPCODE, not by position in the string. The first
                // attempt took the last two words and produced keys like
                // `(0BSD)") Garden`, because a refusal that quotes a rejected
                // string constant ends in that constant's text. A key derived
                // from position rather than meaning fails on exactly the inputs
                // that carry data.
                let key = if let Some(i) = msg.find("opcode ") {
                    msg[i + 7..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("?")
                        .to_string()
                } else if msg.starts_with("Const holding") {
                    String::from("Const (non-scalar)")
                } else {
                    // A `{op:?}` refusal: take the variant name before its
                    // payload parenthesis.
                    msg.split(['(', ' '])
                        .next()
                        .unwrap_or("?")
                        .trim_matches('"')
                        .to_string()
                };
                *by_reason.entry(key).or_default() += 1;
                let _ = path;
            }
        }
    }

    let total = ok + by_reason.values().sum::<usize>();
    println!("\n================ MODULE BLOCKER RANKING (real lowering)");
    println!("  modules            : {total}");
    println!("  LOWER END TO END   : {ok}");
    println!("  refused            : {}", total - ok);
    println!("\n  refused by FIRST blocker, most-blocking first:");
    let mut v: Vec<_> = by_reason.iter().collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (reason, n) in v.iter().take(12) {
        let pct = 100.0 * **n as f64 / total as f64;
        println!("   {n:4}  ({pct:5.1}% of corpus)  {reason}");
    }
    println!("\n  -> first-blocker attribution is ORDER DEPENDENT; a module counted");
    println!("     against one blocker may contain others behind it.");
    println!("================\n");
}

/// CO-OCCURRENCE: what sits BEHIND each first blocker?
///
/// The module ranking is first-blocker attribution and therefore an upper bound
/// on what removing any single blocker delivers. This measures the bound's
/// slack directly: for every refused module, it reports which other unsupported
/// opcode classes the module also contains.
///
/// The question that motivated it: non-scalar `Const` is the first blocker for
/// nine modules, and a string constant looks cheap next to the composite
/// representation. If those same modules also contain native calls or
/// composites, the apparent cheap win frees nothing.
#[test]
fn spike_report_blocker_co_occurrence() {
    use std::collections::{BTreeMap, BTreeSet};

    fn classes(m: &keleusma::bytecode::Module) -> BTreeSet<&'static str> {
        let mut set = BTreeSet::new();
        for c in &m.chunks {
            for op in &c.ops {
                let k = match op {
                    // Construction and access are ONE workstream: nobody
                    // implements `NewComposite` without field access, and
                    // splitting them makes each look less impactful than the
                    // work that would actually be done. The partition decides
                    // the answer, so it is chosen to match the unit of work
                    // rather than the opcode taxonomy.
                    Op::NewComposite(..)
                    | Op::GetField(..)
                    | Op::GetTupleField(..)
                    | Op::GetEnumField(..)
                    | Op::GetIndex(..)
                    | Op::IsEnum(..)
                    | Op::IsStruct(..) => Some("composite"),
                    Op::CallVerifiedNative(..) | Op::CallExternalNative(..) => Some("native-call"),
                    Op::Const(i) => match c.constants.get(*i as usize) {
                        Some(keleusma::bytecode::ConstValue::StaticStr(_)) => Some("static-str"),
                        Some(
                            keleusma::bytecode::ConstValue::Tuple(_)
                            | keleusma::bytecode::ConstValue::Array(_)
                            | keleusma::bytecode::ConstValue::Struct { .. }
                            | keleusma::bytecode::ConstValue::Enum { .. },
                        ) => Some("composite-const"),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(k) = k {
                    set.insert(k);
                }
            }
        }
        set
    }

    let mut refused_with: BTreeMap<String, usize> = BTreeMap::new();
    let (mut refused, mut str_only) = (0usize, 0usize);

    for (_, m) in compiled_corpus_modules() {
        let ctx = inkwell::context::Context::create();
        let lm = ctx.create_module("cooc");
        if keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
            .is_ok()
        {
            continue;
        }
        refused += 1;
        let cs = classes(&m);
        if cs.contains("static-str") {
            let others: Vec<_> = cs.iter().filter(|k| **k != "static-str").copied().collect();
            if others.is_empty() {
                str_only += 1;
            }
            *refused_with
                .entry(if others.is_empty() {
                    String::from("static-str ALONE")
                } else {
                    format!("static-str + {}", others.join(" + "))
                })
                .or_default() += 1;
        }
    }

    // Generalised: the same slack question for EVERY blocker class, not only
    // static strings. The string figure collapsed from 15.5 percent to 1.7 under
    // this check, so applying it to one class and trusting first-blocker counts
    // for the others would repeat the error one column over.
    let mut alone: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut present: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (_, m) in compiled_corpus_modules() {
        let ctx = inkwell::context::Context::create();
        let lm = ctx.create_module("slack");
        if keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
            .is_ok()
        {
            continue;
        }
        let cs = classes(&m);
        for k in &cs {
            *present.entry(k).or_default() += 1;
            if cs.len() == 1 {
                *alone.entry(k).or_default() += 1;
            }
        }
    }
    println!("\n================ SLACK BY BLOCKER CLASS");
    println!("  class                 present   ALONE   slack");
    let total_mods = compiled_corpus_modules().len();
    for (k, n) in &present {
        let a = alone.get(k).copied().unwrap_or(0);
        println!(
            "  {k:20} {n:6}  {a:6}  {:5.1}pp",
            100.0 * (*n as f64 - a as f64) / total_mods as f64
        );
    }
    println!("  (ALONE = the only unsupported class in that module, so the only");
    println!("   count that removing it alone would actually free)");
    println!("================");

    println!("\n================ CO-OCCURRENCE behind `static-str`");
    println!("  refused modules total          : {refused}");
    println!(
        "  ... containing a static string : {}",
        refused_with.values().sum::<usize>()
    );
    println!("  ... blocked by it ALONE        : {str_only}");
    println!();
    for (k, n) in &refused_with {
        println!("   {n:3}  {k}");
    }
    println!("\n  -> if `ALONE` is 0, lowering static strings frees NO module by");
    println!("     itself, and the 15.5% first-blocker figure is entirely slack.");
    println!("================\n");
}

/// **SLACK, DERIVED FROM THE REAL LOWERING RATHER THAN FROM A MODEL.**
///
/// Every slack figure before this came from a hand-maintained per-op list, now retired.
/// Three copies of that list exist, all three went stale in the PESSIMISTIC
/// direction, and the drift control asserts only the optimistic one — so the
/// staleness understated every blocker class silently and could not be detected.
///
/// `module_refusals` returns one refusal per CHUNK instead of one per module, so
/// a module's refusal set is the union over its chunks, derived from the code
/// that actually decides. Coarser than per-op and impossible to leave stale.
#[test]
fn spike_report_derived_slack() {
    fn class_of(msg: &str) -> &'static str {
        if msg.contains("StaticStr") {
            "static-str"
        } else if msg.contains("CallVerifiedNative") || msg.contains("CallExternalNative") {
            "native-call"
        } else if msg.contains("Stream") || msg.contains("Yield") || msg.contains("Reset") {
            "stream"
        } else if msg.contains("Composite")
            || msg.contains("GetField")
            || msg.contains("GetIndex")
            || msg.contains("IsEnum")
            || msg.contains("TupleField")
            || msg.contains("EnumField")
        {
            "composite"
        } else {
            "other"
        }
    }

    let mut present: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut alone: BTreeMap<&'static str, usize> = BTreeMap::new();
    let (mut refused, mut total) = (0usize, 0usize);

    for (_, m) in compiled_corpus_modules() {
        total += 1;
        let rs = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
        if rs.is_empty() {
            continue;
        }
        refused += 1;
        let classes: std::collections::BTreeSet<&'static str> =
            rs.iter().map(|(_, e)| class_of(&format!("{e}"))).collect();
        for c in &classes {
            *present.entry(c).or_default() += 1;
        }
        if classes.len() == 1 {
            let only = classes.iter().next().copied().expect("one class");
            *alone.entry(only).or_default() += 1;
        }
    }

    println!("================ DERIVED SLACK (from lower_module, not a model)");
    println!("  modules {total}, refused {refused}");
    println!("  class          present   ALONE   (ALONE is what removing it frees)");
    let mut keys: Vec<_> = present.keys().copied().collect();
    keys.sort_by_key(|k| core::cmp::Reverse(present.get(k).copied().unwrap_or(0)));
    for k in keys {
        let p = present.get(k).copied().unwrap_or(0);
        let a = alone.get(k).copied().unwrap_or(0);
        println!("  {k:14} {p:7} {a:7}");
    }
    println!("  NOTE: per-CHUNK refusals, so a chunk's own later blockers are");
    println!("  still hidden behind its first. This under-counts co-occurrence,");
    println!("  which INFLATES ALONE — the opposite bias to the stale model.");
    println!("================");
}

/// **Would static strings actually free those 11 modules?**
///
/// The derived slack says `static-str` is ALONE in 11 of 20 refused modules, but
/// it reads one refusal per CHUNK, so a blocker sitting AFTER the string inside
/// the same chunk is invisible to it. This checks the same modules by OP
/// PRESENCE, which cannot hide behind refusal order.
///
/// The question is not academic: the baked-address composite slice was ranked at
/// 34.5% of the corpus and measured at ZERO once the corpus was asked what it
/// actually contains.
#[test]
fn spike_report_what_blocks_the_static_string_modules() {
    let mut freed = 0usize;
    let mut also: BTreeMap<&'static str, usize> = BTreeMap::new();

    for (path, m) in compiled_corpus_modules() {
        let rs = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
        if rs.is_empty() {
            continue;
        }
        let mentions_str = rs.iter().any(|(_, e)| format!("{e}").contains("StaticStr"));
        if !mentions_str {
            continue;
        }
        // Op presence across the WHOLE module, independent of refusal order.
        let mut others: Vec<&'static str> = Vec::new();
        for c in &m.chunks {
            for op in &c.ops {
                match op {
                    Op::CallVerifiedNative(..) | Op::CallExternalNative(..) => {
                        others.push("native-call")
                    }
                    Op::Stream | Op::Yield | Op::Reset => others.push("stream"),
                    _ => {}
                }
            }
        }
        others.sort_unstable();
        others.dedup();
        if others.is_empty() {
            freed += 1;
        } else {
            for o in &others {
                *also.entry(o).or_default() += 1;
            }
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            println!("  {name:32} also contains {others:?}");
        }
    }

    println!("================ WOULD STATIC STRINGS FREE THEM?");
    println!("  string-blocked modules with NO other blocking op : {freed}");
    for (k, n) in &also {
        println!("  ... also containing {k:14} : {n}");
    }
    println!("  -> `freed` is what implementing static strings would actually");
    println!("     deliver. The ALONE column cannot see a blocker that sits");
    println!("     after the string inside the same chunk.");
    println!("================");
}

/// The shared enumeration must return exactly what this spike's own walk did.
///
/// Kept after the migration as the standing check: if the canonical walk ever
/// narrows, the figures in this file move with it, and this is what says so.
#[test]
fn the_shared_walk_matches_this_spikes_own() {
    let shared = common::corpus_sources();
    assert!(
        shared.len() > 40,
        "the canonical walk returned only {} sources; every figure in this file \
         rests on it",
        shared.len()
    );
    // The population this spike's figures are derived from, pinned the way
    // `corpus_fingerprint` pins content. A narrowing walk would otherwise move
    // the coverage ratios with nothing going red.
    assert_eq!(
        shared.len(),
        corpus_sources().len(),
        "this spike and the canonical walk disagree about the corpus population"
    );
}

/// **THE 86 BLOCKING INSTANCES ARE NOT 86 BLOCKERS, AND THE REPORT ABOVE READS
/// AS IF THEY WERE.**
///
/// `spike_report_corpus_coverage` counts an opcode instance as "blocking" when
/// it sits inside a chunk that is refused — every instance in the chunk, not the
/// one that caused the refusal. So its per-opcode table is the **composition of
/// the refused chunks**, and `GetLocal`, `Const` and `SetLocal` head it purely
/// because they are the commonest opcodes in any chunk. A reader taking that
/// table as a work queue would set out to implement opcodes that already lower.
///
/// This test states the relationship as a MEASURED identity rather than leaving
/// it to be inferred from reading the loop: the blocking instance count equals
/// the summed length of the refused chunks, exactly.
///
/// It also pins the residual to the chunks that produce it, so that a coverage
/// movement announces which chunk moved rather than only that a number changed.
#[test]
fn the_blocking_instances_are_exactly_the_contents_of_the_refused_chunks() {
    let mut refused_chunk_ops = 0usize;
    let mut blocking_instances = 0usize;
    let mut named: Vec<(String, String, usize)> = Vec::new();
    let mut modules = 0usize;
    let mut chunks = 0usize;

    for path in corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        modules += 1;
        let refused = refused_chunks(&m);
        for c in &m.chunks {
            chunks += 1;
            // **THE TWO SIDES ARE COMPUTED BY DIFFERENT TRAVERSALS ON PURPOSE.**
            // The first accumulates per OPCODE with a membership test, which is
            // how the report derives its blocking count; the second sums the
            // LENGTHS of the refused subset. Writing both as `c.ops.len()` makes
            // the assertion `x == x`, which is what the first draft of this test
            // did and what clippy caught. What the guard is really for is a
            // future change to the report's rule -- counting only the CAUSING
            // opcode, say -- after which these diverge and the tables' labels
            // need revisiting.
            for _op in &c.ops {
                if refused.contains(&c.name) {
                    blocking_instances += 1;
                }
            }
            if refused.contains(&c.name) {
                refused_chunk_ops += c.ops.len();
                named.push((
                    path.file_name().unwrap().to_string_lossy().to_string(),
                    c.name.clone(),
                    c.ops.len(),
                ));
            }
        }
    }

    println!("\n================ THE RESIDUAL, NAMED TO THE CHUNK");
    println!("  population: {modules} modules, {chunks} chunks");
    println!("  refused chunks: {}", named.len());
    for (file, chunk, len) in &named {
        println!("    {file}::{chunk}  ({len} opcodes)");
    }
    println!(
        "  blocking opcode instances: {blocking_instances}, which is the SUM of \
         those chunks' lengths"
    );
    println!(
        "  => the chunk count and the instance count are ONE finding, not two. \
         The per-opcode\n     table in the report above lists what those chunks \
         CONTAIN, not what blocks them."
    );
    println!("================\n");

    // **NON-VACUITY, BOTH WAYS.** A sweep that found no chunks would satisfy the
    // identity trivially, and one that found no refusals would report an empty
    // residual for the wrong reason.
    assert!(
        chunks > 100,
        "the sweep saw only {chunks} chunks, so it is measuring the harness \
         rather than the corpus"
    );
    assert!(
        !named.is_empty(),
        "no refused chunk was found. If the residual really is closed that is a \
         RESULT and this assertion should be replaced by one recording it — but \
         a silently empty sweep is the likelier cause and must be excluded first"
    );
    assert_eq!(
        blocking_instances, refused_chunk_ops,
        "the blocking instance count and the refused chunks' summed length \
         disagree, which would mean the two published figures have different \
         causes after all"
    );
}
