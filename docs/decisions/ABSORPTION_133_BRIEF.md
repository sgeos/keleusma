# Absorption 133

> **Navigation** [Decisions](./README.md) | [Absorption 132](./ABSORPTION_132_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `33cd50e5`, which is pull request 726 merged into
`v0.3.0`, and upstream `ff48e35c` on 2026-11-04, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-133`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **722** adds narrow-word coverage to `tests/text_ops.rs`.
- **725** is V0.2.X's C5 review of the self-hosted validator, recorded in
  `docs/decisions/C5_SELFHOSTED_VALIDATOR_REVIEW.md`, with a mutation corpus
  of 10,703 corrupted modules in `tests/selfhost_validator/c5_corpus.rs` and
  repairs to three typed-stage defects in `verify_typed.kel`.

## Merge and effect

No file conflicted, and `CLAUDE.md` keeps this line's routing paragraph.
`corpus_fingerprint` reported `verify_typed.kel` modified, which was
re-pinned. The population is unchanged at 849.

**The C5 review answers this line's question 27 and does not close it.** Its
verdict is that C5 is not met. Over its corpus the self-hosted validator now
accepts no module the reference rejects, where it accepted 41 before the
repairs, and two soundness residuals remain open, each pinned by a test that
asserts the present disagreement. The operator's ruling, recorded there, makes
C5 a V0.3.0 condition for both lines with V0.2.X owning the review, so this
line's Order 6 acceptance waits on those two residuals.

## Verification

Made with `RUSTC_WRAPPER=` at `319ba2dc`. The figures were written after the
run, and only this file changed afterwards.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 878 passed, 0 failed, 1 ignored each |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 21 passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | 62 and 123 passed |

The root workspace suite was not run locally. The hosted run on the pull
request covers it.
