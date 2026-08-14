//! What, exactly, do the ten `piano_roll` modules need?
//!
//! `spike_report_what_blocks_the_static_string_modules` establishes that all ten
//! contain static strings AND native calls AND streams together, so no single
//! class frees any of them. That is the right answer to "which class first" and
//! the wrong grain to implement against: "stream" is one word covering both the
//! degenerate shape that already lowers and whatever these modules actually do.
//!
//! This asks the finer question. For every chunk it reports **facts read
//! directly off the bytecode** — block type, first and last op, parameter count,
//! callee categories, yield placement — rather than re-deriving
//! `degenerate_stream_yield`. Three copies of a re-derived `is_lowered` are
//! already stale in this directory; a fourth would be a fourth.
//!
//! Cross-checked against `module_refusals`, which is the real lowering, so a
//! fact that contradicts the refusal shows up as a disagreement rather than as a
//! confident wrong number.
//!
//! Run with `cargo test --test probe_piano_roll -- --nocapture`.
use keleusma::bytecode::{BlockType, Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

fn piano_roll_modules() -> Vec<(String, Module)> {
    let dir = std::path::Path::new("../examples/scripts/piano_roll");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut paths: Vec<_> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("piano_roll") && n.ends_with(".kel"))
        })
        .collect();
    paths.sort();
    for p in paths {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        out.push((name, m));
    }
    out
}

/// The structural facts a `Stream` chunk must satisfy to lower as a plain
/// function, each read off the bytecode rather than inferred.
struct StreamFacts {
    starts_with_stream: bool,
    ends_with_reset: bool,
    param_count: u8,
    non_func_callees: Vec<BlockType>,
    yields: usize,
}

fn stream_facts(chunk: &keleusma::bytecode::Chunk, m: &Module) -> StreamFacts {
    let mut non_func_callees = Vec::new();
    for op in &chunk.ops {
        if let Op::Call(idx, _) = op {
            match m.chunks.get(*idx as usize).map(|c| c.block_type) {
                Some(BlockType::Func) => {}
                Some(other) => non_func_callees.push(other),
                None => non_func_callees.push(BlockType::Stream),
            }
        }
    }
    non_func_callees.sort_by_key(|b| format!("{b:?}"));
    non_func_callees.dedup_by_key(|b| format!("{b:?}"));
    StreamFacts {
        starts_with_stream: matches!(chunk.ops.first(), Some(Op::Stream)),
        ends_with_reset: matches!(chunk.ops.last(), Some(Op::Reset)),
        param_count: chunk.param_count,
        non_func_callees,
        yields: chunk.ops.iter().filter(|o| matches!(o, Op::Yield)).count(),
    }
}

#[test]
fn probe_what_the_piano_roll_family_actually_needs() {
    let mods = piano_roll_modules();
    assert!(
        mods.len() >= 10,
        "expected the ten piano_roll modules, compiled {}; the probe is \
         measuring nothing and every figure below would be vacuous",
        mods.len()
    );

    // Aggregate across the family. Per-module detail is printed, but the
    // decision is made on the aggregate: implementing for one module and
    // discovering the other nine differ is the failure mode this avoids.
    let mut refusal_classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut stream_defect: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut total_streams = 0usize;
    let mut clean_streams = 0usize;

    println!("================ PIANO_ROLL: per-module structure");
    for (name, m) in &mods {
        let refusals =
            keleusma_native::module_refusals(m, keleusma_native::LowerOptions::default());
        let mut kinds: Vec<String> = refusals
            .iter()
            .map(|(_, e)| {
                let s = format!("{e}");
                // Keep the discriminating head of the message, drop indices.
                s.split(&[':', '('][..])
                    .next()
                    .unwrap_or(&s)
                    .trim()
                    .to_string()
            })
            .collect();
        kinds.sort();
        kinds.dedup();
        for k in &kinds {
            *refusal_classes.entry(k.clone()).or_default() += 1;
        }

        let streams: Vec<_> = m
            .chunks
            .iter()
            .filter(|c| c.block_type == BlockType::Stream)
            .collect();
        println!(
            "  {name:20} chunks {:3}  streams {:2}  refusals {:3}",
            m.chunks.len(),
            streams.len(),
            refusals.len()
        );
        for k in &kinds {
            println!("      refusal: {k}");
        }
        for c in streams {
            total_streams += 1;
            let f = stream_facts(c, m);
            let mut defects: Vec<&'static str> = Vec::new();
            if !f.starts_with_stream {
                defects.push("prologue-before-Stream");
            }
            if !f.ends_with_reset {
                defects.push("tail-after-Reset");
            }
            if f.param_count > 1 {
                defects.push("param_count>1");
            }
            if !f.non_func_callees.is_empty() {
                defects.push("non-Func-callee");
            }
            if f.yields == 0 {
                defects.push("no-Yield");
            }
            if defects.is_empty() {
                clean_streams += 1;
            }
            for d in &defects {
                *stream_defect.entry(d).or_default() += 1;
            }
            println!(
                "      stream chunk: params {} yields {} callees {:?} defects {:?}",
                f.param_count, f.yields, f.non_func_callees, defects
            );
        }
    }

    // **The goal condition, as an assertion rather than as printed output.**
    //
    // Every module in the family must lower with no refusal at all. This is
    // deliberately NOT a coverage percentage: a number target is satisfied by
    // picking whichever modules are cheapest, which is the behaviour that
    // produced one slice worth zero of 239 sites and another worth one module
    // where eleven were claimed.
    //
    // `refusals.is_empty()` is a weaker claim than "the family is correct" and
    // is not offered as more. What makes the constructs trustworthy is the
    // differential in `native_calls.rs`, which compares the CALL SEQUENCE
    // against the virtual machine; this guards against a regression that would
    // put a construct back outside the subset.
    let still_refused: Vec<&str> = mods
        .iter()
        .filter(|(_, m)| {
            !keleusma_native::module_refusals(m, keleusma_native::LowerOptions::default())
                .is_empty()
        })
        .map(|(n, _)| n.as_str())
        .collect();
    assert!(
        still_refused.is_empty(),
        "the piano_roll family must lower end to end; still refused: {still_refused:?}"
    );

    println!("================ AGGREGATE across {} modules", mods.len());
    println!("  refusal class -> modules mentioning it");
    for (k, v) in &refusal_classes {
        println!("    {v:3}  {k}");
    }
    println!(
        "  stream chunks {total_streams}, structurally clean {clean_streams}, \
         defective {}",
        total_streams - clean_streams
    );
    println!("  stream defect -> chunks exhibiting it");
    for (k, v) in &stream_defect {
        println!("    {v:3}  {k}");
    }
    println!("  NOTE: a `clean` stream still refuses if its BODY contains an");
    println!("  unlowerable op. These facts describe the ENVELOPE only.");

    // The refusal column above stops at the FIRST blocker in each chunk, so a
    // native call sitting after a static string is invisible to it. This
    // histogram cannot hide anything: it counts every op in every chunk.
    //
    // This is the same failure shape as the earlier `static-str ALONE = 11`,
    // inverted. That number over-stated what removing one class would free.
    // A refusal list under-states what remains. Neither is a plan.
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut str_consts = 0usize;
    for (_, m) in &mods {
        for c in &m.chunks {
            for op in &c.ops {
                let key = match op {
                    Op::Const(i) => match c.constants.get(*i as usize) {
                        Some(keleusma::bytecode::ConstValue::StaticStr(_)) => {
                            str_consts += 1;
                            "Const(StaticStr)".to_string()
                        }
                        Some(v) => {
                            format!(
                                "Const({})",
                                format!("{v:?}").split('(').next().unwrap_or("?")
                            )
                        }
                        None => "Const(??)".to_string(),
                    },
                    other => format!("{other:?}")
                        .split(&['(', ' '][..])
                        .next()
                        .unwrap_or("?")
                        .to_string(),
                };
                *hist.entry(key).or_default() += 1;
            }
        }
    }
    println!("================ EVERY OP IN THE FAMILY (nothing hidden behind a refusal)");
    let mut rows: Vec<_> = hist.iter().collect();
    rows.sort_by_key(|(k, v)| (core::cmp::Reverse(**v), (*k).clone()));
    for (k, v) in rows {
        println!("  {v:6}  {k}");
    }
    println!("  distinct op kinds: {}", hist.len());
    println!("  static string constants: {str_consts}");

    // The two facts that decide the native-call lowering's shape.
    //
    // The argument-count byte is NOT a count: its high bit is the B35 P7
    // error-reify flag, and a native carrying it pushes TWO slots rather than
    // one. Reading the byte as a count would corrupt the operand stack at every
    // such site, silently, in the direction the differential oracle catches only
    // where the corpus happens to set the flag.
    let mut reify_sites = 0usize;
    let mut plain_sites = 0usize;
    let mut argc_hist: BTreeMap<u8, usize> = BTreeMap::new();
    let mut unsignatured: BTreeMap<String, usize> = BTreeMap::new();
    let mut signatured = 0usize;
    for (_, m) in &mods {
        for c in &m.chunks {
            for op in &c.ops {
                let (idx, n) = match op {
                    Op::CallVerifiedNative(i, n) | Op::CallExternalNative(i, n) => (*i, *n),
                    _ => continue,
                };
                if n & 0x80 != 0 {
                    reify_sites += 1;
                } else {
                    plain_sites += 1;
                }
                *argc_hist.entry(n & 0x7F).or_default() += 1;
                let name = m
                    .native_names
                    .get(idx as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("<unresolved #{idx}>"));
                match m.native_return_shapes.get(idx as usize) {
                    Some(keleusma::bytecode::WireShape::Top) | None => {
                        *unsignatured.entry(name).or_default() += 1
                    }
                    Some(_) => signatured += 1,
                }
            }
        }
    }
    println!("================ NATIVE CALL SHAPE");
    println!("  plain sites {plain_sites}, error-reify sites {reify_sites}");
    println!("  argument counts (masked 0x7F):");
    for (k, v) in &argc_hist {
        println!("    argc {k:2} -> {v:5} sites");
    }
    println!("  sites whose native HAS a return shape : {signatured}");
    println!("  sites whose native has NONE (Top/absent), by name:");
    for (k, v) in &unsignatured {
        println!("    {v:5}  {k}");
    }
    println!("================");
}
