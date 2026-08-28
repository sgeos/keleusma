//! **The region planner's non-reuse, enforced rather than written down.**
//!
//! `plan_chunk_region` gives every static construction site in a chunk its own
//! storage and **never reuses**. That property carries real weight:
//!
//! - It is why a wrong confinement verdict **cannot miscompile** on this line —
//!   a planner that reuses a slot must be RIGHT that the previous occupant is
//!   dead; this one never needs a verdict at all.
//! - It is why the ephemeral-arena worst-case-memory hazard **cannot fire**. A
//!   loop body allocating at one site writes the SAME offset every iteration, in
//!   a region sized once per chunk, so a degenerate `step` cannot grow.
//!
//! # Why this file exists now
//!
//! **The property was prose in four places and asserted in none** —
//! `region.rs:8`, `region.rs:114`, `composite_return_aliasing.rs:16`,
//! `loop_composite_census.rs:21`. Nothing failed if it stopped being true.
//!
//! **And `NATIVE_LOWERING_INVENTORY.md`'s safety argument had already rotted
//! once**: it said the arena hazard was "harmless only because composite
//! lowering does not exist", and composites now lower. The conclusion survived
//! only because of the non-reuse this file now checks — a fact that document
//! never mentioned.
//!
//! **This line has also spent three iterations building the case for a REUSING
//! planner**, concluding that max-over-arms reuse would close an 11-module arena
//! shortfall. **The pressure to violate this property is one this line
//! manufactured.** A safety property whose only enforcement is a comment, in a
//! codebase being pushed toward violating it, is worth an increment.
//!
//! # ⚠⚠ SCOPE, SECOND AND MORE IMPORTANT AXIS: STATIC SITES, NOT ITERATIONS
//!
//! **This guard checks that two DISTINCT STATIC SITES never share storage. It
//! does NOT establish that the backend "never reuses", and describing it that way
//! is wrong.**
//!
//! A single site inside a loop writes **the same offset on every iteration** —
//! stated in this file's own header above as the reason the memory hazard cannot
//! fire, and true. **That IS reuse**, in the sense the composite-region-reuse
//! proof and this line's own obligation document mean by the word.
//!
//! `docs/proofs/COMPOSITE_REGION_REUSE.md` §4.1.1, established against the
//! runtime by the `v0.2.3` line:
//!
//! > *"**So slot reuse across iterations is UNSOUND TODAY for any composite that
//! > leaves its iteration by `yield`.** This is the live-defect branch, not the
//! > benign one. It is not caught by the epoch guard…"* — a yielded composite is
//! > a handle, an overwrite in place advances no epoch, so `resolve` **succeeds**
//! > and returns iteration n+1's bytes to a host that asked for iteration n's. **A
//! > silent wrong value, not a `Stale` error.**
//!
//! **So the proof's Appendix D row "backend stops reusing slots of unconfined or
//! unseparated sites" is NOT discharged by this guard.** Two different properties:
//!
//! | property | status |
//! |---|---|
//! | distinct static sites never share storage | **TRUE, enforced here** |
//! | a loop site does not reuse across iterations when its value escapes | **FALSE — it reuses unconditionally** |
//!
//! **This line asserted the row was discharged, on 2026-08-27, and that was
//! wrong.** The claim conflated the two axes. Corrected here and to the proof
//! line.
//!
//! **What IS still true**: the memory bound. Same offset every iteration means no
//! per-iteration growth, so the ephemeral-arena leak cannot occur. **Bounded
//! memory and correct aliasing are different guarantees**, and only the first
//! follows from same-offset reuse.
//!
//! **No corpus module is known to have the escaping shape** — the obligation
//! document says so, and the loop census's "disqualified by `Yield`: 1" is an
//! UPPER BOUND on escape ("cannot rule out"), not a demonstration that a value
//! escapes. Those are consistent, and the difference is exactly the bound
//! direction this line keeps having to restate.
//!
//! # ⚠ SCOPE: WITHIN a chunk, and NOT across chunks
//!
//! Offsets are planned **per chunk from zero**, so two chunks' regions can
//! collide. **That is a known, recorded defect** —
//! `composite_return_aliasing.rs` describes a callee writing its result at the
//! same offset on every call while a caller holds two live: *one buffer, one
//! offset, two live values.*
//!
//! **This guard does not cover that and must not be read as covering it.** It
//! checks within-chunk reuse only. The cross-chunk case is orthogonal, still
//! open, and unaffected by anything here.

use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::region::{self, SitePlacement};

const CORPUS_DIRS: [&str; 4] = [
    "examples/scripts",
    "src/selfhost/kel",
    "examples/rtos/scripts",
    "compiler/kel",
];

fn all_compiling_modules() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let q = e.path();
            if q.is_dir() {
                stack.push(q);
            } else if q.extension().is_some_and(|x| x == "kel") {
                paths.push(q);
            }
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        out.push((
            p.file_name().unwrap_or_default().to_string_lossy().to_string(),
            m,
        ));
    }
    out
}

/// The first overlapping pair in `sites`, if any.
///
/// **DISJOINTNESS, not distinctness.** Two sites can hold different starting
/// offsets and still overlap when a size runs past the next offset, and such a
/// layout aliases exactly as badly as a shared offset would. A check on offsets
/// alone would pass it.
fn first_overlap(sites: &[SitePlacement]) -> Option<(SitePlacement, SitePlacement)> {
    for (i, a) in sites.iter().enumerate() {
        for b in sites.iter().skip(i + 1) {
            let a_end = a.offset.saturating_add(a.size);
            let b_end = b.offset.saturating_add(b.size);
            if a.offset < b_end && b.offset < a_end {
                return Some((*a, *b));
            }
        }
    }
    None
}

/// **THE GUARD CAN FIRE**, shown before its clean verdict is believed.
///
/// Ten filters or guards broke in this session, several by passing while unable
/// to fail. A disjointness check that only ever sees disjoint input is evidence
/// about the input, not about the check.
#[test]
fn the_overlap_detector_detects_an_overlapping_layout() {
    let site = |op_index, offset, size| SitePlacement { op_index, offset, size };

    // Disjoint, adjacent: the shape the planner actually produces.
    assert!(
        first_overlap(&[site(0, 0, 8), site(1, 8, 8), site(2, 16, 4)]).is_none(),
        "adjacent non-overlapping sites must NOT be reported as overlapping"
    );

    // Identical offsets: the reuse this guard exists to catch.
    assert!(
        first_overlap(&[site(0, 0, 8), site(1, 0, 8)]).is_some(),
        "two sites at the SAME offset must be detected"
    );

    // **DISTINCT offsets that still overlap** -- the case a distinctness check
    // would wave through, and the reason this predicate is on ranges.
    assert!(
        first_overlap(&[site(0, 0, 16), site(1, 8, 8)]).is_some(),
        "distinct offsets whose RANGES overlap must be detected; a check on \
         offsets alone would pass this and it aliases just as badly"
    );

    // A zero-size site cannot overlap anything.
    assert!(
        first_overlap(&[site(0, 0, 0), site(1, 0, 8)]).is_none(),
        "a zero-length site occupies nothing and must not be reported"
    );

    assert!(first_overlap(&[]).is_none(), "an empty layout has no pairs");
    assert!(first_overlap(&[site(0, 0, 8)]).is_none(), "one site cannot overlap itself");
}

/// No chunk in the corpus has two construction sites sharing storage.
///
/// **A PROPERTY, NOT A COUNT.** Nothing here pins how many sites or chunks
/// exist, so ordinary corpus growth cannot make it fail. What fails is a planner
/// that begins to reuse.
#[test]
fn no_chunk_plans_two_sites_into_overlapping_storage() {
    let corpus = all_compiling_modules();
    let mut chunks_with_sites = 0usize;
    let mut total_sites = 0usize;

    for (name, m) in &corpus {
        for chunk in &m.chunks {
            let layout = region::plan_chunk_region(chunk);
            if layout.sites.is_empty() {
                continue;
            }
            chunks_with_sites += 1;
            total_sites += layout.sites.len();
            if let Some((a, b)) = first_overlap(&layout.sites) {
                panic!(
                    "{name}::{} plans overlapping storage for two sites: op {} at \
                     [{}, {}) and op {} at [{}, {}).\n\n\
                     THE PLANNER HAS BEGUN TO REUSE. That is not a bug in this test. \
                     Non-reuse is what makes a wrong confinement verdict unable to \
                     miscompile on this line, and what makes the ephemeral-arena \
                     worst-case-memory hazard unable to fire. If reuse is intended, \
                     BOTH of those arguments need rebuilding on the verdicts the new \
                     planner consumes -- and `NATIVE_LOWERING_INVENTORY.md`'s hazard \
                     note has already rotted once for exactly this reason.\n\n\
                     SCOPE: this covers reuse WITHIN a chunk. Cross-chunk collision is \
                     a separate, already-recorded defect and is not what fired here.",
                    chunk.name,
                    a.op_index,
                    a.offset,
                    a.offset + a.size,
                    b.op_index,
                    b.offset,
                    b.offset + b.size,
                );
            }
        }
    }

    // **NON-VACUITY: the walk must have seen something.** A corpus that compiled
    // to nothing, or a planner returning empty layouts, would satisfy the loop
    // above while checking no pair at all.
    assert!(
        chunks_with_sites > 0 && total_sites > chunks_with_sites,
        "the walk examined {chunks_with_sites} chunk(s) carrying {total_sites} site(s). \
         With no chunk holding MORE THAN ONE site there is no pair to be disjoint, \
         so this test would pass without checking anything."
    );
    println!(
        "\n  region non-reuse holds over {total_sites} site(s) in {chunks_with_sites} \
         chunk(s) with at least one site\n"
    );
}
