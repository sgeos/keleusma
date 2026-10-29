//! **THE REPORTS THIS LINE HAS OPEN WITH THE `v0.2.3` LINE, WATCHED RATHER THAN
//! REMEMBERED.**
//!
//! # Why this file exists
//!
//! `REVERSE_PROMPT.md` says *"STILL WITH YOU, NONE ACTED ON"*. That was true when
//! written and rested on **recollection** — a scan of absorption commit messages,
//! thirty-six absorbed commits ago.
//!
//! One increment earlier, this line found that `NATIVE_BOUNDS_TRANSFER.md` had
//! asserted the reference's memory bound was unsound for **four weeks after it was
//! repaired**. **A report that is never re-checked becomes a standing
//! accusation**, and the document that carries these is written TO the other line
//! rather than about them.
//!
//! # The model, which already existed
//!
//! The `confine.rs` report is self-verifying by construction:
//! `lowering_robustness.rs` allows that panic by ORIGIN FILE and asserts it STILL
//! FIRES, so a repair upstream turns this suite red and forces the carve-out to be
//! deleted rather than outlive the defect. **That pattern is generalised here.**
//!
//! # What a guard here does and does not claim
//!
//! It watches a BEHAVIOUR, not a ruling. Two of these items are questions — should
//! a multi-parameter stream compile at all; is the write-before-read check meant to
//! be flow-insensitive — and a question has no right answer this line can assert.
//! What it can assert is **what the reference does today**, so that a change in
//! that behaviour reaches this line as a failure rather than as silence.
//!
//! **A failure here is not a defect in this backend.** Each message says to
//! retract or update the report, not to debug a test.

use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

mod common;

/// **REPORT 1 — a `confine.rs` index panic on a truncated op stream.**
///
/// Guarded in `lowering_robustness.rs`, which asserts the panic STILL FIRES and
/// says to delete its carve-out when it stops. Not duplicated here: a second
/// copy of one guard is the coupling this package has already had to unpick three
/// times. This test exists to record WHERE it is watched, and fails if that file
/// stops watching it.
#[test]
fn report_one_is_watched_by_the_robustness_sweep() {
    let src = std::fs::read_to_string("tests/lowering_robustness.rs")
        .expect("the robustness sweep is readable");
    // Matched on the distinctive instruction rather than on the prose around it:
    // a three-way OR over wrapped fragments passes on the weakest of them, which
    // is the shape of a check that cannot fail.
    assert!(
        src.contains("DELETE this allowance"),
        "`lowering_robustness.rs` no longer asserts that the upstream `confine.rs` \
         panic still fires. If the guard was removed because the defect was fixed, \
         retract report 1 in `REVERSE_PROMPT.md`; if it was removed for another \
         reason, report 1 is now unwatched."
    );
}

/// **REPORT 2 — a multi-parameter stream faults after its first rewind.**
///
/// `Op::Reset` clears every local and the resume writes slot 0 only, so a second
/// parameter has no defined value on re-entry. This backend REFUSES the shape,
/// and the refusal's justification is that the program does not work on the
/// reference either.
///
/// **If the reference stops faulting, that justification is gone** and the
/// refusal must be revisited — which is exactly what this assertion is for.
#[test]
fn report_two_still_reproduces_on_the_reference() {
    const MULTI_PARAM: &str =
        "loop main(a: Word, b: Word) -> Word { let r = yield a + b; yield r + 1 }\n";

    let m = common::try_build(MULTI_PARAM).expect(
        "the reference still COMPILES a multi-parameter stream. If it now rejects the \
         shape, that ANSWERS the open question — the verifier rejects it and no runtime \
         change is needed — so update report 2 rather than this test",
    );

    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");

    // Drive past the rewind. The fault is reported to arrive AFTER the reset, so
    // a run that ends before one proves nothing.
    let mut st = vm.call(&[Value::Int(3), Value::Int(4)]);
    let mut saw_reset = false;
    let mut fault: Option<String> = None;
    for _ in 0..8 {
        match st {
            Ok(VmState::Yielded(_)) => st = vm.resume(Value::Int(1)),
            Ok(VmState::Reset) => {
                saw_reset = true;
                st = vm.resume(Value::Int(1));
            }
            Ok(_) => break,
            Err(e) => {
                fault = Some(format!("{e:?}"));
                break;
            }
        }
    }

    // **NON-VACUITY.** A run that faulted before reaching a reset would satisfy a
    // naive "it still faults" check while testing something else entirely.
    assert!(
        saw_reset,
        "the subject never reached a rewind, so whatever happened says nothing about \
         a fault AFTER one"
    );
    let fault = fault.expect(
        "the multi-parameter stream no longer faults after its rewind. If the `v0.2.3` \
         line fixed it, RETRACT report 2 and revisit this backend's refusal of the \
         shape, whose only justification is that the program does not work there either",
    );
    assert!(
        fault.contains("Unit"),
        "the fault is no longer the reported one — slot 1 left undefined by the rewind. \
         It now reads {fault}. Update report 2 rather than this assertion."
    );
}

/// **REPORT 3 — the write-before-read check is flow-insensitive.**
///
/// The reference REJECTS an unconditional read-before-write of a composite data
/// slot and ACCEPTS a write on one branch followed by an unconditional read; the
/// runtime then faults at execution when the branch is not taken.
///
/// This line asked whether that is intended. It also built machinery on the
/// answer: an initialisation word per composite slot, so native code faults where
/// the reference faults. **If the reference starts rejecting the shape at compile
/// time, that machinery is guarding a case that can no longer arise** and the
/// question is answered.
#[test]
fn report_three_still_reproduces_on_the_reference() {
    const CONDITIONAL: &str = "struct F { a: Word, b: Word }\n\
                               private data log { latest: F, count: Word }\n\
                               fn main(t: Word) -> Word { if t < 0 { log.latest = F { a: 1, b: 2 }; } \
                                 log.latest.a }\n";
    const UNCONDITIONAL: &str = "struct F { a: Word, b: Word }\n\
                                 private data log { latest: F, count: Word }\n\
                                 fn main(t: Word) -> Word { log.latest.a + t }\n";

    // **The pair is the point.** Accepting the conditional shape means nothing
    // unless the unconditional one is still rejected — together they say the
    // check exists and is flow-insensitive, which is the reported observation.
    assert!(
        common::try_build(UNCONDITIONAL).is_none(),
        "the reference now ACCEPTS an unconditional read-before-write of a composite \
         slot. That is a larger change than report 3 describes: the write-before-read \
         contract would no longer be checked at all. Update report 3."
    );
    assert!(
        common::try_build(CONDITIONAL).is_some(),
        "the reference now REJECTS a conditional write followed by a read, which \
         ANSWERS report 3 — the check is flow-sensitive. Retract it, and revisit the \
         per-slot initialisation words, which exist to fault where the reference faults \
         on exactly this shape"
    );
}

/// **REPORT 4 — ANSWERED FROM THE RUNTIME SIDE IN ABSORPTION 82. Retained as
/// a watch on the answer, not as an open report.**
///
/// The report was that the type checker admitted `Fixed % Fixed` and the
/// virtual machine then trapped with `TypeError("cannot modulo Fixed by
/// Fixed")`. Upstream made bare `Op::Mod` total, with a Fixed arm returning the
/// raw remainder of the scaled bits, so the program now completes. No opcode was
/// added. This backend lowers the matched Fixed pair to the same result, which
/// `fixed_kind_reach.rs` and the scalar operator matrix compare.
///
/// Guarded from both sides still: if the compiler starts rejecting the program,
/// or the virtual machine stops completing it, the answer has moved again.
#[test]
fn report_four_is_answered_and_the_answer_holds() {
    const SRC: &str = "fn main(a: Fixed, b: Fixed) -> Fixed { a % b }";

    let Some(m) = common::try_build(SRC) else {
        panic!(
            "the reference compiler now REJECTS `Fixed % Fixed`. Report 4 was answered \
             the other way in absorption 82; re-read the emitter's Fixed remainder arm."
        );
    };

    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("the module still loads and verifies");
    let outcome = vm.call(&[Value::Fixed(200 << 16), Value::Fixed(7 << 16)]);
    assert!(
        matches!(outcome, Ok(VmState::Finished(Value::Fixed(v))) if v == (200 << 16) % (7 << 16)),
        "the reference no longer completes `Fixed % Fixed` with the raw remainder: \
         {outcome:?}. Report 4's answer has moved; re-read the emitter's Fixed \
         remainder arm before anything else."
    );
}

/// **REPORT 5 — `DataSlot` carries no scalar kind.**
///
/// `SharedSlotLayout` carries a kind tag; `DataSlot` carries a name and a
/// visibility. The reference never needs the declaration, keeping a tagged
/// `Value` at run time. A native backend has only the module, so a `Fixed` read
/// back from a private slot is indistinguishable from a `Word`.
///
/// Watched by `fixed_kind_reach.rs`. Its one known consequence, a private-slot
/// `Fixed % Fixed` that trapped upstream and lowered here, stopped diverging in
/// absorption 82: the reference now completes it with the same low bits the
/// integer remainder produces. **The kind is still absent from the module**,
/// so the report stays open, and that file now asserts the route AGREES.
/// Not duplicated: a second copy of one guard is the coupling this package has
/// had to unpick three times. This test records WHERE it is watched.
#[test]
fn report_five_is_watched_by_the_reach_suite() {
    let src =
        std::fs::read_to_string("tests/fixed_kind_reach.rs").expect("the reach suite is readable");
    assert!(
        src.contains("the_private_slot_route_agrees_without_the_kind"),
        "`fixed_kind_reach.rs` no longer watches the private-slot route. If the \
         module now carries a private slot's scalar kind, retract report 5; if the \
         test was removed for another reason, report 5 is now unwatched."
    );
}

/// **Report six re-derives its own figures instead of being believed.**
///
/// The other five reports are reproductions: run a program, see the reference
/// behave as reported. Six is a MEASUREMENT, so the analogous check is that the
/// numbers in the report still come out of the tree.
///
/// It reads the report text and the instruments that produced it, and fails when
/// they disagree. **A disclosure whose figures have drifted is worse than none**,
/// because the other line would act on numbers this line no longer measures.
#[test]
fn report_six_still_reproduces_from_the_tree() {
    let report = std::fs::read_to_string("../docs/process/REVERSE_PROMPT.md")
        .expect("the shared channel is readable");
    let section = report
        .split("## ⚠ SIX — A DISCLOSURE")
        .nth(1)
        .expect("report six is present in the channel; if it was withdrawn, delete this test too");

    // The two instruments the report cites by path must still exist and still
    // pin the quantities it quotes.
    let composition = std::fs::read_to_string("tests/region_composition.rs")
        .expect("the static census is readable");
    for (quantity, needle) in [
        ("the streaming population", "PINNED_STREAMING: usize = 32"),
        ("the majority count", "PINNED_MAJORITY: usize = 28"),
        ("the minimum share", "PINNED_MIN_SHARE: u32 = 12"),
    ] {
        assert!(
            composition.contains(needle),
            "report six quotes {quantity}, and `region_composition.rs` no longer \
             pins it. Re-measure and correct the report, or withdraw it — do not \
             leave the other line reading a figure this line has stopped \
             measuring."
        );
    }

    // **THE CORPUS FIGURES ADDED 2026-09-23 ARE WATCHED TOO.** Report six now
    // publishes a driven count and one module's measured extent; an unwatched
    // published figure is exactly what this test exists to prevent.
    let touch = std::fs::read_to_string("tests/streaming_arena_touch.rs")
        .expect("the streaming arena census is readable");
    for (quantity, needle) in [
        ("the driven count", "const DRIVEN: usize = 16;"),
        (
            "the measured extent",
            "const FRAME_LOG_TOUCHED: usize = 48;",
        ),
        (
            "the plan it is measured against",
            "const FRAME_LOG_PLAN: usize = 600;",
        ),
    ] {
        assert!(
            touch.contains(needle),
            "report six quotes {quantity}, and `streaming_arena_touch.rs` no longer \
             pins it. Re-measure and correct the report, or withdraw the figure — \
             do not leave the other line reading a number this line has stopped \
             measuring."
        );
    }

    let stages = std::fs::read_to_string("tests/stage_differential.rs")
        .expect("the stage differential is readable");
    assert!(
        stages.contains("region_extent, 0,"),
        "report six says the twelve stages touch ZERO arena bytes, and \
         `stage_differential.rs` no longer asserts it. If a stage began using the \
         arena, that is the more interesting finding and the report must say so."
    );

    // And the report must keep saying what it is NOT, since that framing is the
    // difference between a disclosure and an accusation.
    for phrase in [
        "not a defect claim",
        "The code is MINE",
        "does NOT establish",
    ] {
        assert!(
            section.contains(phrase),
            "report six no longer states `{phrase}`. That framing is load-bearing: \
             the reservation is safe and deliberate, the code is this line's own, \
             and the measurement bounds nothing about arbitrary programs. Without \
             it the section reads as an accusation about someone else's defect."
        );
    }
}
