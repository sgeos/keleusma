//! Every chunk the backend still will not lower, named.
//!
//! # Why a census by workstream was not enough
//!
//! `spike_corpus_coverage` reports the survivors by bucket — at the time this
//! was written, one "B (sub-coroutines)" and one "other". **"Other" is not a
//! cause.** Twice now this line has turned a bucket into an actionable result
//! only by naming it to the module, the instruction and the reason, and twice
//! the named cause differed from the obvious guess: the composite refusals were
//! not the adjacent `Call`, and they were not the `Boxed` form either.
//!
//! This asks the backend directly and prints what it says.

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::{LowerOptions, module_refusals};

const CORPUS_DIRS: [&str; 3] = [
    "examples/scripts",
    "examples/scripts/rogue",
    "src/selfhost/kel",
];

fn corpus() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            paths.push(p);
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        if let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        {
            out.push((name, m));
        }
    }
    out
}

#[test]
fn every_remaining_refusal_is_named_to_the_chunk_and_the_reason() {
    let mods = corpus();
    assert!(
        mods.len() >= 20,
        "the corpus loader found only {} modules; a sweep over a corpus that \
         failed to load would report no refusals for the wrong reason",
        mods.len()
    );

    let mut refusals: Vec<(String, String, String)> = Vec::new();
    for (name, m) in &mods {
        for (sym, err) in module_refusals(m, LowerOptions::default()) {
            refusals.push((name.clone(), sym, err.to_string()));
        }
    }

    println!("\n================ EVERY REMAINING REFUSAL, NAMED");
    println!("  modules swept : {}", mods.len());
    println!("  refusals      : {}", refusals.len());
    for (m, sym, why) in &refusals {
        println!("  ------------------------------------------------");
        println!("  {m}::{sym}");
        println!("      {why}");
    }
    println!("================\n");

    // **NON-VACUITY.** A sweep that found nothing would satisfy any claim about
    // what remains. The corpus is known to contain refusals; if it stops doing
    // so that is a large result and must not pass quietly as "named".
    assert!(
        !refusals.is_empty(),
        "no module refuses anything. Either the backend now lowers the whole \
         corpus -- which would be a much larger result than this test is written \
         for -- or the sweep is not reaching the modules."
    );

    // Pinned so a change announces itself. Not a claim that this number is good.
    assert_eq!(
        refusals.len(),
        3,
        "the set of refusals changed: {refusals:?}. Re-derive the coverage \
         figures and say which chunks changed state before altering anything else."
    );

    // **A MODULE-LEVEL REFUSAL IS NOT A CHUNK-LEVEL ONE**, and the difference is
    // what the coverage census cannot see. It marks a chunk unlowerable by
    // matching the refusal's symbol against the CHUNK NAME; a refusal reported
    // against the module as a whole matches nothing, so every chunk of a module
    // the backend cannot lower at all is still counted as lowerable.
    let module_level: Vec<&(String, String, String)> = refusals
        .iter()
        .filter(|(module, sym, _)| {
            mods.iter()
                .find(|(n, _)| n == module)
                .is_some_and(|(_, m)| !m.chunks.iter().any(|c| &c.name == sym))
        })
        .collect();
    println!(
        "  refusals naming no chunk of their own module: {}",
        module_level.len()
    );
    for r in &module_level {
        println!("    {}::{}", r.0, r.1);
    }
    assert!(
        !module_level.is_empty(),
        "expected at least one module-level refusal, which is the case the \
         coverage census cannot attribute to a chunk"
    );
}

/// **THE PUBLISHED COVERAGE FIGURE OVERSTATES ITSELF.**
///
/// `spike_corpus_coverage` counts a chunk as unlowerable when the refusal's
/// symbol equals the chunk's NAME. A module refused as a whole reports against
/// no chunk, so all of its chunks are counted as lowerable even though the
/// backend cannot lower any of them.
///
/// This measures the size of that overstatement rather than asserting it.
#[test]
fn a_module_level_refusal_leaves_its_chunks_counted_as_lowerable() {
    let mods = corpus();
    let mut overstated = 0usize;
    let mut where_: Vec<String> = Vec::new();
    for (name, m) in &mods {
        let refusals = module_refusals(m, LowerOptions::default());
        if refusals.is_empty() {
            continue;
        }
        let names: Vec<&str> = m.chunks.iter().map(|c| c.name.as_str()).collect();
        // A refusal whose symbol is not a chunk name cannot mark any chunk.
        let unattributable = refusals
            .iter()
            .filter(|(sym, _)| !names.contains(&sym.as_str()))
            .count();
        if unattributable > 0 {
            overstated += m.chunks.len();
            where_.push(format!(
                "{name}: {} chunk(s), {unattributable} refusal(s) naming no chunk",
                m.chunks.len()
            ));
        }
    }
    println!("\n================ COVERAGE OVERSTATEMENT FROM MODULE-LEVEL REFUSALS");
    for w in &where_ {
        println!("  {w}");
    }
    println!("  chunks counted lowerable whose MODULE is refused: {overstated}");
    println!(
        "  The published figure is therefore high by {overstated} chunk(s).\n================\n"
    );
    assert!(
        overstated > 0,
        "no module-level refusal was found, so this test measures nothing"
    );
}
