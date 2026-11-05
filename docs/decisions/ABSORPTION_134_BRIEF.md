# Absorption 134

> **Navigation** [Decisions](./README.md) | [Absorption 133](./ABSORPTION_133_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `ee56203b`, which is pull request 728 merged into
`v0.3.0`, and upstream `7a899120` on 2026-11-04, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-134`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **727** closes the two C5 soundness residuals in `verify_typed.kel`, a loop
  fixpoint for the locals a loop body writes and variable-length `Text`
  layouts, and revises the review in
  `docs/decisions/C5_SELFHOSTED_VALIDATOR_REVIEW.md`.
- **729** records V0.2.X's rulings of 2026-11-04 and corrects `GRAMMAR.md` to
  admit a loop calling a loop that shares the yield contract, with the
  independent-arena requirement recorded as an open design question.

## Merge and effect

No file conflicted, and `CLAUDE.md` keeps this line's routing paragraph.
`corpus_fingerprint` reported `verify_typed.kel` modified, which was
re-pinned. The population is unchanged at 849.

**C5.** The revised verdict is that C5 is met for the corpus scope. Over
13,307 corrupted modules the self-hosted validator accepts none that the
reference rejects and rejects none that the reference accepts, and the
residuals the review calls other stay open, among them the common-mode
transport tables. The operator asked on 2026-11-04 for C5's criterion to be
restated, which is this line's question 31, so Order 6 acceptance waits on
that answer rather than on further repair.

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
