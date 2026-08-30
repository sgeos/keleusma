# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-30 (session 58) — the line is audited, the string ruling is received, and the
next increment is the string ABI

## THE STRING RULING IS RECEIVED, AND WHAT CHANGED IS ITS STANDING, NOT ITS CONTENT

You confirmed in session that the string ABI ruling — Option B, make the two embeddings agree —
binds this line. That converts it from a claim read off `origin/v0.3.0` into a ruling of record
here. The receipt, the provenance discipline that held it back until now, the verified technical
claim beneath it, and the scope of the change are recorded in
[`../decisions/STRING_ABI_OPTION_B.md`](../decisions/STRING_ABI_OPTION_B.md). The implementing
increment is queued FIRST. It is an embedder-visible change to the marshalling boundary in
`src/marshall.rs`, and per the roadmap's native ABI item it owes a specification of the agreed
contract alongside the code.

You also asked whether the proof work is merged. Verified against the tree rather than the record:
merge commit `8414a1a1` (pull request #303) is an ancestor of `origin/v0.2.3`, `docs/proofs/`
carries the proof and its three audit rounds, and the merged text is byte-unchanged from the
audited commit `f779be7d`. The standing caveat travels with the answer: this line verified the
proof's premises, not its mathematics.

## SESSION 57'S LAST INCREMENT WAS STRANDED, AND IT IS NOW MERGED

The canary-guidance increment was committed on an unpushed feature branch when session 57 ended.
It was found at resumption, pushed through the gate, opened as pull request #328 with the CI run
counted rather than assumed, and merged at the CI-verified commit on 22 of 22 green. The merge
count on `origin/v0.2.3` is 178 at this writing; derive it rather than trusting this sentence.

## THE AUDIT, AND THE THREE FINDINGS THAT NEED AN OWNER

The full handoff validity block passed, every pin matching. CI was left to verify what CI verifies.
What the audit found that CI cannot see:

1. **49 merged local branches pruned**, safe-delete only, manifest with head hashes at
   `tmp/branch-prune-manifest-20260830.txt`. Recommended but not taken: pruning the 97 merged
   branches on origin, because deleting remote refs is outward-facing and the other line rebases
   from origin. That is yours or theirs to authorize.
2. **`feat/native-coverage-spike` holds 29 commits that exist on neither origin branch.** The
   unverified hypothesis is pre-rebase duplicates of the other line's work relanded under new
   hashes. It is their branch to confirm and dispose of; flagged, not touched.
3. **`docs/decisions/BACKLOG.md` line 1815 says the opcode count is 69** against the actual 66. The
   document is an implementation history and the claim was true when written, but the line is
   undated. Cosmetic; correct it or leave it dated, your call.

Appendix B hygiene is clean: one tracked-file match, and it is the engineering-property class the
tracked documents are permitted to carry.

## THE QUEUE, IN ORDER

1. **The string ABI increment** (ruled, binding, scoped in the decision document).
2. **The region-kind wiring**, scouted at resumption: the six skipped kinds have their formatters
   already dispatched in the stage, so `SHARED_LAYOUT` and `DATA_INIT`, the two carrying no name
   index, are driver-only work with no byte-identity perturbation. The other four wait on the
   undriven `intern_index_of` route.
3. The expression-kind extraction family remains exhausted pending your call, since every remaining
   kind needs a stage change that perturbs the byte-identity oracle. The two-pass parser for the
   twelfth stage likewise remains yours to call.

## QUESTIONS THAT REMAIN YOURS

The standing ones, unchanged: whether a shipped example should demonstrate `Byte`; whether
`01_arithmetic.kel` should be enriched; the two-pass parser; publication, which remains held.
New from the audit: whether to prune the merged branches on origin.

## ON THE ANTICIPATED MODEL HANDOFF

A handoff to a different model for routine work is anticipated at a boundary of your choosing. The
channels are already written for it: every load-bearing figure in them is derived rather than
asserted, the handoff's validity block is executable, and the queue above is ordered with its
blockers named. Nothing in the process depends on which model resumes.
