# Absorption 108

> **Navigation** [Decisions](./README.md) | [Absorption 107](./ABSORPTION_107_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `8a9b1bc5`, which is absorption 107 on
`feat/v3-absorption-107` as pull request 571, and upstream `97f4633d` on
2026-10-26, with 2 unabsorbed commits measured after fetching. The merge base
is `4182e419`. The separate conflict command
`git merge-tree --write-tree --name-only 8a9b1bc5 origin/v0.2.3` exited one
and named `docs/process/REVERSE_PROMPT.md` as its only conflicting file. The
commits are V0.2.X pull request 570, which lowers a match over an `Option`
parameter in the self-hosted compiler. The merge was prepared on
`feat/v3-absorption-108`, cut from absorption 107, with
`--no-ff --no-commit`. This absorption is not on `v0.3.0` until pull request
571 merges first.

## Merge

The reverse prompt was rebuilt as in absorption 107. This line's file was
upstream's earlier 192 lines followed by this line's 717-line block, checked
byte for byte. The merged file is upstream's new 196 lines followed by the
same block, and both parts were compared byte for byte with their sources. No
conflict marker remains. `CLAUDE.md` keeps this line's routing paragraph. The
merged `compiler/`, `src/` and `tests/` equal upstream's except for
`tests/gate_ci_correspondence.rs`, whose difference predates this absorption.

## Effect on this line

One stage source moved, `parse.kel`, and its digest in `corpus_fingerprint.rs`
was re-pinned. It is a native corpus subject, with no native cost pinned for
it by name. The reference compiler is unchanged. Upstream also changed root
`tests/selfhost_parse.rs`, so it was run in addition to the usual set.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-108/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 38 passed |
| Root `selfhost_parse` with `self-host` | 90 passed |

Every pinned native figure held in both feature sets. The full-language count
rose from 37 to 38 because upstream added one test. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and the heavy self-host tier were not run locally. The hosted
run on the pull request is the integration check.
