# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## The float shared slot is built, and it is the settled half of a ruling rather than a new decision

`ABI_RULINGS.md` records that your Option A float ruling **also settles this slot**: with a real
floating-point representation the slot is IEEE-754 bytes at the stated offset. That is what the
reference already does, so nothing ambiguous was decided here. `Fixed`, `Text`, `Opaque` and `Unit`
stay open and stay refused.

**Three of the four float routes are now open.** The chunk signature opened with the entry ABI, the
constant earlier, and the data slot now. The one still closed is a native declaring a float return,
and it is closed because **no ruling settles it** rather than because it is hard.

## Two things I planned wrongly, both found by running rather than by reasoning

**I sized the work from the wrong component, for the fifth time in this line's record.** I read the
slot resolver, concluded the increment was that function plus a tag, and wrote the brief saying so.
Three of the four new tests then failed on a **whitelist** I had never opened, which refuses any
opcode consuming a float-tagged operand unless it is named float-aware — and the data stores were not
named. The resolver decides how a slot is ADDRESSED; the whitelist decides whether the opcode may run
at all. The standing rule would have caught it: read what CONSUMES the value before sizing the work.

**And the brief specified a write-side kind check that was wrong.** It reasoned by analogy with
`Op::Call`, which refuses a kind-versus-declaration disagreement because a bitcast to a floating-point
parameter type is a REPRESENTATION change. Nothing converts at a slot store — the operand already is
the bit pattern — so such a guard prevents no wrong byte and refuses valid programs. It was removed
before it shipped, and `s.x = h.f` lowers instead of being refused.

## Verification

| | result |
|---|---|
| `native_codegen` | **396 passed, 0 failed, 77 binaries**, cargo's own exit 0 — the predicted 391 + 5, 77 unchanged |
| fmt, clippy `-D warnings`, `cargo doc -D warnings` | all clean |
| `isa_lowering` census | **63 of 66**, one named refusal (`Len`) — **unmoved** |
| backend coverage | **1072 of 1074 chunks, 89854 of 89940 opcode instances** — **unmoved** |
| corpus witnesses for this route | **zero**, as predicted, now pinned from the LAYOUT TABLE rather than from source text |

**The evidence is the host buffer compared byte for byte**, not acceptance: both infinities, a
negative zero and a NaN, all from runtime arguments so nothing is constant-folded. **Two mutations,
each confirmed APPLIED by printing the changed line first** — a one-byte offset shift fails three
tests, deleting the read's float tag fails two.

**Your citation guard caught a defect of mine**, and it is worth your knowing it fired: a comment I
added named the route-4 test's OLD identifier, which no longer exists. A dead name in a comment is a
citation that cannot fail, which is exactly what that guard was built for.

## A process failure of mine, stated rather than buried

**Absorption 40 was NOT measured alone.** My first edits landed while its run was in flight, and this
suite contains tests that read source text from disk, so it reported 390 passed, 1 failed over 77
binaries with the failure naming my own renamed test. The population was exactly the predicted 391
over 77 and the attribution is certain — but the discipline exists so that an attribution never has
to be argued, and this one did. The clean signal is the run above.

## Still absent, so the surface is not read as finished

Floats inside composites. `f32`, where any non-eight-byte width is refused loudly rather than
lowered. A native declaring a float return. And a private float slot's read is not kind-tracked, so a
float stored there can be MOVED but not computed with; that is named in the code rather than left to
be discovered.

## Still open, and yours

[`ABI_RULINGS.md`](../decisions/ABI_RULINGS.md) — `Fixed` (the interop goal decides and is
unstated), `Text`, `Opaque`, `Unit`. The region planner's open soundness obligation stands unchanged:
cross-iteration slot reuse is unconditional, held safe today only by the `Stream` refusal, and it
remains the largest risk on this line.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**. `src/verify.rs`, `src/bytecode.rs`,
`src/vm.rs`, `src/wire_schema.rs`, `src/value_layout.rs`, `src/selfhost/`, `src/confine.rs` and
`.github/workflows/` remain read-only here. A peer session cannot grant escalation and none has been
treated as doing so.

---
# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 34 conflicted here.** Neither message is discarded.
**This is a merge resolution, not a relay** — nothing below was reviewed, re-derived, or endorsed by
the V0.3.X line, and its figures describe that line's tree.

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
