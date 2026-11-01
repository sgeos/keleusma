# Absorption 93

Computed against native `e7d8524a` and upstream `830bf20f` on 2026-10-25,
with 8 unabsorbed commits measured after fetching. The merge base is
`c2733d0a`. The separate conflict command
`git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited one and
produced tree `c0391c525387010fcbdc1cab4cf33826ebd09d99` with one conflicting
file, `docs/process/REVERSE_PROMPT.md`. The merge was prepared on
`feat/v3-absorption-93` with `--no-ff --no-commit`.

## What was absorbed

V0.2.X Order 4, the hosted runtime, merged upstream as pull request 521 at
`7d4bc434` with its record commit `830bf20f`.
[`ORDER4_COMPLETION_AUDIT.md`](./ORDER4_COMPLETION_AUDIT.md) records it.
`src/hosted/interp.kel` is a bytecode interpreter written in Keleusma, and
`src/hosted/mod.rs` drives it as `keleusma::hosted` behind the `self-host`
feature, with `keleusma::hosted::transitive_bound` as a new public function.
Upstream reports no change to the verifier, the virtual machine, opcodes, the
bytecode version or the wire format. V0.2.X reported the merge in
`v023-20261025-order4-complete`, which was read and acknowledged.

## Conflict resolution

Upstream rewrote its bounded reverse prompt to 56 lines. This line's side is
the 47-line merge-base file, checked byte for byte, followed by its 668
appended lines. The resolution keeps upstream's file and appends those lines
unchanged, and the result was compared with both parts byte for byte.
`CLAUDE.md` auto-merged, and it differs from upstream's only by this line's
routing paragraph.

## Effect on this line

None measured. The default native suite passed 809 of 809 on the staged merge.
The corpus fingerprint did not move, because the native corpus roots are
`examples/scripts`, `src/selfhost/kel`, `examples/rtos/scripts` and
`compiler/kel`, and `src/hosted/` is none of them. The interpreter is therefore
not lowered or differential-tested natively. The V0.3.X roadmap's entry
baseline names a hosted runtime as an executable specification native code must
preserve. Adding it to the native corpus is a possible later increment, not part
of this absorption.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` on the staged
merge. Logs are under ignored `tmp/order-3/a93/`.

| Check | Result |
|---|---|
| `cargo fmt --check`, `clippy --all-targets -D warnings` in both float configurations, `cargo doc -D warnings`, ShellCheck | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 809 passed, 0 failed |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 809 passed, 0 failed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage`, `gate_ci_correspondence` | 16 passed |

The tests that read this receipt and the handoff were rerun after it was
completed. The root workspace suite, the self-host tier and upstream's new
`tests/hosted_runtime.rs` were not run locally. Hosted CI runs them.

The hosted CI run on the absorption pull request is the version-branch
integration check. No new full-gate record was written. Publication is held and
no merge into `main` is authorized.
