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
| `v0.2.3` | pushed, tree clean, in sync, **no open PRs of this line** |
| PRs merged this session | **thirty-four**, each 22 of 22 green, merged at the commit CI ran |
| `selfhost_wire` | 157 tests; `selfhost_typecheck` 7 |
| Record-shape coverage | **17 of 17**, pinned by a test |
| Type rejection | **16 ill-typed rejected, 7 well-typed accepted**, verdict agreement |

## The three-part goal, and where each stands

**1. THE END-TO-END JOIN — DONE.** The producer's sequence now feeds the interner and the emitters in
one VM call, and `NAMES` and `STRING_POOL` come back byte-identical to `encode_aux_body` with the
module blob as the ONLY input describing names. `intern_run` is untouched; `intern_run_preoffset` is
a second function so the sequential path cannot regress.

**2. THE INPUT-PATH CONSOLIDATION — DONE.** All four channels migrated: the stage joins declarations
to call sites, searches struct field sets, classifies name occurrences, and applies the expression
rules. Every superseded collector on the authoritative path is deleted rather than left unused.
Verdict agreement held at sixteen and seven throughout.

**3. `read_stage` AND STAGING — HALF DONE, and the other half was mis-sized by everyone including
me.**

- `read_stage` is DONE. `wire.kel` joined the stage table because the driver can now emit through it
  (`wire_names_via_kel`), not before.
- The DATA-SLOT contributor is DONE. It was missing entirely and is the difference between the
  producer's 252 and the reference's 627 on `parse`.
- **What remains is one ceiling**: `parse` needs 627 names against a hard 512.

## The measurement that overturned the premise

**"A real stage's 395,804 names" describes no name count.** Measured across all ten stages, the
largest `NAMES` region is **627 records**; 395,804 is a REGION record count belonging to `CONSTS`
(34,782 units for `parse`). The figure came from the pre-run-length-encoding state, when
`SHARED_LAYOUT` held one record per array element, and it outlived the representation it described.
**It made a two-and-a-half-times problem look like a fifteen-hundred-times one**, and the design that
framing implied would have been built and not needed.

I put that number in the goal statement myself, from the plan.

## The two things a green suite would otherwise overstate

**The slot-name intern MODE is unverified by the corpus.** A mutation to fresh mode passes every
test, because a slot name is `<block>.<field>` and cannot collide with anything. The claim rests on
reading `add_data_layout`. Recorded in the source as the weakest link.

**The remaining ceiling raise is not two constants.** Every `for .. limit 256` must rise with
`nm_max_names`, and `for .. limit` TRAPS rather than degrades. Verifying it needs `parse`, whose
artifact does not fit the single-window join harness.

## Open, held by the operator

- **Publication remains HELD.**
- **`v0.2.3-prerebase-backup`**, local only.
- **`MAX_PARSE_DEPTH` on a small stack.** Also: `emit_at` is at eighteen arms, the measured harness
  ceiling for that shape.
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- **MSRV**: 1.85 for `keleusma-arena`, 1.88 for `keleusma`.

## Next intended step

**The ceiling, and it needs a harness decision first**: either the join grows a windowed variant, or
the ceiling rises and a new harness carries `parse`. Specified at "WHAT IS LEFT OF THE CEILING" in
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md).

## Method rules this session paid for

- **Instrument, do not grep**, when asking whether anything ever does X.
- **Verify the ref after a push**, not the gate output.
- **Check a figure against the thing it claims to measure.** 395,804 survived three documents.
- **Append to a slot-addressed block, never insert.** Two off-by-one defects came from ignoring it.
- **Say which fact a green suite does NOT establish.** The slot mode is the example.
- **A guard refusing loudly is the guard working**: `-99`, `-222`, `-233` each surfaced a real gap.
