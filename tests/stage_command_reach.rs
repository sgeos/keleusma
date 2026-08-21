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
//! # This test does not assert they are dead
//!
//! They are the intended route for `CONSTS` and should not be deleted. The test
//! records that they are currently unreached, and fails when that changes, so
//! whoever drives them updates the record rather than discovering the gap again.

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

/// **THE CONSTANT-NODE STREAMING COMMANDS ARE DISPATCHED AND UNREACHED.**
///
/// Pinned in the firing direction: when something drives them, this fails and its
/// author records that the path is now exercised.
#[test]
fn the_constant_streaming_commands_are_dispatched_but_driven_by_nothing() {
    const DRIVER: &str = include_str!("../src/selfhost/mod.rs");
    const STREAM_FNS: &[&str] = &["fl_stream_begin", "fl_stream_step"];

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
            "command {cmd} is no longer dispatched by `wire.kel`, so this record is \
             stale rather than the path being driven"
        );
    }

    // The control: the chunk-streaming command directly below them IS driven, so a
    // driver that stopped naming any command would fail here rather than look like
    // a discovery.
    assert!(
        DRIVER.contains("CMD_STEP: i64 = 175"),
        "the chunk-streaming command is no longer named in the driver, so the \
         absence of 176/177 below says nothing about them specifically"
    );

    for name in STREAM_FNS {
        assert!(
            !DRIVER.contains(name),
            "the driver now names `{name}`. The constant-node streaming path is \
             being driven: record that `CONSTS` no longer requires validating \
             never-executed stage code, and replace this test rather than relaxing it"
        );
    }
}
