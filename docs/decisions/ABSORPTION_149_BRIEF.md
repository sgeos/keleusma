# Absorption 149

> **Navigation** [Decisions](./README.md) | [Absorption 148](./ABSORPTION_148_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `e4483396`, which is pull request 768 merged into
`v0.3.0`, and upstream `d32f7491` on 2026-11-08, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-149`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **769** and **770** are the proofs line's further Rocq mechanisation under
  `docs/proofs/`, and a detached differential test tool,
  `tools/proof-difftest/`, with its own `Cargo.toml` and lockfile, and a line
  in `tools/README.md`.
- **772** applies the shared machine's playbook to the heavy jobs, with
  `scripts/test_machine_guard.py`.
- **773** fixes `scripts/metrics.py`, whose parse of `cloc --json` failed on
  the continuous integration runner.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
under `src/` or `native_codegen/` changed, the root `Cargo.toml` is unchanged,
and the interline checkpoint is not touched. The native workspace guards,
`workspace_coverage_guard` and `inherited_feature_declaration`, pass with the
new detached package present.

**The first run of this pull request failed, and the cause was upstream.**
`v0.2.3` failed its "Runtime footprint and metrics" job from `a4eba5bd`
onwards, with `scripts/metrics.py` unable to parse the output of `cloc --json`.
It did not reproduce here with `cloc` 2.10. This line reported it to Kaizen
and V0.2.X in sequences 402 and 403, held the pull request, and absorbed
Kaizen's fix 773 into it when it landed.

## Verification

Made with `RUSTC_WRAPPER=` on the complete records. The change is
documentation and a detached tool this line does not build, so the checks are
targeted at it, and neither the Rocq check nor the tool was run here.

| Check | Result |
|---|---|
| Native `workspace_coverage_guard` and `inherited_feature_declaration` | 9 passed |
| `scripts/test_metrics.py`, `scripts/test_machine_guard.py` and `shellcheck scripts/*.sh` | passed |
| Root `documentation_links`, `comment_citations` and `state_file_budget` | 10 passed |
| Native `handoff_figures` | 6 passed |

The hosted run on the pull request covers the rest.
