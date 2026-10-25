# Continuous integration load proposal

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

Prepared on 2026-10-25 by the kaizen line, in reply to a load review from the
operator's admin session. **It is a proposal. No workflow was edited.** The operator
decides what is applied. Measurements name their run and moment. Estimates and
inferences are marked.

## Summary

| # | Finding | Class |
|---|---|---|
| 1 | One full CI run costs about 200 runner-minutes. Five jobs carry about 185 of them, the workspace `Test` job and the four self-host shards, with `Test (broad features)`, `Test (signatures feature)` and the compiler subproject. The other 20 jobs run for about a minute each. | Measured, runs 40221696853 and 40219928464 |
| 2 | Compilation, not testing, dominates. Each self-host shard compiles the same binaries for 17 min 40 s before its share of tests. `Test (signatures feature)` compiles for 10 min 45 s to run 66 s of tests. About 113 of the 200 runner-minutes are compilation. | Measured, one run |
| 3 | A workflow-level `paths-ignore` for `docs/**` or `*.md` is unsafe here. At least 17 test binaries read documentation, including the heavy `selfhost_codegen`, which reads `docs/roadmap/V0_2_X_ROADMAP.md`. Two link walkers read every Markdown file. Almost no path is read by nothing. | Measured by search, see the table below |
| 4 | Of the last 40 pull requests merged into a version branch, 5 changed only documentation, 8 only process tooling under `scripts/`, and 5 only `native_codegen/`, which `ci.yml` does not build. | Measured |
| 5 | Of the same 40, 29 merged onto a base that had not moved since the pull request's last CI run, so the branch run after the merge very probably repeated an identical tree. | Inference from commit and run timestamps, not a tree comparison |
| 6 | No test reads git history, so a test result depends on the tree and the environment only. | Measured by search |
| 7 | `max-parallel` cannot bound one push below 20 slots, because the only matrix is the four shards. The peak comes from 29 separate jobs starting together. | Inference from `ci.yml` |
| 8 | On 2026-10-25 the account ran 48 CI runs, 48 Book runs and 19 Native gate runs for this repository. Book runs cost about 3 runner-minutes each. Native gate runs cost two macOS jobs of about 41 minutes each. | Measured |

## Recommended changes

The proposals are independent unless stated. Each keeps a full green run for every
commit on `main`, keeps branch runs unserialised and uncancelled, keeps the book
example verification in this repository against the CLI from the same commit, and
runs every shard wherever the full suite runs.

### A. A change classifier with a light path for pull requests

A first job, `changes`, compares the pull request's tree with its merge base and sets
`full` or `light`. Every other job reads it. **Any error, unknown path or push event
yields `full`**, so the default is the full run.

A pull request is light when every changed file matches the light set.

| Light path | Read by | Covered on the light path by |
|---|---|---|
| `docs/**` except the guarded documents below | `claimed_counts`, `documentation_links`, `gate_ci_correspondence`, `release_process_crate_list`, `llms_txt_currency`, `comment_citations`, `spec_trap_claims`, `invalid_bytecode_indirect_sites`, `proof_evidence_index`, `book_translation_currency`, the `Doc links` job | the `guards` job and the `Doc links` job |
| `*.md` outside crate directories | the two link walkers, several guards | the same |
| `book/**` | `book_example_coverage`, `book_translation_currency`, `push_order_claims`, the `Doc links` job, `book.yml` | the same, and `book.yml` under proposal B |
| `scripts/*.py`, `scripts/*.sh`, `scripts/*.kel` | the `Python tooling` and `Shell scripts` jobs, `gate_ci_correspondence`, the `Doc links` job | those jobs |
| `native_codegen/**`, only if V0.3.X agrees | `native.yml` on `v0.3.0`, and a path string in `release_process_crate_list` | the `guards` job, and `native.yml` |

**Guarded documents count as source**, because a heavy test reads them. Today that is
`docs/roadmap/V0_2_X_ROADMAP.md`, read by `selfhost_codegen`. So are the crate READMEs
that `include_str!` places in the crate documentation, `keleusma-arena/README.md` and
`keleusma-wire/README.md`.

The light run is three jobs.

- `guards`. `cargo nextest run` over the documentation-reading test binaries only,
  selected by name.
- `Doc links`, unchanged. Markdown links, the instruction-set chapter and the book
  examples against the debug CLI.
- `Python tooling` and `Shell scripts` as one job.

**Keeping the list complete.** A new test, `tests/ci_light_path_coverage.rs`, runs in
the full matrix. It scans `tests/*.rs` for literal reads of documentation, book,
workflow and script paths, and fails unless each such binary is in the `guards`
selection or its path is a guarded document. Adding a test is a source change, so
the full run that adds the test also checks that it is listed.

**What it lets through on a pull request.** A documentation change that breaks a test
that reads the file through a path the scanner cannot see, such as one assembled at
run time. **Where it is caught.** The branch run after the merge, which stays full
for every light commit, and by name the commit that caused it.

### B. A paths filter on `book.yml`

Run `book.yml` when any of these change, erring broad.

```yaml
paths:
  - 'book/**'
  - 'src/**'
  - 'keleusma-*/**'
  - 'compiler/**'
  - 'examples/**'
  - 'Cargo.toml'
  - 'Cargo.lock'
  - '.cargo/**'
  - 'rust-toolchain*'
  - '.github/workflows/book.yml'
```

The book has no `{{#include}}` of files outside `book/`, and `verify_examples.py`
reads only `book/src` and the CLI binary. `keleusma-*/**` covers `keleusma-cli`, which
the examples run, and `keleusma-wasm`, which the playground build uses.

**What an incorrect filter would let through.** A change outside the list that alters
example output, the mdbook build, the Japanese catalogue check or the playground
build. **Where it is caught.** The example verification also runs in `ci.yml`'s
`Doc links` job on every full run, so example drift is still caught per commit. The
mdbook build and catalogue check would be caught at the next run that matches, at
the latest on the deploy from `main`. **Saving.** About 3 runner-minutes and two slots
per unmatched run. This is the smallest saving in this proposal.

### C. Skip the branch run when its tree was already verified

The pull-request run tests GitHub's trial merge. When the base has not moved, the
merge commit has the same tree, and no test reads git history, so the branch run
repeats the pull-request run on identical input.

**Merge queue.** It would remove the duplication natively, but to my knowledge it is
offered only to repositories owned by organizations, and `sgeos` is a user account.
The GitHub pages fetched today did not state the availability, so this is
unverified. Running only the branch run, and skipping the pull-request run once the
tip is unchanged, would remove the pre-merge signal, so it is not recommended.

**The proposal.** A final `certify` job in a full, green pull-request run uploads a
one-day artifact named after the tested tree hash, recording the run and
`rustc +stable -V`. On a push to a `v*` branch, `changes` computes the pushed tree.
If a certifying artifact for that tree exists from the last 24 hours with the same
stable toolchain, the heavy jobs are skipped and a summary job reports the
certifying run. `main` always runs full. Whether a run verified by tree identity
satisfies "a green run for each commit" is the operator's ruling.

**What it lets through.** Drift in the environment between the two runs, such as a
new advisory, a runner image change, or a flaky test that passed once. A new stable
toolchain is excluded by the version check. **Where it is caught.** The next full
run on that branch, and a proposed nightly scheduled full run of each `v*` tip, which
bounds the drift to one day at about 150 runner-minutes per branch per day.

### D. Build the self-host tests once, and consolidate the small jobs

`max-parallel` is the wrong instrument here. The only matrix is the four shards, and
capping it at 2 would run them in two waves, raising the run's wall-clock time from
about 37 minutes to about 62 while lowering the peak by only two slots.

Two changes bound one push instead.

1. **Archive and shard.** One job runs `cargo nextest archive --features self-host`.
   Four shard jobs check out the tree, download the archive and run
   `--archive-file ... --workspace-remap . --partition hash:N/4` with no compilation.
   That removes three of the four 17.5-minute compilations, about 50 runner-minutes
   per run. Wall-clock time stays near 39 minutes, the build followed by the slowest
   shard. `tests/ci_shard_coverage.rs` would be extended to check the archive job.
2. **Fewer, longer small jobs.** Merge the small checks by kind into `lint`, `docs`,
   `portability`, `test-packages`, `test-features`, `rtos` and `detached`. Each step is
   kept by name and run with `if: ${{ !cancelled() }}`, so one failure does not hide
   the next. Coverage is unchanged. No branch protection requires a job by name, so
   no rule changes.

**What it lets through.** Nothing in coverage. The new failure modes are an artifact
transfer failure, which turns the shard red rather than passing, and a shard that
runs a stale archive, which the extended shard guard checks by feature set.
**Effect.** Peak concurrent jobs for one source push falls from 31 to about 15
including the Book workflow. Runner-minutes fall from about 200 to about 147, an
estimate, because the combined feature-test job's compile reuse is not measured.

### E. Jobs that could leave the per-commit run

| Job | Measured run | Proposal | What it would let through, and where it is caught |
|---|---|---|---|
| VS Code extension | 0.2 min | Run when `editors/vscode/**` changes, and nightly | Its inputs are only its own files, so nothing per commit. Registry or Node drift is caught nightly |
| Dependency advisories | 0.2 min | Keep per commit | `ci.yml` records turning unrelated pull requests red on a new advisory as a deliberate cost. Moving it reverses that decision, so it is listed for the operator only |
| MSRV, no_std, RTOS, WASM, LSP, Miri, SDL3 examples | 0.2 to 3.6 min each | Keep per commit, consolidated under D | Each depends on the core sources, so a schedule would weaken the per-commit guarantee |

## Job counts

Before is measured. After assumes A, B, C and D applied, and is an estimate.

| Case | Before | After |
|---|---|---|
| Documentation-only pull request, no `book/` change | 31, being 29 CI and 2 Book | 3 |
| Documentation-only pull request touching `book/` | 31 | 5 |
| Source pull request | 31 | about 20, being 18 CI and 2 Book |
| Merge of a source pull request into `v*`, base unchanged | 31 | 4, the classifier, the summary, and 2 Book |
| Merge of a source pull request into `v*`, base moved | 31 | about 20 |
| Merge of a light pull request into `v*` | 31 | about 20, always full |

On `v0.3.0`, add the two macOS Native gate jobs wherever `native.yml`'s own filter
matches.

**Daily estimate.** Applied to the 23 pull-request runs and 25 branch runs of
2026-10-25, with the class mix of the 40-merge sample, CI falls from about 9,600 to
about 4,200 runner-minutes a day. The assumptions are a light run of about 8
runner-minutes, unmeasured, and the inclusion of `native_codegen/**` in the light
set. Without it, the light share falls from 45 to 33 percent.

## Ownership

`ci.yml` and `book.yml` are byte-identical on `v0.2.3` and `v0.3.0`. Changes to them
have landed through `v0.2.3`, by the kaizen and V0.2.X lines, and reach `v0.3.0` by
absorption. `native.yml` exists only on `v0.3.0` and belongs to V0.3.X. This proposal
does not change it. Its own path filter includes `docs/decisions/**`,
`docs/process/REVERSE_PROMPT.md` and `docs/process/handoffs/v0.3.0.md`, so a process
note on that line starts about 82 macOS runner-minutes. That is for V0.3.X to weigh.

## Order of application, if approved

1. D first. It changes no trigger, and it lowers both cost and peak for every run.
2. A and its coverage guard, with B.
3. C, after the operator rules on verification by tree identity.

Each lands as its own pull request into `v0.2.3`, which costs one full run each.

## Limits

- Job timings come from two pull-request runs on one afternoon of contention. Queue
  time in them was under 2.1 minutes a job.
- The 72 percent duplication figure compares the base's last commit time with the
  pull request's last run time. A tree comparison would need the trial-merge hashes,
  which the run records do not keep.
- The scan for documentation-reading tests matches literal paths. A path assembled
  at run time would be missed, which is why A keeps every light commit's branch run
  full.
- Merge queue availability for user-owned repositories is from recall and is
  unverified.
