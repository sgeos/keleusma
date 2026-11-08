# Absorption 92

Computed against native `25822438` and upstream `c2733d0a` on 2026-10-25,
with 2 unabsorbed commits measured after fetching. The merge base is
`50b20ef6`. The separate conflict command
`git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited one and
produced tree `ffc092df2185e60356d7c5e0418f1b814f771972` with one conflicting
file, `docs/process/REVERSE_PROMPT.md`. The merge was prepared on
`feat/v3-absorption-92` with `--no-ff --no-commit`.

## What was absorbed

Upstream pull request 519, merged as `c2733d0a`, which
[`WCET_STRUCTURAL_MARKERS.md`](./WCET_STRUCTURAL_MARKERS.md) records. The
verifier's time walk now charges `Else`, `EndIf` and `EndLoop`, and a counted
loop adds its costliest break path to its full passes. `src/selfhost/kel/analyze.kel`
mirrors the change. Verified time bounds involving `if` or a loop rise. Memory
bounds, opcodes, the bytecode version and the wire format are unchanged. V0.2.X
reported the merge in `v023-20261025-pr519-merged`, which was read and
acknowledged.

## Conflict resolution

Upstream rewrote its bounded reverse prompt to 47 lines. This line's side is
the 50-line merge-base file, checked byte for byte, followed by its 668 appended
lines. The resolution keeps upstream's file and appends those 668 lines
unchanged, and the result was compared with both parts byte for byte.

## Effect on this line

Measured on the staged merge in the default configuration, the native suite
passed 808 tests and failed 1. The failure was
`the_corpus_is_what_the_pinned_figures_were_measured_against`, which reported
`analyze.kel` modified and nothing added or removed. The five censuses it names
to re-derive passed in the same run with their pinned figures, so the pin was
updated in `corpus_fingerprint.rs` with that reason.

No native test pins a verifier time figure. The motor policy example prints
the bytecode cost and labels it as a virtual-machine figure. Its README quoted
19 and 134 cost units and a 352-byte stack, and the example now prints 22 and
339 units and 512 bytes. The quote was updated. The cost rise follows this
absorption. The stack difference does not, by upstream's report that memory
bounds are unchanged, and its cause was not measured. The Order 3
static-library contract exports no time figure, so it is unaffected.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` on the staged
merge after the re-pin. Logs are under ignored `tmp/order-3/a92/`.

| Check | Result |
|---|---|
| `cargo fmt --check`, `clippy --all-targets -D warnings` in both float configurations, `cargo doc -D warnings`, ShellCheck | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 809 passed, 0 failed |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 809 passed, 0 failed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage`, `gate_ci_correspondence` | 16 passed |

The tests that read this receipt and the handoff were rerun after it was
completed. The root workspace suite and the self-host tier were not run
locally.

The hosted CI run on the absorption pull request is the version-branch
integration check. No new full-gate record was written. Publication is held and
no merge into `main` is authorized.
