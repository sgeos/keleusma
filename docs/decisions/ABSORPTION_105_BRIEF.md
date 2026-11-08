# Absorption 105

> **Navigation** [Decisions](./README.md) | [Absorption 104](./ABSORPTION_104_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `4a260cdf`, which is absorption 104 on
`feat/v3-absorption-104` as pull request 565, and upstream `fecc737b` on
2026-10-26, with 6 unabsorbed commits measured after fetching. The merge base
is `e58a3c02`. The separate conflict command
`git merge-tree --write-tree --name-only 4a260cdf origin/v0.2.3` exited zero
and produced tree `0454826c5ba27e1d88941d76131b953838bdc712` with no
conflicting file. The commits are V0.2.X pull requests 562 and 563. They
raise the parser's local binding capacity, type native results for operator
selection, and read destructured untyped native results as boxed tuple
fields. The merge was prepared on `feat/v3-absorption-105`, cut from
absorption 104, with `--no-ff --no-commit`. This absorption is not on
`v0.3.0` until pull request 565 merges first.

## Merge

No file conflicted. The first 183 lines of `docs/process/REVERSE_PROMPT.md`
were compared byte for byte with upstream's file, and this line's block
follows them. `CLAUDE.md` keeps this line's routing paragraph. The merged
`compiler/`, `src/` and `tests/` equal upstream's except for
`tests/gate_ci_correspondence.rs`, whose difference predates this absorption.

The change to `tests/selfhost_codegen.rs` adds one decoding case for the
existing `GetTupleField` opcode with a boxed operand. No opcode was added, and
the pinned construct-support boundary counts are not among the changed lines.
One commit cites the operator ruling of 2026-10-26 that limits may be raised,
which reached this line only through V0.2.X's commits.

## Effect on this line

One stage source moved, `parse.kel`, and its digest in `corpus_fingerprint.rs`
was re-pinned. `parse.kel` is a native corpus subject, and as in absorptions
103 and 104 no test pins a native cost for it by name. The run below shows
whether every pinned figure holds. Upstream also changed root
`tests/selfhost_parse.rs`, so it was run in addition to the usual set.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-105/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 34 passed |
| Root `selfhost_parse` with `self-host` | 90 passed |

Every pinned native figure held in both feature sets. The full-language count
rose from 32 to 34 because upstream added two tests. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and the heavy self-host tier were not run locally. The hosted
run on the pull request is the integration check.
