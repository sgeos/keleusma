# Absorption 100

> **Navigation** [Decisions](./README.md) | [Absorption 99](./ABSORPTION_99_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `c1d5cd78`, which is pull request 552 merged into
`v0.3.0`, and upstream `d84abde1` on 2026-10-26, with 3 unabsorbed commits
measured after fetching. The merge base is `32bc1903`. The separate conflict
command `git merge-tree --write-tree --name-only c1d5cd78 origin/v0.2.3`
exited zero and produced tree `ded7d0abce5386d3c459aa1982c97e032dd65761` with
no conflicting file. The commits are V0.2.X pull request 551, which lowers
variable arms on a scalar match. The merge was prepared on
`feat/v3-absorption-100` with `--no-ff --no-commit`.

## Merge

No file conflicted. The first 145 lines of `docs/process/REVERSE_PROMPT.md`
were compared byte for byte with upstream's file, and this line's block
follows them. `CLAUDE.md` keeps this line's routing paragraph. The brief also
records the merges of absorptions 98 and 99 and the checkpoint of 115
messages.

## Effect on this line

One stage source moved, `parse.kel`, and its digest in `corpus_fingerprint.rs`
was re-pinned. The change does not touch the reference compiler or verifier,
so no native cost or bound figure is expected to move, and the run below
tests that expectation through the pinned figures. Upstream also changed root
`tests/selfhost_typecheck.rs` and `keleusma-cli/tests/self_hosted_refusal.rs`,
so those two were run in addition to the usual set.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-100/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 29 passed |
| Root `selfhost_typecheck` with `self-host` | 103 passed |
| `keleusma-cli` `self_hosted_refusal` | 5 passed |

No pinned native cost or bound figure moved, which agrees with the expectation
above. The full-language count rose from 28 to 29 because upstream added one
test. This table was added after the run. The tests that read this record
were then rerun, `prediction_stamp`, `outstanding_reports`, `handoff_figures`
and the native and root `comment_citations`, with the root
`documentation_links`. The root workspace suite was not run locally. The
hosted run on the pull request is the integration check.
