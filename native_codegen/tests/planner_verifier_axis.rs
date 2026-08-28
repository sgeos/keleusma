//! **Can a planner reproduce the verifier's liveness axis?**
//!
//! The max-over-arms remedy's third precondition. Two increments established
//! that a forty-line syntactic walk over `If`/`Else`/`EndIf` reproduces the
//! verifier's `peak_live` **exactly on 11 of 11** exceeding modules — direct
//! evidence the axes align.
//!
//! **But all 33 of those sites are bare conditionals.** The corpus's other 176
//! sit inside loops, where that walk **deliberately accumulates** rather than
//! treating iterations as alternatives. **The agreement had only been tested on
//! the easy half.**
//!
//! This file tests the other half.
//!
//! # The comparison is not symmetric, and the two directions mean different things
//!
//! `path-max` is an **upper bound** on simultaneous liveness. `peak_live` is
//! derived from the verifier's `max_heap_bytes` — a figure it is willing to
//! certify. So:
//!
//! - **`path-max > peak_live`** is expected wherever the walk is conservative:
//!   a modelling difference, not a defect.
//! - **`path-max < peak_live`** would mean the walk MISSES liveness the verifier
//!   sees. **That is a defect in the walk**, and it is reported apart.
//!
//! # THE VERDICT, MEASURED 2026-08-27: THE PRECONDITION IS **NOT** ESTABLISHED,
//! # AND AN EARLIER CLAIM OF MINE IS REFUTED
//!
//! ```text
//!   DERIVATION INVALID  : 7   quotient exceeds the site count -- not a count at all
//!   COMPARABLE          : 22
//!     AGREE             : 19
//!     OVER              : 0
//!     UNDER             : 3   09_big_numbers, 10_multbyte, fixed_arithmetic
//! ```
//!
//! ## The refuted claim
//!
//! `arena_gap_explanation` states, and this file's own header restates, that
//! **"`path-max` is an UPPER bound on simultaneous liveness, so `path-max >=
//! peak_live` holds by construction."**
//!
//! **THREE COMPARABLE MODULES VIOLATE IT.** `fixed_arithmetic` has 2 sites, no
//! loop, `path-max` 1 against an implied `peak_live` of 2. So either the walk
//! misses liveness, or the quantity being compared is not what it was labelled.
//!
//! ## The derivation is mine and it does not hold up
//!
//! `peak_live = max_heap_bytes / (demand / sites)` was introduced by
//! `arena_gap_explanation`. **`bound_transfer` never divides** — it compares the
//! two figures as an inequality and calls the difference a shortfall.
//!
//! `max_heap_bytes` is documented as **peak per-iteration arena heap OVER EVERY
//! CHUNK**. `region_total_bytes` is an **entry-rooted total across the call
//! tree**. **Those are different axes, and their ratio is not a population
//! count** — which is why on seven modules it exceeds the number of sites that
//! exist. `piano_roll_0` has 80 sites and an implied `peak_live` of 96.
//!
//! ## What survives, and what does not
//!
//! **SURVIVES**: the numeric fact that `path-max == max_heap / size` exactly on
//! all 11 exceeding modules. That is re-derivable and was not a fluke of one
//! module.
//!
//! **DOES NOT SURVIVE**: the interpretation of that quantity as "the verifier's
//! count of simultaneously live composites", and the claim that the inequality
//! holds by construction. **The equality is real; the label on one side of it was
//! not earned.**
//!
//! **NOT RESOLVED HERE**: whether the 11-of-11 agreement reflects branch
//! exclusivity or an unrelated alignment between a per-chunk peak and an
//! entry-rooted total. **Three modules contradict the walk and I cannot tell from
//! aggregates which mechanism is at work.** Naming a replacement mechanism from
//! these numbers would be the same over-reach a second time, so it is left open.
//!
//! ## So the third precondition is NOT met
//!
//! A planner cannot be said to reproduce the verifier's axis when the two
//! disagree on three of twenty-two comparable modules and the comparison itself
//! is undefined on seven more.
//!
//! # The walk is not tuned toward agreement
//!
//! It is the same classification `arena_gap_explanation` uses, unchanged.
//! Adjusting an instrument until it matches the thing it is compared against
//! destroys the comparison, so any divergence here stands as measured.

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

/// The same walk `arena_gap_explanation` uses, unchanged: at an `If` the arms are
/// alternatives, everything else accumulates, loops included.
fn max_sites_on_a_path(ops: &[Op]) -> usize {
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
    while let Some((before, best)) = stack.pop() {
        cur = before + best.max(cur);
    }
    cur
}

/// Sites in this module that sit inside a loop, by any nesting.
fn sites_in_loops(m: &Module) -> usize {
    let mut n = 0usize;
    for c in &m.chunks {
        let mut depth = 0usize;
        for op in &c.ops {
            match op {
                Op::Loop(_) => depth += 1,
                Op::EndLoop(_) => depth = depth.saturating_sub(1),
                o if is_site(o) && depth > 0 => n += 1,
                _ => {}
            }
        }
    }
    n
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

/// **THE WALK DISCRIMINATES**, shown before its output is believed — and it is
/// the SAME walk, so this also pins that it has not been tuned.
#[test]
fn the_walk_is_unchanged_and_still_separates_arms_from_sequence() {
    let site = || {
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Tuple,
            count: 1,
            byte_size: 8,
        })
    };
    assert_eq!(
        max_sites_on_a_path(&[site(), site()]),
        2,
        "sequence accumulates"
    );
    assert_eq!(
        max_sites_on_a_path(&[Op::If(0), site(), Op::Else(0), site(), Op::EndIf]),
        1,
        "arms are alternatives"
    );
    assert_eq!(
        max_sites_on_a_path(&[Op::Loop(0), site(), site(), Op::EndLoop(0)]),
        2,
        "a loop body accumulates -- deliberately, and this is what the corpus-wide \
         comparison is testing"
    );
}

#[test]
fn does_the_path_walk_reproduce_the_verifier_across_the_whole_corpus() {
    let corpus = all_compiling_modules();
    // (name, sites, in-loop sites, path_max, peak_live)
    let mut rows: Vec<(String, usize, usize, usize, u32)> = Vec::new();
    let mut no_sites = 0usize;

    for (name, m) in &corpus {
        let sites = module_sites(m);
        if sites == 0 {
            // **EXCLUDED, and stated.** `peak_live` is derived by dividing by the
            // site count; a module with none would yield a fake agreement at 0.
            no_sites += 1;
            continue;
        }
        let Ok(fp) = module_runtime_footprint(m, &[]) else {
            continue;
        };
        let Some(entry) = m.entry_point else { continue };
        let demand = region::region_total_bytes(m, entry, 0);
        if demand == 0 {
            continue;
        }
        let size = demand / sites as u32;
        if size == 0 {
            continue;
        }
        let peak_live = fp.max_heap_bytes / size;
        rows.push((
            name.clone(),
            sites,
            sites_in_loops(m),
            module_path_max(m),
            peak_live,
        ));
    }

    // **THE DERIVATION IS ONLY VALID WHERE IT PRODUCES A POSSIBLE COUNT.**
    // `peak_live = max_heap / (demand / sites)` treats the verifier's figure as a
    // whole number of same-sized composites. Where the quotient EXCEEDS the site
    // count, it cannot be a count of live sites -- there are not that many sites
    // -- so the derivation has failed and the row must not be compared.
    let mut invalid: Vec<&(String, usize, usize, usize, u32)> = Vec::new();
    let mut comparable: Vec<&(String, usize, usize, usize, u32)> = Vec::new();
    for r in &rows {
        if r.4 as usize > r.1 {
            invalid.push(r);
        } else {
            comparable.push(r);
        }
    }
    let rows: Vec<&(String, usize, usize, usize, u32)> = comparable;

    let mut agree: Vec<&(String, usize, usize, usize, u32)> = Vec::new();
    let mut over: Vec<&(String, usize, usize, usize, u32)> = Vec::new();
    let mut under: Vec<&(String, usize, usize, usize, u32)> = Vec::new();
    for r in &rows {
        match (r.3 as u32).cmp(&r.4) {
            std::cmp::Ordering::Equal => agree.push(r),
            std::cmp::Ordering::Greater => over.push(r),
            std::cmp::Ordering::Less => under.push(r),
        }
    }

    println!("\n================ DOES THE PATH WALK REPRODUCE THE VERIFIER?");
    println!(
        "  modules with a site and a non-zero demand : {}",
        rows.len() + invalid.len()
    );
    println!("  modules EXCLUDED for having no site        : {no_sites}");
    println!("  ------------------------------------------------");
    println!(
        "  ⚠ DERIVATION INVALID on {} module(s): `max_heap / size` exceeds the site",
        invalid.len()
    );
    println!("    count, so the quotient is not a count of live sites and the row cannot");
    println!("    be compared. THE DERIVATION IS THIS FILE'S, NOT THE VERIFIER'S --");
    println!("    `bound_transfer` compares the two figures as an INEQUALITY and never");
    println!("    divides. `max_heap_bytes` is documented as peak per-iteration arena");
    println!("    heap OVER EVERY CHUNK, which is not the entry-rooted total the backend");
    println!("    reports, so their ratio is not a population count.");
    for r in &invalid {
        println!(
            "    INVALID {:<22} {} site(s) but implied peak_live {}",
            r.0, r.1, r.4
        );
    }
    println!("  ------------------------------------------------");
    println!(
        "  COMPARABLE (quotient is a possible count) : {}",
        rows.len()
    );
    println!("  ------------------------------------------------");
    println!("  AGREE  (path-max == peak_live) : {}", agree.len());
    println!(
        "  OVER   (path-max >  peak_live) : {}  <- conservative walk, a modelling",
        over.len()
    );
    println!("                                       difference rather than a defect");
    println!(
        "  UNDER  (path-max <  peak_live) : {}  <- would be a DEFECT IN THE WALK",
        under.len()
    );
    println!("  ------------------------------------------------");
    println!("  module                       sites  in-loop  path-max  peak_live");
    for r in &over {
        println!(
            "  OVER  {:<22} {:>5} {:>8} {:>9} {:>10}",
            r.0, r.1, r.2, r.3, r.4
        );
    }
    for r in &under {
        println!(
            "  UNDER {:<22} {:>5} {:>8} {:>9} {:>10}",
            r.0, r.1, r.2, r.3, r.4
        );
    }
    let (over_loopy, over_flat): (Vec<_>, Vec<_>) = over.iter().copied().partition(|r| r.2 > 0);
    println!("  ------------------------------------------------");
    println!(
        "  OF THE {} OVER-MODULES: {} carry in-loop sites, {} do not.",
        over.len(),
        over_loopy.len(),
        over_flat.len()
    );
    println!("  Loop placement is the candidate explanation; a flat OVER-module is one");
    println!("  the loop account does NOT cover.");
    println!("  ------------------------------------------------");
    println!("  READ THE DIRECTIONS APART: path-max is an UPPER bound on simultaneous");
    println!("  liveness, so OVER is expected wherever the walk is conservative. UNDER");
    println!("  would mean the walk misses liveness the verifier sees, which is a fault");
    println!("  in the walk and not a finding about the planner.");
    println!("  NO REMEDY IS ADOPTED: plan_chunk_region still assigns per static site.");
    println!("================\n");

    assert!(
        !rows.is_empty() || !invalid.is_empty(),
        "no module qualified, so this report describes nothing"
    );
    assert_eq!(
        agree.len() + over.len() + under.len(),
        rows.len(),
        "every module must land in exactly one direction; a mismatch means rows \
         were dropped by the classification"
    );
}

/// Are the UNDER modules mixed-size? If so, `peak_live = max_heap / (demand /
/// sites)` divides by an AVERAGE and the quotient is not a count of anything.
#[test]
fn are_the_disagreeing_modules_mixed_size() {
    let corpus = all_compiling_modules();
    println!("\n================ SITE-SIZE UNIFORMITY");
    for (name, m) in &corpus {
        let mut sizes: Vec<u16> = Vec::new();
        for c in &m.chunks {
            for op in &c.ops {
                if let Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) = op {
                    sizes.push(*byte_size);
                }
            }
        }
        if sizes.is_empty() {
            continue;
        }
        let uniform = sizes.windows(2).all(|w| w[0] == w[1]);
        let mut d: Vec<u16> = sizes.clone();
        d.sort_unstable();
        d.dedup();
        if !uniform {
            println!(
                "  MIXED   {name:<26} {} site(s), sizes {:?}",
                sizes.len(),
                d
            );
        }
    }
    println!("================\n");
}

/// Do the impossible modules allocate through a NON-`Flat` composite form?
///
/// `peak_live = max_heap / (demand / flat_sites)` divides by a denominator that
/// counts ONLY `Flat` sites. Any other allocating form makes that denominator
/// wrong.
#[test]
fn what_composite_forms_do_the_impossible_modules_use() {
    let corpus = all_compiling_modules();
    println!("\n================ COMPOSITE FORMS PER MODULE");
    for want in [
        "rogue_dungen.kel",
        "14_frame_log.kel",
        "09_big_numbers.kel",
        "fixed_arithmetic.kel",
        "10_multbyte.kel",
        "rogue_combat.kel",
    ] {
        let Some((name, m)) = corpus.iter().find(|(n, _)| n == want) else {
            continue;
        };
        let (mut flat, mut other) = (0usize, 0usize);
        for c in &m.chunks {
            for op in &c.ops {
                if let Op::NewComposite(v) = op {
                    if matches!(v, NewCompositeOperand::Flat { .. }) {
                        flat += 1;
                    } else {
                        other += 1;
                    }
                }
            }
        }
        println!("  {name:<26} Flat {flat:>3}   NON-Flat {other:>3}");
    }
    println!("================\n");
}
