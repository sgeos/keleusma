# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: B16 step 12 lands the binary-build narrowing features. Seven Cargo features (`narrow-word-8`, `narrow-word-16`, `narrow-word-32`, `narrow-address-8`, `narrow-address-16`, `narrow-address-32`, `narrow-float-32`) lower the framing-level `RUNTIME_*_BITS_LOG2` constants for binaries that ship only narrow runtimes. The narrowest-wins rule applies per dimension; absence of any narrowing feature retains the i64/u64/f64 default. The default build still passes all 737 lib tests; narrowed builds pass after gating 14 i64-boundary tests on the absence of the relevant narrowing features.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Address item 5: expose framing-level width maximums as a build-time configuration. | Seven Cargo features added to the `keleusma` crate. The `narrow-word-{8,16,32}`, `narrow-address-{8,16,32}`, and `narrow-float-32` features each lower the corresponding `RUNTIME_*_BITS_LOG2` constant in `src/bytecode.rs`. The narrowest-wins rule is encoded through nested `cfg(all(feature = "narrow-word-16", not(feature = "narrow-word-8")))` patterns, preserving Cargo's additive-features semantics: enabling additional narrowing features further tightens the bound rather than relaxing it. Without any narrowing feature the constants remain at the default 6. The narrowing affects three load-time and compile-time paths: `Module::access_bytes` and `Module::from_bytes` reject bytecode that exceeds the configured maximum at the framing level (before reaching the per-Vm width check); `Target::host()` reports the configured maximum so compile-time targets match; `Target::validate_against_runtime` rejects compile-time targets that exceed the configured maximum. The opcode dispatch, the parametric `GenericVm<W, A, F>` shape, and the per-Vm width check at `<W as Word>::BITS_LOG2` are unchanged. Fourteen tests that exercise i64-specific boundary behavior (Q31.32 fixed-point, i64-boundary checked arithmetic, golden bytecode bytes, saturate-keyword newtype contracts, embedded_16 admissibility tests, and a 300-value data-segment test) are gated on the absence of the corresponding narrowing features so they run in the default build but are skipped on narrowed builds. A new test `runtime_width_constants_track_narrowing_features` in `cost_model_tests` pins the constants per feature combination so future refactors do not regress the narrowest-wins rule. |

## Verification matrix

```bash
cargo test -p keleusma --lib                                                    # 737 lib tests pass (was 736; +1 constants test)
cargo test --workspace --features text                                          # all workspace tests pass
cargo test -p keleusma --lib --features narrow-word-16                          # 725 lib tests pass (12 i64-tests gated)
cargo test -p keleusma --lib --features narrow-word-8                           # 720 lib tests pass (additional i64/i16 tests gated)
cargo test -p keleusma --lib --features narrow-float-32                         # 736 lib tests pass (1 golden-bytes test gated)
cargo test -p keleusma --lib --features narrow-word-16,narrow-float-32          # 724 lib tests pass
cargo test -p keleusma --no-default-features --features compile,verify --lib    # 644 lib tests pass (floats off)
cargo check --features shell                                                    # clean
cargo clippy --tests --all-targets --features text -- -D warnings               # clean
cargo fmt --all                                                                 # idempotent

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
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved (twelve steps complete) |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |

## Notes

- The four standing items previously commented on (`truncate_int` workaround, `Address` parameter inertness, no 128-bit Word, `RUNTIME_*_BITS_LOG2` global constants) collapse to three after step 12: `RUNTIME_*_BITS_LOG2` is no longer fixed at 6 (it is now Cargo-feature-configurable). The remaining three items are documented design properties without recommended action.

## Intended Next Step

Awaiting operator prompt. B16 is closed end-to-end across runtime, marshall, library bundles, verifier, build-time configuration, and knowledge-graph documentation. The next development action belongs to the operator's selection from B13, B14, B15, or a new directive.
