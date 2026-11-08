# Absorption 107

> **Navigation** [Decisions](./README.md) | [Absorption 106](./ABSORPTION_106_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `d12c8bbf`, which is absorption 106 on
`feat/v3-absorption-106` as pull request 569, and upstream `4182e419` on
2026-10-26, with 6 unabsorbed commits measured after fetching. The merge base
is `715cb866`. The separate conflict command
`git merge-tree --write-tree --name-only d12c8bbf origin/v0.2.3` exited one
and named `docs/process/REVERSE_PROMPT.md` as its only conflicting file. The
commits are V0.2.X pull requests 566 and 568. The self-hosted compiler now
places a piped operand at a pipe placeholder, as the reference does, and
takes the layouts of the builtin generic signatures `Option<T>` and
`Multiword<N>`. The merge was prepared on `feat/v3-absorption-107`, cut from
absorption 106, with `--no-ff --no-commit`. This absorption is not on
`v0.3.0` until pull request 569 merges first.

## Merge

**The reverse prompt was rebuilt, not hand-merged.** This line's file was
upstream's earlier 183 lines followed by this line's 717-line block, and that
was checked byte for byte before the merge. The merged file is upstream's new
192 lines followed by the same block, and both parts were compared byte for
byte with their sources. No conflict marker remains. `CLAUDE.md` keeps this
line's routing paragraph. The merged `compiler/`, `src/` and `tests/` equal
upstream's except for `tests/gate_ci_correspondence.rs`, whose difference
predates this absorption.

## Effect on this line

Two stage sources moved, `parse.kel` and `reconstruct.kel`, and their digests
in `corpus_fingerprint.rs` were re-pinned. Both are native corpus subjects,
with no native cost pinned for them by name. The reference compiler is
unchanged, so the native compiler's input is unchanged for every other
subject. The brief's opening paragraph was also condensed, so that it names
the last merged absorption and the open ones rather than each merge.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-107/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 37 passed |

Every pinned native figure held in both feature sets. The full-language count
rose from 35 to 37 because upstream added two tests. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and the heavy self-host tier were not run locally. The hosted
run on the pull request is the integration check.
