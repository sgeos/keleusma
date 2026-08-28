# BRIEF — checking the proof's claims ABOUT this line against this tree

**Written**: 2026-08-27, twentieth loop iteration. **For this line's own use.**

## Why

`COMPOSITE_REGION_REUSE_PROOF.md` was absorbed last iteration. It is 703 lines and **assigns
obligations to the V0.3.X line by name**. This session's most reliable habit has been to **verify
claims about this line rather than accept them**, and that applies with more force to a document
written by another line about this one.

**Two phrasings look factually wrong about this backend:**

1. Appendix D: *"backend **stops** reusing slots of unconfined or unseparated sites"* — phrased as a
   transition, implying the backend currently reuses.
2. Appendix B item 7: *"**the backend's unconditional reuse** remaining unsound on such sites"* —
   phrased as though such reuse exists here.

**`plan_chunk_region` has never reused.** It assigns one fixed offset per static site; its own
documentation says so; `region_nonreuse.rs` now fails if it starts, over 256 sites in 35 chunks.

**This matters beyond pedantry.** A reader of the proof concludes the V0.3.X backend carries an
unsoundness it does not have, and that an obligation is outstanding which is already discharged **and
enforced**.

## The second and more interesting question

Item 7 says the counterexample's site is **unconfined**, and unconditional reuse would be unsound
there. **This backend does not reuse — but the max-over-arms remedy this line recorded IS a form of
reuse**: overlapping exclusive arms means two sites sharing storage.

**So: does the counterexample apply to the sites max-over-arms would overlap?** If the exceeding
modules' sites are unconfined — and `confinement_vs_arena_gap` measured that **33 of 36 escape** —
then the counterexample may land directly on the remedy rather than beside it.

**That would be a materially stronger result than the comparison remark**, which only removed
"obviously beneficial". This could remove "sound".

## Prior failures this is exposed to

1. **Reading a charitable phrasing as an error.** Item 7 sits in a *"what this does NOT establish"*
   list; "the backend's unconditional reuse" may mean the proposed one. **State what is certainly
   wrong and what is merely ambiguous, separately.**
2. **Overclaiming a hit.** Whether the counterexample applies depends on confinement of the specific
   sites, and this line's own escape figures are per module, not per site-pair.
3. **Correcting another line's document in this tree.** The proof is theirs. **Record the finding on
   this line and send it; do not edit their proof.**
4. **A vacuous check.** Fourteen guards or filters broke this session.
5. **Reporting a figure without the command that produces it.**
6. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not edit `docs/proofs/`.** It is the other line's artifact, absorbed. A disagreement is a
  message plus a note on this line, not a rewrite of their document.
- **Do not claim the proof is wrong where it is only ambiguous.**
- **Do not conclude the remedy is unsound without checking the confinement of the sites it would
  actually overlap.** "33 of 36 escape" is a module-level figure and the remedy pairs specific sites.
- **Do not treat the peer's summary as the proof.** Read the document.
