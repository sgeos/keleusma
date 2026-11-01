# Decisions Open with the Operator, 2026-10-30

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The operator asked on 2026-10-30 for one list of the concerns that need a decision, with
the obvious choices marked. This list gathers the kaizen line's own items, the open
recommendations of [the CI practices spike](./2026-10-30-ci-practices-spike.md) and
[the R1 timing pass](./2026-10-30-r1-timing-pass.md), and, by pointer, the questions the
other lines hold for the operator. It decides nothing.

Each item lists lettered options with the kaizen line's recommendation first. An item
is marked **obvious** when the recommendation follows from a ruling or a measurement
already on record and no option trades one stated goal against another. Otherwise it
is marked **judgment**. For a peer line's question, the recommendation is that line's
own, and where the kaizen line reads it differently the difference is stated as an
inference. The owning line's text is the authority for every peer question.

## Operator rulings, 2026-10-30

The operator ruled first-hand in this line's session on 2026-10-30, the same day the
list was published. The words are recorded as given, with the disposition after each.

| Item | Ruling as given | Disposition |
|---|---|---|
| K1 | "Adopt obvious choice." | (a). No build cache. Closed by the R1 report |
| K2 | "Implementation detail. We need reasonable converage in the context of the project, for what it is. Admittely, that should be a higher bar than most projects, but solutions should still be right-sized. Therefore, it is important not to go too far the other way and do extra work. The upshot is that redundant testing should be removed barring a good reason." | The kaizen line narrows the Test job to the `keleusma` tests no other job runs, in `ci.yml`, guarded by `tests/ci_shard_coverage.rs` |
| K3 | "Adopt obvious choice." | (a). Requested of V0.2.X, which owns the tests |
| K4, R2 | "Adopt obvious choice." | (a). R2 declined |
| K4a | "Adopt obvious choice." | (a). Requested of V0.2.X. Option (b) is not adopted |
| K5, R3 | "Accept recommendation." | (a). A draft pull request defers the self-host tier, in `ci.yml` and `GIT_STRATEGY.md` |
| K6, R4 | "Adopt obvious choice." | (a). The operator looks in the settings for a merge queue |
| K7, R6 | "Adopt obvious choice." | (a). Requested of V0.2.X. OSS-Fuzz is not adopted |
| K8, R5 | "Adopt obvious choice." | Done by the kaizen line, a JUnit report and `scripts/ci_flaky_report.py` |
| K9 | "Adopt obvious choice." | The operator checks the account plan |
| K10 | "Adopt obvious choice." | The operator checks the billing page |
| K11 | "Adopt obvious choice." | (a). The operator or a blog session applies the fixes |
| K12 | "Accept recommendation." | (a). HTTPS with the `gh` credential helper |
| K13 | "Accept recommendation." | (a). Corrected when the operator releases the proofs branch |
| K14 | "Hold." | Held |

The operator did not rule on the peer questions in this exchange.

## Continuous integration

**K1. R1 build cache.** *Obvious.* The timing pass measured third-party compilation at
under one minute of each compiling job, and sccache cannot cache a crate that links,
which every test binary does.
(a) Add no build cache, and close R1 on the measurement.
(b) Add sccache anyway for the dependency share.

**K2. Tests run twice.** *Judgment.* The Test job and the self-host shards both ran
2,446 of the same tests, about 64 test-minutes each per full run, mostly the library's
own tests and the `trap_free_corpus`, `trap_analysis` and `hosted_runtime` binaries.
The Test job builds `keleusma` with every feature `keleusma-cli` enables, and the
shards build it with `self-host` alone, so the two runs are different feature sets
that `CLAUDE.md` both lists.
(a) Drop from the Test job the integration binaries that also run in the shards and
contain no feature gate, about 26 test-minutes, and keep the library tests in both.
(b) Drop the library tests as well, about 50 test-minutes, which leaves the
library's `shell`, `signatures` and `encryption` paths covered only by the signatures
job and loses the combination of those features with `self-host`.
(c) Leave it.

**K3. Single long tests set the wall clock.** *Obvious.* One test,
`production_compiler_matches_every_complete_stage_artifact`, ran for a median of 35
minutes inside one shard, and the slowest shard ran 42 minutes against 19 for the
fastest. A hash partition cannot balance a test that long. The parked branch
`test/kaizen-split-stage-artifact` already splits it per stage.
(a) Split the three tests that run over 25 minutes into per-stage tests, owned by
V0.2.X, which checks the same artifacts.
(b) Leave them.

**K4. R2, merge the 208 test binaries.** *Obvious.* The timing pass measured the 213
test binaries other than the two slowest at about 3 minutes of wall time per build on
4 cores, and merging keeps their compile time. The move conflicts with every open
branch.
(a) Decline it, and revisit only if a later timing pass shows link time growing.
(b) Schedule it at a quiet moment.

**K4a. The 32-instance grid in `target_descriptor_axis`.** *Obvious for (a), judgment
for (b).* It compiled for 594 to 659 seconds and finished last in each of the three
probe builds of about 12 minutes.
(a) Split the file into four, one per word type, which checks the same 32 cells and
shortens each test build to about 10 minutes by estimate. V0.2.X owns the file.
(b) As (a), and compile the grid in one feature-set build instead of four, which saves
about 10 unit-minutes in each of the other three builds and changes which feature sets
run it.
(c) Leave it.

**K5. R3, run the self-host tier once per ready pull request.** *Judgment.* The tier is
half of every full run, 122 of a median 243 runner-minutes, and did not fail in 203
shard runs. It is also the byte-identical oracle, the project's correctness signal.
(a) Run the shards when a pull request is marked ready and on every branch run, with
`CI result` failing a ready pull request whose shards did not run.
(b) Rely on cancellation of superseded pull-request runs, already in place.
(c) Leave it.

**K6. R4, a merge queue.** *Obvious for the first step.*
(a) The operator looks in the repository settings for "Require merge queue". If it is
offered, adopt it with a `merge_group` trigger. If not, take (d).
(b) Move the repository to a free organization.
(c) Mergify, a third party with write access.
(d) Keep branch runs as the after-merge check and formalize V0.2.X's batch pull
requests as the rollup practice.

**K7. R6, fuzzing.** *Obvious for the first step, judgment for the second.*
(a) V0.2.X adds cargo-fuzz targets for the loader, the signed loader, the verifier,
the bound entries and the front end, with a seed corpus replayed as an ordinary test,
then a nightly scheduled run of about 30 minutes at an idle hour.
(b) As (a), and apply to OSS-Fuzz, which is outward-facing.
(c) Neither.

**K8. R5, record which tests needed a retry.** *Obvious.* No ruling is needed, because
it changes no check's result. None of the 10 sampled runs logged a retry. The kaizen
line takes it unless the operator objects.

**K9. The account plan.** *Obvious.* Only the operator can see it. It decides whether
GitHub Pro would double the concurrent job limit and whether a merge queue is offered.
The probe for the R1 timing pass waited about 20 minutes for a runner on 2026-10-30.
(a) The operator checks the plan and reports it.

**K10. Artifact storage.** *Obvious.* The self-host archive is about 1.1 GB per full run,
kept one day. Whether that storage is free for this public repository is not
verified.
(a) The operator checks the billing page once, and the kaizen line records the answer.

## Other kaizen items

**K11. The blog's CI fixes.** *Obvious in substance, open in ownership.* The audit in
the blog's untracked `tmp/2026-10-30-ci-runner-audit.md` names three fixes. Load the
source records once in the verify step. Stop retrying the 94 PDFs that always fail.
Skip the daily build when nothing is due.
(a) The operator, or a session the operator starts in the blog, applies them before
the blog's next large push.
(b) The kaizen line applies them, which needs the operator to grant it the blog
checkout and to allow it to run the blog's scripts, which auto mode refused here.

**K12. SSH under the sandbox.** *Judgment.*
(a) HTTPS with the `gh` credential helper, which keeps `~/.ssh` closed and works with
the sandbox's host allowance.
(b) A pinned `known_hosts` outside `~/.ssh` with an ssh-agent socket.

**K13. The stale texts on `origin/proofs`.** *Judgment.* The branch, unchanged since
2026-08-29, holds an old `wire_schema.rs` comment and four lines that match kept
disclosure terms. Its merge into V0.2.X waits only on the operator releasing the
branch.
(a) Correct them on the proofs branch when the operator releases it for merging.
(b) Correct them now.
(c) Leave them.

**K14. The pydromon changeover.** *Held by the operator.* At the changeover, set
`fresh_session_each_turn` on the labeller line and confirm on its first live run that
the second case sees nothing of the first. Only the operator may do either.

## Questions held by V0.2.X

They are in `docs/process/REVERSE_PROMPT.md` on `v0.2.3`, numbered there.

| Question | V0.2.X recommends | Mark | Kaizen note |
|---|---|---|---|
| 1, definite initialization, which refuses 62 light-tier tests. V0.3.X holds the same rule as its question 20 | (a) and (b), change the compiler so every path writes and give composite private fields initializers | possibly answered | V0.3.X's notice to V0.2.X at 16:14 UTC on 2026-10-30 reports that the operator answered its question 20 first-hand with "Proceed with option a", which narrows the rule to what it can prove and corresponds to V0.2.X's (c). This is a peer relay, not verified here, and V0.2.X's question 1 has not been answered in its own terms. The operator's confirmation for question 1 would settle both |
| 2, host struct layout accepted by kind and size only | (b), a host contract with a size check now and field layouts later | obvious | Field layouts on the wire would need a `BYTECODE_VERSION` authorization, which (b) defers |
| 5, the auto-sized arena omits composite host inputs | (a), a second entry point that counts them | judgment | An inference. An arena size that undercounts sits badly with the definitive WCMU claim, which favours (a) |
| 6, private slots across a hot swap | (a), keep them private | obvious | Privacy is by design |
| 7, the bundled `Text<N>` operations | (a), restore them with `Text<N>` signatures | obvious | The reason for removing them, unboundedness, no longer holds |
| 8, the `Text<N>` surface form | (a), an `as` cast for a literal and free functions | judgment | Language design taste |
| 9, `Text` reads the layout proof cannot cover | (a), a constant-cost range check | obvious | (c) would record a trust assumption where the 2026-10-20 ruling rejects undefined behaviour |
| 10, a halting error for hosts | (a), add `VmError::HostHalt` | judgment | An inference. (b), non-exhaustive first, avoids the next break of the same kind |

V0.2.X also reports three concerns it does not ask about. It cancelled 16 superseded
runs on 2026-10-30, an over-rejection in `tests/selfhost_typecheck.rs` was not
isolated, and its question 3 cites the wrong pull request, which the kaizen line
corrected by notice 155.

## Questions held by V0.3.X

V0.3.X's open item that needs the operator was question 20, the same rule as V0.2.X's
question 1, which V0.3.X reports answered, as the table above records. Its handoff at `docs/process/handoffs/v0.3.0.md` on `v0.3.0` holds older
questions as history. Whether any other is still open was not verified here.

## Verification limits

The peer questions were read from `origin/v0.2.3` and `origin/v0.3.0` on 2026-10-30.
The marks are the kaizen line's judgment, not the owning lines'. The figures in K2, K3
and K5 come from 10 green full runs on 2026-10-30 and are in the R1 report's evidence.
