# Absorption 135

> **Navigation** [Decisions](./README.md) | [Absorption 134](./ABSORPTION_134_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `f0daae75`, which is pull request 733 merged into
`v0.3.0`, and upstream `e0488c25` on 2026-11-04, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-135`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **730** adds V0.2.X's `docs/decisions/NESTED_LOOP_ARENAS.md` and seven
  measurement tests in `tests/delegated_loop_reset.rs`, with no runtime
  change. An inner loop's Reset reclaims the arena top region, the outer
  frame's composite body included, and no soundness defect was found in
  admitted programs, since the outer loop never runs again after it
  delegates. Seven questions are open with the operator.
- **731** and **732** are Kaizen's ruling record and its handling of orphaned
  scratch links in `scripts/worktree.sh`.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, and no
fingerprint moved. V0.2.X noted that it read this line's
`rebind_delegate_parameter` without running it. Three of 730's programs,
an outer loop that holds a sixteen-element array, writes private data, or
yields once before delegating, now also run natively in
`delegated_loop_resets_rebind_the_innermost_parameter_like_the_vm`, and agree
with the virtual machine at both optimization levels. The other four tests
measure the virtual machine's arena or the type checker's refusals.

The interline checkpoint is not exported. Kaizen asked in sequence 295 that
no line export it until the operator rules on six messages of another project
in the shared store that match a disclosure term. No absorbed change touches
the checkpoint.

## Verification

Made with `RUSTC_WRAPPER=` on the records tree. The change is one native test, documentation and
scripts, so the checks are targeted at it.

| Check | Result |
|---|---|
| `retcon_bytecode`, default features and `narrow-float-32` | 100 passed each |
| Native `corpus_fingerprint`, `handoff_figures`, `test_population_guard` and `comment_citations` | 18 passed |
| Native fmt and clippy in both float configurations | passed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 21 passed |
| `scripts/test_worktree_scratch.py` and `shellcheck scripts/worktree.sh` | passed |

The native suites were not run in full. The hosted run on the pull request
runs both native gates and the root workspace suite.
