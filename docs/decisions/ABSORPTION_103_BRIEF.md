# Absorption 103

> **Navigation** [Decisions](./README.md) | [Absorption 102](./ABSORPTION_102_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `42ef444d`, which is pull request 558 merged into
`v0.3.0`, and upstream `c309e3d3` on 2026-10-26, with 4 unabsorbed commits
measured after fetching. The merge base is `23a38251`. The separate conflict
command `git merge-tree --write-tree --name-only 42ef444d origin/v0.2.3`
exited zero and produced tree `98be2ac27561a01e90ffa7f3799a99f4a8aa8a3e` with
no conflicting file. The commits are V0.2.X pull request 556. It adds a trap
analysis agreement test and a rejection corpus for the self-hosted validator,
and repairs `verify_depth.kel`, which accepted a root region that fell
through without a final return. The merge was prepared on
`feat/v3-absorption-103` with `--no-ff --no-commit`.

## Merge

No file conflicted. Upstream changed `docs/process/REVERSE_PROMPT.md`, and
the first 180 lines of the merged file were compared byte for byte with
upstream's file. This line's block follows them. `CLAUDE.md` keeps this
line's routing paragraph. The merged `compiler/`, `src/` and `tests/` equal
upstream's except for `tests/gate_ci_correspondence.rs`, whose difference
predates this absorption. The detached `compiler/` change is therefore
upstream's own and was not run here.

The upstream commit message records an operator ruling of 2026-10-26 that
limits may be raised. It reached this line through V0.2.X's commit and
`REVERSE_PROMPT.md`, not from the operator directly, and this line takes no
action on it.

## Effect on this line

One stage source moved, `verify_depth.kel`, and its digest in
`corpus_fingerprint.rs` was re-pinned. **Unlike the recent absorptions, this
source is also a native corpus subject.** The native differential compiles
`verify_depth.kel` and compares its result with the reference virtual
machine, so its native figures may move. The repair adds a fall-through check
to the root region, so a small rise in its instruction count or cost is
plausible. The run below tests this through the pinned figures.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-103/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 32 passed |

Every pinned native figure held. **This does not show that the repair left
`verify_depth.kel`'s native cost unchanged.** No test pins a cost or
instruction figure for that module by name. It is listed among the seeded
stage subjects in `corpus_differential.rs`, and the native suites, which
compare the native result with the reference virtual machine on those
subjects, passed. The plausible rise named above is therefore neither
confirmed nor excluded, and it is not measured here. The full-language count
rose from 30 to 32 because upstream added two tests. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and the detached `compiler/` tests were not run locally. The
hosted run on the pull request is the integration check for the root.
