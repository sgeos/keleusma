# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 8 cleanup follow-on landed on the `V0.2.0-isa` branch. The two open concerns from the prior session round (live soft-warning trigger, narrow-bytecode-on-wide-runtime `CheckedXxx` flag and high half) are resolved. Repository hygiene tightened by ignoring `*.kel.bin` artefacts. R41 added rejecting the five-opcode dynamic-string-builder proposal. The branch is ready for merge to `main` and for the V0.2.0 publication step.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| `.gitignore` should ignore `*.kel.bin` and stale fixtures should be removed. | New `.gitignore` entry covers `*.kel.bin` with rationale about wire-format staleness across V0.2.x patch releases. Retired `examples/zero_copy_demo.kel.bin` and `examples/regenerate_zero_copy_bytecode.rs`. Rewrote `examples/zero_copy_include_bytes.rs` to compile the script at runtime through `include_str!` of `examples/zero_copy_demo.kel`; example now requires the `compile` and `verify` features and demonstrates the zero-copy execution path against an `AlignedVec<8>` populated from a freshly compiled module. `Cargo.toml` cleaned up to drop the regenerator example entry and gate `zero_copy_include_bytes` on the required features. |
| Document and reject the five-opcode dynamic-string-builder proposal. | New R41 in `docs/decisions/RESOLVED.md` enumerating the proposed opcodes (`BuildKStr`, `KStrAppendStatic`, `KStrAppendInt`, `KStrAppendFloat`, `KStrAppendBool`, `KStrFinalize`) and the three rejection reasons: dispatch-table cost versus host-side responsibility, WCMU bound looseness under over-declared capacity, and conflict with the V0.2.0 opcode-count target (current 69, proposal would have raised to 74). Records the recommended alternative path: host-registered `format` native delivering a `Value::KStr`. |
| Concern: live soft-warning trigger test. | Extracted `compiler::check_chunk_size_against_limits(chunk, span, &mut warnings)` from the inline `compile_function` check so the threshold logic is now testable in isolation. Three new tests directly exercise the helper with synthetic `Chunk` instances: `soft_warning_fires_on_long_chunk` at threshold + 1 ops, `hard_cap_rejects_oversize_chunk` at the hard cap + 1 ops, and `boundary_chunk_size_no_warning` at exactly the threshold. The previous live-trigger impracticality (synthetic source program at > 52,428 ops) is now sidestepped because the helper is the unit under test, not the surface compile path. |
| Concern: narrow-bytecode-on-wide-runtime `CheckedXxx` flag and high half. | Replaced the per-arm `(low, high, flag)` computation in `Op::CheckedAdd` / `CheckedSub` / `CheckedMul` / `CheckedNeg` with a shared `checked_arith_outputs::<W>(r: W::Wide, word_bits_log2: u8) -> (W, W, W)` helper in `src/vm.rs`. The helper computes the declared `[min, max]` range in `W::Wide` (using `WideWord` shift, negate, and subtract; no `i128` literals so it works for every `Word` impl), reports `flag` direction (`0` ok, `1` overflow, `2` underflow) at the declared range rather than the runtime range, and computes the `high` half as `(r - low_widened) >> declared_bits` so the `(high, low)` pair reconstructs the true wide result. Nine new unit tests in `vm::tests` (`checked_arith_outputs_*`) cover runtime-width in-range / overflow / underflow, declared 32 / 16 / 8 -bit overflow and underflow, and the reconstruction invariant `r == (high << declared_bits) + low_signed_at_declared_width`. The unused `declared_width_range` helper in `src/bytecode.rs` is removed; the helper computes the range inline through `WideWord` ops. |

## Verification matrix

```bash
cargo test --workspace                                                          # 797 lib + 53 rogue-script + 17 marshall tests, all green
cargo clippy --tests --workspace --all-features -- -D warnings                  # clean
cargo build --examples --workspace                                              # clean
cargo run --example zero_copy_include_bytes                                     # runtime-compile path returns 42
cargo fmt --all                                                                 # idempotent
(cd examples/rtos && cargo build --release --bin three-task-std)                # host RTOS build clean
```

## Open concerns

None. The two carried-forward concerns from the prior round are resolved.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b, 7c, 8 complete) |

## Intended Next Step

V0.2.0-isa branch is ready for merge to `main`. The natural next step is one of:

- Merge the `V0.2.0-isa` branch into `main` and tag the release.
- Manual `cargo publish` of the V0.2.0 crate (the publication step is operator-owned; the agent does not run `cargo publish`).
- A B15 follow-on: remove `Type::Unknown` entirely now that the V0.2.0 ISA work is closed.
- Operator selection of a different directive.
