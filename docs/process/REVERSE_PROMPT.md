# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 5 open-concern follow-up landed on the `V0.2.0-isa` branch. Native classification mismatch is now detected at the entry of `Vm::call_function` through `Vm::verify_native_classifications`, replacing the per-dispatch check that fired only at the offending call site. External natives' per-call WCMU contribution is explicitly zeroed at the verifier handoff to guard against unsound over- or under-counting.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Address Phase 5 open concerns: load-time classification check, external WCMU integration. | **Concern 2 (load-time check).** New `Vm::verify_native_classifications(&mut self)` walks every `Op::CallVerifiedNative` / `Op::CallExternalNative` site in the loaded module, looks up the registered `NativeEntry` by name, and compares classifications. Native names referenced by the bytecode but not yet registered are skipped (the dispatch path surfaces them as `InvalidBytecode` at the first invocation). The check is run lazily at the entry of `Vm::call_function`; the result is cached on the Vm (`native_classifications_verified: bool`) and invalidated by every `register_*` method and by `replace_module`. The per-dispatch check is removed; the load-time check is the source of truth. **Concern 1 (external WCMU).** External natives' per-call WCMU contribution is explicitly zeroed at the `verify_resources` / `auto_arena_capacity` handoff (replacing the previous `n.wcmu_bytes` blanket collection). The host's `max_invocations_per_iteration` attestation remains recorded on the entry; full chunk-level integration that bounds external-native cost as `max_invocations * per_call_wcmu` per chunk is documented as forward-looking work because it requires verifier-side classification awareness that the current `module_wcmu` API does not have. |

## Verification matrix

```bash
cargo test --workspace                                                          # 755 lib + 53 rogue-script + 17 marshall tests, all green
cargo clippy --tests --all-targets -- -D warnings                               # clean
cargo build --examples                                                          # clean
cargo fmt --all                                                                 # idempotent

# Bare-metal STM32N6570-DK build, full pipeline.
(cd examples/rtos && cargo build --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # clean
```

## Open concerns

| Item | Note |
|------|------|
| External-native chunk-level WCMU integration deferred. | The verifier's `module_wcmu` API takes `native_wcmu: &[u32]` and applies the value per static call site. For external natives the sound bound is per-chunk: `max_invocations_per_iteration * per_call_wcmu` regardless of static call-site count. Implementing this correctly requires extending the verifier API with per-native classification awareness and a separate per-chunk pass over external-native references. The current handoff zeroes external natives so neither under- nor over-counting occurs through the existing path; the host accepts that external natives are outside the script's resource contract. |
| Multiple registrations of the same name. | `register_*` methods push a new `NativeEntry`; duplicate names are not deduplicated. The dispatch `find` returns the first matching entry, so the second registration is shadowed. `set_native_bounds` updates every matching entry. Documented behaviour; consider a deduplication pass in a future API hardening. |

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5 complete; Phases 6–8 pending; external-native WCMU integration is a Phase 5 follow-on) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 6: control-flow operand narrowing `u32` → `u16` with 80% soft warning.
- B20 Phase 7: wire format with fixed-size opcode records and operand pool.
- B20 Phase 8: documentation alignment and `BYTECODE_VERSION` reset to 1.
- External-native chunk-level WCMU integration (verifier API extension).
- A narrow-width overflow-detection follow-up that brings `CheckedXxx` flag and high-half reporting in line with the bytecode's declared word width.
- Operator selection of a different directive.
