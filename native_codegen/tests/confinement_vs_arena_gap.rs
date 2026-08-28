//! **Does the confinement analysis account for the arena-bound gap?**
//!
//! Two measured facts have sat beside each other without being joined.
//!
//! `bound_transfer` reports that some modules demand more arena from the backend
//! than their verified heap figure allows: `backend = sites * size`,
//! `verified = peak_live * size`, so `shortfall = (sites - peak_live) * size`.
//! That is recorded as Workstream E having no bound covering the backend's pool.
//!
//! `keleusma::confine` answers, per construction site, *is this region
//! unreachable once its enclosing scope ends?* -- `Confined`, `CannotEstablish`
//! or `Escapes`. **Confinement is exactly the property that would license reusing
//! a region's space.**
//!
//! So: **for the modules that exceed, are their sites confined?**
//!
//! # ⚠ WHAT THIS MEASUREMENT DOES NOT LICENSE
//!
//! **A count of confined sites is NOT an achievable demand, and no figure here is
//! a proposed bound.** Confinement says a region is dead after its scope ends,
//! which licenses reuse **across** scopes, not within one: two confined sites
//! live in the same scope still need separate space. **This bounds what reuse
//! COULD reach; it does not compute anything.**
//!
//! # Direction of the bounds, because they run opposite ways
//!
//! `Confined` is the analysis's *sound* answer -- every disqualifier is an UPPER
//! bound on escape -- so **the confined count is a LOWER bound** on how many
//! sites are genuinely reusable. `CannotEstablish` must be read as `Escapes` for
//! soundness and is reported separately only so that improvements to the analysis
//! are visible as a shift between the two.
//!
//! # THE VERDICT, MEASURED 2026-08-27: CONFINEMENT DOES NOT EXPLAIN THE GAP
//!
//! The naive comparison looks damning for confinement -- corpus-wide **43%** of
//! sites are confined, but across the eleven exceeding modules only **8%**. That
//! reading is **wrong**, and a 2x2 shows why.
//!
//! ```text
//!   cell                     modules   sites   confined   rate
//!   rogue_*, EXCEEDS              11      36          3    8%
//!   rogue_*, within bound          6      22          0    0%
//!   other,   EXCEEDS               0       0          0    n/a
//!   other,   within bound         12     195        105   53%
//! ```
//!
//! **Every exceeding module is a `rogue_*` source, and the `other, EXCEEDS` cell
//! is EMPTY.** So the corpus cannot separate "exceeding implies low confinement"
//! from "the rogue family is what exceeds" by that comparison alone.
//!
//! **But the within-family cells settle it in the opposite direction.** Rogue
//! modules that stay within their bound are **0%** confined -- LOWER than the
//! exceeding ones at 8%. **Within the only family that exceeds, the exceeding
//! members are MORE confined, not less.** The apparent effect is entirely family:
//! rogue 8%/0% against other 53%.
//!
//! **So region reuse driven by confinement would not close this gap.** The
//! modules that exceed are not the ones whose sites are unreusable; they are the
//! ones from a family whose sites escape regardless of whether it exceeds.
//!
//! ## The strength of that claim, stated rather than implied
//!
//! **The within-family comparison rests on THREE confined sites against zero.**
//! That is enough to refuse the naive causal reading and **not** enough to
//! establish a quantitative relationship. What is established is negative: the
//! data do not support confinement as the explanation, and one cell that would
//! have tested it does not exist in this corpus.
//!
//! # Nothing here can affect emitted code
//!
//! `plan_chunk_region` consumes **no** escape reasoning: every static site gets
//! its own offset. That is why the gap exists, and also why this measurement
//! carries no soundness risk -- nothing reads a confinement verdict.

use keleusma::bytecode::Module;
use keleusma::confine::{Confinement, module_confinement};
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

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
struct Tally {
    confined: usize,
    cannot: usize,
    escapes: usize,
}

impl Tally {
    fn total(&self) -> usize {
        self.confined + self.cannot + self.escapes
    }
    fn add(&mut self, o: Tally) {
        self.confined += o.confined;
        self.cannot += o.cannot;
        self.escapes += o.escapes;
    }
}

fn tally(m: &Module) -> Tally {
    let mut t = Tally::default();
    for chunk in module_confinement(m) {
        for v in chunk {
            match v.verdict {
                Confinement::Confined => t.confined += 1,
                Confinement::CannotEstablish => t.cannot += 1,
                Confinement::Escapes => t.escapes += 1,
            }
        }
    }
    t
}

/// **THE INSTRUMENT DISCRIMINATES, shown before its output is believed.**
///
/// Three filters broke in one session by matching everything or nothing, so a
/// tally that reports a distribution has to demonstrate it can report DIFFERENT
/// distributions. This finds two corpus modules whose tallies differ.
#[test]
fn the_tally_reports_different_answers_for_different_modules() {
    let corpus = all_compiling_modules();
    let mut seen: Vec<(String, Tally)> = Vec::new();
    for (n, m) in &corpus {
        let t = tally(m);
        if t.total() > 0 {
            seen.push((n.clone(), t));
        }
    }
    assert!(
        seen.len() >= 2,
        "fewer than two modules have any construction site, so this instrument \
         cannot be shown to discriminate over this corpus"
    );
    let distinct: std::collections::BTreeSet<_> = seen
        .iter()
        .map(|(_, t)| (t.confined, t.cannot, t.escapes))
        .collect();
    assert!(
        distinct.len() >= 2,
        "every module with sites reports the SAME tally {:?}. A distribution that \
         cannot vary is measuring the instrument, not the corpus.",
        seen.first().map(|(_, t)| *t)
    );
    // And the verdicts are not all one value, which would make the three-way
    // split decorative.
    let all: Tally = seen.iter().fold(Tally::default(), |mut a, (_, t)| {
        a.add(*t);
        a
    });
    assert!(
        [all.confined, all.cannot, all.escapes]
            .iter()
            .filter(|n| **n > 0)
            .count()
            >= 2,
        "every site corpus-wide lands in ONE verdict ({all:?}); the three-way split \
         asserts nothing on this population"
    );
}

#[test]
fn how_are_the_exceeding_modules_sites_confined() {
    let corpus = all_compiling_modules();

    let mut corpus_wide = Tally::default();
    let mut corpus_modules_with_sites = 0usize;
    let mut exceeding: Vec<(String, u32, u32, Tally)> = Vec::new();
    let mut compared = 0usize;

    for (name, m) in &corpus {
        let t = tally(m);
        if t.total() > 0 {
            corpus_wide.add(t);
            corpus_modules_with_sites += 1;
        }
        let Ok(fp) = module_runtime_footprint(m, &[]) else {
            continue;
        };
        let Some(entry) = m.entry_point else { continue };
        let demand = region::region_total_bytes(m, entry, 0);
        compared += 1;
        if demand > fp.max_heap_bytes {
            exceeding.push((name.clone(), demand, fp.max_heap_bytes, t));
        }
    }

    println!("\n================ CONFINEMENT vs THE ARENA-BOUND GAP");
    println!("  modules compared                    : {compared}");
    println!("  CORPUS-WIDE, over the {corpus_modules_with_sites} module(s) with any site:");
    println!(
        "    confined {} / cannot-establish {} / escapes {}  (total {})",
        corpus_wide.confined,
        corpus_wide.cannot,
        corpus_wide.escapes,
        corpus_wide.total()
    );
    println!("  ------------------------------------------------");
    println!(
        "  THE EXCEEDING MODULES ({}) -- A DIFFERENT POPULATION, reported apart:",
        exceeding.len()
    );
    let mut exc_tally = Tally::default();
    for (n, d, v, t) in &exceeding {
        exc_tally.add(*t);
        println!(
            "    {n:<26} backend {d} vs verified {v}  |  confined {} cannot {} escapes {}",
            t.confined, t.cannot, t.escapes
        );
    }
    println!(
        "    TOTALS: confined {} / cannot-establish {} / escapes {}  (total {})",
        exc_tally.confined,
        exc_tally.cannot,
        exc_tally.escapes,
        exc_tally.total()
    );
    println!("  ------------------------------------------------");
    println!("  WHAT THIS DOES NOT SAY:");
    println!("    A confined count is NOT an achievable demand and NOT a bound.");
    println!("    Confinement licenses reuse ACROSS scopes, not within one, so two");
    println!("    confined sites in the same scope still need separate space.");
    println!("    `confined` is the SOUND answer, so its count is a LOWER bound on");
    println!("    what is reusable; `cannot-establish` must be read as `escapes`.");
    println!("    Nothing in code generation reads any of this: `plan_chunk_region`");
    println!("    assigns per static site and consumes no escape reasoning.");
    println!("================\n");

    // **REPORTED, NOT PINNED.** The exceeding set moves with the corpus, so an
    // equality here would fail on ordinary growth and teach the next reader to
    // delete it. What is asserted is only that the join was actually performed
    // over a non-empty population -- without which the report above is a shape
    // with no content.
    assert!(
        compared > 0,
        "no module was compared, so this report describes nothing"
    );
    assert!(
        corpus_wide.total() > 0,
        "no construction site was found anywhere in the corpus, so the confinement \
         tally is vacuous"
    );
}

/// **IS THE EXCEEDING SET A PROPERTY OF EXCEEDING, OR OF ONE PROGRAM FAMILY?**
///
/// Every exceeding module is a `rogue_*` source. So "exceeding modules are less
/// confined than the corpus" may be **confounded**: it could equally read "the
/// rogue family is less confined, and the rogue family is what exceeds."
///
/// **Those lead different places.** The first says confinement is unrelated to
/// the gap in general; the second says one family drives the whole finding and a
/// second family might behave differently. This separates them by measuring the
/// rogue family's NON-exceeding members against its exceeding ones.
#[test]
fn is_the_low_confinement_a_property_of_exceeding_or_of_the_rogue_family() {
    let corpus = all_compiling_modules();
    let (mut rogue_exceed, mut rogue_ok, mut other_exceed, mut other_ok) = (
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    );
    let (mut n_re, mut n_ro, mut n_oe, mut n_oo) = (0usize, 0usize, 0usize, 0usize);

    for (name, m) in &corpus {
        let t = tally(m);
        if t.total() == 0 {
            continue;
        }
        let Ok(fp) = module_runtime_footprint(m, &[]) else {
            continue;
        };
        let Some(entry) = m.entry_point else { continue };
        let exceeds = region::region_total_bytes(m, entry, 0) > fp.max_heap_bytes;
        let rogue = name.starts_with("rogue_");
        match (rogue, exceeds) {
            (true, true) => {
                rogue_exceed.add(t);
                n_re += 1;
            }
            (true, false) => {
                rogue_ok.add(t);
                n_ro += 1;
            }
            (false, true) => {
                other_exceed.add(t);
                n_oe += 1;
            }
            (false, false) => {
                other_ok.add(t);
                n_oo += 1;
            }
        }
    }

    let pct = |t: Tally| {
        if t.total() == 0 {
            "n/a".to_string()
        } else {
            format!("{}%", t.confined * 100 / t.total())
        }
    };
    println!("\n================ IS IT EXCEEDING, OR IS IT THE FAMILY?");
    println!("  cell                     modules   sites   confined   rate");
    println!(
        "  rogue_*, EXCEEDS         {n_re:>7}   {:>5}   {:>8}   {}",
        rogue_exceed.total(),
        rogue_exceed.confined,
        pct(rogue_exceed)
    );
    println!(
        "  rogue_*, within bound    {n_ro:>7}   {:>5}   {:>8}   {}",
        rogue_ok.total(),
        rogue_ok.confined,
        pct(rogue_ok)
    );
    println!(
        "  other,   EXCEEDS         {n_oe:>7}   {:>5}   {:>8}   {}",
        other_exceed.total(),
        other_exceed.confined,
        pct(other_exceed)
    );
    println!(
        "  other,   within bound    {n_oo:>7}   {:>5}   {:>8}   {}",
        other_ok.total(),
        other_ok.confined,
        pct(other_ok)
    );
    println!("================\n");

    // **THE CELL THAT DECIDES IT IS `other, EXCEEDS`.** If it is empty, this
    // corpus CANNOT separate the two readings, and saying so is the result --
    // not a weaker version of the stronger claim.
    if n_oe == 0 {
        println!(
            "  VERDICT: the `other, EXCEEDS` cell is EMPTY, so this corpus cannot \n\
             separate \"exceeding implies low confinement\" from \"the rogue family \n\
             is what exceeds\". The confound is NOT resolved and the corpus-wide \n\
             comparison must not be read as though it were."
        );
    }
    assert!(
        n_re + n_ro + n_oe + n_oo > 0,
        "no module fell into any cell, so the table describes nothing"
    );
}

/// How common is the un-lowered composite form, and is it what the refused
/// modules use?
#[test]
fn how_common_is_the_boxed_composite_form() {
    use keleusma::bytecode::NewCompositeOperand as N;
    let corpus = all_compiling_modules();
    let (mut flat, mut boxed) = (0usize, 0usize);
    let mut boxed_modules: Vec<String> = Vec::new();
    for (name, m) in &corpus {
        let mut b = 0usize;
        for c in &m.chunks {
            for op in &c.ops {
                if let keleusma::bytecode::Op::NewComposite(v) = op {
                    match v {
                        N::Flat { .. } => flat += 1,
                        _ => {
                            boxed += 1;
                            b += 1;
                        }
                    }
                }
            }
        }
        if b > 0 {
            boxed_modules.push(format!("{name} ({b})"));
        }
    }
    println!("\n================ COMPOSITE FORMS CORPUS-WIDE");
    println!("  Flat  (lowered)      : {flat}");
    println!("  non-Flat (NOT lowered): {boxed}");
    println!("  modules using non-Flat: {:?}", boxed_modules);
    println!("================\n");
}

/// **WHICH refusal condition blocks the last composite modules?**
///
/// The `Flat` arm refuses for exactly three reasons: the region pointer is
/// absent, no region placement exists for the site, or **an operand has unknown
/// packed width**. Naming which one fires is the difference between a finding and
/// a count.
///
/// # The prediction being tested, which is NOT its own evidence
///
/// `spike_composite_split` argued the composite class is two blockers, not one:
/// every composite READ bakes what a lowering needs, and **only
/// `NewComposite::Flat` is short** — it carries the total body size, not the
/// per-field breakdown, so packing requires each operand's width. *"That, and
/// only that, is what type recovery buys."*
///
/// **If the observed cause is the width condition, the split is confirmed on the
/// residue it predicted.** If it is either of the other two, the prediction does
/// not explain what is left.
///
/// # A candidate ruled out by measurement first
///
/// The obvious guess was the `Boxed` form, which the backend does not lower.
/// **The corpus contains ZERO non-`Flat` composites** — all 256 sites are `Flat`
/// — so that guess was wrong before anything was built on it.
#[test]
fn which_condition_refuses_the_last_composite_modules() {
    let corpus = all_compiling_modules();
    let mut hits: Vec<(String, String)> = Vec::new();

    for (name, m) in &corpus {
        for (chunk, err) in
            keleusma_native::module_refusals(m, keleusma_native::LowerOptions::default())
        {
            let text = format!("{err:?}");
            if text.contains("NewComposite") {
                hits.push((format!("{name}::{chunk}"), text));
            }
        }
    }

    println!("\n================ WHICH CONDITION REFUSES THE COMPOSITE MODULES?");
    if hits.is_empty() {
        println!("  NONE. No module refuses on NewComposite any more.");
        println!("  That is NEWS: the recorded residue of two has closed, and the");
        println!("  coverage figures quoted elsewhere need re-deriving.");
    }
    for (where_, text) in &hits {
        // Name the condition, not merely the opcode.
        let cause = if text.contains("unknown packed width") {
            "UNKNOWN OPERAND WIDTH  <- the condition spike_composite_split predicted"
        } else if text.contains("needs the region pointer") {
            "REGION POINTER ABSENT  <- not the predicted condition"
        } else if text.contains("no region placement") {
            "NO REGION PLACEMENT    <- not the predicted condition"
        } else {
            "OTHER / not one of the three Flat conditions"
        };
        println!("  {where_}");
        println!("      cause: {cause}");
        println!("      text : {}", &text[..text.len().min(150)]);
    }
    println!("  ------------------------------------------------");
    let width = hits
        .iter()
        .filter(|(_, t)| t.contains("unknown packed width"))
        .count();
    println!(
        "  {} of {} composite refusals are the WIDTH condition.",
        width,
        hits.len()
    );
    if !hits.is_empty() && width == hits.len() {
        println!("  => spike_composite_split's split is CONFIRMED on the residue: what");
        println!("     remains is construction needing operand-width recovery, exactly");
        println!("     the half it said reads do not need.");
    }
    println!("  NOTHING WAS LOWERED HERE. Establishing the cause is the increment.");
    println!("================\n");

    // **NON-VACUITY: a cause read from no refusal is not a cause.** If this
    // number is zero the report above says so explicitly rather than passing
    // quietly, and the assertion below keeps that from being mistaken for a
    // clean result.
    assert!(
        !hits.is_empty(),
        "no module refuses on NewComposite, so no cause could be read. If the \
         residue has genuinely closed, the coverage figures elsewhere are stale \
         and this test should be re-pointed rather than deleted."
    );
}
