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

/// Every command number the DRIVER names, with comments stripped.
///
/// # THIS FUNCTION EXISTS BECAUSE THE PREVIOUS TWO GUARDS COULD NOT FIRE
///
/// The 176/177 guard searched the driver for the STAGE's function names, which
/// the driver never writes because it addresses the stage by number. Its
/// replacement searched for the declaration form `i64 = 179` — and the driver
/// went on to drive 179 and 180 by passing them as **literal arguments**, so that
/// guard did not fire either.
///
/// **BOTH FAILURES ARE THE SAME MISTAKE**: matching a shape the checker imagined
/// rather than the shape the code takes. Deriving every number the file mentions
/// removes the guess entirely — a driver that names a command in any form is
/// found, and one that stops naming it is found too.
///
/// Comments are stripped because a doc comment saying "commands 179 and 180"
/// would otherwise count as driving them. That is the too-loose direction of the
/// same error, and this line has four recorded instances of a guard firing on the
/// prose that explains it.
/// Every integer literal appearing in the driver's non-comment code.
///
/// # THIS IS A PROXY, IT IS FIT FOR LARGE DISTINCTIVE NUMBERS ONLY, AND A POPULATION CENSUS OF
/// THE DISPATCHED COMMANDS IS NOT SUPPORTABLE BY IT
///
/// The tests here use this for commands 178 to 181. Those numbers are large and appear nowhere
/// else in the driver, so "the number is present" is a sound reading of "the driver issues the
/// command".
///
/// **It does not generalise, and the attempt was made and abandoned on 2026-08-30.** `wire.kel`
/// dispatches 182 commands, and the file above records the transferable lesson — presence and
/// dispatch are not evidence that code runs — which makes "how many of the 182 does the driver
/// actually reach?" the obvious next question. It is the same shape as a gap this repository
/// closed by measuring a population it had only described.
///
/// **Run against all 182, this reader reports near-total coverage and is wrong.** Commands 1, 2
/// and 3 are dispatched, and those integers appear throughout the driver as array indices, field
/// counts and widths. The census would be confidently wrong in the flattering direction, which is
/// the worst one.
///
/// **A better instrument would need a precise call form to match, and there is none.** The driver
/// addresses the stage by writing a command number into a shared slot, and it does so through
/// several differently shaped helpers rather than one. Matching the write sites is possible but is
/// real work, not a grep.
///
/// **So the census is UNMEASURED, and the reason is recorded rather than the number.** Anyone
/// costing work on the strength of "most commands are driven" or "most are dead" should know that
/// neither statement has been established, and that the cheap-looking way to establish it produces
/// a wrong answer rather than no answer.
fn driver_command_numbers() -> Vec<u32> {
    const DRIVER: &str = include_str!("../src/selfhost/mod.rs");
    let mut out: Vec<u32> = Vec::new();
    for line in DRIVER.lines() {
        let code = match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        };
        let mut digits = String::new();
        for ch in code.chars().chain(core::iter::once(' ')) {
            if ch.is_ascii_digit() {
                digits.push(ch);
                continue;
            }
            if !digits.is_empty() {
                if let Ok(n) = digits.parse::<u32>() {
                    out.push(n);
                }
                digits.clear();
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// **THE DRIVER NOW REACHES 179 AND 180, AND THE GUARD THAT SHOULD HAVE SAID SO
/// DID NOT FIRE.**
///
/// `sh_stream_step` (179) and `sg_stream_step` (180) emit the `SHAPES` and
/// `SIGNATURES` regions from the driver. The previous revision of this test
/// asserted the driver did **not** reach them and **kept passing**, because it
/// searched for `i64 = 179` and the driver passes the number as a literal
/// argument.
///
/// # The lesson is not "that guard was too narrow"
///
/// It was mutation-tested before being trusted, and the mutation passed. But the
/// mutation added a `const ... i64 = 178;` declaration — **the exact form the
/// guard already matched**. A mutation shaped like the checker's own assumption
/// confirms the assumption instead of testing it, and that is a sharper rule than
/// "construct the input that makes it fire": the input has to be the one the real
/// change would produce, not the one the guard expects.
///
/// # What is pinned now
///
/// 179 and 180 are driven; **178 (`ds_stream_step`) and 181 (`ev_stream_step`)
/// are not**, because `DATA_SLOTS` and `ENUM_VARIANTS` records carry a name index
/// the host does not hold. Both directions are asserted, so this fails when
/// either fact changes.
#[test]
fn the_driver_reaches_the_shape_and_signature_formatters_only() {
    let dispatched = dispatched_commands();
    assert!(
        dispatched.len() > 100,
        "only {} dispatched commands were derived from `wire.kel`; the parse is broken",
        dispatched.len()
    );
    for cmd in [178u32, 179, 180, 181] {
        assert!(
            dispatched.contains(&cmd),
            "command {cmd} is no longer dispatched by `wire.kel`"
        );
    }

    let driven = driver_command_numbers();
    // MUST-FIRE on the derivation. A reader that found no numbers would satisfy
    // the "not driven" half perfectly and establish nothing.
    assert!(
        driven.contains(&176) && driven.contains(&177),
        "the constant-stream commands 176/177 are not among the numbers this reader found, \
         so the reader is broken and every absence below is meaningless"
    );

    for cmd in [179u32, 180] {
        assert!(
            driven.contains(&cmd),
            "the driver no longer names command {cmd}. If `SHAPES`/`SIGNATURES` stopped being \
             emitted, say what emits them instead and update the coverage figure"
        );
    }
    for cmd in [178u32, 181] {
        assert!(
            !driven.contains(&cmd),
            "the driver now names command {cmd}. `DATA_SLOTS` and `ENUM_VARIANTS` carry a name \
             index the host does not hold, so reaching them means that dependency was solved: \
             record how, and update the region coverage"
        );
    }
}
