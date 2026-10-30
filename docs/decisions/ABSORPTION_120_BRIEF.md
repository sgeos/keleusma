# Absorption 120

> **Navigation** [Decisions](./README.md) | [Absorption 119](./ABSORPTION_119_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `4e14c895`, which is pull request 657 merged into
`v0.3.0`, and upstream `81926a75` on 2026-10-30, with 16 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-120`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **644** merges `main` back into `v0.2.3` after pull request 631, which
  records that Dependabot security updates are on.
- **647** completes Kaizen's R1 timing pass and lists its open decisions.
- **642 and 643** are Dependabot bumps of `vscode-languageclient` in
  `editors/vscode` and of `tokio` in `keleusma-lsp`.
- **649** changes `.github/workflows/ci.yml` on the operator's rulings of
  2026-10-30 as relayed by Kaizen in its notice of that date. A draft pull
  request's full run skips the self-host archive and shards, the
  `ready_for_review` event starts the run that includes them, and `CI result`
  fails a non-draft full run that skipped them. The Test job keeps only the
  `keleusma` tests no other job runs, pinned by `tests/ci_shard_coverage.rs`,
  and each test step lists the tests nextest retried, by
  `scripts/ci_flaky_report.py`.
- **654** is Kaizen's record of the change.

No file under `src/`, `tests/` other than `tests/ci_shard_coverage.rs`, or
`native_codegen/` changed upstream.

## Merge and effect

One file conflicted. The interline checkpoint was regenerated from the mailbox
store at 165 messages, and none of upstream's 164 identifiers is missing.
`CLAUDE.md` merged cleanly, takes upstream's draft rule, and keeps this line's
routing paragraph. `docs/process/REVERSE_PROMPT.md` did not change upstream,
so it was not rebuilt.

This line's pull requests are not opened as drafts, so the draft rule changes
nothing for them. `native.yml` is this line's own workflow and is unchanged.

## Verification

Made with `RUSTC_WRAPPER=` on this absorption.

| Check | Result |
|---|---|
| Native `corpus_fingerprint` | passed, no digest moved |
| Native documentation readers, `prediction_stamp`, `outstanding_reports`, `handoff_figures` and `comment_citations` | passed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| `scripts/test_ci_flaky_report.py` | passed |

The full native suites were not rerun, because nothing they compile or read
changed except the records above. The hosted run on the pull request covers
the root workspace.
