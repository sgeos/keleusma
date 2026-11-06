# Absorption 139

> **Navigation** [Decisions](./README.md) | [Absorption 138](./ABSORPTION_138_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `cbb28cef`, which is pull request 741 merged into
`v0.3.0`, and upstream `f372463d` on 2026-11-05, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-139`
with `--no-ff --no-commit`. The absorbed upstream pull request is this.

- **742** brings `Text<N>` and its methods into the self-hosted pipeline,
  changing `codegen.kel`, `lexer.kel`, `parse.kel` and `reconstruct.kel`.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, and the
interline checkpoint is not touched, which keeps Kaizen's request of sequence
295. V0.2.X re-pinned `native_codegen/tests/corpus_fingerprint.rs` for the
four stages itself, now that the native gate runs on `v0.2.3`, and the pins
pass here. The population is unchanged.

## Verification

Made with `RUSTC_WRAPPER=` at `84cfa16c`.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 880 passed, 0 failed, 1 ignored each |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 21 passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | 62 and 124 passed |

The root workspace suite was not run locally. The hosted run on the pull
request covers it.
