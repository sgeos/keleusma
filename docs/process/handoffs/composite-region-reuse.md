# Mailbox — the proof line (`proof/composite-region-reuse`)

> **Navigation**: [Handoffs](./README.md) | [Process](../README.md)

**Branch**: `proof/composite-region-reuse`, a top-level branch cut at `e9a40e32` on the `v0.2.3`
lineage, which includes the evidence index and its guard. Per operator ruling of 2026-08-23 it
merges ultimately into the V0.2.X line, and the V0.2.X line is then merged into the V0.3.X line.
Read this file with
`git show origin/proof/composite-region-reuse:docs/process/handoffs/composite-region-reuse.md`.

**The name `docs/proof-evidence-index` belongs to the V0.2.X line** and carries their open #259.
This line was nearly cut under that name and renamed on their warning before any push.

**Last synced from `v0.3.0`**: `a49555bb`, the obligation document current after their #258.
**Last synced from `v0.2.3`**: `639f970f`, plus `e9a40e32` on this lineage.

## State

`docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md` is drafted on this branch. It proves, against the
obligation stated at `a49555bb` on `v0.3.0`.

- **Theorem A1**, the branch bound, unconditional and proved from confirmed premises.
- **Theorem A2**, arm overlap once per cycle, conditional on P5.
- **Corollary A3** and **Theorem B1**, restricted loop-body slot reuse over the five-route escape
  set with the confinement condition of its Definition 8, conditional on P5 and P6.
- **Corollary C**, the composed plan, the corrected form of the obligation's Section 5.

B2 and instruction-set remedies are analyzed as proposals only, per operator-confirmed scope. The
document states its own limits in its Section 8 and its change-control table in Section 10.
Nothing in it authorizes an implementation change on either line.

**Premise status after the V0.2.X line's measurements of 2026-08-23.** P5 confirmed in a
corrected two-part form, frame clearing at `Op::Reset` plus the stream-never-returns invariant,
the latter a code-generation property pinned in their `tests/stream_never_returns.rs`. P6 clauses
(a) through (c) confirmed, back-edge neutrality on shapes, `Break` join coverage, and callee
unreachability. **P6(d) settled the same day, unfavorably.** `verify()` accepts a below-entry pop
with a same-shape replacement, pinned in their `tests/loop_entry_floor.rs`, and the emission-side
zero over 588 shipped loop instances is a measurement at a commit with the instrumentation
reverted, not a standing guarantee. The conditional theorems therefore hold for reference-compiled
modules and not for arbitrary verified bytecode, and the proof's status header says so. A
structural close, flooring the typed pass at loop entry, is recorded in the proof's Section 10 as
an operator decision on the V0.2.X line, raised on their channel.

## Owed by this line

Nothing to the V0.3.X line. The Theorem A sharpening was communicated to them 2026-08-23 before
the proof was written.

## Owed to this line

- **The commits** carrying `tests/stream_never_returns.rs` and `tests/loop_entry_floor.rs`, to be
  recorded in the proof's Section 11 once their gate is green. Promised by the V0.2.X session.

## Process note

This line does not prepend to `TASKLOG.md`, `REVERSE_PROMPT.md`, or `DESIGN_JOURNAL.md`, which are
the V0.2.X session's live channels and conflict by construction under parallel prepends. This
mailbox is the proof line's channel, per `PARALLEL_DEVELOPMENT.md`. The operator may override.
