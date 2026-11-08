# Absorption 116

> **Navigation** [Decisions](./README.md) | [Absorption 115](./ABSORPTION_115_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `12b06a67`, which is pull request 622 merged into
`v0.3.0`, and upstream `95dec782` on 2026-10-30, with 42 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-116`
with `--no-ff --no-commit`. Upstream pull request 625 landed after the fetch
and is left to the next absorption.

The absorbed pull requests are these.

| Upstream | Content |
|---|---|
| 594, 592, 593 | Dependency updates, among them `syn` 3 for the macro crates and, by V0.2.X's notice in sequence 146, `ed25519-dalek` 3 and `aes-gcm` 0.11 |
| 596 | The verifier rejects a native index past the native table at admission, in the reference and in `verify_structural.kel` |
| 598 | A callee-aware time bound for text callees, and `RUNTIME_FAULTS.md` rewritten to the Trap-only fault contract |
| 601 | A cast to the operand's own type lowers as identity, in the reference and in `parse.kel` |
| 603 | Root scripts no longer exit a pipe consumer early under `pipefail` |
| 615 | The verifier refuses backward targets at the module and whole-chunk entries |
| 605 | An in-memory hot swap is refused while a trust matrix is registered |
| 618 | The wrapping default of checked arithmetic is typed, and omitted with the last arm's class test when unguarded arms cover every class, in both compilers and `codegen.kel` |
| 620 | Kaizen's template hygiene rerun, which rewords the three phrases its sequence 141 named |
| 621 | `hosted::seeded_shared` and the `hosted::protocol` constants |

## Merge and effect

Only the interline checkpoint conflicted. It was regenerated from the mailbox
store and holds 148 messages, and none of upstream's 141 identifiers is
missing. `docs/process/REVERSE_PROMPT.md` did not change upstream.
`CLAUDE.md` keeps this line's routing paragraph. `native_codegen/Cargo.lock`
moves the two macro crates to `syn` 3.

`corpus_fingerprint` reported `codegen.kel`, `parse.kel` and
`verify_structural.kel` modified, and their digests were re-pinned with the
cause of each. No other corpus file moved.

One native test failed, and it failed for the reason sequence 136 was sent.
`an_operand_no_instruction_reads_needs_no_run_time_kind` required
`interp.kel` to stay dynamic at chunk 2 instruction 51. With the default
omitted, `interp.kel` is static, so the test now requires that it is static
and that it lowers with static kinds. The kind census reads 29 static
coroutines, 1 dynamic and 2 refused, with 1 of 188375 instructions needing a
run-time kind, against 28, 2 and 2 and 21 of 188131 before. The one dynamic
coroutine is `14_frame_log.kel`, which question 20 decides. The rulings plan,
question 20 and the Order 5 amendment record the new figures.

Questions 5 and 6 are unblocked. Question 7 is done on V0.2.X's side.

Kaizen's scan of sequence 141 was repeated on the merged tree, counting
matches per term of the pruned list with `git grep -c -I -i -F` and printing
no term. All 21 terms match no line.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` after the re-pins and the test change. Every step
passed, and `--locked` held for the native lockfile as recorded.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suite, default features | 855 passed, 0 failed |
| Native suite, `narrow-float-32` | 855 passed, 0 failed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis` and `rogue_scripts` | passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | passed |

The test change was checked against the old tree in effect, since the old
assertion failed on the merged tree. The root workspace suite was not run
locally. The hosted run on the pull request covers it.
