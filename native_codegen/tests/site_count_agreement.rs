//! Two ways of counting composite construction sites, over one named
//! population.
//!
//! # Why this exists
//!
//! The tree quoted the corpus's construction sites as **239** in `region.rs` and
//! as **256 in 35 chunks** in the handoff. A count over 35 chunks cannot exceed a
//! corpus-wide count, so either the populations differed and neither said so, or
//! one was wrong.
//!
//! **Measured: 239 has no current producer.** The spike its comment cited no
//! longer reports that figure at all — it reports chunks returning and taking
//! composites instead. It is a carried number, which is the class the standing
//! rule on this line exists to catch.
//!
//! # What is checked here
//!
//! That the **planner** and a **raw opcode scan** agree on how many sites exist,
//! over the same corpus. Every `Flat` construction must receive exactly one
//! placement: a planner that dropped one would under-reserve, and one that
//! duplicated would over-reserve. **The two walks are independent** — one reads
//! `plan_chunk_region`, the other reads the instruction stream — so agreement is
//! evidence rather than restatement.
//!
//! The population is the four-root corpus, named in `CORPUS_DIRS` below, and it
//! is stated with every figure this file prints.

use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma_native::region;

/// The four roots the censuses on this line read.
const CORPUS_DIRS: [&str; 4] = [
    "../examples/scripts",
    "../src/selfhost/kel",
    "../examples/rtos/scripts",
    "../compiler/kel",
];

fn corpus() -> Vec<(String, Module)> {
    let mut stack: Vec<std::path::PathBuf> =
        CORPUS_DIRS.iter().map(std::path::PathBuf::from).collect();
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
    paths.dedup();
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

/// Sites as the PLANNER sees them, and as the INSTRUCTION STREAM does.
///
/// **Every `Flat` construction must receive exactly one placement.** A planner
/// dropping one under-reserves the region; one duplicating a site over-reserves
/// it. Neither shows up as a wrong value in a differential, because a region that
/// is too large is invisible and a site that is never placed is refused rather
/// than mispacked — so this is the only place the equality is checked.
#[test]
fn the_planner_and_the_instruction_stream_agree_on_the_site_count() {
    let corpus = corpus();
    assert!(
        corpus.len() > 50,
        "only {} modules compiled; a count over a corpus that failed to load \
         would agree with itself for the wrong reason",
        corpus.len()
    );

    let mut planned = 0usize;
    let mut raw = 0usize;
    let mut chunks_with_sites = 0usize;
    let mut disagreeing: Vec<String> = Vec::new();

    for (name, m) in &corpus {
        for c in &m.chunks {
            let p = region::plan_chunk_region(c).sites.len();
            let r = c
                .ops
                .iter()
                .filter(|o| matches!(o, Op::NewComposite(NewCompositeOperand::Flat { .. })))
                .count();
            if p != r {
                disagreeing.push(format!("{name}::{} planner {p} vs stream {r}", c.name));
            }
            if p > 0 {
                chunks_with_sites += 1;
            }
            planned += p;
            raw += r;
        }
    }

    println!("\n================ COMPOSITE SITES, FOUR-ROOT CORPUS");
    println!("  modules compiled            : {}", corpus.len());
    println!("  chunks with at least one site: {chunks_with_sites}");
    println!("  sites as the planner sees them: {planned}");
    println!("  sites in the instruction stream: {raw}");
    println!(
        "\n  BOTH FIGURES ARE OVER THE FOUR-ROOT CORPUS. A figure from a three-root\n  \
         walk is a different number about a different set, and the tree once\n  \
         carried 239 and 256 side by side without saying so.\n================\n"
    );

    assert!(
        planned > 0 && raw > 0,
        "no sites found by either walk, so their agreement says nothing"
    );
    assert!(
        disagreeing.is_empty(),
        "the planner and the instruction stream disagree per chunk: {disagreeing:?}. \
         Every Flat construction must receive exactly one placement; a dropped site \
         under-reserves the region and a duplicated one over-reserves it, and \
         neither is visible to a differential."
    );
    assert_eq!(planned, raw, "corpus-wide totals must agree");
}

/// **THE EQUALITY MUST BE ABLE TO FAIL**, or it is a restatement rather than a
/// check.
///
/// A chunk is built whose instruction stream carries a construction the planner
/// is not asked about, by comparing the planner's view of one chunk against the
/// stream of another. If the comparison could not distinguish these it could not
/// distinguish a dropped site either.
#[test]
fn the_comparison_distinguishes_a_mismatched_pair() {
    let corpus = corpus();
    let with_sites: Vec<_> = corpus
        .iter()
        .flat_map(|(_, m)| m.chunks.iter())
        .filter(|c| !region::plan_chunk_region(c).sites.is_empty())
        .collect();
    assert!(
        with_sites.len() >= 2,
        "need two site-bearing chunks to build a mismatched pair"
    );

    let a_planned = region::plan_chunk_region(with_sites[0]).sites.len();
    // Deliberately the WRONG chunk's stream.
    let b_raw = with_sites
        .iter()
        .map(|c| {
            c.ops
                .iter()
                .filter(|o| matches!(o, Op::NewComposite(NewCompositeOperand::Flat { .. })))
                .count()
        })
        .find(|n| *n != a_planned);

    assert!(
        b_raw.is_some(),
        "every site-bearing chunk has the same site count, so a mismatched pair \
         cannot be built from this corpus and the check above is untested"
    );
}
