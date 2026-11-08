# Absorption 97

> **Navigation** [Decisions](./README.md) | [Absorption 96](./ABSORPTION_96_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `51800928`, which is pull request 542 merged into
`v0.3.0`, and upstream `66de2778` on 2026-10-26, with 8 unabsorbed commits
measured after fetching. The merge base is `22cb258d`. The separate conflict
command `git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited zero
and produced tree `b2071ea4bf0613eea0b5d9a4727efc92748d4551` with no
conflicting file. The commits are V0.2.X pull request 544, Order 6 slices 5 to
5d and an abstract syntax tree census. The merge was prepared on
`feat/v3-absorption-97` with `--no-ff --no-commit`.

## Merge

No file conflicted. `docs/process/REVERSE_PROMPT.md` merged on its own, and its
first 119 lines were compared byte for byte with upstream's file, followed by
this line's block. `CLAUDE.md` keeps this line's routing paragraph.

## Effect on this line

Four stage sources moved, `codegen.kel`, `lexer.kel`, `parse.kel` and
`reconstruct.kel`, and their digests in `corpus_fingerprint.rs` were
re-pinned. Upstream adopted this line's typed `use` declaration in
`examples/scripts/rogue/rogue_dungen.kel`, as agreed in
`v023-20261026-full-language-table-reply`. Its digest did not move, so the
shared file is now identical on both lines and the last difference absorption
95 found is closed.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-97/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 22 passed |

This table was added after the run. The tests that read this record were then
rerun, `prediction_stamp`, `handoff_figures` and the native and root
`comment_citations`, with the root `documentation_links`. The root workspace
suite was not run locally. The hosted run on the pull request is the
integration check.
