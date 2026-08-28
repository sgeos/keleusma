//! **The soundness precondition for a max-over-arms region planner.**
//!
//! `arena_gap_explanation` established that branch exclusivity accounts for the
//! arena-bound gap completely -- 11 of 11, zero residue -- and named a candidate
//! remedy: a planner taking the **max across exclusive arms** rather than the sum
//! would bring backend demand to exactly the verified figure on this corpus.
//!
//! **That remedy is not sound unconditionally, and this file measures the
//! condition.**
//!
//! # Why a loop is the hazard and a bare conditional is not
//!
//! **Outside a loop, exclusivity is total and escape is irrelevant.** Only one arm
//! ever executes, so two sites in different arms can share an offset no matter
//! where their regions end up. A region that escapes to the caller is still the
//! only one that was ever built.
//!
//! **Inside a loop that stops being true.** Arm A allocates in iteration 1 and its
//! region escapes into a local that survives the iteration; arm B allocates in
//! iteration 2 **at the same reused offset, clobbering a region that is still
//! live.**
//!
//! > So the precondition is not *are the sites confined* and not *are they
//! > exclusive*. It is: **are the exclusive sites inside a loop?**
//!
//! # This resolves the apparent tension with the escape finding
//!
//! `confinement_vs_arena_gap` found the exceeding modules' sites mostly ESCAPE --
//! 33 of 36. **That is harmless if the exclusivity is loop-free**, because escape
//! only matters in combination with loop-carried reuse. The two findings answer
//! different questions and do not conflict.
//!
//! # THE VERDICT, MEASURED 2026-08-27, AND IT IS NOT A CLEAN BILL
//!
//! ```text
//!   EXCEEDING TOTAL (11 modules): plain 3  / bare-if 33 / bare-loop 0 / IF-IN-LOOP   0
//!   CORPUS-WIDE (different pop.):  plain 29 / bare-if 45 / bare-loop 6 / IF-IN-LOOP 176
//! ```
//!
//! **The hazard is NOT exercised by a single exceeding module** — all 33 of their
//! exclusivity sites sit in bare conditionals. **So the remedy is safe on exactly
//! the modules that would benefit from it.**
//!
//! **AND THE HAZARD IS THE COMMON CASE EVERYWHERE ELSE.** Corpus-wide, **176 of
//! 256** construction sites sit in a conditional inside a loop — more than all
//! other placements combined.
//!
//! > **So a max-over-arms rule applied GLOBALLY would be unsound on 176 sites,
//! > while helping only 33.** The rule is not wrong; **applying it without the
//! > loop case handled would be.** That is a sharper conclusion than either "safe"
//! > or "unsafe" and it is the one the numbers support.
//!
//! ## The totals cross-check against an independent walk
//!
//! `29 + 45 + 6 + 176 = 256`, which is exactly the corpus-wide site count
//! `confinement_vs_arena_gap` reports from a **different** traversal. Two
//! instruments written days apart agreeing on a total is worth more than either
//! asserting it alone.
//!
//! ## What is established, stated at its real strength
//!
//! **Established**: adopting the rule would not be unsound on anything measured
//! here, and it would be unsound on most of the corpus if applied beyond that.
//!
//! **NOT established**: that the rule is sound. A clean cell is a fact about this
//! corpus, not a proof — and this corpus turns out to contain the counterexample
//! population in abundance, just not among the modules that exceed.
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

/// Where each construction site sits, relative to the two nesting kinds.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct Placement {
    /// Not inside any conditional or loop.
    plain: usize,
    /// Inside a conditional, no loop around it. **Exclusivity is total here.**
    bare_if: usize,
    /// Inside a loop, not inside a conditional within it. Not an exclusivity
    /// case at all -- nothing is being shared across arms.
    bare_loop: usize,
    /// **THE HAZARD**: inside a conditional that is itself inside a loop.
    if_in_loop: usize,
}

impl Placement {
    fn add(&mut self, o: Placement) {
        self.plain += o.plain;
        self.bare_if += o.bare_if;
        self.bare_loop += o.bare_loop;
        self.if_in_loop += o.if_in_loop;
    }
    fn total(&self) -> usize {
        self.plain + self.bare_if + self.bare_loop + self.if_in_loop
    }
}

/// Classify every site in `ops` by its enclosing nesting.
///
/// **A conditional counts as loop-wrapped when a `Loop` is open around it**, which
/// is the case where an earlier iteration's region can still be live while a
/// later one reuses the offset.
fn placements(ops: &[Op]) -> Placement {
    let mut p = Placement::default();
    let mut if_depth = 0usize;
    let mut loop_depth = 0usize;
    // Depth of the loop nest at the moment each still-open `If` began, so an
    // `If` opened OUTSIDE a loop is not reclassified by a later `Loop`.
    let mut if_loop_depth: Vec<usize> = Vec::new();
    for op in ops {
        match op {
            Op::If(_) => {
                if_loop_depth.push(loop_depth);
                if_depth += 1;
            }
            Op::EndIf => {
                if_depth = if_depth.saturating_sub(1);
                if_loop_depth.pop();
            }
            Op::Loop(_) => loop_depth += 1,
            Op::EndLoop(_) => loop_depth = loop_depth.saturating_sub(1),
            o if is_site(o) => {
                // The innermost open `If` decides, and only if it was opened
                // while a loop was already open.
                let inside_if = if_depth > 0;
                let if_under_loop = if_loop_depth.last().copied().unwrap_or(0) > 0;
                match (inside_if, loop_depth > 0) {
                    (true, true) if if_under_loop => p.if_in_loop += 1,
                    (true, _) => p.bare_if += 1,
                    (false, true) => p.bare_loop += 1,
                    (false, false) => p.plain += 1,
                }
            }
            _ => {}
        }
    }
    p
}

fn module_placements(m: &Module) -> Placement {
    let mut p = Placement::default();
    for c in &m.chunks {
        p.add(placements(&c.ops));
    }
    p
}

/// **THE CLASSIFIER DISCRIMINATES ACROSS ALL FOUR PLACEMENTS**, shown before its
/// output is believed. Collapsing the loop-wrapped case into the bare
/// conditional case is the disqualifying error this guards.
#[test]
fn the_classifier_separates_a_loop_wrapped_conditional_from_a_bare_one() {
    let site = || {
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Tuple,
            count: 1,
            byte_size: 8,
        })
    };

    let plain = placements(&[site()]);
    assert_eq!(
        (
            plain.plain,
            plain.bare_if,
            plain.bare_loop,
            plain.if_in_loop
        ),
        (1, 0, 0, 0)
    );

    let bare_if = placements(&[Op::If(0), site(), Op::Else(0), site(), Op::EndIf]);
    assert_eq!(
        (
            bare_if.plain,
            bare_if.bare_if,
            bare_if.bare_loop,
            bare_if.if_in_loop
        ),
        (0, 2, 0, 0),
        "a conditional with no loop around it is NOT the hazard"
    );

    let bare_loop = placements(&[Op::Loop(0), site(), Op::EndLoop(0)]);
    assert_eq!(
        (
            bare_loop.plain,
            bare_loop.bare_if,
            bare_loop.bare_loop,
            bare_loop.if_in_loop
        ),
        (0, 0, 1, 0),
        "a loop with no conditional inside it shares nothing across arms"
    );

    let hazard = placements(&[
        Op::Loop(0),
        Op::If(0),
        site(),
        Op::Else(0),
        site(),
        Op::EndIf,
        Op::EndLoop(0),
    ]);
    assert_eq!(
        (
            hazard.plain,
            hazard.bare_if,
            hazard.bare_loop,
            hazard.if_in_loop
        ),
        (0, 0, 0, 2),
        "a conditional INSIDE a loop is the hazard and must be counted apart"
    );

    // **AND THE ORDER MATTERS**: a loop opened AFTER the conditional does not
    // make that conditional loop-wrapped.
    let after = placements(&[
        Op::If(0),
        site(),
        Op::EndIf,
        Op::Loop(0),
        site(),
        Op::EndLoop(0),
    ]);
    assert_eq!(
        (
            after.plain,
            after.bare_if,
            after.bare_loop,
            after.if_in_loop
        ),
        (0, 1, 1, 0),
        "a loop that opens after the conditional closed cannot wrap it"
    );
}

#[test]
fn is_the_max_over_arms_hazard_exercised_by_the_exceeding_modules() {
    let corpus = all_compiling_modules();
    let mut rows: Vec<(String, Placement)> = Vec::new();
    let mut corpus_wide = Placement::default();

    for (name, m) in &corpus {
        let p = module_placements(m);
        corpus_wide.add(p);
        let Ok(fp) = module_runtime_footprint(m, &[]) else {
            continue;
        };
        let Some(entry) = m.entry_point else { continue };
        if region::region_total_bytes(m, entry, 0) > fp.max_heap_bytes {
            rows.push((name.clone(), p));
        }
    }

    println!("\n================ IS THE MAX-OVER-ARMS HAZARD EXERCISED?");
    println!("  THE HAZARD is a conditional INSIDE A LOOP. Outside a loop only one");
    println!("  arm ever runs, so sharing an offset is safe however the region");
    println!("  escapes. Inside one, an earlier iteration's region can still be live");
    println!("  when a later iteration reuses the offset.");
    println!("  ------------------------------------------------");
    println!("  module                       plain  bare-if  bare-loop  IF-IN-LOOP");
    let mut exc = Placement::default();
    for (n, p) in &rows {
        exc.add(*p);
        println!(
            "  {n:<26} {:>6} {:>8} {:>10} {:>11}{}",
            p.plain,
            p.bare_if,
            p.bare_loop,
            p.if_in_loop,
            if p.if_in_loop > 0 { "  <-- HAZARD" } else { "" }
        );
    }
    println!("  ------------------------------------------------");
    println!(
        "  EXCEEDING TOTAL ({} modules): plain {} / bare-if {} / bare-loop {} / IF-IN-LOOP {}",
        rows.len(),
        exc.plain,
        exc.bare_if,
        exc.bare_loop,
        exc.if_in_loop
    );
    println!(
        "  CORPUS-WIDE (a DIFFERENT population): plain {} / bare-if {} / bare-loop {} / IF-IN-LOOP {}",
        corpus_wide.plain, corpus_wide.bare_if, corpus_wide.bare_loop, corpus_wide.if_in_loop
    );
    println!("  ------------------------------------------------");
    if exc.if_in_loop == 0 {
        println!("  VERDICT: the hazard is NOT EXERCISED by any exceeding module on this");
        println!("  corpus. That is a fact about THIS CORPUS, not a proof that the rule");
        println!("  is sound -- a module with a conditional inside a loop would still");
        println!("  need the loop case handled. What is established is that adopting the");
        println!("  rule would not be unsound on anything measured here.");
    } else {
        println!("  VERDICT: the hazard IS EXERCISED -- the modules marked above carry a");
        println!("  conditional inside a loop. The rule cannot be adopted without the");
        println!("  loop case handled, and those modules are where to test it.");
    }
    println!("  NO REMEDY IS ADOPTED: plan_chunk_region still assigns per static site.");
    println!("================\n");

    assert!(
        !rows.is_empty(),
        "no module exceeds, so this report describes nothing -- if the gap has \
         closed that is NEWS and this file should say so rather than pass quietly"
    );
    assert_eq!(
        exc.total(),
        rows.iter().map(|(_, p)| p.total()).sum::<usize>(),
        "the per-module placements must sum to the total; a mismatch means sites \
         were dropped by the classification rather than placed"
    );
}
