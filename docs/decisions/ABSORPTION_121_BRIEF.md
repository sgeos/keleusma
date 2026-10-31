# Absorption 121

> **Navigation** [Decisions](./README.md) | [Absorption 120](./ABSORPTION_120_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `51a4a645`, which is pull request 659 merged into
`v0.3.0`, and upstream `cfeac9cd` on 2026-10-30, with 11 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-121`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **652** is V0.2.X's batch of three fixes.
  - `src/verify.rs` refuses a float opcode on a build without floats.
  - `keleusma-cli` refuses to compile a module whose memory bound cannot be
    computed.
  - `src/compiler.rs` refuses a private composite slot beyond the 16-bit
    record.
- **656** stops Dependabot moving the `rust-toolchain` pins and records two
  operator confirmations as relayed by Kaizen.

## Merge and effect

No file conflicted. `CLAUDE.md` keeps this line's routing paragraph, and
`docs/process/REVERSE_PROMPT.md` did not change upstream, so it was not
rebuilt. The interline checkpoint was regenerated from the mailbox store at
165 messages.

`corpus_fingerprint` passed with no digest moved, and no native figure moved.
This line infers from the passing suites, and did not separately measure,
that the float refusal does not reach the native corpus, which builds with
floats in both configurations, and that no corpus module declares a private
composite slot beyond the 16-bit record.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` on this absorption. Every step passed.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suite, default features | 863 passed, 0 failed, 1 ignored |
| Native suite, `narrow-float-32` | 863 passed, 0 failed, 1 ignored |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis` and `rogue_scripts` | passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | passed |

The records were written after that run, and the documentation readers were
rerun on them. The root workspace suite was not run locally. The hosted run on
the pull request covers it.
