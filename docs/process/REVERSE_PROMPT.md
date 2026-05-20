# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: B16 complete. All seven steps of the parametric `Vm<W, A, F>` design landed on `v0.2.0`. The bundled `Vm<'a, 'arena>` aliases `GenericVm<'a, 'arena, i64, u64, f64>` so pre-existing call sites compile unchanged. Hosts targeting narrower native runtimes instantiate `GenericVm<W, A, F>` directly. The worked demonstrator and the cookbook recipe document the host-side ergonomics.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Merge step 5 to `v0.2.0`. | Fast-forward-free merge `fa68a3f`. Six WIP checkpoints from the `V0.2.0-parametric-vm` feature branch travel into trunk as one merge. |
| Step 6: parameterize the marshall layer and `KeleusmaType`. | Commit `4f7be84`. The marshall layer's `KeleusmaType<W, F>`, `IntoNativeFn<W, F, Args, R>`, `IntoFallibleNativeFn<W, F, Args, R>`, and `BoxedNativeFn<W, F>` are parametric over the runtime's word and float types. The `stddsl::Library<W, A, F>` trait carries all three. Universal impls for `i64`, `u8`, `bool`, `()`, `f64`, `Option<T>`, fixed arrays, and tuples (arities 2-5) bridge canonical Rust types to the script's narrower words and floats through `Word::to_i64`, `Word::from_i64_wrap`, `Float::to_f64`, and `Float::from_f64`. The `#[derive(KeleusmaType)]` macro emits universal impls with synthetic generic parameters `__KW: Word` and `__KF: Float` so user-side type parameters do not collide. The `register_fn`, `register_fn_fallible`, and `register_library` methods move back into the generic `impl<W, A, F> GenericVm` block. Crate-root re-exports added for `Address`, `Float`, `Word`, and `GenericValue` so derived impls compile without users touching the implementation modules. |
| Step 7: narrow-runtime demonstrator and cookbook recipe. | `examples/narrow_runtime.rs` exercises `GenericVm<i16, u16, f32>` against bytecode compiled with `Target::embedded_16()`. Three scenarios: plain arithmetic, wrapping at the word boundary (30_000 + 10_000 = -25_536 in i16), and host-side `register_fn` with a natural Rust `i64` closure that the marshall layer truncates to `i16`. Integration test `tests/narrow_vm.rs` pins all three. Cookbook recipe at `docs/guide/COOKBOOK.md` under *Narrow-runtime type alias* documents the `type NarrowVm<'a, 'arena> = GenericVm<'a, 'arena, i16, u16, f32>` pattern, the marshall-widening behaviour, the standard-library-bundle bound to the default shape, and the word-width arithmetic discipline. |

## Verification matrix

```bash
cargo test -p keleusma --lib                                                    # 734 lib tests pass
cargo test --workspace                                                          # all workspace tests pass
cargo test -p keleusma --no-default-features --features compile,verify --lib    # 642 lib tests pass (floats off)
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
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved (all seven steps complete) |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |

## Notes

- The deprecated `register_utility_natives_with_ctx` alias remains specialized to the default `Vm<'a, 'arena>`; it is the V0.2.0 transition wrapper and is left untouched.
- Standard `stddsl` bundles (`Math`, `Audio`, `Text`, `Shell`) impl `Library<i64, u64, f64>` only. Hosts targeting narrow runtimes write their own `Library<W, A, F>` impls.
- A future enhancement could add load-time validation that bytecode's declared `word_bits_log2` matches `<W as Word>::BITS_LOG2`. Not in scope for B16; the present runtime permits wider Vm running narrower bytecode through `Word::from_i64_wrap`, which is the same wrap-on-load discipline `truncate_int` provides for the default shape.

## Intended Next Step

Awaiting operator prompt. B16 is closed end-to-end. The next development action belongs to the operator's selection from the remaining backlog (B13, B14, B15) or a new directive.
