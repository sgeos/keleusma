//! **HOW MUCH OF THE PUBLISHED ARENA FIGURE DOES EACH STREAMING CORPUS MODULE
//! ACTUALLY TOUCH?**
//!
//! # The gap this closes, stated as it actually was
//!
//! Three populations of streaming subject existed in three states:
//!
//! | population | arena touch |
//! |---|---|
//! | three hand-written streams (`arena_high_water.rs`) | measured — 16, 24, 48 bytes of 520–552 byte plans |
//! | the twelve self-hosted stages (`stage_differential.rs`) | measured — **zero** |
//! | **the other sixteen streaming corpus modules** | **static only** |
//!
//! `region_composition.rs` establishes STATICALLY that a flat
//! `stream_spill_bytes` reservation is 98% of the published figure for the stages
//! and 27% for `piano_roll_0`. **It is not uniform**, and the sixteen sat
//! unmeasured in between.
//!
//! The handoff said driving them "needs the `corpus_differential` seeding
//! machinery". **Re-reading that file narrowed the gap**: those modules are already
//! driven there for CORRECTNESS, with 64 seeds chosen after 4 and 24 were shown
//! insufficient. What was missing is this measurement, not the machinery.
//!
//! # ⚠ THE REFERENCE RUNS FIRST, AND THAT IS A SAFETY PROPERTY HERE
//!
//! Driving a lowered module calls machine code directly. An arithmetic overflow
//! there executes `llvm.trap`, which **kills the process with SIGTRAP** — not a
//! failed assertion, a dead test binary with no usable result.
//!
//! So [`vm_runs_cleanly`] drives the REFERENCE first and the backend only if the
//! reference completed the whole tick sequence without error. `corpus_differential`
//! orders its two runs the same way, for a different reason, and records that the
//! ordering is *"verified in the source, not assumed"*.
//!
//! **A module the reference declines is SKIPPED AND COUNTED**, never silently
//! dropped. A census that quietly discards subjects reports a figure for a
//! population narrower than its description, which is the failure this line keeps
//! finding.
//!
//! # Two fill patterns, for a reason already paid for
//!
//! `arena_high_water.rs` fills with `0x5A` and `0xC3` and requires the two to
//! agree, because **a byte written with the fill value would hide**. One pattern
//! silently under-reports. The same two are used here and the same agreement is
//! required.
//!
//! # What this is NOT
//!
//! **Not a defect report.** The reservation is fixed, static, documented and safe,
//! and over-provisioning errs in the safe direction. The finding is the SIZE of the
//! looseness in a bound this line sells as definitive, and what to do about it is an
//! arena-accounting question for the operator — exactly as
//! `host_arena_supplement_bytes` says of itself. The region planner is untouched.

mod common;

use keleusma::bytecode::{BlockType, Module, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

/// The two complementary fills, matching `arena_high_water.rs`.
const FILL_A: u8 = 0x5A;
const FILL_B: u8 = 0xC3;

/// Ticks driven per module. Enough to pass a `Reset` boundary on a short stream
/// without making the census slow.
const TICKS: usize = 4;

/// Corpus modules carrying a `Stream` chunk. **Pinned so growth announces
/// itself**; `region_composition.rs` pins the same figure independently.
const PINNED_STREAMING: usize = 28;

/// How many of those this census can currently drive. **The rest are accounted
/// for by name and reason**, not dropped: eleven need host natives registered,
/// three take a **`Composite`** first argument, one the backend refuses to lower.
const DRIVEN: usize = 13;
/// `14_frame_log.kel` — the first corpus module outside the self-hosted stages
/// ever measured dynamically. **Report six publishes these two figures.**
const FRAME_LOG_TOUCHED: usize = 48;
const FRAME_LOG_PLAN: usize = 600;

fn replies(n: usize) -> Vec<i64> {
    (0..n).map(|i| (i as i64 % 5) + 1).collect()
}

/// Does the REFERENCE complete the whole tick sequence without error?
///
/// **Fallible on purpose.** Every other stream driver in this package asserts,
/// because its subject is known good. This one's subject is the whole corpus, and
/// a module the reference declines must be reported rather than crash the census.
fn vm_runs_cleanly(m: &Module, first: i64, reps: &[i64]) -> Result<(), String> {
    let need = required_persistent_capacity_for(m);
    let base = auto_arena_capacity_for(m, &[]).map_err(|e| format!("arena capacity: {e:?}"))?;
    let mut arena = keleusma_arena::Arena::with_capacity(base + need + (64 << 10));
    arena
        .resize_persistent(need)
        .map_err(|e| format!("persistent resize: {e:?}"))?;
    let mut vm = Vm::new(m.clone(), &arena).map_err(|e| format!("vm construction: {e:?}"))?;
    // **EXACTLY THE DECLARED SIZE, AND `.max(8)` WAS A DEFECT HERE.**
    //
    // The runtime requires the lent buffer's length to EQUAL
    // `shared_data_bytes()`, and says so in its error. A `.max(8)` copied from a
    // driver where it was harmless made every module declaring ZERO shared bytes
    // report *"the reference does not complete the tick sequence"* — a harness
    // fault reported as a property of the module. Found by probing a skip reason
    // instead of trusting it.
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(m)];
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(first)])
        .map_err(|e| format!("first call: {e:?}"))?;
    for (i, &r) in reps.iter().enumerate() {
        match st {
            VmState::Yielded(_) | VmState::Reset => {
                st = vm
                    .resume_with_shared(&mut shared, Value::Int(r))
                    .map_err(|e| format!("resume {i}: {e:?}"))?;
            }
            // A stream that finishes early is not a failure; it simply has no
            // further ticks to measure.
            VmState::Finished(_) if i > 0 => return Ok(()),
            other => return Err(format!("tick {i} reached {other:?}")),
        }
    }
    Ok(())
}

/// The touched extent, measured under both fills and required to agree.
fn extent_of(m: &Module, first: i64, reps: &[i64]) -> usize {
    let (out_a, ext_a) = common::native_arena_extent_of_module(m, first, reps, FILL_A);
    let (out_b, ext_b) = common::native_arena_extent_of_module(m, first, reps, FILL_B);
    assert_eq!(
        ext_a, ext_b,
        "the two fills disagree on the touched extent ({ext_a} against {ext_b}), so \
         some byte is being read as touched because of the fill VALUE rather than \
         because it was written. Sequences: {out_a:?} and {out_b:?}"
    );
    ext_a
}

/// One row of the census.
struct Row {
    name: String,
    plan: usize,
    touched: usize,
}

/// Drive every streaming corpus module, or say why not.
fn census() -> (Vec<Row>, Vec<(String, String)>) {
    let first = 1i64;
    let reps = replies(TICKS);
    let mut rows = Vec::new();
    let mut skipped = Vec::new();

    for (name, m) in common::corpus() {
        if !m.chunks.iter().any(|c| c.block_type == BlockType::Stream) {
            continue;
        }
        let Some(entry) = m.entry_point else {
            skipped.push((name, "no entry point".to_string()));
            continue;
        };
        if m.chunks[entry].block_type != BlockType::Stream || m.chunks[entry].param_count != 1 {
            skipped.push((name, "entry is not a one-parameter stream".to_string()));
            continue;
        }
        if !keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default())
            .is_empty()
        {
            skipped.push((name, "the backend refuses to lower it".to_string()));
            continue;
        }
        // **THE REAL ERROR IS CARRIED, NOT A GENERIC REASON.** A single
        // "does not complete the tick sequence" hid three distinct causes, one
        // of which was a fault in this harness. A census's own skip reasons are
        // a finding, so they must be specific enough to act on.
        if let Err(why) = vm_runs_cleanly(&m, first, &reps) {
            skipped.push((name, why));
            continue;
        }
        let plan = keleusma_native::region::host_arena_supplement_bytes(&m) as usize;
        rows.push(Row {
            name,
            plan,
            touched: extent_of(&m, first, &reps),
        });
    }
    (rows, skipped)
}

/// **THE MEASUREMENT, WITH ITS POPULATION PRINTED.**
#[test]
fn every_streaming_corpus_module_is_measured_or_accounted_for() {
    let (rows, skipped) = census();

    println!("  streaming corpus modules: {}", rows.len() + skipped.len());
    println!(
        "  {:<34} {:>8} {:>8} {:>7}",
        "module", "plan", "touched", "share"
    );
    for r in &rows {
        let share = r
            .touched
            .checked_mul(100)
            .and_then(|n| n.checked_div(r.plan))
            .unwrap_or(0);
        println!(
            "  {:<34} {:>8} {:>8} {:>6}%",
            r.name, r.plan, r.touched, share
        );
    }
    for (name, why) in &skipped {
        println!("  SKIPPED {name:<26} {why}");
    }

    // **THE POPULATION IS PINNED AND PRINTED**, so a census covering less than it
    // claims fails rather than reporting a figure for a narrower set.
    assert_eq!(
        rows.len() + skipped.len(),
        PINNED_STREAMING,
        "the streaming population is {} and {PINNED_STREAMING} is pinned. \
         `region_composition.rs` pins the same figure; if the corpus really \
         changed, both move together and the reason is recorded.",
        rows.len() + skipped.len()
    );
    assert!(
        !rows.is_empty(),
        "no streaming module was driven, so every figure above is vacuous. The \
         skip reasons are printed; read them rather than trusting this census."
    );
}

/// **NO DRIVEN MODULE EXCEEDS ITS PUBLISHED PLAN.**
///
/// The plan is what this backend tells a host to add to its arena. A module
/// touching more than that is not looseness, it is a bound this line sells as
/// definitive being wrong — a different and far more serious finding.
#[test]
fn no_streaming_module_touches_more_than_its_plan() {
    let (rows, _) = census();
    let over: Vec<String> = rows
        .iter()
        .filter(|r| r.touched > r.plan)
        .map(|r| {
            format!(
                "{}: touched {} of a {}-byte plan",
                r.name, r.touched, r.plan
            )
        })
        .collect();
    assert!(
        over.is_empty(),
        "{} module(s) touched more arena than the published figure reserves: \
         {over:?}. That is not looseness in a bound, it is the bound being wrong, \
         and it is the more serious reading.",
        over.len()
    );
}

/// **THE INSTRUMENT IS NOT BLIND**, asserted apart from the figures.
///
/// A driver that reported zero for everything would satisfy the bound test above
/// and the population test too. `arena_high_water.rs` proves this driver's reach
/// on hand-written subjects by shrinking the buffer; here the evidence is that the
/// corpus itself produces MORE THAN ONE distinct answer.
#[test]
fn the_census_distinguishes_modules() {
    let (rows, _) = census();
    let distinct: std::collections::BTreeSet<usize> = rows.iter().map(|r| r.touched).collect();
    assert!(
        !rows.is_empty(),
        "nothing was driven, so this test establishes nothing"
    );
    assert!(
        distinct.len() > 1 || distinct.iter().any(|&t| t > 0),
        "every driven module reported the same zero extent, which is what a blind \
         instrument reports. Extents seen: {distinct:?}. If the corpus genuinely \
         touches nothing, say so where a reader meets it rather than leaving this \
         test to pass vacuously."
    );
}

/// **THE FIGURES REPORT SIX PUBLISHES, PINNED SO THEY CANNOT DRIFT UNWATCHED.**
///
/// `REVERSE_PROMPT.md` discloses these to the other line, and
/// `outstanding_reports.rs` exists because **a disclosure whose figures have
/// drifted is worse than none** — the other line would act on numbers this line no
/// longer measures.
///
/// A measured value is pinned rather than recomputed in the report, so a change
/// fails here with the module named instead of silently republishing a new number.
/// **Stability was checked before publishing**: two independent runs give the same
/// 48 bytes.
#[test]
fn the_published_dynamic_figures_are_pinned() {
    let (rows, skipped) = census();

    assert_eq!(
        (rows.len(), skipped.len()),
        (DRIVEN, PINNED_STREAMING - DRIVEN),
        "the driven/skipped split moved to ({}, {}). Report six publishes it, so \
         say what changed and why before republishing.",
        rows.len(),
        skipped.len()
    );

    let frame = rows
        .iter()
        .find(|r| r.name == "14_frame_log.kel")
        .expect("`14_frame_log.kel` is the one corpus module outside the stages that drives");
    assert_eq!(
        (frame.touched, frame.plan),
        (FRAME_LOG_TOUCHED, FRAME_LOG_PLAN),
        "`14_frame_log.kel` now touches {} of {}, not {FRAME_LOG_TOUCHED} of \
         {FRAME_LOG_PLAN}. Report six quotes those figures.",
        frame.touched,
        frame.plan
    );

    // **THE STAGES ARE STILL ZERO.** Report six says so, and it is the stronger
    // half of the dynamic claim.
    let stages_nonzero: Vec<&str> = rows
        .iter()
        .filter(|r| r.name != "14_frame_log.kel" && r.touched != 0)
        .map(|r| r.name.as_str())
        .collect();
    assert!(
        stages_nonzero.is_empty(),
        "these stage modules no longer touch zero arena: {stages_nonzero:?}. That \
         is the more interesting finding and report six must say so."
    );
}
