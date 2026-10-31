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
| N3 | (a). Attempted. One kaizen branch remained, `test/kaizen-split-stage-artifact` at `e7a59d20`. The remote refused an archive tag because the commit carries a private author address, and auto mode refused the branch deletion. A local tag `archive/test/kaizen-split-stage-artifact` keeps the commit. The deletion is left to the operator |
| N4 | (a). Requested of V0.2.X |
| N5 | (a). Re-measured, and item 3 is withdrawn on the measurement. N6 below replaces it |
| V0.2.X item 4 | (a). The `Text<N>` wire item is deferred with the version. Relayed to V0.2.X |
| V0.2.X C7 D2, D3, D5, D6, D7 | (a) each, as `RING_BUFFER_DESIGN.md` recommends. Relayed to V0.2.X |

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
about 25 minutes, an estimate from the one run, with the same runner-minutes.
(a) Assign the twenty or so tests over 250 seconds to shards by measured duration and
hash-partition the rest, with `tests/ci_shard_coverage.rs` checking that every assigned
test exists and runs once. The list goes stale as tests change, and a stale entry fails
the guard rather than dropping a test.
(b) A fifth shard, which shortens each shard by about a fifth and adds a job.
(c) Leave it.

## The operator's own checks, open since 2026-10-30

These need an action, not a decision. K6, look in the settings for "Require merge
queue". K9, the account plan. GitHub refused the job-log interface on 2026-10-30 with an
account egress limit, which bears on it. K10, whether the self-host archive storage is
billed. K11, the blog's fixes. K14, the pydromon changeover, held.

K13 may be closed. V0.2.X records that the operator released the proofs branch under
its ruling F1, which reaches this line as a relay. If so, the proofs line corrects its
texts when it merges, and nothing remains for the operator.

## Questions held by V0.2.X

From its "Decisions waiting on the operator", with its recommendation first.

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
4. **The `Text<N>` wire item.** Ruled (a) above.
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
