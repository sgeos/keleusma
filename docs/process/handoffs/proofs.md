# Mailbox and Handoff of the Proof Line `proofs`

> **Navigation**: [Handoffs](./README.md) | [Process](../README.md)

The proof line's mailbox and its self-contained resume prompt in one file, following the
per-branch practice of `PARALLEL_DEVELOPMENT.md`. Read it with
`git show origin/proofs:docs/process/handoffs/proofs.md`.

> **THE PROJECT MAY BE VERY DIFFERENT FROM WHAT THIS FILE DESCRIBES.** This file records the
> state on 2026-11-01. This line works in long idle stretches, and the other lines keep moving.
> Before this line's previous resume, `v0.2.3` had moved 1112 commits past this file's prior
> stamp. Treat every commit, line reference, branch, worktree, file path, peer request, and
> open item below as a claim to re-measure, not as current fact. Measure how far `v0.2.3`,
> `v0.3.0`, and `main` have moved since `208df9e9`, read the peer handoffs and the mailbox, and
> report what changed before acting on anything here.

> **REFRESHED 2026-11-01, fourth stamp, describing the line in a holding state after its
> release.** The operator released the branch, a ruling relayed by the V0.2.X line on
> 2026-10-30 and confirmed first-hand in this line's session on 2026-10-31. This line merged
> `origin/v0.2.3` at `364e33fa` into `proofs` at `5921a7c5`, which replaced the five lines in
> four files that matched the kept private term list, scanned to zero at the merged tip, and
> landed the Appendix E correction at `dc1b784b`. The V0.2.X line merged pull request 708 into
> `v0.2.3` as `b24d05c1` on 2026-11-01, on the operator's direction, so every commit of this
> line up to `dc1b784b` is on `v0.2.3`. This stamp's own commit, which refreshes this file and
> exports the interline checkpoint, is held on `proofs` with no pull request, for the reason
> under the hold below. The interline mailbox is this line's message channel, read with
> `interline inbox --line proofs`, and it was empty when this stamp was written.

## Validity

Validate by ancestry and by content, never by a hash match.

```sh
# Ancestry. All four must succeed. They detect a reset, not staleness, so also measure
# how far v0.2.3 has moved past 208df9e9, as the warning at the top of this file asks.
git merge-base --is-ancestor 8414a1a1 origin/v0.2.3     # the proof's landing merge
git merge-base --is-ancestor f779be7d origin/v0.2.3     # the CLEAN-verdict tip, its own branch pruned 2026-10-31
git merge-base --is-ancestor 613a2b98 origin/proofs     # the no-ff merge into this line
git merge-base --is-ancestor dc1b784b origin/v0.2.3     # the release commit, landed by pull request 708

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
| `proofs` | top-level integration branch | level with `v0.2.3` at `208df9e9` except this stamp's held commit |
| `proof/composite-region-reuse` | the first proof's feature branch | complete and merged at tip `f779be7d`, its remote branch pruned by the V0.2.X line and its local branch and worktree removed by this line on 2026-10-31 |

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

## THE HOLD ON MERGES INTO V0.2.3

On 2026-11-01 the V0.2.X line asked this line to hold further merges into `v0.2.3` until that
line reports its merge of `v0.2.3` into `main`, because each move of `v0.2.3` restarts that
merge's gates. The request arrived as peer message `v02-20261101-708-merged`, which states
that the V0.2.X operator directed the `main` merge. It is a peer request, not a ruling of this
line's operator, and this line honors it as coordination. Check the mailbox and `origin/main`
for that report before opening any pull request into `v0.2.3`.

## QUEUED FOR THE NEXT LANDING

One commit, this stamp's, held on `proofs`. It refreshes this file and exports
`docs/process/interline/checkpoint.json`. Every line exports that shared checkpoint, so it
will likely conflict when this line next merges `v0.2.3`. Resolve such a conflict by exporting
the checkpoint again from the live store after the merge, not by combining the two JSON files
by hand.

## OPEN, ALL WITH THE OPERATOR

1. **`src/verify.rs:1337`**, the loop accounting the proof's Appendix E cites, adopting
   confined-site accounting, explicitly unruled, lowering a published worst-case-memory-usage
   figure. The line number was taken from Appendix E as `v0.2.3` updated it and will drift. The V0.2.X line's landed `src/confine.rs`
   analysis is assembling the measured consequence, and the proof's Theorem C remark is the
   piece to read first, footprint can exceed the branch-max bound on branch-dominated shapes,
   so adoption should be per-site by comparing both bounds.
2. **B2 adoption**, explicitly unruled, a proved specification with seven named obligations in
   the proof's Appendix C.
3. **Mechanization**, the recorded follow-on that upgrades the proof from audited prose to
   checked proof, its cost much reduced by the axiomatization the audits forced.
4. **The floating-point application binary interface question**, which predates this line.
   This file recorded it as awaiting one word in `docs/process/REVERSE_PROMPT.md`. As of
   2026-11-01 that file no longer mentions it, and no other file on `v0.2.3` does. Whether it
   was answered or dropped is unverified, so ask the operator before treating it as open.

## GOVERNING RULES A RESUMING SESSION MUST NOT LOSE

- **Work in a worktree.** This line operates in `../keleusma-worktrees/proofs`, with each new
  proof in a sibling worktree on a `proof/*` branch. The main checkout belongs to the V0.2.X
  session. A shared checkout silently changes what a long-running command measures.
- **Build output lives on the scratch volume.** The worktree's `target` is a link to
  `/Volumes/Scratch/keleusma/proofs/target`. A `cargo clean` deletes the link, so run
  `scratch.sh link target` again before building. Inside the agent sandbox the global
  `sccache` wrapper is refused, so build with `RUSTC_WRAPPER=` set to empty.
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

Nothing to any peer line. This line honors the V0.2.X hold above.

## Owed to this line

Nothing.

## WHAT A RESUMING SESSION SHOULD DO FIRST

Assume the project has changed, as the warning at the top of this file says. Fetch, run the
validity block, and measure how far `v0.2.3`, `v0.3.0`, and `main` have moved since
`208df9e9`. Read and acknowledge `interline inbox --line proofs`, and check whether the V0.2.X
line has reported its `main` merge, which ends the hold. Read the peer handoffs,
`git show origin/v0.2.3:docs/process/handoffs/v0.2.3.md` and
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`, for anything addressed to this
line. Read the three audit records and the proof's Appendix E when the next task touches the
proof. Report the differences you found, then wait for the operator. Nothing was in flight when
this stamp was written. The line's next action is whichever the operator commissions, landing
the held commit once the hold ends, a second proof cut as a feature branch from `proofs`, or the
mechanization effort.
