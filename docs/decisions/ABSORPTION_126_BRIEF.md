# Absorption 126

> **Navigation** [Decisions](./README.md) | [Absorption 125](./ABSORPTION_125_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `208d652e`, which is pull request 681 merged into
`v0.3.0`, and upstream `2065aa3e` on 2026-10-31, with 5 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-126`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **675**, Kaizen's, adds `.github/workflows/fuzz.yml`, a scheduled nightly
  fuzz run, and a CI step that checks the fuzz package's format and that its
  lockfile is current, with Kaizen's open-decisions record of 2026-10-31.
- **679**, V0.2.X's, splits the shipping validator test in
  `keleusma-cli/tests/validator.rs`.

No file under `src/` or `native_codegen/` changed upstream.

## Merge and effect

No file conflicted. `CLAUDE.md` keeps this line's routing paragraph,
`docs/process/REVERSE_PROMPT.md` did not change upstream, and the interline
checkpoint is unchanged at 178 messages. Kaizen's record lists no question
held by this line, which matches this line's records. The fuzz workflow runs
on a schedule only from `main`, so it does not run on `v0.3.0`.

## Verification

Made with `RUSTC_WRAPPER=` on this absorption.

| Check | Result |
|---|---|
| Native `corpus_fingerprint`, `native_workflow` and `reach_census` | passed, no digest moved |
| Native documentation readers | passed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |

The full native suites were not rerun, because nothing they compile changed.
The hosted run covers the root workspace and the new CI step.
