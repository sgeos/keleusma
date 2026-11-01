# Absorption 102

> **Navigation** [Decisions](./README.md) | [Absorption 101](./ABSORPTION_101_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `7967e8dd`, which is absorption 101 on
`feat/v3-absorption-101` as pull request 557, and upstream `23a38251` on
2026-10-26, with 2 unabsorbed commits measured after fetching. The merge base
is `219d13b6`. The separate conflict command
`git merge-tree --write-tree --name-only 7967e8dd origin/v0.2.3` exited one
and named `docs/process/interline/checkpoint.json` as its only conflicting
file. The commits are Kaizen pull request 554 on `v0.2.3`. It unquotes the
`cargo tns` filterset in `.cargo/config.toml`, sets the executable bit on ten
shebang scripts under `scripts/`, and adds `scripts/test_local_tooling.py` to
pin both. The merge was prepared on `feat/v3-absorption-102`, cut from
absorption 101, with `--no-ff --no-commit`. This absorption is not on `v0.3.0`
until pull request 557 merges first.

## Merge

**The checkpoint conflict was resolved by regeneration, not by choosing a
side.** This line's checkpoint held 115 messages and upstream's held 116, and
the union of their identifiers is 116. The file was regenerated from the
shared mailbox store and holds 116 messages, and no identifier from either
side is missing from it. `docs/process/REVERSE_PROMPT.md` and `CLAUDE.md` did
not change upstream, and `CLAUDE.md` keeps this line's routing paragraph.

## Effect on this line

No stage source moved, so no digest was re-pinned. The new
`scripts/test_local_tooling.py` passed its 3 tests in this worktree, which is
a nested worktree of the kind its alias note describes.

**The same defect class exists outside the new test's scope, and none of it
fails today.** Four tracked non-Rust files carry a shebang without the
executable bit. They are this line's `native_codegen/tools/mutation_sweep.py`,
`native_codegen/tools/test-gate-status.sh` and
`native_codegen/pending/fix_workstream_label.py`, and the shared
`book/ci/verify_examples.py`. Every invocation of them in the workflows,
tests and this line's documentation names the interpreter, so none exits 126
there. The usage line inside `verify_examples.py` shows the bare form, which
would exit 126 if followed literally. This absorption leaves them
unchanged and reports them to Kaizen, because a mode change is outside an
absorption's scope.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt was written, on a tree that did not change during the run. Logs are
under ignored `tmp/abs-102/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 30 passed |
| `scripts/test_local_tooling.py` | 3 passed |

No pinned native figure moved and no count changed. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and `cargo tns` itself were not run locally. The hosted run
on the pull request is the integration check.
