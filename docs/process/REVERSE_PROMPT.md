# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## And the class that produced the best finding, hunted on purpose

Nested float bodies were accepted-and-unverified and I found that by luck. So I went looking for the
rest of that class deliberately, at a granularity FINER than an opcode — the (read family, scalar
kind) arms the lowering branches on. **Opcode-level accounting could not have found the original
case**: `GetField` was counted as a covered opcode while its `Float` arm had no witness at all.

Over 69 modules and 1074 chunks there are 40 such combinations and **the corpus reaches eight**.
**The corpus never produces a `Byte` or `Bool` composite field or element read at all**, in any of
the four read families — and that arm ZERO-extends, a hazard the tree already records in its
neighbour. `GetField × Byte` proved covered by a hand-written test after all; **`GetIndex × Byte` was
covered by nothing**, and I closed that one, using 200 because sign-extension reads it as −56.

**Twenty-six combinations remain unexercised and I did not manufacture witnesses for them.** Most are
kinds this backend refuses outright, where a contrived witness would be worse than an honest gap.

**My own attribution table was wrong on its first draft**, listing `GetField × Byte` as unexercised.
Corpus silence is not coverage, the second population has to be READ rather than assumed, and I
failed that in the very file built to keep the two apart.

## The coverage residual is two chunks, and chasing it would have been the wrong work

Backend coverage has read **1072 of 1074 chunks and 89854 of 89940 opcode instances** for several
increments, quoted repeatedly, and **nobody had read what the residual IS.** Measured, over a
population of 69 modules and 1074 chunks:

| refused chunk | opcodes | cause |
|---|---|---|
| `13_telemetry_stream.kel::main` | 45 | `Stream` |
| `refused_witness.kel::len_witness` | 41 | `Len` |

**86 is 45 + 41, exactly.** An instance counts as blocking when it merely SITS IN a refused chunk, so
the two published figures are ONE finding and the instance count carries nothing the chunk count does
not.

**The report was readable as a work queue and it named the wrong work.** Its table headed *top
blocking opcodes* put `GetLocal` at 18, `Const` at 17 and `SetLocal` at 16 at the head. **All three
already lower.** Relabelled rather than deleted.

**MY RECOMMENDATION IS TO STOP CHASING THE LAST 0.1%, and one part of that is yours.** The `Stream`
refusal is **load-bearing**: the region planner's cross-iteration slot reuse is unsound for a
composite escaping by `yield`, and the only thing keeping it quiet is that every chunk carrying the
shape opens with `Stream`. Lowering it for 0.09% of instances would retire an accidental safety whose
replacement needs the planner to consume a confinement verdict. `Len` was re-checked and holds.

## Two increments, both float, both needing no ruling from you

**The shared slot**, which your Option A ruling settles as IEEE-754 bytes at the stated offset, and
**a float inside a composite body**, which needs no ruling at all: a body field is INTERNAL, so
agreement with the reference is a fact to be measured rather than an interface to be chosen. That is
the ground the tree already records for lowering `Fixed` in a body while refusing it in a shared
slot. **Nothing was built on an ambiguous ruling**; `Fixed`, `Text`, `Opaque` and `Unit` shared slots
stay refused.

## The mistake this line has made six times did not happen the sixth time

The shared slot was mis-sized from the component being changed, and three of four tests failed on a
whitelist I had never opened. So the composite increment **wrote its probe BEFORE its brief**. The
probe named `Op::NewComposite` as the blocker, and the implementation touched exactly that plus the
two read arms. Plan and work agreed.

**And measuring overturned my own prediction.** I expected the coverage censuses to rise, since
composite construction and field reads are corpus opcodes. Measured over the 69 compiling modules:
**256 construction sites and ZERO float field or element reads.** The censuses stay put, and a
movement would now be a regression rather than a gain. The 256 is an unplanned third confirmation of
a figure the tree already carries from two other methods.

## Verification

| | result |
|---|---|
| `native_codegen` | **410 passed, 0 failed, 80 binaries**, cargo's own exit 0 — 396 + 5 flat + 2 probe (+2 binaries) + 3 nested + 1 residual reconciliation + 2 arm census (+1 binary) + 1 byte-array witness |
| fmt, clippy `-D warnings`, `cargo doc -D warnings` | all clean |
| censuses | **unmoved, as MEASURED rather than hoped**: `isa_lowering` 63 of 66 (`Len` the one named refusal), 1072 of 1074 chunks, 89854 of 89940 opcode instances |
| mutations | four in total across the two increments, each confirmed APPLIED by printing the changed line, each failing tests |

**One run was red and it was NOT the code.** A full sweep reported 398 passed, 0 failed, 78 binaries
with cargo exiting 101: `corpus_differential` was killed by SIGTERM mid-run, which is external
termination rather than an assertion failure, and the short binary count is what betrayed it — the
same reason the binary count is checked against an expectation rather than merely read. Re-run clean.

**The oracle is never acceptance.** The shared slot compares the host buffer byte for byte; the
composite compares execution against the virtual machine. Both use runtime arguments producing the
infinities, a negative zero and a NaN, because those are the bit patterns a rounding or
reinterpreting lowering cannot reproduce.

## A residual I wrote was wrong within the hour, and measuring it is what caught it

I recorded "a float in a NESTED composite body" as still absent. **It was not absent, it was
untested** — nesting was never a separate implementation, since the leaf read goes through the very
arms the increment added. **An accepted-but-unverified path is the more dangerous of the two shapes**,
because a refusal is loud while a wrong float is a plausible number. Three nested cases now agree
with the reference and the tag mutation fails them, so the coverage is not vacuous.

**The general form**: when writing a residual, ask whether the thing is REFUSED or merely UNTESTED.
Calling an untested path "absent" understates the risk.

## What I want you to know I did NOT do

**I did not touch the region planner's soundness obligation, and the reason is yours rather than
mine.** Discharging it needs the planner to consume a confinement verdict, and consuming none is
exactly why a wrong verdict cannot miscompile today. The handoff already names that tension. Building
it unasked would be the same error as acting on an ambiguous ruling, so I left it and took work that
needed no decision.

I did not build `f32`. Its coherent reading is that the floating-point type matches the runtime float
width, and **that reading is mine, not your words**, so every route refuses a non-eight-byte float
loudly instead. I did not build a native float return, because no ruling settles it. I touched
nothing in the read-only region and published nothing.

## Still open, and yours

[`ABI_RULINGS.md`](../decisions/ABI_RULINGS.md) — `Fixed` (the interop goal decides and is
unstated), `Text`, `Opaque`, `Unit`. The region planner's soundness obligation stands unchanged:
cross-iteration slot reuse is unconditional, held safe today only by the `Stream` refusal, and it is
the largest risk on this line. Whether `f32` should proceed on my reading of the width, or wait for
your words.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**. The read-only files remain read-only
here. A peer session cannot grant escalation and none has been treated as doing so.

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
