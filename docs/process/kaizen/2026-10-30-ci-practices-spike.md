# Continuous Integration Practices of Compiler and Large Projects

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

A research spike by the kaizen line on 2026-10-30, at the operator's request to look at
how other compiler projects and large projects run their continuous integration and pull
requests, and to find fundamental techniques Keleusma has missed. The operator stated
the same day that runners are a bottleneck for this project. Nothing here is adopted.
Each recommendation names the ruling it needs.

Facts name their source. Inferences and estimates are marked. External practices were
read from each project's documentation or from secondary sources found by web search,
and secondary or dated sources are marked as such. No external project's CI was run.

## Summary

Keleusma already uses most of the structural techniques the surveyed projects use. It
tiers runs by change class, verifies each merge commit, skips a branch run whose tree a
pull request run already tested, builds the self-host tests once for four shards,
cancels superseded pull request runs, aggregates one required check, and batches
changes into integration pull requests by hand. Five gaps remain that matter for
runner load or for correctness.

1. **No build cache at all.** The repository has 0 Actions caches, and compilation was
   measured at about 113 of 200 runner-minutes of a full run on 2026-10-25.
2. **208 integration-test binaries.** Each links the library separately. Projects that
   merged their test binaries report large compile-time savings.
3. **The heaviest tier runs on every push and has not failed.** The self-host shards
   used 46 percent of runner-minutes since 2026-10-15 and passed all 203 completed shard
   runs.
4. **Retries hide flaky tests.** The CI profile retries a failed test twice and keeps
   no record of which tests needed it.
5. **No fuzzing.** The hostile-input defects found this month, a verifier hang and a
   loader panic, are the kind a fuzzer finds.

A merge queue, the technique most of the surveyed projects rely on, appears to be
unavailable to a repository owned by a personal account. That is unverified.

## Keleusma measured, 100 CI runs since 2026-10-15

Source. The Actions API, every run of `ci.yml` created on or after 2026-10-15, 100 runs
and 2,161 jobs, collected on 2026-10-30. Runner-minutes count jobs that succeeded or
failed, from start to completion. Cancelled jobs are excluded, so the totals understate
real load. The raw rows are in
[`evidence/ci-practices/2026-10-30-ci-jobs.tsv`](./evidence/ci-practices/2026-10-30-ci-jobs.tsv).

| Job | Succeeded | Failed | Cancelled | Runner-minutes |
|---|---|---|---|---|
| Test (self-host feature, shard N/4) | 203 | 0 | 18 | 5,721 |
| Test | 56 | 2 | 14 | 2,366 |
| Test (signatures, and the broad signatures,shell set) | 55 | 0 | 13 | 1,450 |
| Self-hosted compiler subproject | 61 | 0 | 12 | 1,020 |
| Build the self-host tests | 56 | 0 | 12 | 694 |
| RTOS demonstrator (host test and N6 build) | 58 | 0 | 10 | 286 |
| Examples (SDL3 feature) | 62 | 0 | 11 | 187 |
| Detached crates (LSP and WASM playground) | 58 | 0 | 10 | 102 |
| Format, check, clippy and advisories | 56 | 3 | 9 | 59 |
| Every other job, together | | 5 | | 463 |
| **Total** | | | | **12,348** |

Five jobs carry 91 percent of the runner-minutes. The self-host shards and the build
that feeds them carry 52 percent. Of the 10 job failures in the period, not counting
the 9 failures of the `CI result` aggregate that report them, none was in those two
jobs.

**Caveat on the zero.** A tier that never fails may never fail because authors run it
before pushing. The pre-push hook runs the `quick` profile, which excludes the
self-host binaries, so the zero is not explained by the hook. Whether lines run
`cargo tns` by hand before pushing was not measured.

Other facts used below, read on 2026-10-30.

- `GET /repos/sgeos/keleusma/actions/cache/usage` reported 0 active caches. No step in
  `ci.yml` uses `Swatinem/rust-cache`, `sccache` or `actions/cache`.
- `.config/nextest.toml` sets `retries = 2` in `[profile.ci]`. It has no `junit`
  section.
- `tests/` holds 208 `.rs` files, each built by Cargo as its own test binary.
- No fuzz target exists on `v0.2.3`.
- `main` requires the `CI result` check. Version branches have a ruleset that forbids
  deletion and force pushes, and no required check.
- `[profile.dev]` already sets `debug = "line-tables-only"`, and Rust 1.90 made `lld`
  the default linker on `x86_64-unknown-linux-gnu`, so two common link-time techniques
  are already in effect.

## What the surveyed projects do

### Rust, rust-lang/rust

The rustc-dev-guide's CI page describes three kinds of build.

- **PR builds.** "A small subset of tests and checks are run after each push to the
  PR", about 40 minutes of lints, a Windows cross-compile check and tests against a
  system LLVM.
- **Auto builds.** After approval, bors creates a merge commit and runs the full
  suite, which can take several hours, before `main` moves. Manual merges are forbidden.
  The guide puts throughput at roughly ten pull requests a day.
- **Try builds.** `@bors try` runs chosen jobs on a pull request without merging it,
  for benchmarks, crater runs or a specific platform.
- **Rollups.** A team member combines several pending trivial pull requests into one
  that is tested once. rustc-perf's triage notes ask authors of performance-sensitive
  changes to mark them `rollup=never`, because a rollup obscures which change moved a
  benchmark.
- **Caching.** Docker images are cached in a registry, keyed on a hash of the
  Dockerfiles, and LLVM is built through sccache with an S3 bucket.
- **Flakes.** A flaky job can be marked `optional-` with `continue_on_error`, and is
  watched before it is allowed to block merges.
- **Performance.** perf.rust-lang.org benchmarks merge commits, and a weekly triage
  rotation labels regressions and asks authors to justify or follow up on them.

The bors bot, written by Graydon Hoare in 2013, enforces what was later called the
"not rocket science rule". `main` only ever holds a tree that passed the full suite as
merged.

### LLVM

LLVM runs a light pre-merge check on pull requests and does most testing after commit,
on Buildbot at lab.llvm.org. The design goal is testing every commit, so a break can be
attributed to one commit and reverted. The developer policy expects authors to respond
promptly to post-commit failures, and reverting to green is the routine remedy. This
summary comes from search results over LLVM's policy and design pages, some of which
are older revisions.

### Chromium

Chromium's commit queue runs try jobs before landing. Older infrastructure documents,
from about 2016 and 2017 and since superseded by LUCI, describe retrying a failing
shard with the patch, then running the failing suites without the patch. A failure that
also happens without the patch is treated as pre-existing or flaky and does not block
the change. Disabling a flaky test was done in separate changes.

### Go

Go runs on LUCI. A `Commit-Queue+1` vote runs a default set of TryBot builders before
submission, and SlowBots add more builders on request, through a `Cq-Include-Trybots`
line. Most platforms are tested after submission on the post-submit dashboard.
Infrastructure failures are shown differently from test failures, so they are rerun
rather than debugged.

### Mozilla

Firefox lands changes through Autoland, with Try for pre-landing experiments. Sheriffs
watch the tree, back out a breaking change, and classify each intermittent failure in
Treeherder. This summary rests on sheriffing pages from about 2016 and 2017.

### Practices across Rust projects

- **sccache and rust-cache.** Reported setups use `rust-cache` for the registry and
  index, and sccache with the GitHub Actions cache backend for compiled crates. The
  evidence is per-project and anecdotal, with savings from about 10 to 50 percent and
  no controlled comparison.
- **Single integration-test binary.** matklad's "Delete Cargo Integration Tests" argues
  that each `tests/*.rs` file re-links the library, and that one binary with modules is
  faster to build and run. The jj project reported recompiling all tests in about 4
  seconds after the change, against 32 to 50 seconds before.
- **Flaky-test records.** nextest reports a test that passes on retry as FLAKY. A JUnit
  report from a `[profile.ci.junit]` section makes that machine-readable. Flaky-test
  services advise against retries, because retries hide what they measure.
- **Mutation testing.** cargo-mutants' `--in-diff` mutates only the lines a pull request
  changed. Its documentation says this is not a substitute for a full scheduled run.
- **Fuzzing in CI.** cargo-fuzz drives libFuzzer for Rust, and OSS-Fuzz accepts Rust
  projects. GCC's RISE effort runs Csmith continuously. An ICST 2025 industry paper ran
  a short fuzzing campaign on each pull request to the Dafny compiler and found three
  unknown bugs.
- **Selective testing.** Some projects test only changed crates and their dependents.
  Cargo has no built-in support, and the technique gains little when, as here, almost
  every test depends on one core crate.

### GitHub platform facts

- **Concurrency.** GitHub's limits reference gives 20 concurrent standard-runner jobs
  for Free accounts and 40 for Pro, and says GitHub Support can raise the limit. On
  2026-10-30 the kaizen line counted up to 28 Keleusma jobs running at once by job
  timestamps, which suggests that this account's limit is above 20. The account's plan
  could not be read through the API.
- **Merge queue.** GitHub's 2023 general-availability changelog offers it on public
  repositories owned by organizations and on Enterprise Cloud. Secondary sources say
  personal repositories are excluded. The current documentation pages fetched on
  2026-10-30 state no availability, so this remains unverified for `sgeos/keleusma`.
- **Cache.** Every repository gets 10 GB of Actions cache at no cost. Since November
  2025, Pro and higher accounts can pay to exceed it. Least recently used entries are
  evicted.
- **Runners.** Standard hosted runners are free on public repositories, and since 2024
  they have four vCPUs there. Larger runners are billed even on public repositories.
  GitHub advises against self-hosted runners on public repositories, because a fork's
  pull request can run code on them.
- **Artifact storage.** No official statement that artifact storage is free for public
  repositories was found. The kaizen line's concern about the 1.1 GB self-host archive
  stays open.

## What Keleusma already does, matched to the survey

| Technique | Where it is used | Keleusma |
|---|---|---|
| A light run for low-risk changes | rustc PR builds, Go default TryBots | `scripts/ci_classify.py` light path, since 2026-10-25 |
| Verify every merge commit | LLVM post-commit, Go post-submit | Branch runs on `v*`, kept uncancelled by the operator's ruling of 2026-10-29 |
| Do not repeat a tested tree | Merge queues test the merged tree once | `certify` and `ci-certified.sh` skip a branch run whose tree was tested |
| Build once, test in shards | rustc's builders, LUCI | `nextest archive` and `--partition hash` |
| Cancel superseded runs | Common | Pull request concurrency group |
| One required aggregate check | rustc `bors` status, LUCI CQ | `CI result` on `main` |
| Batch changes | rustc rollups | V0.2.X's `integrate/v02-batch-*` pull requests |
| Differential testing against a reference | Csmith, crater | The byte-identical self-host oracle |
| Revert or fix to green at once | LLVM, Mozilla sheriffs | CLAUDE.md requires a red version branch to be remedied immediately |
| Pinned actions, lockfile checks | Common | Pinned SHAs, Dependabot cooldown, `--locked` since PR 624 |

## Recommendations

Each recommendation gives its expected effect, how sure that is, its cost, and the
ruling it needs. The order is by the kaizen line's estimate of value to the operator's
stated bottleneck.

### R1. Measure the compile time, then cache what does not change

**Gap.** No build cache exists, and compilation dominates a full run.

**Proposal.** First run one full CI pass with `cargo build --timings` uploaded, to split
compile time between dependencies, the `keleusma` crate and the test binaries. Then add
`sccache` with its GitHub Actions cache backend to the compiling jobs, with
`rust-cache` limited to the registry, the pattern the surveyed projects use. Save the
cache only on version-branch pushes and restore it read-only on pull requests, so that
pull requests cannot evict each other's entries.

**Effect.** An estimate, not a measurement. Dependencies and unchanged test binaries
would come from cache. The `keleusma` crate changes in most pull requests and would
still compile, and everything that depends on it with it. The saving could be anywhere
from small to a third of compile time, which is why the timing pass comes first.

**Cost.** About two workflow changes and the 10 GB cache budget. With five feature sets
and the self-host build, the cache may thrash, which the timing pass would also show.

**Ruling.** None for the timing pass, which is measurement. The cache is a workflow
change within this line's remit, and it changes no check.

**Outcome, 2026-10-30.** [The timing pass](./2026-10-30-r1-timing-pass.md) found
dependencies at 2 to 5 percent of each build and one test binary setting each build's
length, so no cache was added. The gap statement above, that compilation dominates a
full run, is superseded. Compiling is 63 of a median 243 runner-minutes.

### R2. Merge the integration-test binaries

**Gap.** 208 test binaries each link the library. matklad's post and the jj project's
measurement suggest that this is a large share of build time for a test-heavy crate.

**Proposal.** Group `tests/*.rs` into a few binaries, for example one per tier, each a
`main.rs` with the existing files as modules. The self-host tier would stay its own
binary, so the `quick` profile can still exclude it.

**Effect.** An estimate. Linking 208 test binaries against the library becomes linking
a few. The jj project went from 32 to 50 seconds down to about 4 for its test rebuild.
Keleusma's figure is unknown until R1's timing pass.

**Cost.** Large and cross-line. Tests are named by binary in `nextest` filters,
`tests/ci_light_path_coverage.rs`, `tests/gate_ci_correspondence.rs`, the guard job's
`--test` list, changelog entries and documents. Every line's open branches would
conflict with the move. It needs a quiet moment, like the version change.

**Ruling.** The operator, because it touches every line's work and the guard
selections. The kaizen line recommends it only if R1's timing pass shows linking
test binaries is a large share.

### R3. Run the self-host tier once per pull request, not on every push

**Gap.** The tier used 46 percent of runner-minutes and has not failed in 203 shard runs
since 2026-10-15. rustc runs a subset on each push and the full suite once, before
merge.

**Options.**

- **(a)** Run the shards on a pull request only when it is marked ready, by a label
  such as `full-ci` or by moving out of draft, and on every branch run. Pushes before
  that get the rest of the full run.
- **(b)** Keep the shards on every push, but cancel them when a newer push arrives,
  which the pull request concurrency group already does.
- **(c)** Leave it unchanged.

**Effect.** For (a), an estimate. A pull request that is pushed three times runs the
shards once rather than three times. The saving scales with how often pull requests are
pushed again, which was not measured.

**Cost and risk.** The byte-identical oracle is this project's correctness signal. Under
(a) it still runs before every merge, so no merge loses it. A pull request could look
green before it is marked ready, and `CI result` would have to require the shards on a
ready pull request. The `certify` job would certify only runs that included them.

**Ruling.** The operator, because it changes when a gate runs. The kaizen line
recommends (a), with `CI result` failing a ready pull request whose shards did not run.

### R4. Decide how to get a merge queue, or confirm that branch runs replace it

**Gap.** Every surveyed project that merges at scale tests the merged tree before the
target branch moves. Keleusma tests GitHub's trial merge, and the base can move before
the merge. The branch run catches the result afterwards, the LLVM model, and a red
branch is fixed at once.

**Options.**

- **(a)** Check in the repository settings whether "Require merge queue" is offered.
  If it is, a queue with a group size above one gives rollups natively. It needs a
  `merge_group` trigger in `ci.yml`.
- **(b)** Move the repository to a free organization, where the 2023 changelog says
  merge queues are available for public repositories. It changes the repository's
  URL, with GitHub redirects, and affects Pages, badges and crates.io links.
- **(c)** Mergify, whose open-source plan is described as free for public
  repositories. It is a third party with write access to the repository.
- **(d)** Keep the current model, and formalize V0.2.X's batch pull requests as the
  rollup practice.

**Ruling.** The operator. The kaizen line recommends (a) first, because it costs one
look, and (d) if no queue is offered.

### R5. Record which tests needed a retry

**Gap.** `retries = 2` makes a flaky test pass silently. Flaky-test practice keeps the
record that retries hide.

**Proposal.** Add a `[profile.ci.junit]` section, upload the report from each test job,
and add a step to `CI result` that lists every test nextest marked FLAKY. Keep the
retries, so a flake does not fail a merge, but make every flake visible in the run
summary.

**Effect.** No runner saving. It turns a hidden failure mode into a visible one, and
gives the count that decides whether a test should be fixed or quarantined.

**Ruling.** None. It changes no check's result.

### R6. Fuzz the hostile-input surface

**Gap.** The verifier and loader must refuse hostile bytecode. This month's
pykeleusma findings, a verifier walk that hung on a backward `If` target and a signed
header over a short declared total that panicked, are the shapes a coverage-guided
fuzzer finds.

**Proposal.** cargo-fuzz targets for `Vm::load_bytes` and the signed loader,
`verify`, the WCET and WCMU entries, and the lexer and parser, each asserting no panic
and a bounded run. Commit a seed corpus from `tests/` fixtures, and replay it as a
regression test in the ordinary suite. Run real fuzzing on a schedule, for example
nightly for 30 minutes at an hour when runners are idle, or apply to OSS-Fuzz, which
runs it off GitHub's runners.

**Effect.** A correctness gain, not a runner saving. A scheduled run adds one runner for
its duration, off peak.

**Ruling.** The operator for the schedule and for an OSS-Fuzz application, which is
outward-facing. The targets and the corpus replay are ordinary test work that the
owning line could take.

### R7. Leads, less urgent

- **Bound tracking.** The rustc-perf analogue for this project is the WCET and WCMU
  bound of a fixed corpus, recorded per merge commit and compared. A change that
  raises a bound would then be flagged like a performance regression, rather than
  found in review.
- **Mutation testing of the verifier.** cargo-mutants with `--in-diff` on pull requests
  that touch `src/verify*.rs`, and a full monthly run, would show which verifier lines no
  test can tell apart from a mutant. Its value is in the soundness-critical code, where
  a surviving mutant marks an unpinned rule.
- **Account concurrency.** If the account is on GitHub Free, Pro doubles the concurrent
  job limit from 20 to 40. GitHub Support can also raise limits. The observed peak of 28
  suggests the limit is already above 20, so the operator should check the plan before
  acting on this.
- **Not recommended.** Self-hosted runners, because the repository is public and a
  fork's pull request could run code on them. Larger runners, because they are billed
  on public repositories. Selective testing by crate, because nearly every test depends
  on the core crate.

## Verification limits

- No recommendation was trialled. Every saving is an estimate from the measurements
  above and from other projects' reports.
- External practices come from documentation and search results. The Chromium and
  Mozilla descriptions are from pages about a decade old. The sccache comparisons are
  anecdotal.
- Merge queue availability for a personal account is unverified.
- The per-job table excludes cancelled jobs, and its 100 runs are the API's first page
  for the period.

## Sources

- [rustc-dev-guide, Testing with CI](https://rustc-dev-guide.rust-lang.org/tests/ci.html)
- [Mergify, the origin story of merge queues](https://mergify.com/blog/the-origin-story-of-merge-queues)
- [rustc-perf triage, 2021-07-06](https://github.com/rust-lang/rustc-perf/blob/master/triage/2021-07-06.md)
- [LLVM Code-Review Policy and Practices](https://rocm.docs.amd.com/projects/llvm-project/en/docs-10.0.0/LLVM/llvm/html/CodeReview.html)
- [What does it take to run LLVM buildbots, 2022](https://www-new.llvm.org/devmtg/2022-11/slides/TechTalk5-WhatDoesItTakeToRunLLVMBuildbots.pdf)
- [LLVM, stop running premerge checks on main](https://llvm.googlesource.com/llvm-project/+/1b788255775e9076b04c3c2a8818714ba96652e1)
- [Chromium commit queue documentation](https://chromium.googlesource.com/chromium/src/+/282cd8a/docs/infra/cq.md)
- [Go Wiki, LUCI](https://go.dev/wiki/LUCI)
- [Mozilla sheriffing, backouts](https://wiki.mozilla.org/Sheriffing/How_To/Backouts)
- [Sheriffing at Mozilla, 2017](https://blog.mozilla.org/tomcat/2017/04/03/sheriffing-and-backouts/)
- [Xuanwo, why you should try sccache, 2023](https://xuanwo.io/en-us/reports/2023-04/)
- [Devolutions gateway, rust-cache and sccache timings](https://github.com/Devolutions/devolutions-gateway/pull/1603)
- [matklad, Delete Cargo Integration Tests](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests)
- [Rust blog, faster linking with lld on 1.90.0](https://blog.rust-lang.org/2025/09/01/rust-lld-on-1.90.0-stable)
- [nextest, retries and flaky tests](https://nexte.st/docs/features/retries/)
- [Trunk, flaky tests for Rust](https://docs.trunk.io/flaky-tests/get-started/frameworks/rust)
- [cargo-mutants, incremental tests of modified code](https://mutants.rs/in-diff.html)
- [Compiler Fuzzing in Continuous Integration, ICST 2025](https://conf.researchr.org/details/icst-2025/icst-2025-industry/13/Compiler-Fuzzing-in-Continuous-Integration-a-Case-Study-on-Dafny)
- [GitLab, how to fuzz Rust code](https://about.gitlab.com/blog/how-to-fuzz-rust-code/)
- [GitHub, Actions usage limits and billing](https://docs.github.com/en/actions/reference/usage-limits-billing-and-administration)
- [GitHub changelog, merge queue generally available, 2023](https://github.blog/changelog/2023-07-12-pull-request-merge-queue-is-now-generally-available/)
- [GitHub, managing a merge queue](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue)
- [GitHub changelog, cache can exceed 10 GB, 2025](https://github.blog/changelog/2025-11-20-github-actions-cache-size-can-now-exceed-10-gb-per-repository/)
- [GitHub, hosted runners double the power for open source, 2024](https://github.blog/news-insights/product-news/github-hosted-runners-double-the-power-for-open-source/)
- [GitHub, how use of Actions is measured](https://docs.github.com/en/billing/concepts/product-billing/github-actions)
- [Mergify, migrate from Bors-NG](https://docs.mergify.com/migrate/bors-ng)
