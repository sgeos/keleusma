# Absorption 150

> **Navigation** [Decisions](./README.md) | [Absorption 149](./ABSORPTION_149_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `68c7a99a`, which is pull request 771 merged into
`v0.3.0`, and upstream `cf5c8d1f` on 2026-11-08, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-150`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **775** adds the proofs line's Kani harnesses for the verifier and the wire
  format, in the detached tool `tools/proof-kani/`, with its check script.
- **774** gives `cloc` a timeout in `scripts/loc.sh` and `scripts/metrics.py`,
  with the test.
- **776** is Kaizen's plan for running the proofs in continuous integration.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
under `src/` or `native_codegen/` changed, the root `Cargo.toml` is unchanged,
and the interline checkpoint is not touched. The native workspace guards pass
with the new detached tool present.

## Verification

Made with `RUSTC_WRAPPER=` on the complete records. The change is a detached
tool, scripts and documentation, so the checks are targeted at it, and the
Kani harnesses were not run here.

| Check | Result |
|---|---|
| `scripts/test_metrics.py` and `shellcheck scripts/loc.sh tools/proof-kani/check.sh` | passed |
| Native `workspace_coverage_guard` and `inherited_feature_declaration` | 9 passed |
| Root `documentation_links`, `comment_citations`, `state_file_budget` and `gate_ci_correspondence` | 14 passed |
| Native `handoff_figures` | 6 passed |

The hosted run on the pull request covers the rest.
