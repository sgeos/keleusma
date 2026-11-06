# Absorption 142

> **Navigation** [Decisions](./README.md) | [Absorption 141](./ABSORPTION_141_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `ab2073ee`, which is pull request 749 merged into
`v0.3.0`, and upstream `f01c0ba2` on 2026-11-06, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-142`
with `--no-ff --no-commit`. The absorbed upstream pull request is this.

- **748** is V0.2.X's handoff at `7bc3a7f4` after the cryptography rulings, in
  `docs/process/HANDOFF.md`, `docs/process/TASKLOG.md` and V0.2.X's section of
  `docs/process/REVERSE_PROMPT.md`.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
under `src/` or `native_codegen/` changed, and the interline checkpoint is not
touched.

**The reverse prompt no longer needs rebuilding.** Until the unification
through `main`, this line spliced its block onto upstream's file with
`tmp/rebuild_rp.sh`. Upstream's file now carries the block, under the heading
it was appended with, and V0.2.X leaves it as this line wrote it. The merged
file starts with upstream's new section, and the block that follows it is
byte-identical to the block before the merge, 723 lines, compared directly.

## Verification

Made with `RUSTC_WRAPPER=` on the merge. The change is documentation only.

| Check | Result |
|---|---|
| The reverse prompt block before and after the merge | identical |
| Root `documentation_links`, `comment_citations` and `state_file_budget` | 10 passed |
| Native `handoff_figures` | 6 passed |

The hosted run on the pull request covers the rest.
