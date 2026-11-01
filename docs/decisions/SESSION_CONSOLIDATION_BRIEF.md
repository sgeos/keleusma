# BRIEF — consolidate, verify, and say what is left

## Why this rather than more work

**The measurable work this line owns alone is done.** Backlog zero. Nine
increments landed and gate-verified. Two leads chased this iteration produced
nothing: the declared features are both covered by the gate, and the one test that
can skip on a missing toolchain is already named by `skippable_tests.rs` and does
not skip on this machine. **A negative result worth the minute it took, and not
worth an increment.**

What remains is either blocked on other people — five reproductions and one
disclosure with the `v0.2.3` line, and the unresolved question of where this line's
outgoing reports should live — or was deliberately declined with the reasoning
recorded.

**Inventing work to justify motion is the failure available here.** The honest
increment is to leave the line resumable.

## What consolidation means concretely

1. **The handoff describes the tree a reader will find.** Four increments landed
   after its last session block: absorption 63, the stage arena measurement,
   report six, and the optimiser work. A reader following the protocol must not
   meet a block that stops before the newest work.
2. **The ancestry block gains an anchor for this session**, so a future validity
   run covers it. Anchor on a commit, never on a branch tip — that mistake is
   recorded in the block itself.
3. **Every figure re-derived**, not copied. The state table has a guard; run it
   rather than trusting the last edit.
4. **The whole ancestry block RUN**, not read. Its own instruction says a count
   written in prose disagreed with the block twice.

## Prior failures to avoid

- **Refreshing the stamp without re-deriving the table.** That drifted for five
  consecutive increments and is why `handoff_figures.rs` exists.
- **Adding a seventh section titled "RESUME HERE".** The file once had seven, with
  the one labelled *LAST* above the one labelled *LATEST*. New work goes in the
  existing newest block or replaces it; it does not add a rival.
- **Claiming completeness.** The right closing statement names what is unfinished
  and who it is blocked on, not that everything is done.
- **A commit message describing work the tree does not contain.** Twice this
  session, both times from a guarded edit chained to an unguarded commit with
  `&&`. Verify the edit landed before writing the message that claims it.

## What must be true at the end

The next session, following the startup protocol, reaches a valid handoff whose
newest block describes the newest work, runs the ancestry block clean, and can see
in one place what is blocked on whom.
