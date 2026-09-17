//! **DOES THE ARENA GROW ACROSS TICKS? AND DOES IT STAY INSIDE ITS OWN PLAN?**
//!
//! # The gap this fills, in the tree's own words
//!
//! `stream_depth.rs` closes its header with: *"Nothing about memory. Whether the
//! arena grows across ticks is not measured here."* The session handoff repeats
//! it as an open item. **It is the largest unevidenced claim on this line**,
//! because "the arena IS the coroutine instance" is the sentence the region
//! design rests on, and the value proposition of the ecosystem is a definitive
//! worst-case memory bound.
//!
//! # What a canary says, and what it does not
//!
//! The canaries added on 2026-09-14 answer **"did anything write past the
//! buffer?"** They are silent on **"how much of it is used, and does that amount
//! depend on the tick count?"** A stream consuming four more bytes per tick
//! passes every canary in this package for hundreds of ticks and then corrupts —
//! **while the sequences keep agreeing right up until they do not**, both
//! implementations computing the same values from a buffer one has overrun.
//!
//! # Method
//!
//! Fill the arena region with a known byte, drive the stream, and take the
//! highest index that no longer holds it. That is the cumulative touched extent,
//! and it needs no knowledge of the backend's internal bookkeeping — deliberately,
//! because reading the backend's own cursor would report what it believes rather
//! than what it did.
//!
//! **A single fill pattern would be unsound.** A byte legitimately written with
//! the same value as the fill is invisible, so the extent is under-reported and
//! the result reads stronger than it is. **Two runs with two distinct fills,
//! taking the larger extent, closes that hole completely**: a byte can equal one
//! pattern or the other, never both.
//!
//! # The controls, without which this would be a ritual
//!
//! - **Non-vacuity**: the extent must be greater than zero for at least one
//!   subject, or the instrument is reporting that nothing was written.
//! - **Discrimination**: subjects with different arena appetite must produce
//!   different extents. An instrument returning one number for every subject is
//!   measuring the buffer, not the program.
//! - **Fidelity**: a zero fill must reproduce the shipping driver's output
//!   exactly, or this is measuring a different program from the one under test.
//!
//! # ⚠ WHAT THIS DOES NOT ESTABLISH
//!
//! **This is not a static bound and cannot become one.** It is evidence about
//! these shapes at these tick counts. A dynamic high-water measurement can refute
//! the hypothesis that usage grows with tick count; it cannot prove worst-case
//! memory usage for inputs never driven. Stated here because the failure this
//! package keeps finding is a record that outlives, or overstates, its subject.

mod common;

/// Subjects, chosen for DIFFERENT arena appetite rather than for coverage.
const SHAPES: &[(&str, &str)] = &[
    (
        "plain two-yield",
        "loop main(t: Word) -> Word { (yield t) + (yield t + 1) }",
    ),
    (
        "a local carried across a yield",
        "loop main(t: Word) -> Word { let keep = t + 100; let r = yield 1; yield r + keep }",
    ),
    (
        "a composite built every tick",
        "struct P { a: Word, b: Word }\nloop main(t: Word) -> Word { let p = P { a: t, b: t + 1 }; let s = p.a + p.b; (yield s) + (yield s + 1) }",
    ),
];

/// Two distinct fills. **Neither may equal the driver's canary byte**, which the
/// helper asserts; a fill colliding with the sentinel would make the overrun
/// check report on the fill instead of on a write.
const FILL_A: u8 = 0x5A;
const FILL_B: u8 = 0xC3;

/// Shallow and deep tick counts. The previous deepest witness in this package
/// was six, and nothing measured memory at any depth.
const SHALLOW: usize = 2;
const DEEP: usize = 200;

fn replies(n: usize) -> Vec<i64> {
    (0..n).map(|i| (i as i64 % 7) - 3).collect()
}

/// The touched extent under BOTH fills, taking the larger.
///
/// Also asserts the two runs produced the same values. **If they differ, the
/// lowered program's output depends on how the host initialised the arena**,
/// which is a finding in its own right and is reported as one rather than worked
/// around by reverting to a zero fill.
fn extent(src: &str, first: i64, replies: &[i64]) -> usize {
    let (out_a, ext_a) = common::general_native_arena_extent(src, first, replies, FILL_A);
    let (out_b, ext_b) = common::general_native_arena_extent(src, first, replies, FILL_B);
    assert_eq!(
        out_a, out_b,
        "the lowered program's OUTPUT depends on how the arena buffer was \
         initialised: {FILL_A:#04x} gives {out_a:?}, {FILL_B:#04x} gives {out_b:?}. \
         That is a dependence on host memory contents, not a measurement \
         artefact, and it belongs in the record before this file's other \
         assertions mean anything."
    );
    ext_a.max(ext_b)
}

/// **THE QUESTION THIS FILE EXISTS FOR.**
#[test]
fn the_arena_extent_does_not_grow_with_tick_count() {
    for (label, src) in SHAPES {
        let shallow = extent(src, 1, &replies(SHALLOW));
        let deep = extent(src, 1, &replies(DEEP));
        assert_eq!(
            shallow,
            deep,
            "`{label}` touches {shallow} arena byte(s) over {SHALLOW} tick(s) and \
             {deep} over {DEEP}. The arena is being CONSUMED rather than reused, \
             at about {:.2} byte(s) per tick. `Op::Reset` rewinds the arena \
             between runs and the arena IS the coroutine instance, so a figure \
             that scales with the tick count contradicts the claim the region \
             design rests on.",
            (deep as f64 - shallow as f64) / (DEEP - SHALLOW) as f64
        );
    }
}

/// **The backend plans an arena size. Does the program stay inside it?**
///
/// The drivers size the region as the planned supplement PLUS slack, so a write
/// past the plan lands in the slack and disturbs no canary. Nothing in this
/// package compared the two numbers until now.
#[test]
fn the_touched_extent_stays_within_the_planned_arena_bound() {
    for (label, src) in SHAPES {
        let planned =
            keleusma_native::region::host_arena_supplement_bytes(&common::build(src)) as usize;
        let touched = extent(src, 1, &replies(DEEP));
        assert!(
            touched <= planned,
            "`{label}` touched {touched} arena byte(s) over {DEEP} ticks, but the \
             backend PLANNED {planned}. The drivers add slack past the plan, so \
             this overrun disturbs no canary and would be invisible to every \
             other instrument here. A worst-case memory bound that the program \
             itself exceeds is the claim this line sells."
        );
    }
}

/// **Non-vacuity.** An extent of zero everywhere would pass the growth test
/// trivially, and would mean the instrument sees nothing at all.
#[test]
fn the_extent_instrument_sees_something() {
    let seen: Vec<(&str, usize)> = SHAPES
        .iter()
        .map(|(label, src)| (*label, extent(src, 1, &replies(DEEP))))
        .collect();
    assert!(
        seen.iter().any(|(_, e)| *e > 0),
        "every subject touched ZERO arena bytes: {seen:?}. The growth test then \
         passes because nothing was measured, which is the shape this package \
         keeps finding — a guard with no reach, sitting in the file looking like \
         coverage."
    );
}

/// **Discrimination.** Different appetites must produce different numbers, or
/// the instrument is reporting a property of the buffer.
#[test]
fn the_extent_instrument_distinguishes_subjects() {
    let seen: Vec<(&str, usize)> = SHAPES
        .iter()
        .map(|(label, src)| (*label, extent(src, 1, &replies(DEEP))))
        .collect();
    let distinct: std::collections::BTreeSet<usize> = seen.iter().map(|(_, e)| *e).collect();
    assert!(
        distinct.len() > 1,
        "every subject reported the SAME extent: {seen:?}. Either these shapes \
         genuinely have identical arena appetite — in which case a subject with a \
         different one belongs here — or the number is a property of the buffer \
         rather than of the program. Do not delete this test to make the file \
         green; it is the one that says the measurement means anything."
    );
}

/// **Fidelity — and this check is WEAKER THAN ITS FIRST NAME CLAIMED.**
///
/// It was called `a_zero_fill_reproduces_the_shipping_driver`. **Perturbed by
/// passing a non-zero fill, it still passes**, so it does not establish anything
/// about the fill at all: the two drivers agree for these subjects whatever the
/// region holds, because their output does not depend on arena initialisation.
///
/// What it DOES establish is that the measuring driver and the shipping driver
/// are the same program — the same buffers, the same call sequence — which is
/// worth asserting, since a measurement taken through a divergent driver would
/// describe something no other differential here runs. **Renamed to that.**
///
/// The fill-independence it appeared to check is genuinely checked, by the
/// `assert_eq!` inside `extent` comparing two complementary fills.
#[test]
fn the_measuring_driver_is_the_shipping_driver() {
    for (label, src) in SHAPES {
        let r = replies(SHALLOW * 4);
        let (measured, _) = common::general_native_arena_extent(src, 1, &r, 0);
        let shipped = common::general_native_sequence(src, 1, &r);
        assert_eq!(
            measured, shipped,
            "`{label}`: the measuring driver and the shipping driver disagree \
             under an identical zero fill. Then this file measures a different \
             program from the one every other differential here drives."
        );
    }
}

/// **And the measured program must still agree with the reference.**
///
/// Without this, a file about memory could pass while the values were wrong.
#[test]
fn the_measured_subjects_still_agree_with_the_reference() {
    for (_, src) in SHAPES {
        common::assert_general_stream_agrees(src, 1, &replies(SHALLOW * 4));
    }
}

/// **THE PINNED APPETITE, AND WHY A NUMBER IS PINNED RATHER THAN A BOUND.**
///
/// The census idiom of this package: pin a population, fail on movement, and
/// demand a classification rather than a patched constant. Arena appetite is now
/// a tracked quantity — a change to the region planner or to the lowering that
/// makes a stream touch more arena announces itself here instead of being
/// absorbed into slack that no canary watches.
///
/// **The touched figures are ordered by appetite**, which is what makes them
/// evidence that the instrument reads the program: a plain stream, a stream
/// carrying a local across a yield, and one building a composite every tick
/// touch strictly increasing amounts. An instrument returning one number for all
/// three would be measuring the buffer.
#[test]
fn the_arena_appetite_of_each_subject_is_pinned() {
    const PINNED: &[(&str, usize)] = &[
        ("plain two-yield", 16),
        ("a local carried across a yield", 24),
        ("a composite built every tick", 48),
    ];
    let measured: Vec<(&str, usize)> = SHAPES
        .iter()
        .map(|(label, src)| (*label, extent(src, 1, &replies(DEEP))))
        .collect();
    let pinned: Vec<(&str, usize)> = PINNED.to_vec();
    assert_eq!(
        measured, pinned,
        "arena appetite MOVED. Say which change moved it and whether the new          figure is correct, then re-pin. Do not edit the constant to match: the          pin exists because a stream quietly touching more arena is invisible to          every other instrument in this package, which watch for overruns of a          buffer sized with slack."
    );
}

/// **⚠ THE REGION PLAN IS DOMINATED BY A RESERVATION BARELY ANY SUBJECT USES.**
///
/// `stream_spill_bytes` reserves `MAX_STACK * 8` — **512 bytes** — for every
/// stream chunk, unconditionally. Measured against it, the three subjects touch
/// 16, 24 and 48 bytes of a 520, 536 and 552 byte plan, and **the deepest reach
/// into the spill block itself is a single eight-byte slot.** Over 98% of the
/// dominant term of the published host figure is never written.
///
/// > ⚠ **THE FIRST VERSION OF THIS TEST CLAIMED THE BLOCK WAS UNTOUCHED
/// > ENTIRELY, AND ITS OWN ASSERTION REFUTED THAT ON THE FIRST RUN.** Touched
/// > extents of 16 and 48 run eight bytes past the chunk-plus-locals prefix, so
/// > one slot is in use. **The claim was corrected to the measurement rather than
/// > the measurement to the claim**, which is the only reason the number above is
/// > worth anything.
///
/// **This is not a defect.** The reservation is deliberate, documented, fixed and
/// static, and its rationale — that a predicate disagreeing with the lowering's
/// own would be a worse defect than unused bytes — is sound. Over-provisioning is
/// safe in the direction that matters.
///
/// **What it is, is a quantified looseness in a bound this line sells as
/// definitive.** `stream_spill_bytes` describes its own waste as *"a few unused
/// bytes"*; measured, it is 504 of 512 on every subject here, and the dominant
/// term of the whole plan. A host sizing an arena from
/// `host_arena_supplement_bytes` provisions roughly twenty times what these
/// streams use. Whether that is acceptable is an arena-accounting question for
/// the operator, exactly as that function's own documentation says of the figure
/// it publishes.
#[test]
fn the_spill_reservation_is_nearly_all_unused() {
    /// The most spill any subject here is permitted to reach before this stops
    /// being a statement about waste. One operand slot.
    const MAX_SPILL_SLOTS_USED: usize = 1;

    for (label, src) in SHAPES {
        let m = common::build(src);
        let chunk_ref = &m.chunks[m.entry_point.expect("entry")];
        let spill = keleusma_native::region::stream_spill_bytes(chunk_ref) as usize;
        let prefix = keleusma_native::region::plan_chunk_region(chunk_ref).bytes as usize
            + keleusma_native::region::stream_locals_bytes(chunk_ref) as usize;
        let planned = keleusma_native::region::host_arena_supplement_bytes(&m) as usize;
        let touched = extent(src, 1, &replies(DEEP));

        assert!(
            spill > 0 && planned > 0,
            "`{label}`: the plan or the spill term is zero, so this comparison              says nothing. A stream chunk reserves spill unconditionally; if that              stopped being true, this test is the record of when."
        );
        let spill_used = touched.saturating_sub(prefix);
        assert!(
            spill_used <= MAX_SPILL_SLOTS_USED * 8,
            "`{label}` reaches {spill_used} byte(s) into a {spill}-byte spill              reservation, past the permitted {MAX_SPILL_SLOTS_USED} slot(s).              Spill use is a REAL quantity here, not a rounding artefact, so a rise              means the lowering is spilling more across yields. Classify it before              re-pinning; the whole point of the reservation is that it is never              the binding constraint."
        );
        assert!(
            spill_used * 20 < spill,
            "`{label}` now uses {spill_used} of {spill} spill bytes, which is no              longer the 'nearly all unused' this test is named for. The comment              in `stream_spill_bytes` calling the waste 'a few unused bytes' would              need re-reading in the other direction."
        );
    }
}

// ---------------------------------------------------------------------------
// REACH, MEASURED 2026-09-16 — WHICH OF THESE ASSERTIONS CAN ACTUALLY FAIL
// ---------------------------------------------------------------------------
//
// **A clean guard proves its reach first.** Every assertion in this file passed
// on the day it was written, which is evidence about the checker before it is
// evidence about the backend. Each was then perturbed AT ITS SUBJECT — never by
// editing the assertion or its expected value — and the outcome recorded.
//
// Two perturbations, applied in the driver rather than to the tests:
//
//   CREEP  — one byte of the arena region touched per tick, the shape of a
//            coroutine instance that is consumed rather than reused.
//   BEYOND — a single write past the planned bound, constant in tick count.
//
// | assertion | perturbation | fires |
// |---|---|---|
// | `the_arena_extent_does_not_grow_with_tick_count` | CREEP | **yes** |
// | `the_touched_extent_stays_within_the_planned_arena_bound` | BEYOND | **yes** |
// | `the_arena_appetite_of_each_subject_is_pinned` | either | **yes** |
// | `the_spill_reservation_is_nearly_all_unused` | either | **yes**, and its own first version fired |
// | `the_extent_instrument_distinguishes_subjects` | CREEP | **yes** |
// | `the_extent_instrument_sees_something` | — | **no subject perturbation exists** |
// | `the_measuring_driver_is_the_shipping_driver` | a different fill | **NO — it does not discriminate** |
// | `the_measured_subjects_still_agree_with_the_reference` | — | inherits the differential's reach |
//
// **THE TWO NEGATIVE ROWS ARE THE POINT OF THE EXERCISE**, not a shortfall in it.
//
// **The discrimination is as informative as the firing.** CREEP fires the growth
// test and NOT the bound test — 200 bytes of creep stays inside a 520-byte plan.
// BEYOND fires the bound test and NOT the growth test — it is constant in tick
// count. The two assertions detect different defects, which is the property a
// reader needs and which neither passing alone would have shown.
//
// **`the_extent_instrument_sees_something` cannot be fired by choosing a
// subject.** Every stream this driver can carry reserves locals in the region, so
// no drivable shape touches zero arena; the smallest observed is sixteen bytes.
// It guards against a regression in the instrument, not against a state the
// subjects can reach, and recording that is better than inventing a perturbation
// that passes for reach. `stream_depth.rs` already records the same distinction
// about its varying replies.
