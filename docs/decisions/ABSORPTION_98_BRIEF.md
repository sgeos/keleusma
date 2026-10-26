# Absorption 98

> **Navigation** [Decisions](./README.md) | [Absorption 97](./ABSORPTION_97_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `7e4bd77b`, which is pull request 547 merged into
`v0.3.0`, and upstream `52e904c8` on 2026-10-26, with 6 unabsorbed commits
measured after fetching. The merge base is `66de2778`. The separate conflict
command `git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited zero
and produced tree `d0fdfc95cd84d419f7a9dd7c777fd6dc0158adfe` with no
conflicting file. The commits are V0.2.X pull requests 545 and 546, Order 6
slices 6 to 8. They lower struct data fields and enum-returning natives,
accept tuple native returns in `use` signatures, and repair a match time
bound in the self-hosted analysis. The merge was prepared on
`feat/v3-absorption-98` with `--no-ff --no-commit`.

## Merge

No file conflicted. The first 137 lines of `docs/process/REVERSE_PROMPT.md`
were compared byte for byte with upstream's file, and this line's block
follows them. `CLAUDE.md` keeps this line's routing paragraph.

## Effect on this line

Two stage sources moved, `analyze.kel` and `parse.kel`, and their digests in
`corpus_fingerprint.rs` were re-pinned.

**The match time-bound repair is in the self-hosted stage, not in the
reference.** Upstream found that `analyze.kel` charged a single-pass enum
match loop its costliest break alone, where the reference verifier charges the
larger of the pass and that break. The stage now matches the reference. This
line's native cost claims are composed from the reference verifier's figures,
so the repair is expected to move none of them, and the run below tests that
expectation through the pinned cost and bound figures.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-98/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 26 passed |

No pinned native cost or bound figure moved, which agrees with the expectation
above. The full-language count rose from 22 to 26 because upstream added four
tests. This table was added after the run. The tests that read this record
were then rerun, `prediction_stamp`, `outstanding_reports`, `handoff_figures`
and the native and root `comment_citations`, with the root
`documentation_links`. The root workspace suite was not run locally. The
hosted run on the pull request is the integration check.
