# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 6 landed on the `V0.2.0-isa` branch alongside the dedup follow-on for Phase 5 concern #2. Control-flow operands narrow from `u32` to `u16`; chunks are hard-capped at 65,535 ops with a soft warning at 80%. Native re-registration now deduplicates by name. Opcode count remains 69; the change is structural (operand width) rather than enum-membership.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Proceed with Phase 6. Address open concerns if possible. | **Phase 6 main work.** `Op::If`, `Op::Else`, `Op::Loop`, `Op::EndLoop`, `Op::Break`, `Op::BreakIf` carry `u16` jump targets instead of `u32`. `FuncCompiler::patch_jump` narrows the cast; the post-emit hard-cap check guarantees the cast never truncates an admissible chunk. Approximately 18 `as u32` arithmetic sites in `src/compiler.rs` rewrote to `as u16` (loop counter increment, EndLoop back-edge, after-EndLoop, after-Loop). VM dispatch arms continue to cast `u16 as usize` unchanged. **Compile-time enforcement.** New `pub struct CompileWarning { message, chunk_name }` and constants `CHUNK_SIZE_HARD_LIMIT = u16::MAX as usize` and `CHUNK_SIZE_SOFT_WARN_THRESHOLD = CHUNK_SIZE_HARD_LIMIT * 80 / 100`. New `pub fn compile_with_warnings(program, target) -> Result<(Module, Vec<CompileWarning>), CompileError>` runs the full compile pipeline and returns the warnings vector. `compile_with_target` delegates and discards warnings (preserving the 30+ existing call sites). Hard cap: any chunk whose op count exceeds `CHUNK_SIZE_HARD_LIMIT` produces a `CompileError` with the offending function's source span. Soft warning: any chunk crossing `CHUNK_SIZE_SOFT_WARN_THRESHOLD` produces one `CompileWarning` in the returned vector. **Concern #2 (dedup).** The eight `register_*` methods on `GenericVm` now retain only entries whose name differs from the new registration before pushing; a re-registration of the same name replaces the prior entry rather than appending. Combined with the cache invalidation introduced in the Phase 5 follow-on, this makes re-registration semantically predictable. |

## Verification matrix

```bash
cargo test --workspace                                                          # 759 lib + 53 rogue-script + 17 marshall tests, all green
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
| External-native chunk-level WCMU integration deferred. | Inherited from Phase 5. The verifier's `module_wcmu` API takes `native_wcmu: &[u32]` and applies the value per static call site. For external natives the sound bound is per-chunk: `max_invocations_per_iteration * per_call_wcmu` regardless of static call-site count. Implementing this requires extending the verifier API with per-native classification awareness and a separate per-chunk pass over external-native references. The current handoff zeroes external natives so the per-site path remains neutral. |
| Live soft-warning trigger test not added. | The hard-cap path is covered through `chunk_size_thresholds_are_consistent` and inline through the compile-with-warnings flow. A live soft-warning trigger would require constructing a synthetic source program with > 52,428 ops; the compile time alone for such a test makes it impractical. The threshold logic is exercised through code review; an integration test that constructs a Module directly would bypass the inline check entirely. |

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6 complete; Phases 7–8 pending; external-native WCMU integration is a Phase 5 follow-on) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 7: wire format with fixed-size opcode records and operand pool.
- B20 Phase 8: documentation alignment and `BYTECODE_VERSION` reset to 1.
- External-native chunk-level WCMU integration (verifier API extension).
- A narrow-width overflow-detection follow-up that brings `CheckedXxx` flag and high-half reporting in line with the bytecode's declared word width.
- Operator selection of a different directive.
