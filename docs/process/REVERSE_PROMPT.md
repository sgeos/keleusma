# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 5 complete on the `V0.2.0-isa` branch. Native ABI split landed: `use external module::name` parses; the compiler emits `Op::CallVerifiedNative` / `Op::CallExternalNative` based on the source-level classification; `Op::CallNative` is retired. The host's `Vm::register_verified_native(name, fn, wcet, wcmu_bytes)` and `Vm::register_external_native(name, fn, max_invocations_per_iteration)` mirror the split, and the call-site dispatch cross-checks the registered classification against the opcode and rejects mismatches as `VmError::VerifyError`. Opcode count is 69 (was 70 after Phase 4).

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Proceed with Phase 5. | Parser: `External` token added to the lexer; `parse_use_decl` accepts an optional `external` modifier between `use` and the first path segment and records it on `UseDecl::is_external`. Compiler: `compile_with_target` builds a parallel `native_externals: BTreeMap<String, bool>` map alongside `native_map`; the call-emission paths at the BinOp-style pipeline site and at the direct-call site consult the map to pick `Op::CallVerifiedNative` vs `Op::CallExternalNative`. Bytecode: `Op::CallNative` removed from the Op enum, the rkyv ArchivedOp conversion, the nominal cost table, the stack-effect dispatch, the verifier's WCMU walk, and the `text_size` module. VM: `NativeEntry` gains `classification: NativeClassification` and `max_invocations_per_iteration: Option<u32>`. New public `Vm::register_verified_native(name, fn, wcet, wcmu_bytes)` and `Vm::register_external_native(name, fn, max_invocations_per_iteration)` methods. The dispatch arm for both call opcodes computes the expected classification from the opcode match arm and compares against the registered entry's classification; mismatch returns `VmError::VerifyError` with a diagnostic naming both sides. Tests: five new tests covering the parser positive paths and the two mismatch directions; golden-bytes test updated for the smaller archived-op tag. |

## Verification matrix

```bash
cargo test --workspace                                                          # 752 lib + 53 rogue-script + 17 marshall tests, all green
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
| The `max_invocations_per_iteration` attestation is recorded on the `NativeEntry` but not yet consumed by the verifier. | Forward-looking. The current verifier folds the per-call `wcmu_bytes` attestation into the iteration budget for both verified and external natives. A follow-up pass should account for external-call cost through the invocation-count attestation rather than the per-call WCMU. The structural marker is in place; only the cost-model integration is pending. |
| Mismatch is detected at the call-site dispatch rather than at `Vm::new`. | The documented intent in `INSTRUCTION_SET.md` originally referenced an `Vm::new`-time check. The implementation detects the mismatch at the first invocation of the affected native rather than module load time. The trade-off favors not requiring all natives to be registered before `Vm::new` returns. Hosts that wish for load-time detection can call the natives once after registration to force the check. |
| Bounds parameters on `register_verified_native` flow into `NativeEntry::wcet` / `wcmu_bytes` directly. | Previously, hosts called `register_native` and then `set_native_bounds`. The new API folds the bound declaration into the registration call. `set_native_bounds` still exists for post-registration adjustment. |

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5 complete; Phases 6–8 pending) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 6: control-flow operand narrowing `u32` → `u16` with 80% soft warning.
- B20 Phase 7: wire format with fixed-size opcode records and operand pool.
- B20 Phase 8: documentation alignment and `BYTECODE_VERSION` reset to 1.
- Verifier integration for the external-native invocation-count attestation.
- A narrow-width overflow-detection follow-up that brings `CheckedXxx` flag and high-half reporting in line with the bytecode's declared word width.
- Operator selection of a different directive.
