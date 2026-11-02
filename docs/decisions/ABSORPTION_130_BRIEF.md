# Absorption 130

> **Navigation** [Decisions](./README.md) | [Absorption 129](./ABSORPTION_129_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `690ce30b`, which is pull request 715 merged into
`v0.3.0`, and upstream `7c76d771` on 2026-11-01, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-130`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **710** records Kaizen's second ruling of 2026-11-01 and its open
  decisions, and adds `scripts/release_manifest.py` with its test, a
  release-signing proposal, and a change to `scripts/worktree.sh` with its
  test.
- **711** records the first run of Kaizen's nightly fuzz workflow in its
  handoff.
- **716** is V0.2.X's merge of `main` back into `v0.2.3` after pull request
  714. Since `main` held this line's work, it brings nothing new here beyond
  V0.2.X's interline checkpoint.

## Merge and effect

Only the interline checkpoint conflicted, and it was resolved by a fresh
export from the shared store, which holds 250 messages and matched no
disclosure term. `CLAUDE.md` keeps this line's routing paragraph. Nothing
under `src/`, `tests/` or `native_codegen/` changed, and no fingerprint moved.

## Verification

Made with `RUSTC_WRAPPER=`. The change is documentation and scripts, so the
checks are targeted at it.

| Check | Result |
|---|---|
| `scripts/test_release_manifest.py` and `scripts/test_worktree_scratch.py` | passed |
| `shellcheck scripts/worktree.sh` | passed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 21 passed |
| Native `handoff_figures` and `comment_citations` | 14 passed |

The native suites were not run, since nothing they read changed. The hosted
run on the pull request covers the rest.
