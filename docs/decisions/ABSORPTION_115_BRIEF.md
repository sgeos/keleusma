# Absorption 115

> **Navigation** [Decisions](./README.md) | [Absorption 114](./ABSORPTION_114_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `95d102b4`, which is pull request 617 merged into
`v0.3.0`, and upstream `fc38cc9b` on 2026-10-30, with 7 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-115`
with `--no-ff --no-commit`. The commits are upstream pull requests 587, 588
and 591.

- Pull request 587 records operator rulings of 2026-10-28 in V0.2.X's
  records. V0.2.X records them as first-hand. By that record the operator
  accepted the first-pass gates of all six orders of the V0.2.X roadmap, and
  `CLAUDE.md` no longer says that Orders 5 and 6 await acceptance. This line
  did not witness the rulings and records them as V0.2.X's account.
- Pull request 588 ends decryption at the declared total length rather than
  at the end of the supplied buffer, in both `src/wire_format.rs` and
  `src/crypto_frame/frame.kel`, and adds a test to `tests/crypto_framing.rs`.
- Pull request 591 is a Dependabot update to the editor extension's lock file.

## Merge and effect

Nothing conflicted. `docs/process/REVERSE_PROMPT.md` merged on its own and
was rebuilt by `tmp/rebuild_rp.sh` against `c6366172`, which reproduced the
same file, upstream's 299 lines followed by this line's 720. `CLAUDE.md`
keeps this line's routing paragraph.

`frame.kel` is one of the fourteen stages and a native corpus root, so
`corpus_fingerprint` reported it modified. Its digest was re-pinned to
`0x94e3eec49e2f8451`. No other corpus file moved.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` after the digest was re-pinned. Every step passed.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suite, default features | 855 passed, 0 failed |
| Native suite, `narrow-float-32` | 855 passed, 0 failed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis` and `rogue_scripts` | passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | passed |

The native suites include the corpus figures that `corpus_fingerprint` names
for re-derivation, and none of them moved. The root workspace suite was not
run locally. The hosted run on the pull request covers it.

The same branch writes one absolute home directory path in this line's
archive with `~`, as Kaizen asked in sequence 141.
