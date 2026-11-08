# Absorption 148

> **Navigation** [Decisions](./README.md) | [Absorption 147](./ABSORPTION_147_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `a2191c58`, which is pull request 767 merged into
`v0.3.0`, and upstream `502d2385` on 2026-11-08, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-148`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **766** is the proofs line's Rocq mechanisation under
  `docs/proofs/rocq/region_core/`, with its check script, and updates to the
  region reuse mechanisation.
- **756** adds Kaizen's `scripts/metrics.py` with its test, and updates
  `scripts/loc.sh` and its test.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
under `src/` or `native_codegen/` changed, and the interline checkpoint is not
touched.

## Verification

Made with `RUSTC_WRAPPER=` on the complete records. The change is
documentation and scripts, so the checks are targeted at it, and the Rocq
check was not run here.

| Check | Result |
|---|---|
| `shellcheck scripts/loc.sh`, `scripts/test_loc.py` and `scripts/test_metrics.py` | passed |
| Root `documentation_links`, `comment_citations` and `state_file_budget` | 10 passed |
| Native `handoff_figures` | 6 passed |

The hosted run on the pull request covers the rest.
