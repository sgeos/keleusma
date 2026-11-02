# Absorption 132

> **Navigation** [Decisions](./README.md) | [Absorption 131](./ABSORPTION_131_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `1a597b61`, which is pull request 724 merged into
`v0.3.0`, and upstream `066fdd6a` on 2026-11-02, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-132`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **720** keys trait impls on `Text`, `Fixed` and `Multiword` by type family
  in the type checker.
- **723** closes five self-hosted coverage residuals byte-identically, typed
  let patterns among them, in `lexer.kel`, `parse.kel` and `codegen.kel`.

## Merge and effect

No file conflicted, and `CLAUDE.md` keeps this line's routing paragraph.
`corpus_fingerprint` reported `codegen.kel`, `lexer.kel` and `parse.kel`
modified by 723, which were re-pinned, and the derived figures it names
passed in the same verification run. With typed let patterns in the subset,
the out-of-subset witness in `kel_native_cli.rs` follows upstream's own
witness in `tests/self_hosted_backend.rs` to a variant whose payload holds an
enum, which the self-hosted parser does not model. The population is
unchanged at 849.

## Verification

Made with `RUSTC_WRAPPER=` at the records commit.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | FILL |
| Native suites, default features and `narrow-float-32` | FILL |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | FILL |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | FILL |

The root workspace suite was not run locally. The hosted run on the pull
request covers it.
