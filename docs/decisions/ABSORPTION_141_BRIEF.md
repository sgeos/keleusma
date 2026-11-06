# Absorption 141

> **Navigation** [Decisions](./README.md) | [Absorption 140](./ABSORPTION_140_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `2ccb1892`, which is pull request 747 merged into
`v0.3.0`, and upstream `7bc3a7f4` on 2026-11-06, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-141`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **745** adds V0.2.X's `SELF_HOSTED_CRYPTOGRAPHY_PLAN.md` and its record of
  deferred features.
- **746** shards the C5 mutation corpus of `tests/selfhost_validator` into
  sixteen continuous integration partitions, with a shard coverage check in
  `tests/ci_shard_coverage.rs`, and updates the C5 review with the measured
  times. The verdict is unchanged, C5 met for the corpus scope, which counts
  13,305 mutants since Encoding F.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
under `src/` or `native_codegen/` changed, and the interline checkpoint is not
touched. This line's question 31, what C5 must establish, is still open with
the operator.

## Verification

Made with `RUSTC_WRAPPER=` on the merge. The change is documentation, one
root test and continuous integration, so the checks are targeted at it.

| Check | Result |
|---|---|
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage`, `gate_ci_correspondence`, `state_file_budget` and `ci_light_path_coverage` | 27 passed |
| `scripts/test_ci_classify.py` | passed |
| Native `handoff_figures` | 6 passed |

The hosted run on the pull request covers the rest.
