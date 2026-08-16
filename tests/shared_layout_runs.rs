//! `SHARED_LAYOUT` is run-compressible, measured rather than assumed.
//!
//! ## Why this test exists
//!
//! `SharedSlotRecord` is ONE word today, and a run-length record needs
//! `first_slot` (for binary search on the `get_shared`/`set_shared` hot path)
//! alongside `run` and `stride`, which makes it TWO words. **So run-length
//! encoding this table is a PESSIMISATION unless the mean run exceeds two**,
//! and the plan that proposed it quantified the expected saving without ever
//! measuring the distribution the saving depends on.
//!
//! Measured 2026-08-13 across all eleven stage sources: 643,276 slots collapse
//! to 18 runs, a mean of 35,737 against a break-even of 2. The table goes from
//! 5,146,208 bytes to 400. The concern was worth checking and is refuted by a
//! wide margin.
//!
//! ## What it guards
//!
//! The encoding's payoff is a property of how the stages DECLARE shared data,
//! not a property of the encoder. A future stage that declared many small
//! distinct shared slots instead of a few large arrays would fragment the runs
//! and silently turn the encoding into a size regression that nothing else
//! would report. This test fails first.
//!
//! ## Vacuity
//!
//! A run detector that never groups anything reports "all runs are 1", and a
//! detector that groups everything reports one enormous run. Neither is
//! distinguishable from a correct one by the headline number alone, so both
//! directions are pinned by an encoded control below rather than by inspection.

// This test compiles stage sources, so it needs the `compile` feature. Without
// the gate the file fails to build under `--no-default-features`, where
// `lexer`, `parser` and `compiler` are absent -- which is exactly how CI caught
// it. Same gate `tests/wire_corpus.rs` and its siblings carry.
#![cfg(feature = "compile")]

use keleusma::bytecode::{Module, SharedSlotLayout};

const STAGES: &[(&str, &str)] = &[
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("../src/selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_typed",
        include_str!("../src/selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_yield",
        include_str!("../src/selfhost/kel/verify_yield.kel"),
    ),
    (
        "verify_depth",
        include_str!("../src/selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
    // The wire emitter itself, which is the largest artifact this line builds.
    ("wire", include_str!("../src/selfhost/kel/wire.kel")),
];

fn compile_stage(src: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    keleusma::compiler::compile(&program).expect("compile")
}

/// Greedily group consecutive slots into maximal runs.
///
/// A run is consecutive entries sharing `kind` and `len` whose `offset`
/// advances by a CONSTANT stride. That is exactly the shape an array of
/// uniform elements produces, and it is the only shape a `(first_slot, run,
/// stride)` record can reproduce.
fn runs_of(slots: &[SharedSlotLayout]) -> Vec<usize> {
    let mut runs = Vec::new();
    let mut i = 0usize;
    while i < slots.len() {
        let a = &slots[i];
        // A run needs at least two entries to establish a stride.
        if i + 1 >= slots.len() {
            runs.push(1);
            break;
        }
        let b = &slots[i + 1];
        if b.kind != a.kind || b.len != a.len {
            runs.push(1);
            i += 1;
            continue;
        }
        let stride = b.offset.wrapping_sub(a.offset);
        let mut n = 2usize;
        while i + n < slots.len() {
            let c = &slots[i + n];
            let prev = &slots[i + n - 1];
            if c.kind != a.kind || c.len != a.len || c.offset.wrapping_sub(prev.offset) != stride {
                break;
            }
            n += 1;
        }
        runs.push(n);
        i += n;
    }
    runs
}

#[test]
fn measure_shared_layout_run_distribution() {
    println!(
        "\n{:<20} {:>8} {:>8} {:>9} {:>7} {:>11} {:>11} {:>9}",
        "stage", "slots", "runs", "mean_run", "max_run", "now_bytes", "rle_bytes", "delta"
    );
    println!("{}", "-".repeat(92));

    let mut tot_slots = 0usize;
    let mut tot_runs = 0usize;
    let mut tot_now = 0usize;
    let mut tot_rle = 0usize;

    for (name, src) in STAGES {
        let m = compile_stage(src);
        let empty: Vec<SharedSlotLayout> = Vec::new();
        let slots: &Vec<SharedSlotLayout> = m
            .data_layout
            .as_ref()
            .map(|d| &d.shared_layout)
            .unwrap_or(&empty);
        if slots.is_empty() {
            println!(
                "{name:<20} {:>8} {:>8} {:>9} {:>7} {:>11} {:>11} {:>9}",
                0, 0, "-", 0, 0, 0, 0
            );
            continue;
        }
        let runs = runs_of(slots);
        let mean = slots.len() as f64 / runs.len() as f64;
        let max = runs.iter().copied().max().unwrap_or(0);

        // Current encoding: one 8-byte record per LOGICAL slot.
        let now = slots.len() * 8;
        // Proposed: one 16-byte record per RUN -- but `run` is a u16, so a
        // run longer than 65,535 chunks into several records, exactly as
        // DATA_SLOTS already does. Counting logical runs would understate
        // the record count by 7x on `lexer`.
        let records: usize = runs.iter().map(|&n| n.div_ceil(65_535)).sum();
        let rle = records * 16;

        tot_slots += slots.len();
        tot_runs += runs.len();
        tot_now += now;
        tot_rle += rle;

        println!(
            "{name:<20} {:>8} {:>8} {:>9.2} {:>7} {:>11} {:>11} {:>+8.1}%",
            slots.len(),
            runs.len(),
            mean,
            max,
            now,
            rle,
            (rle as f64 - now as f64) / now as f64 * 100.0
        );
    }

    println!("{}", "-".repeat(92));
    println!(
        "{:<20} {:>8} {:>8} {:>9.2} {:>7} {:>11} {:>11} {:>+8.1}%",
        "TOTAL",
        tot_slots,
        tot_runs,
        tot_slots as f64 / tot_runs.max(1) as f64,
        "",
        tot_now,
        tot_rle,
        (tot_rle as f64 - tot_now as f64) / tot_now.max(1) as f64 * 100.0
    );

    // The break-even point, stated so the number is not left to the reader.
    let mean_run = tot_slots as f64 / tot_runs.max(1) as f64;
    println!(
        "\nBreak-even mean run is 2.00 (16-byte run record vs 8-byte slot record).\nMeasured overall mean run: {mean_run:.2}"
    );

    // THE GUARD. A two-word run record must beat a one-word slot record, and
    // it only does so above a mean run of two. Asserted with headroom rather
    // than at the break-even point: landing at 2.5 would technically pass
    // while meaning the encoding had stopped being worth its complexity.
    assert!(
        mean_run > 8.0,
        "SHARED_LAYOUT runs have fragmented to a mean of {mean_run:.2}. The \
         run-length encoding costs a 16-byte record per run against an 8-byte \
         record per slot, so it stops paying below a mean of 2 and stops being \
         worth its complexity well before that. Either a stage now declares \
         many small distinct shared slots instead of a few large arrays, or \
         the record shape changed. Re-derive the trade before assuming the \
         encoding is still correct."
    );

    // The encoded table must actually be smaller, which is the property the
    // encoding exists for and is not implied by the mean alone.
    assert!(
        tot_rle < tot_now,
        "the run-length encoding produced {tot_rle} bytes against {tot_now} \
         unencoded; it is a size regression"
    );

    // CONTROL: the run detector must be able to find a long run when one
    // exists. Without this, an all-ones result is indistinguishable from a
    // detector that never groups anything.
    let synthetic: Vec<SharedSlotLayout> = (0..100)
        .map(|k| SharedSlotLayout {
            offset: 64 + k * 8,
            kind: 0,
            len: 0,
        })
        .collect();
    let ctrl = runs_of(&synthetic);
    assert_eq!(
        ctrl,
        vec![100],
        "MUST-FIRE CONTROL FAILED: the detector could not group 100 uniform \
         stride-8 slots into one run, so every measurement above is vacuous"
    );

    // MUST-NOT-FIRE CONTROL: a non-uniform stride must NOT be grouped.
    let broken: Vec<SharedSlotLayout> = [0u32, 8, 24, 32]
        .iter()
        .map(|&o| SharedSlotLayout {
            offset: o,
            kind: 0,
            len: 0,
        })
        .collect();
    assert_eq!(
        runs_of(&broken),
        vec![2, 2],
        "MUST-NOT-FIRE CONTROL FAILED: the detector grouped a non-constant stride"
    );

    // CONTROL ON THE GUARD ITSELF, not on the detector.
    //
    // The two controls above establish that `runs_of` measures what it claims.
    // Neither establishes that the ASSERTION above can ever fail. Measured at
    // 35,737 against a threshold of 8, the guard passes by four orders of
    // magnitude, which is exactly the shape of a check that has quietly stopped
    // being able to report anything.
    //
    // So: build the fragmented layout the guard exists to catch -- distinct
    // kinds, no two adjacent slots groupable -- and assert the guard's own
    // predicate rejects it.
    let fragmented: Vec<SharedSlotLayout> = (0..64)
        .map(|k| SharedSlotLayout {
            offset: k * 8,
            // Alternating kinds defeat grouping without needing a broken stride.
            kind: (k % 2) as u8,
            len: 0,
        })
        .collect();
    let frag_runs = runs_of(&fragmented);
    let frag_mean = fragmented.len() as f64 / frag_runs.len() as f64;
    assert!(
        frag_mean <= 8.0,
        "GUARD CONTROL FAILED: a fully fragmented layout measured a mean run of \
         {frag_mean:.2}, which would PASS the guard above. The guard therefore \
         cannot report the regression it was written to catch."
    );
    // And the encoding really would be a pessimisation on that layout, which is
    // the substantive claim the threshold stands in for.
    let frag_now = fragmented.len() * 8;
    let frag_rle = frag_runs.iter().map(|&n| n.div_ceil(65_535)).sum::<usize>() * 16;
    assert!(
        frag_rle > frag_now,
        "GUARD CONTROL FAILED: the fragmented layout encodes to {frag_rle} bytes \
         against {frag_now} unencoded, so run-length encoding would not actually \
         be a regression there and the threshold is guarding the wrong property"
    );
}
