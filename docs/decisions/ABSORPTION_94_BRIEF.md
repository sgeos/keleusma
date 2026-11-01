# Absorption 94

Computed against native `f1885bf2` and upstream `fde7b65e` on 2026-10-25,
with 7 unabsorbed commits measured after fetching. The merge base is
`830bf20f`. The separate conflict command
`git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited one and
produced tree `f78e643ff983922ddfaa9c56bfae9880ac4523f5` with two conflicting
files, `docs/process/REVERSE_PROMPT.md` and
`docs/process/interline/checkpoint.json`. The merge was prepared on
`feat/v3-absorption-94` with `--no-ff --no-commit`, after this line's Order 4
merged as pull request 527 at `f1885bf2`.

## What was absorbed

Three upstream pull requests.

- Pull request 524, merged as `ebfd1071`, refuses a signed header whose
  declared total is shorter than the signature it carries, recorded in
  [`SIGNATURE_VERIFY_BOUNDS.md`](./SIGNATURE_VERIFY_BOUNDS.md).
- Pull request 525, merged as `f31d7a72`, is Kaizen's second Jev shadow
  cohort, `scripts/jev_selection.py`, with its configuration and store. Kaizen
  reported it in `kaizen-20261025-pr525-v0.3.0`, which was read and
  acknowledged.
- Pull request 526, merged as `607c0328` with its record commit `fde7b65e`,
  is V0.2.X Order 5, the signing and encryption framing written in Keleusma at
  `src/crypto_frame/frame.kel` and driven by `keleusma::crypto_frame`,
  recorded in [`ORDER5_COMPLETION_AUDIT.md`](./ORDER5_COMPLETION_AUDIT.md).
  V0.2.X reported it in `v023-20261025-order5-merged-v0.3.0`, which was read
  and acknowledged.

Upstream reports no change to opcodes, the bytecode version or the wire format
beyond the refused short header. V0.2.X's Order numbers differ from this
line's, so its Order 5 is unrelated to this line's Order 5.

## Conflict resolution

Upstream rewrote its bounded reverse prompt to 50 lines. This line's side is
the 56-line merge-base file, checked byte for byte, followed by its 688
appended lines, which now include the Order 4 report. The resolution keeps
upstream's file and appends those lines unchanged, and the result was compared
with both parts byte for byte. The checkpoint was regenerated from the live
store and holds 96 messages after the replies of this absorption. `CLAUDE.md` auto-merged, and it keeps this line's
routing paragraph.

## Effect on this line

`src/crypto_frame/frame.kel` lies outside the native corpus roots, so no
corpus figure moves and it is not lowered here. Whether to add it, with the
hosted interpreter from absorption 93, is an open question for a later
increment.

## Verification

The figures are in the table below, run locally with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` on the staged merge, which did not change during
the run.

| Check | Result |
|---|---|
| `cargo fmt --check` in `native_codegen/` | status zero |
| `cargo clippy --locked --all-targets -- -D warnings`, default and `narrow-float-32` | status zero in both |
| `cargo doc --locked --no-deps` with `RUSTDOCFLAGS="-D warnings"` | status zero |
| `shellcheck tools/*.sh examples/lexer_host/run.sh` | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 827 passed, 0 failed, status zero |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 827 passed, 0 failed, status zero |
| Root `--test documentation_links --test comment_citations --test ci_shard_coverage --test gate_ci_correspondence` | 16 passed, status zero |

The checkpoint was regenerated after the run to include this line's replies,
and the guards that read this receipt and the handoff were rerun afterwards.
These checks do not run the root workspace suite, the self-host tier or the
backend gate's optimised phase. The hosted run on the pull request is the
version-branch integration check.
