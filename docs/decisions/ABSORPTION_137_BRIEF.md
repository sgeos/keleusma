# Absorption 137

> **Navigation** [Decisions](./README.md) | [Absorption 136](./ABSORPTION_136_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `3ab52b86`, which is pull request 738 merged into
`v0.3.0`, and upstream `ce897a5b` on 2026-11-05, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-137`
with `--no-ff --no-commit`. The absorbed upstream pull request is this.

- **736** unifies the native gates into root continuous integration, on the
  operator's ruling of 2026-11-04 that CI be unified. The two jobs that ran
  `native_codegen/tools/backend-gate.sh` in each float configuration move
  from `.github/workflows/native.yml`, which is deleted, into `ci.yml` under
  `CI result`, and run whenever `scripts/ci_classify.py` reports a native
  input changed. `native_workflow.rs`, the native README, `CLAUDE.md` and the
  gate policy and git strategy follow.

## Merge and effect

Two files conflicted. V0.2.X had re-pinned its older copy of `native_codegen`
when the native gate began running on `v0.2.3`.

- `corpus_fingerprint.rs`. Both lines recorded the same re-pins of upstream
  723, 725 and 727, and the pinned values merged without conflict. Both
  notes are kept.
- `kel_native_cli.rs`. Both lines moved the out-of-subset witness to the
  same enum pair. This line's stream form is kept, so the native route the
  test runs lowers a stream, with upstream's more exact description that the
  self-hosted parser refuses a payload whose enum is declared after the enum
  holding it.

`CLAUDE.md` keeps this line's routing paragraph. `ci.yml` triggers on every
`v*` branch, so `v0.3.0` keeps its required `CI result` check, which now
includes the native gates. The interline checkpoint is not exported, at
Kaizen's request of sequence 295.

## Verification

Made with `RUSTC_WRAPPER=` at the merge commit. The change is continuous
integration, documentation and eight native test and tool files, so the
checks are targeted at it.

| Check | Result |
|---|---|
| Native `corpus_fingerprint`, `kel_native_cli`, `native_workflow`, `comment_citations` and `spike_native_frame` | 21 passed |
| `shellcheck native_codegen/tools/*.sh` and `scripts/test_ci_classify.py` | passed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage`, `gate_ci_correspondence`, `state_file_budget` and `ci_light_path_coverage` | 25 passed |

The hosted run on the pull request runs both native gates in full.
