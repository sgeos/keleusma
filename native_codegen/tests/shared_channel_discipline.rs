//! **THE OUTGOING CHANNEL IS SHARED WITH THE OTHER LINE, AND BOTH LINES WRITE
//! IT.**
//!
//! # What happened
//!
//! Absorption 60 conflicted on `docs/process/REVERSE_PROMPT.md`. Both lines
//! overwrite it wholesale — **106 lines against 1988** — so a conflict is
//! guaranteed whenever both write between absorptions.
//!
//! It was resolved by keeping the other line's document **entire** and appending
//! this line's section, attributed. Nothing was dropped. **This file asserts that
//! shape survives**, so a future increment overwriting the file wholesale fails
//! here rather than being discovered by the next absorption.
//!
//! # The stale commitment it uncovered
//!
//! This line's handoff said *"I touch **none** of `REVERSE_PROMPT.md`,
//! `DESIGN_JOURNAL.md`, or `TASKLOG.md`."* This line has written two of the three
//! continuously — eight of the last twelve commits touching the journal are this
//! line's. **A commitment recorded as kept while being broken every increment**,
//! which is a fifth record outliving its subject in a file that already
//! catalogues four.
//!
//! # ⚠ WHAT THIS FILE DELIBERATELY DOES NOT DO
//!
//! **It does not decide where this line's reports belong.** That is the
//! operator's call; the question is asked in the file itself and answering it
//! unilaterally would be worse than leaving it open.
//!
//! **It does not key on the other line's wording.** Their headings are theirs to
//! change, and a guard that fails when they edit their own channel would be this
//! line imposing a format on them. It keys on THIS line's marker plus a
//! structural property: substantial content precedes the addendum.
//!
//! **It makes no claim about `DESIGN_JOURNAL.md`.** Both lines prepend there at
//! the same anchor, which is the classic conflict shape, and it auto-merged at
//! absorption 60 by luck of position. **It has not conflicted.** Predicted is not
//! observed, and this session has twice put a figure in prose that measurement
//! did not support.

use std::path::PathBuf;

/// This line's own marker in the shared channel. Owned here, so the assertion
/// does not depend on the other line's prose.
const ADDENDUM_MARKER: &str = "APPENDED BY THE V0.3.X LINE";

fn channel() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("native_codegen has a parent")
        .join("docs/process/REVERSE_PROMPT.md");
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn the_shared_channel_keeps_both_lines_material() {
    let s = channel();
    // A broken probe fails rather than passes: an empty or missing file would
    // otherwise satisfy every "does not contain" check trivially.
    assert!(
        s.len() > 4096,
        "the shared channel is {} bytes, far shorter than either line's document. \
         Something replaced it rather than appending to it.",
        s.len()
    );

    let at = s.find(ADDENDUM_MARKER).unwrap_or_else(|| {
        panic!(
            "this line's section is gone from the shared channel. It carries the \
             open reports to the `v0.2.3` line; removing it to simplify a merge \
             loses the communication the file exists for."
        )
    });

    // **The structural property, keyed on nothing the other line owns.** Their
    // document precedes this line's addendum. If this line ever overwrites the
    // file wholesale again, the addendum lands at the top and this fires.
    assert!(
        at > 8192,
        "only {at} bytes precede this line's addendum. The other line's document \
         should come first and entire — at absorption 60 it was 1988 lines against \
         this line's 106. A wholesale overwrite is what caused that conflict; \
         append instead."
    );

    // The reports must still be reachable, since `outstanding_reports.rs` watches
    // their subjects and this is where they are addressed.
    for report in ["report 4", "report 5", "reports 4 and 5", "Fixed % Fixed"] {
        if s.contains(report) {
            return;
        }
    }
    panic!(
        "this line's addendum no longer mentions any of its open reports. They are \
         questions the other line has not answered; dropping them loses the \
         communication rather than closing it."
    );
}
