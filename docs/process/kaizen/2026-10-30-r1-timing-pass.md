# R1 Timing Pass, Where a Full CI Run Spends Its Time

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

Recommendation R1 of [the CI practices spike](./2026-10-30-ci-practices-spike.md) asked
for a timing pass before any build cache. The operator asked on 2026-10-30 to complete
R1. This report is the timing pass and the decision it supports. **No build cache is
added.** The measurement shows that a cache could save at most about one minute of a
compiling job, and that the time goes elsewhere.

## Method

Two sources, both from 2026-10-30.

1. **Ten green full runs of `ci.yml`.** Runs 41013192234, 41017658200, 41027091310,
   41033805972, 41039702260, 41040130272, 41044435024, 41047427793, 41049344098 and
   41049951298, five branch runs and five pull-request runs. Every log line carries a
   timestamp. Cargo's `Finished ... in` line gives each step's compile time, and
   nextest's `Summary [...]` line gives its test-running time. nextest also prints a
   duration for every test. The scripts are `evidence/ci-practices/r1_*.py`, and their
   outputs are the two `2026-10-30-r1-*.tsv` files beside them.
2. **One probe run with `--timings`.** Pull request 645, closed unmerged, replaced
   `ci.yml` with three jobs that repeat the builds of the Test job, the self-host
   archive and the signatures step with `cargo test --no-run --timings`, on fresh
   4-core runners. The summary is `evidence/ci-practices/2026-10-30-r1-timings-summary.txt`.
   The reports themselves were kept for 7 days as artifacts of run 41092634891.

## Findings

**A full run.** Median 243 runner-minutes and 56 minutes of wall time, from 199 to 264.
Cargo steps account for 235 runner-minutes. **Compiling is 63 runner-minutes and
running tests is 166.** The figure of about 113 compiling minutes of 200 in the
2026-10-25 load proposal predates the build-once self-host archive and is superseded.

**Dependencies are not the cost.** In every compiling job, the last third-party crate
starts within 0.4 minutes of the step's start. In the probe, dependencies took 0.5 to
1.3 unit-minutes per build, 2 to 5 percent of the build's unit time.

**One test binary sets the length of every test build.** The probe's three builds each
took 12.1 to 12.6 minutes of wall time. Of the 215 `keleusma` test targets, the median
compiled in 0.4 to 0.7 seconds. `target_descriptor_axis` compiled in 594 to 659 seconds
and finished last in each build. Its grid instantiates `GenericVm<W, A, F>` for 16 pairs
of word and address types, each on two float types, so the whole VM is compiled 32
times at `opt-level = 1`. `narrow_vm` follows at 106 to 130 seconds. The 213 other test
binaries together took about 9 to 11 unit-minutes per build, which on 4 cores is about
3 minutes of wall time.

**Test running dominates, and some of it is done twice.** The self-host shards ran 419
test-minutes per run on four runners, and the Test job 101.

- 2,446 tests ran in both the Test job and the shards, about 64 test-minutes on each
  side, mostly the library's own tests and the `trap_free_corpus`, `trap_analysis` and
  `hosted_runtime` binaries. The Test job builds `keleusma` with the features
  `keleusma-cli` enables, `shell`, `signatures`, `encryption` and `self-host`, and the
  shards build it with `self-host` alone, so the two runs are two of the five feature
  sets `CLAUDE.md` lists. None of the three integration files above contains a feature
  gate on `shell`, `signatures` or `encryption`, by a search of the test files only.
  The library code they call may still differ between the two builds.
- The comment above the Test job says its filter left 20 test-minutes. The measured
  figure is 101, of which 37 run only there, mostly the `keleusma-cli` binaries
  `validator` and `self_hosted_stage_corpus`.
- Three tests run longer than 25 minutes in the shards,
  `production_compiler_matches_every_complete_stage_artifact` at a median of 35,
  `every_source_has_its_pinned_self_hosted_outcome` at 27 and
  `every_stage_matches_complete_resource_admission` at 27. The shards' cargo steps took
  medians of 19, 31, 36 and 42 minutes. A hash partition cannot balance tests that long.
- No test in the sampled logs was retried or marked flaky.

**Runner contention.** The probe's jobs waited about 20 minutes for a runner on
2026-10-30.

## Decision on R1

**No build cache.** sccache documents that it cannot cache a crate that invokes the
system linker, which every test binary does, so it could cache only the dependencies
and an unchanged library. `rust-cache` keeps only dependencies by default. Both would
save less than a minute per compiling job, about 4 runner-minutes per run, against a
cache restore on every job and the 10 GB cache budget. This is within the line's remit
and changes no check, and it closes R1.

## What the measurement points to instead

Each is listed with options in [the open decisions](./2026-10-30-open-decisions.md).
The savings are estimates from the figures above, not measurements.

1. **Split `target_descriptor_axis` into four files, one per word type.** The same 32
   cells are checked, so coverage does not change. The grid would compile in parallel
   rather than in sequence, and each test build would shorten from about 12 minutes
   to about 10. The self-host archive is on the shards' path, so a full run would
   finish about 2 minutes sooner. Compiling the grid in one feature-set build rather
   than four would also remove about 10 unit-minutes from each of the other three, but
   that changes which builds run the grid and needs a ruling.
2. **Split the three longest self-host tests per stage**, as the parked branch
   `test/kaizen-split-stage-artifact` does for the first. The slowest shard sets the
   run's wall time, and the shards cannot balance below 35 minutes while one test
   takes 35.
3. **Remove the duplicated tests** from one of the two jobs, after checking that the
   feature difference does not reach them. The integration binaries alone are about
   26 test-minutes per run.
4. **R2, merging the test binaries, would save little.** The 213 other test binaries
   cost about 3 minutes of wall time per build on 4 cores, and merging them keeps
   their compile time and removes only their link steps.

## Verification limits

- Ten runs from one day. Medians are reported, and the range of total runner-minutes
  is given. Cancelled and failed runs are excluded.
- The probe ran each build once. Its three builds agree within 0.5 minutes of wall
  time.
- Unit-seconds overlap in a parallel build, so they measure work, not elapsed time.
- The duplicate count matches tests by binary and test name. It does not establish
  that the two builds execute the same code.
- The split savings in the section above are estimates and were not measured.
