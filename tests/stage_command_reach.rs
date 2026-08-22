//! Which of `wire.kel`'s dispatched commands anything actually drives.
//!
//! # Why this file exists
//!
//! Commands **176 `fl_stream_begin`** and **177 `fl_stream_step`** — the
//! one-node-in, one-record-out streaming path for constant nodes — were written,
//! dispatched, and announced to the `v0.3.0` line as landed. **No driver and no
//! test has ever called them.**
//!
//! That matters because it changes the cost of `CONSTS`, Order 1 item 1. The
//! tree's analysis says the flattener already emits a byte-identical region, that
//! the 170-node walk cap is the only blocker, and that batching is the route.
//! Reading that alongside "a streaming variant already exists" makes the remaining
//! work look like driver wiring. It is not: the stage side has never executed, so
//! taking `CONSTS` means writing the driver *and* validating code that has never
//! run.
//!
//! # The class, which is the transferable part
//!
//! Third instance this week of the same shape. The `v0.3.0` line found `Op::Reset`
//! credited as lowered because a *chunk* containing it lowered, while the op sat
//! in a region no edge reaches — a mutation crediting it moved their figure to 57
//! of 66 **with every test still green**. `Op::IsStruct` is emitted only on a
//! fallback nothing has reached. And now two commands announced as delivered.
//!
//! **Presence, dispatch, and even an announcement are not evidence that code
//! runs.** The cheap check is to search for callers before costing work that
//! depends on it.
//!
//! # UPDATE 2026-08-22: THE DRIVER REACHES THEM, AND THIS TEST HAD A HOLE
//!
//! `wire_consts_via_kel` emits every stage's `CONSTS` region through commands 176
//! and 177, byte-identically to the reference encoder — see
//! `tests/selfhost_consts_driver.rs`. So the fact this file pinned is no longer
//! true, and the test is inverted rather than deleted.
//!
//! **The guard did not announce that, because it could not.** It searched the
//! driver for the stage's function names, and the driver addresses the stage by
//! command NUMBER. It would have kept passing however completely the route was
//! wired. That is a second instance of this tree's "a guard that cannot fire is
//! worse than none", and the reason the replacement derives from the numbers.
//!
//! # UPDATE 2026-08-21: THEY HAVE NOW BEEN EXECUTED, AND THIS TEST NARROWED
//!
//! `tests/selfhost_wire.rs` now drives both commands directly: a scalar node
//! streams a record matching the documented layout, and all three refusals
//! (`-264` a node with children, `-265` an interning tag, `-266` a range-carrying
//! tag) have been made to fire with an accepting control beside them.
//!
//! **So "driven by nothing" is no longer true and this test no longer claims it.**
//! What remains true, and is what this file now pins, is that the DRIVER does not
//! drive them — the route to `CONSTS` is still unwired. That distinction is the
//! whole point: the stage side is validated, so a divergence found while wiring
//! the driver is attributable to the driver rather than to three-way uncertainty
//! between stage, driver and seam.
//!
//! # This test does not assert they are dead
//!
//! They are the intended route for `CONSTS` and should not be deleted. The test
//! records that the DRIVER has not yet reached them, and fails when that changes,
//! so whoever wires it updates the record rather than discovering the gap again.

#![cfg(feature = "self-host")]

/// The `wire.kel` source, read at compile time so the dispatch is the thing
/// measured rather than a copy of it.
const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");

/// Every command number `wire.kel` dispatches, derived from the stage rather than
/// listed here — a hand-written list is how a table comes to disagree with the
/// thing it describes.
fn dispatched_commands() -> Vec<u32> {
    let mut out: Vec<u32> = WIRE
        .lines()
        .filter_map(|l| l.trim().strip_prefix("if cmd == "))
        .filter_map(|rest| {
            rest.split_whitespace()
                .next()
                .and_then(|n| n.parse::<u32>().ok())
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// **THE DRIVER NOW REACHES THE CONSTANT-NODE STREAMING COMMANDS, AND THE GUARD
/// THAT WAS SUPPOSED TO ANNOUNCE THAT COULD NOT HAVE FIRED.**
///
/// This test asserted that the driver did not name `fl_stream_begin` or
/// `fl_stream_step`, and it was written "pinned in the firing direction: when the
/// driver drives them, this fails". **It did not fire.** The driver addresses the
/// stage by COMMAND NUMBER — `const CMD_BEGIN: i64 = 176` — and never writes the
/// stage's function names at all, so the strings it searched for could not appear
/// however thoroughly the path was wired.
///
/// That is this tree's recorded "a guard that cannot fire is worse than none",
/// and the second instance of it: the earlier one compared `directory.len()`
/// against a stage buffer when that length is the shared array's size, false by
/// construction. **Before adding a check, construct the input that makes it
/// fire.** This one is now derived from the driver's command NUMBERS, which is
/// the thing that changes when the path is wired, and it was verified to fail
/// against the tree as it stood before `wire_consts_via_kel` existed.
///
/// # What it pins now
///
/// The route is wired, so the fact worth guarding has inverted: `CONSTS` no
/// longer requires validating never-executed stage code, and a driver that stops
/// reaching these commands has lost a region rather than gained simplicity.
#[test]
fn the_driver_reaches_the_constant_streaming_commands() {
    const DRIVER: &str = include_str!("../src/selfhost/mod.rs");

    let commands = dispatched_commands();
    // MUST-FIRE on the derivation working at all. A parse that found nothing would
    // satisfy every assertion below while measuring the empty set.
    assert!(
        commands.len() > 100,
        "only {} dispatched commands were derived from `wire.kel`; the parse is \
         broken and this test measures nothing",
        commands.len()
    );
    for cmd in [176u32, 177] {
        assert!(
            commands.contains(&cmd),
            "command {cmd} is no longer dispatched by `wire.kel`, so the driver below \
             is calling into nothing"
        );
    }

    // BY NUMBER, because that is how the driver addresses the stage. The previous
    // form searched for the stage's function names, which the driver has never
    // written and never will.
    for decl in ["CMD_BEGIN: i64 = 176", "CMD_STEP: i64 = 177"] {
        assert!(
            DRIVER.contains(decl),
            "the driver no longer declares `{decl}`. If the constant-streaming route \
             was removed, say what emits `CONSTS` instead; if it was merely renamed, \
             this test needs the new name rather than deletion"
        );
    }

    // The control: the CHUNK-streaming command is driven too, so a driver that
    // stopped naming any command at all would fail here rather than look like a
    // discovery about constants specifically.
    assert!(
        DRIVER.contains("CMD_STEP: i64 = 175"),
        "the chunk-streaming command is no longer named in the driver, so the \
         presence of 176/177 above says nothing about them specifically"
    );
}

/// **COMMANDS 178 TO 181 ARE DRIVEN BY A TEST AND NOT BY THE DRIVER.**
///
/// `ds_stream_step`, `sh_stream_step`, `sg_stream_step` and `ev_stream_step` — the
/// one-record-per-call formatters for `DATA_SLOTS`, `SHAPES`, `SIGNATURES` and
/// `ENUM_VARIANTS`. **Until 2026-08-22 nothing in Rust named any of the four
/// numbers.** Fourth instance of this shape on this line, after `Op::Reset`,
/// `Op::IsStruct`, and commands 176/177.
///
/// `tests/selfhost_wire.rs` now drives all four and each formats a record the
/// reference agrees with, mutation-verified in both directions. So the stage side
/// is validated; the wiring is not written.
///
/// # Why the distinction is worth a test rather than a sentence
///
/// Eight region kinds are still skipped by the windowed assembler, and three of
/// them — `SHAPES` at 341 records, `SIGNATURES` at 486, `DATA_SLOTS` at 388 —
/// exceed a single 1,024-word `fin` batch. These four commands are the
/// batching-free route to them. Reading "streaming commands already exist"
/// alongside "the stage has an emitter for every kind" makes the remaining work
/// look like wiring, and until this was measured it was wiring plus validating
/// never-run code.
///
/// # THE SHAPE OF THE GUARD, WHICH THE PREVIOUS ONE GOT WRONG
///
/// The 176/177 version of this searched the driver for the STAGE's function names
/// and could not have fired, because the driver addresses the stage by COMMAND
/// NUMBER. This one searches for the numbers, and **was made to fire** by adding
/// a matching declaration to the driver.
#[test]
fn the_driver_does_not_yet_reach_the_record_formatters() {
    const DRIVER: &str = include_str!("../src/selfhost/mod.rs");

    let commands = dispatched_commands();
    assert!(
        commands.len() > 100,
        "only {} dispatched commands were derived; the parse is broken",
        commands.len()
    );
    for cmd in [178u32, 179, 180, 181] {
        assert!(
            commands.contains(&cmd),
            "command {cmd} is no longer dispatched by `wire.kel`, so this record is stale \
             rather than the path being unwired"
        );
    }

    // BY NUMBER, in the form the driver writes when it drives a command. The
    // control below proves the form is the one that would appear.
    for n in [178u32, 179, 180, 181] {
        assert!(
            !DRIVER.contains(&format!("i64 = {n}")),
            "the driver now declares command {n}. The record formatters are being driven: \
             record the new region coverage and replace this test rather than relaxing it"
        );
    }

    // The control: the constant-streaming commands ARE declared in that exact
    // form, so the absence of 178..181 above is a fact about them rather than
    // about the search.
    for n in [176u32, 177] {
        assert!(
            DRIVER.contains(&format!("i64 = {n}")),
            "command {n} is no longer declared in the driver in the form this test searches \
             for, so the absence of 178..181 says nothing about them specifically"
        );
    }
}
