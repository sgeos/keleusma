# Absorption 147

> **Navigation** [Decisions](./README.md) | [Absorption 146](./ABSORPTION_146_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `981b1166`, which is pull request 761 merged into
`v0.3.0`, and upstream `9bebdbdc` on 2026-11-08, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-147`
with `--no-ff --no-commit`. The absorbed upstream pull request is this.

- **762** is the proofs line's Rocq mechanisation of core bounds, under
  `docs/proofs/rocq/core_bounds/`, with its check script, and small updates to
  the region reuse mechanisation and the proof line's handoff.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
outside `docs/` changed, and the interline checkpoint is not touched.

## Verification

Made with `RUSTC_WRAPPER=` on the complete records. The change is
documentation only, and the Rocq check was not run here.

| Check | Result |
|---|---|
| Root `documentation_links`, `comment_citations` and `state_file_budget` | 10 passed |
| Native `handoff_figures` | 6 passed |

The hosted run on the pull request covers the rest.
