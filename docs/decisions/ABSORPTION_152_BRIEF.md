# Absorption 152

> **Navigation** [Decisions](./README.md) | [Absorption 151](./ABSORPTION_151_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against upstream `4accffbe` on 2026-11-19, with 0 unabsorbed commits
measured after fetching. The merge was made on `docs/v3-resume-20261118`, after
absorption 151.

- **763**, **784** and **786** are Dependabot updates, to the GitHub Actions,
  the root dependencies and `keleusma-wasm`.
- **785** adds Kaizen's proofs CI job, with its classifier rule in
  `scripts/ci_classify.py`.
- **787** is Kaizen's handoff of 2026-11-18, with the interline checkpoint it
  exported after its export hold was lifted.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, and nothing
under `src/` or `native_codegen/` changed. The checkpoint was scanned with this
line's disclosure-term scan. Of its 559 messages none matches a term, and the
only home paths are the four the operator accepted.

## Verification

Made with `RUSTC_WRAPPER=` on the merge. The change is CI, scripts, dependency
updates and documentation, so the checks are targeted at it.

| Check | Result |
|---|---|
| `scripts/test_ci_classify.py` | passed |
| Root `documentation_links`, `gate_ci_correspondence`, `ci_shard_coverage`, `ci_light_path_coverage` and `comment_citations` | 25 passed |
| Native `handoff_figures` | 6 passed, 0 failed |

The hosted run on the pull request covers the rest.
