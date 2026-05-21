# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: Cross-architecture rkyv-decode regression on the STM32N6570-DK fixed and hardware-verified. The V0.2.0 ISA branch is now hardware-clean across both `--no-default-features --features stm32n6570dk-platform` and `--features stm32n6570dk-platform,keleusma-verify` configurations. The branch is ready for merge to `main`.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Address the bare-metal rkyv-decode regression without reverting. | Root cause: V0.2.0 Phase 7c (593f541) cut `Module::from_bytes` over to `wire_format::module_from_wire_bytes` without porting the pre-cutover `AlignedVec<8>` copy step. `rkyv::from_bytes` calls `rkyv::access` internally, which requires the input buffer to be 8-byte aligned. The post-cutover code passed a raw `&[u8]` subslice from arbitrary input alignment; on x86_64 the host bin (`three-task-std`) uses the `keleusma-compile` runtime-compile path and never exercised the regression, so it was masked. On the N6 target (`include_bytes!` precompiled into the `.text` section, no compile-path), the aux-body subslice landed at a 4-byte boundary and rancor rejected the decode with the opaque "failed without error information" message. Fix: copy `aux_body_bytes` into a `rkyv::util::AlignedVec<8>` before calling `rkyv::from_bytes`, mirroring the legacy pattern. The owned-decode contract of `Module::from_bytes` and `Module::view_bytes` is now uniform: both tolerate arbitrarily aligned input. The zero-copy alignment contract is preserved by `Module::access_bytes` and `Vm::view_bytes_zero_copy`, which still check `aux_body.as_ptr() % 8 == 0` explicitly. Test `bytecode_view_bytes_rejects_unaligned_input` (which encoded the legacy reject-on-unaligned behaviour) is rewritten as `bytecode_view_bytes_handles_unaligned_input` asserting the new tolerance plus round-trip soundness. Verified on hardware: the N6 binary now boots, loads led/sensor/heartbeat/event_listener/faulty, enters the scheduler loop, and exercises the supervised-restart path on the faulty task across both no-verify and verify configurations. |

## Verification matrix

```bash
cargo test --workspace                                                          # 956 tests across 16 suites, all green
cargo clippy --tests --workspace --all-features -- -D warnings                  # clean
cargo fmt --all -- --check                                                      # idempotent
(cd examples/rtos && cargo build --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # clean
(cd examples/rtos && cargo run  --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # boots, all tasks load, scheduler runs
(cd examples/rtos && cargo run  --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform,keleusma-verify)                          # boots, all tasks load+verify, scheduler runs
```

## Open concerns

None.

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
