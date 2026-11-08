# Absorption 110

> **Navigation** [Decisions](./README.md) | [Absorption 109](./ABSORPTION_109_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `bebec2a6`, which is pull request 575 merged into
`v0.3.0`, and upstream `a5e3a290` on 2026-10-27, with 3 unabsorbed commits
measured after fetching. The merge base is `d478d86d`. The separate conflict
command `git merge-tree --write-tree --name-only bebec2a6 origin/v0.2.3`
exited one and named `docs/process/REVERSE_PROMPT.md` as its only conflicting
file. The commits are V0.2.X pull request 576, which instantiates a generic
function over a scalar type argument in the self-hosted parser stage. The
merge was prepared on `feat/v3-absorption-110` with `--no-ff --no-commit`.

## Merge

The reverse prompt was rebuilt by the ignored helper `tmp/rebuild_rp.sh`, as
in absorption 109. The merged file is upstream's new 224 lines followed by
this line's unchanged 717-line block, and both parts were compared byte for
byte with their sources. No conflict marker remains. `CLAUDE.md` keeps this
line's routing paragraph. The merged `compiler/`, `src/` and `tests/` equal
upstream's except for `tests/gate_ci_correspondence.rs`, whose difference
predates this absorption.

**One construct-support boundary row moved.** In
`self_hosted_construct_support_boundary`, upstream changed
`scope/generic_fn__GAP`, expected `Refuses`, to `scope/generic_fn`, expected
`SOk`, and added a new gap row, `scope/generic_pipe__GAP`, for a pipe into a
template, which the reference keeps generic. The test asserts a floor on the
supported count rather than exact counts, and the root heavy tier that runs it
is the hosted run's. This line made no change to the table.

## Effect on this line

One stage source moved, `parse.kel`, and its digest in `corpus_fingerprint.rs`
was re-pinned. It is a native corpus subject, with no native cost pinned for
it by name. The reference compiler is unchanged. Upstream also changed root
`tests/selfhost_typecheck.rs` and `tests/selfhost_head_frames.rs`, so both were
run in addition to the usual set. The brief and checkpoint now hold 119
messages.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-110/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 41 passed |
| Root `selfhost_head_frames` and `selfhost_typecheck` with `self-host` | 3 and 103 passed |

Every pinned native figure held in both feature sets. The full-language count
rose from 40 to 41 because upstream added one test. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and the heavy self-host tier, including the boundary test,
were not run locally. The hosted run on the pull request is the integration
check.
