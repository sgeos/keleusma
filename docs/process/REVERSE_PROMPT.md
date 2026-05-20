# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: B16 step 8 lands the soundness-closure follow-up pass. The three gaps recorded in the previous gap audit are all addressed: load-time width validation rejects bytecode that exceeds the runtime's chosen `W`/`A`/`F` widths; `stddsl::Math` and `stddsl::Audio` lift to be generic over `(W, A)` so narrow runtimes can register them; and `Word::to_usize_checked` joins the trait surface as a default-method mirror of `Address::to_usize_checked`. The `Address` parameter `A` now carries runtime semantics through the width check.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Close the three gaps surfaced in the gap audit. | (1) Load-time width validation. New private helper `GenericVm::check_runtime_widths` rejects bytecode whose declared widths exceed the runtime's `<W as Word>::BITS_LOG2`, `<A as Address>::BITS_LOG2`, or `<F as Float>::BITS_LOG2`. Wired into the top of `construct` (catches `Vm::new` and `Vm::new_unchecked` through the shared path) and into `view_bytes_zero_copy` (which reads the widths directly from the framing header bytes 10-12). Rejection surfaces as `VmError::VerifyError` with a message naming the offending field. (2) Standard-library bundle lift. `stddsl::Math` and `stddsl::Audio` move from `Library<i64, u64, f64>` to `impl<W: Word, A: Address> Library<W, A, f64>`. The inner `math::register` and `audio_natives::register_audio_natives` take `&mut GenericVm<W, A, f64>` so the closures can compile against the universal `KeleusmaType<W, f64>` impls. `stddsl::Text` and `stddsl::Shell` remain `Library<i64, u64, f64>` because their inner natives use `&[Value]` directly. (3) `Word::to_usize_checked` added as a default trait method delegating to `to_i64` and `usize::try_from`. Two unit tests pin the positive and negative branches across `i8`, `i16`, and `i64`. Four new integration tests in `tests/narrow_vm.rs` verify the lifted bundles and the rejection paths. |

## Verification matrix

```bash
cargo test -p keleusma --lib                                                    # 736 lib tests pass (was 734; +2 to_usize_checked)
cargo test --workspace                                                          # all workspace tests pass
cargo test -p keleusma --no-default-features --features compile,verify --lib    # 644 lib tests pass (floats off; was 642; +2)
cargo test --test narrow_vm                                                     # 7 tests pass (was 4; +3 width / bundle / f32)
cargo clippy --tests --all-targets -- -D warnings                               # clean
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
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved (all eight steps complete) |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |

## Notes

- The deprecated `register_utility_natives_with_ctx` alias and `register_utility_natives` itself remain specialized to the default `Vm<'a, 'arena>` because their native function signatures take `&[Value]`. Lifting these to `&[GenericValue<W, F>]` is the path to letting `stddsl::Text` work on narrow runtimes; the work is out of scope for this pass and would touch every `native_*` function in `utility_natives.rs`.
- Address-bound runtime opcodes that would consume the `A` parameter for more than the load-time width check remain a future enhancement. The current pass elevates `A` from purely-phantom to "validated at load time"; further weight (host-side `A::MAX` bound checks in pointer-shaped opcodes) waits for a concrete consumer.
- The `truncate_int` workaround in `Op::Add` and `binary_arith` is retained as documented backward-compatibility scaffolding; it is now genuinely backward-compat rather than load-bearing, because the load-time width check rejects the mismatch case that would have required it.

## Intended Next Step

Awaiting operator prompt. B16 is closed end-to-end with the soundness gaps addressed. The next development action belongs to the operator's selection from the remaining backlog (B13, B14, B15, or a Text-bundle lift follow-up) or a new directive.
