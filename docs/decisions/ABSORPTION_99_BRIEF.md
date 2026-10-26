# Absorption 99

> **Navigation** [Decisions](./README.md) | [Absorption 98](./ABSORPTION_98_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `158fd301`, which is absorption 98 on
`feat/v3-absorption-98` as pull request 550, and upstream `32bc1903` on
2026-10-26, with 5 unabsorbed commits measured after fetching. The merge base
is `52e904c8`. The separate conflict command
`git merge-tree --write-tree --name-only 158fd301 origin/v0.2.3` exited zero
and produced tree `0266783f8db405ba285b837600328604aebc5263` with no
conflicting file. The commits are V0.2.X pull requests 548 and 549. They lower
a break in a bare `for` loop, lower wildcard payload patterns, and name unused
payload slots. The merge was prepared on `feat/v3-absorption-99`, cut from
absorption 98, with `--no-ff --no-commit`. This absorption is not on `v0.3.0`
until pull request 550 merges first.

## Merge

No file conflicted. The first 145 lines of `docs/process/REVERSE_PROMPT.md`
were compared byte for byte with upstream's file, and this line's block
follows them. `CLAUDE.md` keeps this line's routing paragraph.

## Effect on this line

Two stage sources moved, `codegen.kel` and `parse.kel`, and their digests in
`corpus_fingerprint.rs` were re-pinned. Neither change touches the reference
compiler or verifier, so no native cost or bound figure is expected to move,
and the run below tests that expectation through the pinned figures.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-99/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 28 passed |

No pinned native cost or bound figure moved, which agrees with the expectation
above. The full-language count rose from 26 to 28 because upstream added two
tests. This table was added after the run. The tests that read this record
were then rerun, `prediction_stamp`, `outstanding_reports`, `handoff_figures`
and the native and root `comment_citations`, with the root
`documentation_links`. The root workspace suite was not run locally. The
hosted run on the pull request is the integration check.
