# BRIEF — the outstanding reports, made self-verifying

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## The class, established twice in two increments

1. `NATIVE_BOUNDS_TRANSFER.md` asserted the reference's memory bound was unsound for **four weeks
   after it was repaired**.
2. `REVERSE_PROMPT.md` says, right now, **"STILL WITH YOU, NONE ACTED ON"** about two defects reported
   to the `v0.2.3` line.

**The second claim rests on recollection.** It was last checked by scanning absorption commit
messages; absorptions 57 and 58 have brought thirty-six commits since. If either has been repaired,
this line is repeating the exact error it just spent an increment correcting — in a document written
*to* the other line rather than about them.

## What is already right, and is the model

The `confine.rs` report is **self-verifying by construction**. `lowering_robustness.rs` allows the
panic by ORIGIN FILE and asserts it STILL FIRES, so a repair upstream turns my suite red and forces
the carve-out to be deleted rather than outlive the defect. **That is the pattern**, and it was
written before the lesson that justifies it.

The other reports have no such guard. They are prose in a channel document.

## The wrong turns

1. **Do not assert a repair from a green suite.** A guard that fires proves the defect is open; a
   guard that does not exist proves nothing either way. Absence of failure is not evidence here.
2. **Do not build a guard that fails when the other line is RIGHT.** The point is to detect a repair,
   not to punish one. The failure message must say "they fixed it, retract the report", not "a test
   broke".
3. **Do not re-report what is already reported.** This line's job is to know the status, not to file
   again. A guard that fires should send me to the reverse prompt, not to their tracker.
4. **Do not guard a question as though it were a defect.** One outstanding item is a QUESTION — should
   a multi-parameter stream compile at all — and a question has no reproduction to watch. What can be
   watched is the behaviour it is about.
5. **Do not touch the repository-root `src/` or `tests/`.** The ownership boundary is checked on every
   absorption and is not a suggestion.

## What done looks like

Every outstanding report's current status is established by measurement rather than by memory; each
one that can be watched has something in the tree that fails when it is repaired, with a message
saying to retract rather than to debug; and the channel document states status with its date rather
than an unqualified "none acted on".
