# Decisions Open with the Operator, 2026-10-31

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The operator asked on 2026-10-31 for a consolidated list of the concerns that need a
decision, with the obvious choices listed. This list follows
[the list of 2026-10-30](./2026-10-30-open-decisions.md), whose items K1 to K14 keep
their numbers. New kaizen items are N1 to N5. The other lines' questions are summarized
from their own records, `docs/process/REVERSE_PROMPT.md` on `v0.2.3` and
`docs/process/handoffs/v0.3.0.md` on `v0.3.0`, read on 2026-10-31, and those records
are the authority for every peer question.

An item is marked **obvious** when the recommendation follows from a ruling or a
measurement already on record and no option trades one stated goal against another.
Otherwise it is marked **judgment**.

## Operator ruling, 2026-10-31

The operator ruled first-hand in this line's session on 2026-10-31, in these words.

> Accept obvious choices, take appropriate action, then give me the updated list of
> questions and obvious choices.

The ruling adopts the recommendation of every item marked obvious below, as the list
was presented. It decides no item marked judgment.

| Item | Disposition |
|---|---|
| N1 | (a). `.github/workflows/fuzz.yml`, a nightly run of every target |
| N2 | (a). A Dependabot entry for `/fuzz` and a `--locked` lockfile check in the `detached` job |
| N3 | (a). One kaizen branch remained, `test/kaizen-split-stage-artifact` at `e7a59d20`. The remote refused an archive tag because the commit carries a private author address, and auto mode refused this line's deletion. V0.2.X reports in sequence 175 that it deleted the remote branch on the operator's instruction to prune stale branches, keeping the commit under the local tag `archive/test-kaizen-split-stage-artifact`. The kaizen line removed its clean worktree `kaizen-test-duration` |
| N4 | (a). Requested of V0.2.X |
| N5 | (a). Re-measured, and item 3 is withdrawn on the measurement. N6 below replaces it |
| V0.2.X item 4 | (a) as this list presented it, the `Text<N>` wire item deferred with the version, relayed to V0.2.X in sequence 173. **Contested.** V0.2.X replied in sequence 175 that the operator had ruled (b), authorize the collapse now, first-hand in its session, and confirmed (b) first-hand there on 2026-10-31. That reaches this line as a relay, and V0.2.X's tracked record did not yet show it when read. Open for the operator's confirmation here |
| V0.2.X C7 D2, D3, D5, D6, D7 | (a) each, as `RING_BUFFER_DESIGN.md` recommends. Relayed to V0.2.X |

## Operator ruling, 2026-10-31, second

The operator was given the updated list later on 2026-10-31 and ruled first-hand in
this line's session, in these words.

> Re: Decisions
>
> 1. Treat the V0.2.X lines ruling as the source of truth for this particular item.
> 2. Treat the V0.2.X lines ruling as the source of truth for this particular item.
> 3. A. Use recommended option.
> 4. Will circle back to you.
>
> Re: Obvious choices. All good. Proceed.

Decision 1 of that list was V0.2.X item 4, and decision 2 was V0.2.X's other questions.
Decision 3 was N6, and decision 4 was the operator's own checks below.

| Item | Disposition |
|---|---|
| V0.2.X item 4 | (b), authorize the `Text<N>` wire collapse now, as V0.2.X recorded the operator's ruling. The contest above is closed. The ruling here governs this item only and does not make a relayed ruling binding in general |
| V0.2.X questions 1, 2, 3, 5, 6 and 7 | Answered as V0.2.X records the operator's answers. V0.2.X's record is the authority for what each answer is |
| N6 | (a). Carried out in the pull request that adds the pinned lists, with the measurement and its limits in N6 below |
| K6, K9, K10, K11, K13, K14 | Open. The operator will return to them |
| Obvious choices O1 to O7 | Adopted. O1 merges `main` back into `v0.2.3`. O2 confirms the first nightly fuzz run. O3 asks V0.2.X to exempt `fuzz/corpus/` from the final-newline rule, item N7. O4 leaves V0.2.X's compiler defects with V0.2.X. O5 leaves the stale worktree record to the operator. O6 and O7 are the handoff and the next items |

## Kaizen items

**N1. A nightly fuzzing workflow.** *Obvious.* The fuzz targets of K7 merged into
`v0.2.3` in pull request 658.
(a) A scheduled run at an idle hour, about 6 minutes per target for the five targets.
A crash fails the run and uploads the crashing inputs.
(b) As (a), and open an issue on a crash.
(c) Weekly.

**N2. The fuzz package's lockfile is unguarded.** *Obvious.* V0.2.X reported that no CI
check and no Dependabot entry covers `fuzz/Cargo.lock`.
(a) A Dependabot entry for `/fuzz`, and a `--locked` check in every full run.
(b) The Dependabot entry alone.
(c) Leave it.

**N3. The kaizen line's merged remote branches.** *Obvious.* The ruling G1 was made in
V0.2.X's session and reached this line as a relay.
(a) Delete them, tagging an unmerged branch under `archive/` first.
(b) Keep them.

**N4. The validator test bounds the Test job.** *Obvious, by the precedent of K3.*
`keleusma-cli`'s `shipping_validator_reaches_every_current_stage` ran 20.6 minutes in
the draft run of 2026-10-30. It also failed once and passed on a rerun in V0.2.X's run,
with the failing output not captured.
(a) Ask V0.2.X to split it per stage.
(b) Leave it.

**N5. The shards are uneven after the split of K3.** *Obvious.*
(a) Move the self-host documentation tests into their own job, then measure again.
(b) Leave it.

The measurement withdrew (a)'s first half. The documentation tests ran 40 to 63
seconds in shard 1 across four runs of 2026-10-30, about what a new job spends on its
checkout and compile, so the move saves no runner time.

**N6. Partition the self-host tests by measured duration.** *Judgment.* On the
`v0.2.3` run 41165060704 of 2026-10-31, the shards summed 7,786, 4,121, 5,349 and
7,123 test-seconds and the jobs ran 36, 19, 26 and 34 minutes. Each shard's wall time is close
to its test-seconds over four threads, and the longest test, 884 seconds, is shorter
than every shard, so the cause is the hash placing uneven work in each shard, not one
long test. Shard 2 was the shortest and shard 4 among the longest on each of the four
`v0.2.3`-lineage runs after pull request 662. A balanced partition would run each shard
about 25 minutes, an estimate from the one run, with the same runner-minutes. That
estimate was wrong, as the measurement below shows.
(a) Assign the twenty or so tests over 250 seconds to shards by measured duration and
hash-partition the rest, with `tests/ci_shard_coverage.rs` checking that every assigned
test exists and runs once. The list goes stale as tests change, and a stale entry fails
the guard rather than dropping a test.
(b) A fifth shard, which shortens each shard by about a fifth and adds a job.
(c) Leave it.

*Carried out as (a), 2026-10-31.* The logs of the twelve shard jobs of runs
41226417780, 41225820890, 41219329660 and 41195469596 give 3,547 tests and 28,919
median test-seconds. Over 16 threads that is about 30 minutes a shard, so 25 minutes
was never reachable. The longest test is now 1,354 seconds. The jobs ran 26 to 37
minutes. A simulation that fills four threads in nextest's order put the current
partition's longest shard at 35.2 minutes.

nextest's filter language has no hash predicate, and `--partition` applies to every
test a run selects, so a pinned test cannot share a run with the hashed tests. Each
shard therefore runs twice, first its pinned list and then the hash partition of every
test no list names. Threads left idle at the end of the pinned run are the cost of
that. With the threshold of 250 seconds the simulated longest shard was 33.0 minutes.
With 150 seconds, 43 tests holding two thirds of the test-seconds, it was 31.8 minutes,
and that threshold was taken. The assignment was a longest-first placement followed by
single moves and swaps that lowered the longest shard.

The expected saving is about three minutes on the critical path, with total
runner-minutes about the same. That is an inference from a simulation, and the same
shard ranged from 26 to 37 minutes between runs with nearly the same tests, so runner
variance is as large as the saving. Whether it holds is measured on the runs after the
change merges.

## The operator's own checks, open since 2026-10-30

These need an action, not a decision. K6, look in the settings for "Require merge
queue". K9, the account plan. GitHub refused the job-log interface on 2026-10-30 with an
account egress limit, which bears on it. K10, whether the self-host archive storage is
billed. K11, the blog's fixes. K14, the pydromon changeover, held.

K13 may be closed. V0.2.X records that the operator released the proofs branch under
its ruling F1, which reaches this line as a relay. If so, the proofs line corrects its
texts when it merges, and nothing remains for the operator.

## Questions held by V0.2.X

From its "Decisions waiting on the operator", with its recommendation first. V0.2.X
reports in sequence 175 that every question below has since been answered first-hand
in its session. The operator's second ruling of 2026-10-31 above makes V0.2.X's record
the source of truth for these items, so none is open with the kaizen line. The text is
kept as it was presented.

1. **B9 with B2 (c).** *Judgment.* (d) Field layouts on the wire, which closes the
   host struct reorder case and the flat `Text` boundary for every live source and
   needs a wire change. (a) A range check at the read as a stopgap. (c) The documented
   trust assumption alone, as now. Kaizen note, an inference: a violated contract is
   undefined behaviour, which the 2026-10-20 ruling rejects, and that favours (d).
   Whether (d) also needs `BYTECODE_VERSION` raised is not stated.
2. **B7, typing the restored `Text` operations.** *Judgment.* (a) Generic native
   declarations over const parameters, a language extension with no opcode. (b)
   Compiler intrinsics. (c) One native per capacity.
3. **B10.** Not yet decidable. It waits on pykeleusma's use case, requested in sequence
   159.
4. **The `Text<N>` wire item.** Contested, as the table above records.
5. **The eight C3 decisions in `ARRAY_WIRE_SLOT_DESIGN.md`.** *Judgment.* Decision 8 is
   done. Decisions 1 and 2 choose the encoding and authorize the wire change. Decision 3
   recommends a fingerprint roll, since raising the version would reverse the 2026-09-01
   policy. Decision 5 changes every admitted program's WCMU figure. Decision 7 is a
   breaking change to the public `DataLayout` type within V0.2.x.
6. **The seven C7 decisions in `RING_BUFFER_DESIGN.md`.** D2, D3, D5, D6 and D7 are
   ruled above. D1, the surface form, follows item 2. D4, overwrite the oldest element
   or drop the new one, which is the choice between a ring and a queue, is *judgment*.
7. **Two behaviour changes of the B1 widening.** *Judgment.* A hot swap's `Unit` seed now
   takes the new module's initializer, which changes a public interface's behaviour, and
   a `Text` private slot read before its first write returns the empty string. (a) Keep
   both. (b) Revert the hot swap change. Either way the changelog should record them.

V0.2.X also reports four compiler defects not yet fixed and a private data rule without
a self-hosted twin. They are V0.2.X's to schedule and need no ruling.

## Questions held by V0.3.X

None. Its handoff records the operator's first-hand answers to questions 22, 24, 25 and
27 on 2026-10-31 and states that no question is open.

## Verification limits

N5's documentation-test figures come from the R1 evidence file
`evidence/ci-practices/2026-10-30-r1-step-split.tsv`. N6's shard figures come from the
job interface for four runs after pull request 662 on the `v0.2.3` lineage, and its
per-shard test-seconds and per-test figures from the logs of one run, so runner variance
is not measured. The fuzz workflow was dry-run locally before it
was proposed and has not run on GitHub, because a scheduled workflow runs only from
`main`. The peer questions were read on 2026-10-31, and the marks are the kaizen line's
judgment, not the owning lines'.
