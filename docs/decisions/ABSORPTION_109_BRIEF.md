# Absorption 109

> **Navigation** [Decisions](./README.md) | [Absorption 108](./ABSORPTION_108_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `b13232a3`, which is absorption 108 on
`feat/v3-absorption-108` as pull request 574, and upstream `d478d86d` on
2026-10-26, with 6 unabsorbed commits measured after fetching. The merge base
is `97f4633d`. The separate conflict command
`git merge-tree --write-tree --name-only b13232a3 origin/v0.2.3` exited one
and named `docs/process/REVERSE_PROMPT.md` as its only conflicting file. The
commits are V0.2.X pull requests 572 and 573, which lower `Option` value
construction and a match whose scrutinee is an `Option` in the self-hosted
compiler. The merge was prepared on `feat/v3-absorption-109`, cut from
absorption 108, with `--no-ff --no-commit`. This absorption is not on
`v0.3.0` until pull request 574 merges first.

## Merge

The reverse prompt was rebuilt as in absorptions 107 and 108, now by the
ignored helper `tmp/rebuild_rp.sh`, which stops on any mismatch. This line's
file was upstream's earlier 196 lines followed by this line's 717-line block,
checked byte for byte. The merged file is upstream's new 208 lines followed by
the same block, and both parts were compared byte for byte with their
sources. No conflict marker remains. `CLAUDE.md` keeps this line's routing
paragraph. The merged `compiler/`, `src/` and `tests/` equal upstream's
except for `tests/gate_ci_correspondence.rs`, whose difference predates this
absorption.

## Effect on this line

One stage source moved, `parse.kel`, and its digest in `corpus_fingerprint.rs`
was re-pinned. It is a native corpus subject, with no native cost pinned for
it by name. The reference compiler is unchanged.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-109/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 40 passed |

Every pinned native figure held in both feature sets. The full-language count
rose from 38 to 40 because upstream added two tests. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and the heavy self-host tier were not run locally. The hosted
run on the pull request is the integration check.
