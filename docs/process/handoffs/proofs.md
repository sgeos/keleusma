# Mailbox and Handoff of the Proof Line `proofs`

> **Navigation**: [Handoffs](./README.md) | [Process](../README.md)

The proof line's mailbox and its self-contained resume prompt in one file, following the
per-branch practice of `PARALLEL_DEVELOPMENT.md`. Read it with
`git show origin/proofs:docs/process/handoffs/proofs.md`.

> **REFRESHED 2026-08-29, second stamp, describing the state at `391f0b51` with only handoff
> commits atop it.** Every validity check below was re-run and passed at refresh time, and both
> peer mailboxes were swept, the V0.3.X mailbox independently recording the topology correction
> and the backend-row retraction consistently with this file. The line's first proof is
> complete and landed on both peer lines. Nothing is in flight, nothing is owed to or by either
> peer, no loop, cron, or monitor is armed, and every open item is an operator decision. A
> resuming session should validate below, read the three audit records, and then wait for the
> operator.

## Validity

Validate by ancestry and by content, never by a hash match.

```sh
# Ancestry. All three must succeed.
git merge-base --is-ancestor 8414a1a1 origin/v0.2.3     # the proof's landing merge
git merge-base --is-ancestor f779be7d origin/proof/composite-region-reuse   # the CLEAN-verdict tip
git merge-base --is-ancestor 613a2b98 origin/proofs     # the no-ff merge into this line

# Content, measured at 391f0b51. If any differs, say so rather than acting on the state below.
P=docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md
grep -c '^\*\*Definition' $P        # 9
grep -c '^| M[1-9] |' $P            # 18, nine axiom rows and nine instantiation rows
grep -c '^\*\*Lemma' $P             # 5
grep -c '^\*\*Theorem' $P           # 7
grep -c '^\*\*Corollary' $P         # 4
grep -c 'blacksquare' $P            # 15, one per proof block
ls docs/proofs/ | wc -l             # 4, the proof and three AUDIT records
```

## Structure of the line

| branch | role | state |
|---|---|---|
| `proofs` | top-level integration branch | the merged proof plus this file, `391f0b51` and handoff commits |
| `proof/composite-region-reuse` | the first proof's feature branch | complete, tip `f779be7d`, merged |

Cut from the `v0.2.3` lineage at `e9a40e32` per the operator ruling of 2026-08-23. Individual
proofs are cut from `proofs` as feature branches and merged back with no-fast-forward merges.
This line merges into V0.2.X, and each peer operator rules their own absorption mechanism, see
the topology record below. The name `docs/proof-evidence-index` belongs to the V0.2.X line.

## THE STATE

`docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md` discharges the composite-region-reuse obligation
written on the `v0.3.0` line, read at `a49555bb` and corrected there at `d5b706e8` and
`c3ff3c06`. It landed on `v0.2.3` at merge `8414a1a1`, on a 22-of-22 run with conclusion
success at the commit continuous integration ran, and the V0.2.X session verified the merged
proof **byte-identical to the audited proof** at the CLEAN verdict commit `f779be7d`. The
V0.3.X line absorbed the result **by merge**, their absorption 17, verified independently on
their side.

**What is proved, in one paragraph.** Part I is a general theory over an abstract
epoch-guarded bump-arena machine, nine definitions, nine axioms, a shared first-divergence
comparison method, five lemmas including an ordered composition induction, and the results,
Theorem A1 unconditional, Theorem A2 and Corollary A3 for arm overlap, Theorems B1 and B1r for
confined-site reuse with a boundary-dead-slot refinement, Theorem C's footprint-and-occupancy
bound, and Theorem B2 with its corollaries for an escape-copy discipline machine. Part II
instantiates every axiom for Keleusma with measured standing per row. The reuse results are
scoped to **reference-compiled modules whose composites are transitively scalar**, under
exactly two producer emission invariants, streams never return, and iterating loops emit no
value-carrying `Break`, the second resting on the grammar's expressionless `break`. Theorem B2
is a **proved specification only**, the discipline machine not existing in Keleusma.

**How it was verified.** Three adversarial audit rounds by fresh contexts, then a converging
delta-check sequence, nine findings, four, one non-governing ambiguity, ending CLEAN, all
recorded in `docs/proofs/AUDIT_2026-08-24.md`, `AUDIT_2026-08-24_ROUND2.md`, and
`AUDIT_2026-08-26_ROUND3.md`, which travel beside the proof. The standing caveat, disclosed in
the proof's Section 9 item 8, is that the correspondence argument is prose rather than a
mechanized bisimulation. **Every pin the appendices cite is runnable from the merged `v0.2.3`
tree**, the V0.2.X commits `435a8f6d`, `92e5696a`, `a288ae26`, and `f90fe688` all being on it.

## THE TOPOLOGY RECORD, because a relayed ruling was propagated once

Three operators ruled on absorption and their rulings must stay attributed. This line's
operator ruled, verbatim, that the V0.2.X line is then **merged** into the V0.3.X line. The
V0.2.X operator ruled that V0.3.X **rebases**. The V0.3.X operator ruled **sync** with no
mechanism mandated. This line propagated the relayed rebase form into this mailbox and the
proof's Appendix E as though settled, which was an error, corrected here at `26431da4`. The
V0.3.X line absorbed by merge, consistent with this line's own ruling, and the mechanism
question rests with their operator. **Never record a relayed ruling as settled. Name whose
ruling it is and how it arrived.**

## RETRACTIONS AND CORRECTIONS, kept because the causes generalize

- **The Appendix D backend-row discharge is retracted by its reporter.** The V0.3.X
  `region_nonreuse.rs` guard enforces that two distinct static sites never share storage,
  which bounds memory. A single site inside a loop still writes the same offset every
  iteration unconditionally, which is reuse in the proof's sense and remains the live aliasing
  hazard of the obligation's Section 4.1.1 for escaping sites. Two guarantees, one enforced.
  The row stands as written, required for soundness and not discharged.
- **Round two found the round-one repair of Theorem A2 defective in statement form**, the
  proof body patched while the box kept the refuted hypothesis. A repair applied in a proof
  body while the statement is left unchanged is not a repair. The round-one record carries the
  annotation.
- **This session's style scans excluded blockquote lines** and passed three times over a live
  violation. A checker's clean report is evidence about its reach before it is evidence about
  the tree.
- **A directional claim of this line was measured wrong**, that the reachability motivator
  would only strengthen with dispatch scopes included. Measured, it existed only because of
  them.

## QUEUED FOR THE NEXT LANDING

One item only. The proof's Appendix E contains the one-clause relayed-rebase wording, "with
V0.3.X rebasing", to be corrected to the attributed three-ruling form the topology record
above carries, folded into whatever this line lands next rather than a single-sentence pull
request through the full gate.

## OPEN, ALL WITH THE OPERATOR

1. **`src/verify.rs:1079`**, adopting confined-site accounting, explicitly unruled, lowering a
   published worst-case-memory-usage figure. The V0.2.X line's landed `src/confine.rs`
   analysis is assembling the measured consequence, and the proof's Theorem C remark is the
   piece to read first, footprint can exceed the branch-max bound on branch-dominated shapes,
   so adoption should be per-site by comparing both bounds.
2. **B2 adoption**, explicitly unruled, a proved specification with seven named obligations in
   the proof's Appendix C.
3. **Mechanization**, the recorded follow-on that upgrades the proof from audited prose to
   checked proof, its cost much reduced by the axiomatization the audits forced.
4. **The floating-point application binary interface question** in
   `docs/process/REVERSE_PROMPT.md`, predating this line entirely, still awaiting one word.

## GOVERNING RULES A RESUMING SESSION MUST NOT LOSE

- **Work in a worktree.** This session operates in `../keleusma-worktrees/proofs` and
  `../keleusma-worktrees/composite-region-reuse`. The main checkout belongs to the V0.2.X
  session. A shared checkout silently changes what a long-running command measures.
- **A pull request based on anything but `main` or `v*` triggers no continuous integration,
  silently.** Merge on a green run at the commit it ran, reading the conclusion field.
- **Nothing is promoted from read to executed without an execution**, and rows carry their
  provenance labels.
- **Peer surfaces are theirs.** Classification disputes go to the table in
  `tests/composite_escape_routes.rs` on the V0.2.X line, runtime questions to that session,
  backend questions to the V0.3.X session, and authority routes through operators, never
  through peers.
- **The operator's prose style governs all documents of this line**, no contractions, no
  em-dashes, en-dashes, colons, semicolons, or parentheticals in prose, acronyms spelled out
  on first use, and the style scan must cover blockquotes.
- **Adversarial audit before merge**, by fresh contexts scoped by section, with failed attacks
  reported, and repairs closed by a converging delta-check sequence rather than another full
  round.

## Owed by this line

Nothing to either peer line.

## Owed to this line

Nothing.

## WHAT A RESUMING SESSION SHOULD DO FIRST

Run the validity block. Read the three audit records and the proof's Appendix E, which carry
the full provenance. Check both peer mailboxes,
`git show origin/v0.2.3:docs/process/handoffs/v0.2.3.md` and
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`, for anything addressed to this
line. Then wait for the operator. Nothing is blocked, no work is in flight, and the line's
next action is whichever the operator commissions, a second proof cut as a feature branch from
`proofs`, the queued Appendix E correction folded into it, or the mechanization effort.
