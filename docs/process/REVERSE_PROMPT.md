# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-14 (session 43, continued)

## Where things stand

| | |
|---|---|
| `v0.2.3` | pushed, tree clean, in sync with origin |
| PRs merged this session | **#54, #57, #58, #59, #60**, each 22 of 22 green, merged at the commit CI ran |
| Open PRs of this line | **none** |
| `selfhost_wire` | **154 tests** |
| Record-shape coverage | **17 of 17**, pinned by a test rather than by this note |

## The three-part goal, and where each stands

**C. Emitter record-shape coverage — DONE.** Measured by instrumenting every emit command across the
whole suite: sixteen of seventeen shapes emitted with at least one record, `STRUCT_TEMPLATES` under
none. The gap was a **missing capability**, not a weak assertion: no decoder and no dispatch arm, so
the emitter refused the kind with `-222`. Closed from real compiler output, with a targeted must-fire.

**A. Drive the emitter from the pipeline — PARTLY DONE, and the remainder is the large half.**

| | |
|---|---|
| dedup-scan contradiction settled and recorded | done |
| module-input encoding defined | done |
| producer: chunk names | done (#58) |
| producer: enum layouts, both intern modes | done (#59) |
| producer: the constant walk's names, interned inline | **not started** |
| per-chunk ranges | **not started** |
| `wire.kel` removed from the `read_stage` exclusion | **not started** |
| residency staging for a stage's 395,804 names | **not started** |

The plan is explicit that the last two are **the same increment** and that doing either alone is
wasted.

**B. Self-hosted type rejection — PLAN MERGED, NOTHING BUILT.** Six slices over the fifteen shapes.

## What the measurements changed

**A grep would have reported this closed.** All seven previously-empty kinds appear in the test file,
seven hits out of seven, because a kind can be named in a stride table or a negative test without any
record of that shape ever being written. The instrumented count is the only thing that answers it.

**All six formerly-empty shapes are reachable from real compiled modules**, including `STRUCT_AUX`
and `ENUM_AUX` via `const data`. The wire-format plan expected hand-built artifacts to be necessary.
They are not, and real sources are the stronger oracle.

**The two dedup scans are different scans.** `intern_run` is batch-local and capped at 256, where a
1024-slot table costs 1024 probes against roughly 256 comparisons, because a total language has no
early exit — do not replace it. The walk-nested scan through `NAMES` is the one the 782-second lesson
bears on, and it is to be measured at stage scale. The roadmap cell was stale and now points at the
settlement.

## The mistake worth not repeating

**A push reported success and did not push.** The gate ran, printed "all checks passed", and the ref
was never created. `git ls-remote` caught it. The output had been truncated with `tail -3`, which cut
the line that would have said so. That is the truncation rule in a new place: not a verification
whose result I meant to quote, but a command whose **effect** I meant to rely on. **Verify the ref,
not the gate.**

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **`v0.2.3-prerebase-backup`**, local only, a deliberate pre-rebase safety copy.
- **`MAX_PARSE_DEPTH` on a small stack.** Unchanged. Related and new: `emit_at` is now at eighteen
  arms, the measured ceiling for that shape in the test harness. A nineteenth needs the chain
  restructured, not extended.
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Next intended step

**The constant walk's interner coupling**, which is the next contributor to the interning sequence
and the last one before per-chunk ranges. Then the input-encoding question the type-checker plan
shares: **neither line should invent a second encoding**, and the checker must not be built before
its input encoding exists.

## Parallel development

`v0.3.0` carries native code generation. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries. **Tell the two lines
apart by BASE BRANCH, not by author.**

## Method rules this session paid for

- **Instrument, do not grep, when the question is "does anything ever do X".** Seven hits out of
  seven meant nothing.
- **Verify the ref after a push, not the gate output.** A gate can pass on a push that did not land.
- **Write the encoding down before relying on it.** The enum count would have read correctly from
  zero-filled memory whether or not the encoder wrote it.
- **A guard refusing loudly is the guard working.** `-99` on an unregistered command, `-222` on an
  unhandled kind: both surfaced real gaps as refusals rather than as wrong artifacts.
