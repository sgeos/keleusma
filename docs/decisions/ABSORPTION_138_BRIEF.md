# Absorption 138

> **Navigation** [Decisions](./README.md) | [Absorption 137](./ABSORPTION_137_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `629f90ad`, which is pull request 739 merged into
`v0.3.0`, and upstream `cc4e62a0` on 2026-11-05, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-138`
with `--no-ff --no-commit`. The absorbed upstream pull request is this.

- **740** is Encoding F with the H2 schema hash, on the operator's array
  design rulings. The wire carries one data field record per declared field,
  `DataLayout` is field-based with one initialiser per field, the retired
  region kinds are refused, and every hash of a module with an array changes.
  `BYTECODE_VERSION` stays 2 and the opcode count stays 66.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, and the
interline checkpoint is not touched, which keeps Kaizen's request of sequence
295.

**This line's adaptation is part of it.** By the sequence V0.2.X proposed in
sequence 254 and this line accepted in sequence 275, this line adapted
`native_codegen` on a branch of V0.2.X's, as `aa4873bc`, and V0.2.X reviewed
and merged it before 740 merged, so the unified tree never held a
`native_codegen` that did not build. The lowering stays slot-indexed.
`native_codegen/src/data_view.rs` expands the fields into the per-slot shared,
private pool and initialiser tables once per module, and the per-operation
analyses use `DataLayout`'s own lookups. Two tests that corrupt a layout on
purpose split a field so that one element moves. The `verify_datalayout.kel`
driver in `stage_differential.rs` feeds the stage its field rows in batches of
256, with four real layouts, six constructed ones and a batch boundary, and
the stage returned to `corpus_differential`'s known vacuous set. Every target
built on the merged tree, including the native work this line merged after
the copy the adaptation was made on.

## Verification

Made with `RUSTC_WRAPPER=` at `15f4c733`.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 880 passed, 0 failed, 1 ignored each |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 21 passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | 62 and 123 passed |

The two more native tests than absorption 137 are the unit tests of
`data_view.rs`. The root workspace suite was not run locally. The hosted run
on the pull request covers it.
