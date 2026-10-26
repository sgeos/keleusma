# Absorption 101

> **Navigation** [Decisions](./README.md) | [Absorption 100](./ABSORPTION_100_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `853c8c48`, which is absorption 100 on
`feat/v3-absorption-100` as pull request 555, and upstream `219d13b6` on
2026-10-26, with 2 unabsorbed commits measured after fetching. The merge base
is `d84abde1`. The separate conflict command
`git merge-tree --write-tree --name-only 853c8c48 origin/v0.2.3` exited zero
and produced tree `4c39eab45dd19039f692400136287495d94a370a` with no
conflicting file. The commits are V0.2.X pull request 553, which lowers
irrefutable tuple and struct destructuring arms. The merge was prepared on
`feat/v3-absorption-101`, cut from absorption 100, with `--no-ff --no-commit`.
This absorption is not on `v0.3.0` until pull request 555 merges first.

## Merge

No file conflicted. Upstream changed `docs/process/REVERSE_PROMPT.md`, and
the first 158 lines of the merged file were compared byte for byte with
upstream's file. This line's block follows them. `CLAUDE.md` keeps this
line's routing paragraph.

## Effect on this line

One stage source moved, `parse.kel`, and its digest in `corpus_fingerprint.rs`
was re-pinned. The change does not touch the reference compiler or verifier,
so no native cost or bound figure is expected to move, and the run below
tests that expectation through the pinned figures. Upstream also changed root
`tests/selfhost_typecheck.rs`, so it was run in addition to the usual set.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-101/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 30 passed |
| Root `selfhost_typecheck` with `self-host` | 103 passed |

No pinned native cost or bound figure moved, which agrees with the expectation
above. The full-language count rose from 29 to 30 because upstream added one
test. This table was added after the run. The tests that read this record
were then rerun, `prediction_stamp`, `outstanding_reports`, `handoff_figures`
and the native and root `comment_citations`, with the root
`documentation_links`. The root workspace suite was not run locally. The
hosted run on the pull request is the integration check.
