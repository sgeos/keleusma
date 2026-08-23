# Mailbox of the proof line `proofs`

> **Navigation**: [Handoffs](./README.md) | [Process](../README.md)

**Branch.** `proofs`, the proof line's top-level branch, cut at `e9a40e32` on the `v0.2.3`
lineage, which includes the evidence index and its guard. Individual proofs are cut from this
branch as feature branches and merged back with no-fast-forward merges, per the operator ruling
of 2026-08-23. This branch merges ultimately into the V0.2.X line, and the V0.2.X line is then
merged into the V0.3.X line. This file is the line's mailbox and lives on this branch, never on a
feature branch. Read it with `git show origin/proofs:docs/process/handoffs/proofs.md`.

**The name `docs/proof-evidence-index` belongs to the V0.2.X line** and carries their #259.

## Structure of the line

| branch | role | state |
|---|---|---|
| `proofs` | top-level integration branch | this mailbox only, so far |
| `proof/composite-region-reuse` | first proof, cut at `e9a40e32` | drafted, see below |

## Proofs in flight

**`proof/composite-region-reuse`** carries `docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md`, written
against the obligation at `a49555bb` on `v0.3.0`. Per operator direction of 2026-08-23 the
document is structured as a general theory over an abstract epoch-guarded bump-arena machine,
with every Keleusma-specific fact, premise instantiation, and provenance row in appendices at the
end. Proved generally are the unconditional branch bound, cross-epoch arm overlap, confined-site
slot reuse, the composed plan, and, by operator-directed scope expansion later the same day,
**Theorem B2**, universal slot reuse under an escape-copy discipline, with an accounting
corollary and a per-site hybrid corollary. The Keleusma instantiation discharges the axioms with
measured standing recorded per row, and two axioms rest on reference-compiler emission
invariants that `verify()` does not enforce, so the reuse theorems apply to reference-compiled
modules and not to arbitrary verified bytecode. B2 is additionally a proved specification rather
than a description, since no escape route copies in Keleusma today, and its six adoption
obligations are named in the proof's Appendix C. M1's immutability clause is confirmed by the
V0.2.X session on four independent grounds, one pinned from the `Op` enum, with the clause
scoped to the ephemeral region deliberately since the persistent region is mutated in place. No
instantiation row is open. The write-accessor pin landed in their `a288ae26`, recorded in the
proof's Appendix E against ground one only, since grounds two through four are read from
dispatch and in no test.

## Owed by this line

Nothing to either peer line.

## Owed to this line

Nothing. The V0.2.X line's pin commit `435a8f6d` landed and is recorded in the proof's
Appendix E with the two files' differing standings kept distinct, an invariant pin that re-runs
every build and a gap pin that fails deliberately if the gap is closed.

## Process note

This line does not prepend to `TASKLOG.md`, `REVERSE_PROMPT.md`, or `DESIGN_JOURNAL.md`, which
are the V0.2.X session's live channels and conflict by construction under parallel prepends.
This mailbox is the proof line's channel, per `PARALLEL_DEVELOPMENT.md`. The proof-line session
operates in an isolated worktree and does not touch the main checkout, which belongs to the
V0.2.X session.
