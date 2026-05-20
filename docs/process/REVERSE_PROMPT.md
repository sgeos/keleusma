# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: B16 step 10 lifts `stddsl::Math` and `stddsl::Audio` to be generic over `F` and runs the documentation-prose audit. All four `stddsl` bundles now impl `Library<W, A, F>` universally; the parametric `GenericVm<W, A, F>` shape is documented across the architecture, design, decisions, and guide knowledge-graph sections. Three documentation files were updated to remove stale prose that treated the default 64-bit runtime as the only runtime shape.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Lift `Math` and `Audio` to be generic over `F`. | `impl<W: Word, A: Address, F: Float> Library<W, A, F>` for both bundles. The inner `math::register` and `audio_natives::register_audio_natives` quantify the same way. The closures retain their `f64` argument and return signatures; on a runtime whose `F` is `f32`, every closure argument and return passes through `Float::from_f64` / `Float::to_f64` at the marshall boundary, narrowing constants and intermediates. The narrowing is mathematically defined and silent. New test `f32_narrow_runtime_can_register_math_library_via_lifted_impl` pins `math::sqrt(9.0) = 3.0_f32` on `GenericVm<i64, u64, f32>`. |
| Update the cookbook recipe. | The *Narrow-runtime type alias* recipe in `docs/guide/COOKBOOK.md` is rewritten to reflect that all four `stddsl` bundles register on narrow runtimes. The former *Standard library bundles remain on the default shape* heading becomes *Standard library bundles work on narrow runtimes*; the body documents the `Float::from_f64` / `Float::to_f64` narrowing path for Math and Audio on f32 runtimes and the precision tradeoff. |
| Audit architecture and design prose for stale narrow-runtime references. | Three knowledge-graph files updated. `docs/architecture/LANGUAGE_DESIGN.md`: the cost-model paragraph replaces "current 64-bit Keleusma runtime" with parametric-aware text describing the bundled default and the narrow shape; the checked-arithmetic paragraph replaces the literal `i128` with `W::Wide` and adds a concrete mapping table for each `Word` impl. `docs/architecture/EXECUTION_MODEL.md`: the bytecode-load paragraph distinguishes the binary's framing-level upper bound (the `RUNTIME_*_BITS_LOG2` constants) from the per-Vm bound (the `<W as Word>::BITS_LOG2` and siblings) and explains how the two compose. `docs/design/TYPE_SYSTEM.md` and `docs/design/GRAMMAR.md`: the primitive-type tables annotate `Word` and `Float` sizes as defaults that vary under the parametric shape, with cross-references to the cookbook recipe. |

## Verification matrix

```bash
cargo test -p keleusma --lib                                                    # 736 lib tests pass
cargo test --workspace --features text                                          # all workspace tests pass; 11 narrow_vm tests (was 10)
cargo test -p keleusma --no-default-features --features compile,verify --lib    # 644 lib tests pass (floats off)
cargo check --features shell                                                    # clean
cargo clippy --tests --all-targets --features text -- -D warnings               # clean
cargo fmt --all                                                                 # idempotent
cargo run --example narrow_runtime                                              # prints expected output

# Bare-metal STM32N6570-DK build, full pipeline.
(cd examples/rtos && cargo check --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # clean
```

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Deferred to V0.3 |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved (ten steps complete) |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |

## Notes

- All four `stddsl` bundles (`Math`, `Audio`, `Text`, `Shell`) now register on any admissible `GenericVm<W, A, F>` shape. `Math` and `Audio` carry their inner closures in `f64`; on an `f32` runtime the marshall boundary narrows through `Float::from_f64` / `Float::to_f64`, a documented design tradeoff.
- The `truncate_int` workaround retained in `Op::Add` and `binary_arith` is intentional backward-compat scaffolding for the supported direction (wide Vm running narrow bytecode). The load-time width check rejects the opposite direction.
- The `Address` parameter `A` participates in load-time width validation but not in any opcode's runtime dispatch. Future opcodes that consume `A::MAX` would tighten its semantic weight. The `_phantom_a: PhantomData<A>` field encodes the present status.
- `RUNTIME_*_BITS_LOG2` global constants remain at 6 (i64) as the binary build's framing-level upper bound. Each Vm enforces a tighter per-instance bound through `<W as Word>::BITS_LOG2` and siblings. Reducing the global constants for builds that exclude i64 bytecode entirely is a build-configuration question rather than a runtime gap.

## Intended Next Step

Awaiting operator prompt. B16 is closed end-to-end across the runtime, the marshall layer, all standard library bundles, the demonstrator example, the integration tests, the cookbook recipe, and the architecture/design knowledge-graph prose. The next development action belongs to the operator's selection from B13, B14, B15, or a new directive.
