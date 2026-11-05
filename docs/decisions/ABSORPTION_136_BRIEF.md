# Absorption 136

> **Navigation** [Decisions](./README.md) | [Absorption 135](./ABSORPTION_135_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `1b00535c`, which is pull request 734 merged into
`v0.3.0`, and upstream `d183fe24` on 2026-11-05, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-136`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **735** adds `tests/state_file_budget.rs`, which holds a per-line state file
  that declares a budget to it, and a step in `ci.yml` that runs it.
- **737** records Kaizen's follow-ups of 2026-11-05 and updates the pilot's
  selection checkpoint.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, and no
fingerprint moved. Nothing under `src/` or `native_codegen/` changed. The new
budget test checks only a file that states a budget, and this line's archive
states none, so it is unaffected. The interline checkpoint is still not
exported, at Kaizen's request of sequence 295.

## Verification

Made with `RUSTC_WRAPPER=` on the records tree. The change is documentation, one
root test and one CI step, so the checks are targeted at it.

| Check | Result |
|---|---|
| Root `state_file_budget`, `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 23 passed |
| Native `handoff_figures` and `comment_citations` | 14 passed |

The hosted run on the pull request covers the rest.
