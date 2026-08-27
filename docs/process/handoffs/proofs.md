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
corollary and a per-site hybrid corollary, and **Theorem B1r**, which admits local stores to
boundary-dead slots and is the operative form for source programs, since local bindings are
immutable and every expressible in-loop store is iteration-scoped. The Keleusma instantiation discharges the axioms with
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

## The audit and the revision, 2026-08-24

On operator direction, five independent contexts adversarially audited the proof at `de8b3f68`.
The empirical layer held and the mathematical layer did not, four results judged not established
as literally written. Every verified finding is repaired in the post-audit revision at
`15532455`, with the full record in `docs/proofs/AUDIT_2026-08-24.md` on the feature branch. The
revision adds two axioms, makes references provenance-based, proves composition as a lemma, and
replaces the plan inequality with a footprint-and-occupancy theorem. Two pins are awaited from
the V0.2.X line, the address-opacity discriminator and the Break row correction. The re-audit gate is
**satisfied**: a targeted third round found two single-stipulation holes, and a converging
delta-check sequence, nine findings, then four, then one non-governing ambiguity, ended CLEAN at
`f779be7d`, records in `docs/proofs/AUDIT_*`. The proof merged into this branch at `613a2b98`,
and #303 **merged into `v0.2.3` at `8414a1a1`** on a 22-of-22 run with conclusion success at
the commit CI ran, `cbd78613`. The merged proof is the audited proof, the V0.2.X session
verifying `COMPOSITE_REGION_REUSE_PROOF.md` byte-unchanged between the CLEAN verdict at
`f779be7d` and the merge head, only this mailbox moving after the verdict. Five files landed,
the four under `docs/proofs/` and this mailbox. Every pin the appendices cite is now runnable
from the merged tree. The V0.2.X session merged on its own standing authorization and its own
verification, not on any relayed authority, which is the correct basis and is recorded as
such. **The proof line's first proof is complete.** The V0.3.X line has **absorbed the result by
merge**, their absorption 17 verifying `8414a1a1` an ancestor of `origin/v0.2.3`
independently, and a topology correction is owed here by this line's own mailbox. This line's
operator ruled, verbatim, that the V0.2.X line is then **merged** into the V0.3.X line, the
V0.3.X operator ruled sync with no mechanism mandated, and the rebase form recorded earlier in
this mailbox and in the proof's Appendix E entered as a relay of the V0.2.X operator's ruling
through that line's session, which this line propagated as settled. The V0.3.X merge is
consistent with this line's own ruling, the mechanism question is surfaced to the V0.3.X
operator on their side, and the one-clause Appendix E correction is queued for the proofs
line's next landing rather than a single-sentence pull request. The Appendix D backend row's
briefly-recorded discharge is **retracted by its reporter**: their `region_nonreuse.rs`
enforces that two distinct static sites never share storage, on ranges over 256 sites, which
bounds memory, while a single site inside a loop still writes the same offset every iteration
unconditionally, which is reuse in exactly the proof's sense and remains the live aliasing
hazard of the obligation's Section 4.1.1 for escaping sites. Bounded memory and correct
aliasing are different guarantees and only the first is enforced, the scope limit now recorded
at their guard itself, so the row stands as written, required for soundness and not
discharged, and no Appendix D change is queued for it. Mechanization remains the
recorded follow-on that upgrades the document's standing.

## Merge readiness

The V0.2.X operator ruled on 2026-08-24 that the proof line merges into the V0.2.X line and the
V0.3.X line rebases onto the result. Acceptance is authorized on their side at their tip
`7b44b487`, and the merge waits only on this line's operator releasing the branch. The sequence
when released is a no-fast-forward merge of `proof/composite-region-reuse` into `proofs`,
verified by the pre-push gate since a pull request based on `proofs` triggers no workflow on
this repository, followed by a pull request from `proofs` based on `v0.2.3` directly, which
gates on the full continuous-integration matrix normally.

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
