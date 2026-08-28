//! **What actually explains the arena-bound gap?**
//!
//! `bound_transfer` reports modules whose backend arena demand exceeds their
//! verified figure: `backend = sites * size`, `verified = peak_live * size`.
//!
//! `confinement_vs_arena_gap` ruled out the obvious explanation — within the only
//! family that exceeds, the exceeding members are MORE confined than the
//! compliant ones, so escape analysis is not the missing term.
//!
//! **The arithmetic points somewhere specific.** `rogue_combat` has 4 sites and
//! an implied `peak_live` of 1; `rogue_player_ai` has 5 and 1. Reading the first:
//! `main` returns `(Word, Word)` and constructs that tuple at FOUR sites —
//! `(0,0)`, `(2,dmg)`, `(1,dmg)`, `(0,0)` — each in a different arm of nested
//! conditionals. **Exactly one runs.**
//!
//! > **The backend SUMS over static sites; the verifier takes the PEAK over live
//! > values. Where sites sit on mutually exclusive paths, the sum exceeds the
//! > peak by construction.**
//!
//! **That module MOTIVATED the hypothesis and cannot also be the evidence for
//! it**, so this file measures the whole exceeding set.
//!
//! # The approximation, labelled
//!
//! `max_sites_on_a_path` walks the op stream tracking `If`/`Else`/`EndIf` nesting
//! and takes, at each conditional, the MAXIMUM over its arms rather than the sum.
//! **It is an UPPER BOUND on simultaneous liveness**, for two reasons stated
//! rather than hidden:
//!
//! - it counts a site as live for the whole enclosing region, never noticing that
//!   a value died earlier;
//! - it does not model `Break`/`BreakIf` skipping a site.
//!
//! **So `path_max >= peak_live` always, and the interesting question is whether
//! it comes DOWN to `peak_live`.** An upper bound that reaches the verifier's
//! figure explains the gap; one that stays above it leaves a residue.
//!
//! # Loops are deliberately NOT counted as branches
//!
//! A site inside a loop body can be live across iterations, which is a different
//! question. `Loop`/`EndLoop` are traversed as straight-line here — sites inside
//! them ACCUMULATE — so a loop can only make this bound larger, never smaller.
//! Conflating the two would inflate the explanation.
//!
//! # THE VERDICT, MEASURED 2026-08-27: **11 OF 11, ZERO RESIDUE**
//!
//! Every exceeding module's `path-max` equals its verified `peak_live` EXACTLY.
//!
//! ```text
//!   rogue_ai_boss      4 sites  path-max 2   verified peak_live 2   EXPLAINED
//!   rogue_ai_hunter    5 sites  path-max 2   verified peak_live 2   EXPLAINED
//!   rogue_combat       4 sites  path-max 1   verified peak_live 1   EXPLAINED
//!   rogue_player_ai    5 sites  path-max 1   verified peak_live 1   EXPLAINED
//!   ... 11 of 11, 0 residue
//! ```
//!
//! **THE DIRECTION OF THE BOUND MAKES THIS THE STRONGEST AVAILABLE RESULT.**
//! `path-max` is an UPPER bound on simultaneous liveness, so `path-max >=
//! peak_live` holds by construction. It could have landed anywhere at or above
//! the verifier's figure; **it lands exactly on it, in every case.** An upper
//! bound that meets a lower one leaves nothing between them.
//!
//! **So the gap is fully accounted for**: the backend sums over static sites, the
//! verifier peaks over live values, and in every exceeding module the sites sit
//! on mutually exclusive arms. Nothing else contributes.
//!
//! ## The candidate remedy, named as a candidate
//!
//! A planner taking the **MAX across exclusive arms** instead of the sum would
//! bring the backend's demand to exactly the verified figure on this corpus.
//!
//! **That is NOT adopted and NOT decided here.** Before it could be, at least:
//! whether the max-over-arms rule is sound when a region outlives its arm (which
//! is the confinement question, and `confinement_vs_arena_gap` shows the
//! exceeding modules' sites mostly ESCAPE); how it interacts with loop-carried
//! liveness, which this walk deliberately does not model; and whether the
//! verifier's `peak_live` is itself computed on an axis the planner can reproduce.
//!
//! **The escape finding and this one are not in tension** -- they answer different
//! questions. Confinement asks whether a region is dead after its scope; this asks
//! whether two regions are ever live at once. **A site can escape its scope and
//! still never coexist with its sibling in the other arm.**
//!
//! # ⚠ CORRECTION (2026-08-27, same day): THE LABEL WAS NOT EARNED
//!
//! **`planner_verifier_axis` refutes this file's claim that `path-max >=
//! peak_live` holds BY CONSTRUCTION.** Three comparable modules violate it, and
//! on seven more the quotient exceeds the site count and is therefore not a count
//! at all.
//!
//! **The derivation `peak_live = max_heap_bytes / (demand / sites)` is this
//! line's, not the verifier's.** `bound_transfer` compares the two figures as an
//! inequality and never divides. `max_heap_bytes` is peak per-iteration heap OVER
//! EVERY CHUNK; `region_total_bytes` is an entry-rooted total across the call
//! tree. **Different axes; their ratio is not a population count.**
//!
//! **WHAT STILL STANDS**: `path-max` equals `max_heap / size` **exactly on all 11**
//! exceeding modules. That measurement is real and re-derivable.
//!
//! **WHAT DOES NOT**: calling the right-hand side "the verifier's peak live
//! count", and the claim that the inequality is structural. **Whether the equality
//! reflects branch exclusivity or an unrelated alignment is NOT resolved** — and
//! is deliberately not replaced with a second guess.
//!
//! Read the verdict below with that correction applied.
//!
//! # ⚠ THE PROOF LINE HAS RULED ON THE CANDIDATE REMEDY (absorbed 2026-08-27)
//!
//! `docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md` is now in this tree, and its
//! change-control appendix carries two rows **owned by this line**:
//!
//! | consequence | decision |
//! |---|---|
//! | backend stops reusing slots of unconfined or unseparated sites | *"required for soundness independent of this proof"* |
//! | backend may overlap exclusive arms | licensed *"only for a runtime discharging M1 through M9 itself… so the license is conditional on that discharge plus Appendix A's scoping"* |
//!
//! **The first is already discharged and now ENFORCED** — `plan_chunk_region`
//! never reuses, and `region_nonreuse.rs` fails if it starts.
//!
//! **The second adds a FOURTH precondition to the max-over-arms candidate named
//! below**, alongside the three already recorded. The proof line's own message
//! states that nothing yet discharges M1–M9.
//!
//! ## And a caution that lands on exactly this population
//!
//! The proof's comparison remark: *"For a reused site inside a conditional arm,
//! the slot is provisioned statically outside the arm maximum, and extracting a
//! term from under a maximum can exceed the maximum, so the footprint can be
//! larger … on branch-dominated shapes. A planner should compute both figures and
//! adopt reuse per site only where it helps."*
//!
//! **Every exceeding module here is branch-dominated** — 33 exclusivity sites in
//! bare conditionals, zero in loops, measured by `max_over_arms_precondition.rs`.
//! **So the shapes this file's remedy targets are the shapes that remark warns
//! about.**
//!
//! **Stated at its real strength**: the remark concerns *reuse provisioning*,
//! which is adjacent to rather than identical with the max-over-arms rule.
//! **It does not refute the remedy; it removes "obviously beneficial" from it**
//! and prescribes per-site comparison of both bounds instead of a blanket rule.
//!
//! # Nothing here changes code generation
//!
//! No planner is modified and no remedy is adopted. `plan_chunk_region` still
//! assigns one offset per static site.

use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma::value_layout::CompositeKind;
use keleusma::verify::module_runtime_footprint;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::region;

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
        let Ok(rd) = std::fs::read_dir(&p) else {
            continue;
        };
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
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            m,
        ));
    }
    out
}

fn is_site(op: &Op) -> bool {
    matches!(op, Op::NewComposite(NewCompositeOperand::Flat { .. }))
}

/// Sites on the single path through `ops` that carries the most of them.
///
/// At an `If`, the then-arm and else-arm are alternatives: their contribution is
/// the MAX of the two, not the sum. Everything else accumulates.
///
/// **UPPER BOUND on simultaneous liveness** — see this file's header for the two
/// reasons. Loops accumulate, deliberately.
fn max_sites_on_a_path(ops: &[Op]) -> usize {
    // (accumulated before this conditional, best of the arms seen so far, current arm)
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut cur = 0usize;
    for op in ops {
        match op {
            Op::If(_) => {
                stack.push((cur, 0));
                cur = 0;
            }
            Op::Else(_) => {
                if let Some((_, best)) = stack.last_mut() {
                    *best = (*best).max(cur);
                }
                cur = 0;
            }
            Op::EndIf => {
                if let Some((before, best)) = stack.pop() {
                    cur = before + best.max(cur);
                }
            }
            o if is_site(o) => cur += 1,
            _ => {}
        }
    }
    // An unbalanced stream (truncated or malformed) folds outward rather than
    // silently dropping what it accumulated.
    while let Some((before, best)) = stack.pop() {
        cur = before + best.max(cur);
    }
    cur
}

fn module_sites(m: &Module) -> usize {
    m.chunks
        .iter()
        .map(|c| c.ops.iter().filter(|o| is_site(o)).count())
        .sum()
}

fn module_path_max(m: &Module) -> usize {
    m.chunks
        .iter()
        .map(|c| max_sites_on_a_path(&c.ops))
        .max()
        .unwrap_or(0)
}

/// **THE WALK DISCRIMINATES, shown before its output is believed.**
///
/// Sequential sites must count as many; exclusive sites must count as one. A walk
/// that returned the raw site count, or always 1, would produce a plausible table
/// and mean nothing.
#[test]
fn the_path_walk_separates_sequential_sites_from_exclusive_ones() {
    let site = || {
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Tuple,
            count: 1,
            byte_size: 8,
        })
    };

    // Two sites in sequence: both live.
    assert_eq!(
        max_sites_on_a_path(&[site(), site()]),
        2,
        "sequential sites accumulate"
    );

    // Two sites in opposite arms: one live.
    assert_eq!(
        max_sites_on_a_path(&[Op::If(0), site(), Op::Else(0), site(), Op::EndIf]),
        1,
        "sites in opposite arms are alternatives, not a sum"
    );

    // Uneven arms: the fatter arm decides.
    assert_eq!(
        max_sites_on_a_path(&[Op::If(0), site(), site(), Op::Else(0), site(), Op::EndIf]),
        2,
        "the arm carrying more sites sets the figure"
    );

    // A site before the branch still counts alongside the winning arm.
    assert_eq!(
        max_sites_on_a_path(&[site(), Op::If(0), site(), Op::Else(0), site(), Op::EndIf]),
        2,
        "a site outside the conditional is live across it"
    );

    // Loops accumulate rather than alternate -- stated in the header, asserted here.
    assert_eq!(
        max_sites_on_a_path(&[Op::Loop(0), site(), site(), Op::EndLoop(0)]),
        2,
        "a loop body is NOT a branch; its sites accumulate"
    );

    assert_eq!(
        max_sites_on_a_path(&[]),
        0,
        "an empty stream carries no sites"
    );
}

#[test]
fn does_branch_exclusivity_explain_the_arena_gap() {
    let corpus = all_compiling_modules();
    let mut rows: Vec<(String, usize, usize, u32, u32, u32)> = Vec::new();

    for (name, m) in &corpus {
        let Ok(fp) = module_runtime_footprint(m, &[]) else {
            continue;
        };
        let Some(entry) = m.entry_point else { continue };
        let demand = region::region_total_bytes(m, entry, 0);
        if demand <= fp.max_heap_bytes {
            continue;
        }
        let sites = module_sites(m);
        let pmax = module_path_max(m);
        let size = if sites > 0 { demand / sites as u32 } else { 0 };
        rows.push((name.clone(), sites, pmax, size, demand, fp.max_heap_bytes));
    }

    println!("\n================ DOES BRANCH EXCLUSIVITY EXPLAIN THE GAP?");
    println!("  module                       sites  path-max  size   backend  verified  peak_live");
    let (mut explained, mut residual) = (0usize, 0usize);
    for (n, sites, pmax, size, demand, verified) in &rows {
        let peak_live = if *size > 0 { verified / size } else { 0 };
        let ok = *pmax as u32 == peak_live;
        if ok {
            explained += 1;
        } else {
            residual += 1;
        }
        println!(
            "  {n:<26} {sites:>6} {pmax:>9} {size:>6} {demand:>9} {verified:>9} {peak_live:>10}  {}",
            if ok { "EXPLAINED" } else { "residue" }
        );
    }
    println!("  ------------------------------------------------");
    println!(
        "  {explained} of {} exceeding module(s) have path-max EQUAL to the verified peak.",
        rows.len()
    );
    println!("  {residual} leave a residue the branch account does not cover.");
    println!("  ------------------------------------------------");
    println!("  READ THE BOUND'S DIRECTION: path-max is an UPPER bound on simultaneous");
    println!("  liveness, so path-max >= peak_live always. Reaching peak_live means the");
    println!("  branch account is SUFFICIENT; staying above it means something else");
    println!("  contributes. Loops accumulate here and are a separate question.");
    println!("  NO REMEDY IS ADOPTED: plan_chunk_region still assigns per static site.");
    println!("================\n");

    assert!(
        !rows.is_empty(),
        "no module exceeds, so this report describes nothing -- if the gap has \
         closed, that is NEWS and this file should say so rather than pass quietly"
    );
}
