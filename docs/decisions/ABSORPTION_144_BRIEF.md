# Absorption 144

> **Navigation** [Decisions](./README.md) | [Absorption 143](./ABSORPTION_143_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `1dc5c065`, which is pull request 753 merged into
`v0.3.0`, and upstream `6f0378b7` on 2026-11-07, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-144`
with `--no-ff --no-commit`. The absorbed upstream pull request is this.

- **752** adds Kaizen's footprint tracking: a detached bare-metal package
  `footprint/` built by `scripts/footprint.sh` against a budget in
  `footprint/budget.tsv`, measuring what embedding the runtime costs on the
  Cortex-M55, with its test, a CI step and a metrics record.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
under `src/` or `native_codegen/` changed, and the interline checkpoint is
not touched. The root `Cargo.toml` change only excludes `footprint/` from the
published package. The native workspace guards, `workspace_coverage_guard`
and `inherited_feature_declaration`, pass with the new detached package
present.

## Verification

Made with `RUSTC_WRAPPER=` on the merge. The change is a detached package,
scripts and continuous integration, so the checks are targeted at it.

| Check | Result |
|---|---|
| `shellcheck scripts/footprint.sh scripts/loc.sh` and `scripts/test_footprint.py` | passed |
| Root `documentation_links`, `gate_ci_correspondence`, `ci_light_path_coverage`, `ci_shard_coverage` and `comment_citations` | 25 passed, 0 failed |
| Native `workspace_coverage_guard`, `inherited_feature_declaration`, `corpus_fingerprint` and `handoff_figures` | 18 passed |

The footprint package itself was not built locally, since it needs the
Cortex-M55 target. The hosted run on the pull request builds it.
