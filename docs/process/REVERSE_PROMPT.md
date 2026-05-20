# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: B16 step 6 complete. The marshall layer is now parametric over `(W, F)`; the `KeleusmaType`, `IntoNativeFn`, `IntoFallibleNativeFn`, and `stddsl::Library` traits all quantify universally over the runtime's word and float types. The `register_fn`, `register_fn_fallible`, and `register_library` methods moved back into the generic `impl<W, A, F> GenericVm` block. The `#[derive(KeleusmaType)]` macro now emits universal impls. Standard `stddsl` bundles remain bound to the default `(i64, u64, f64)` shape because their inner closures pin `f64`. Step 5 of B16 landed via merge commit `fa68a3f` on 2026-05-19.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Merge `V0.2.0-parametric-vm` into `v0.2.0`. | Fast-forward-free merge committed as `fa68a3f`. Six WIP checkpoints from the feature branch land as one merge commit. 734 lib tests pass against the merged state. |
| Step 6 of B16: parameterize the marshall layer and `KeleusmaType`. | `src/marshall.rs` rewritten. `KeleusmaType<W: Word, F: Float>` carries a parametric `GenericValue<W, F>` in its `from_value` and `into_value` signatures. Impls for `i64`, `u8`, `f64`, `bool`, `()`, `Option<T>`, fixed-length arrays, and tuples (arities 2-5) all quantify universally over `<W, F>` and use trait methods (`W::to_i64`, `W::from_i64_wrap`, `F::to_f64`, `F::from_f64`) to bridge canonical Rust types to the script word and float. `IntoNativeFn<W, F, Args, R>` and `IntoFallibleNativeFn<W, F, Args, R>` reshape with the new parameters; the macro expansion uses the trait-fully-qualified `<$name as KeleusmaType<W, FloatT>>::from_value` and `<R as KeleusmaType<W, FloatT>>::into_value`. The internal closure type parameter is renamed `Func` to avoid colliding with the outer `F: Float`. Tests in `src/marshall.rs` and `tests/marshall.rs` updated with type ascription on free-standing `into_value()` calls. `register_fn`, `register_fn_fallible`, and `register_library` move into the generic `impl<W: Word, A: Address, F: Float> GenericVm` block. `stddsl::Library<W, A, F>` carries the three parameters; standard bundles impl `Library<i64, u64, f64>` because their inner closures pin `f64`. The `#[derive(KeleusmaType)]` macro emits `impl<existing_generics, __KW: Word, __KF: Float> KeleusmaType<__KW, __KF> for #name`; the synthetic param names `__KW` and `__KF` avoid colliding with user type parameters. `keleusma::Address`, `keleusma::Float`, `keleusma::Word`, and `keleusma::GenericValue` re-exported at the crate root so derived impls compile without users touching the `address`, `float`, `word`, or `bytecode` modules directly. |

## Verification matrix

```bash
cargo build --quiet                                                            # clean
cargo test -p keleusma --lib --quiet                                            # 734 lib tests pass
cargo test --workspace --quiet                                                  # all workspace + doctest crates clean
cargo test --no-default-features --features compile,verify --quiet              # 642 lib tests pass (floats off)
cargo clippy --tests --all-targets --quiet -- -D warnings                       # clean
cargo fmt --all                                                                # idempotent

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
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Steps 1-6 complete; step 7 (demonstrator + cookbook) pending |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |

## Notes

- Branch `v0.2.0` carries the B16 step 5 merge (`fa68a3f`) ahead of `origin/v0.2.0`. Step 6 work is unstaged at the time of this writing; will be committed in this session round.
- The deprecated `register_utility_natives_with_ctx` alias remains specialized to the default `Vm<'a, 'arena>`; it is left untouched because it is a temporary wrapper for the V0.2.0 transition.
- Standard `stddsl` bundles (`Math`, `Audio`, `Text`, `Shell`) implement `Library<i64, u64, f64>` only. Hosts targeting narrow runtimes write their own `Library<W, A, F>` impls; step 7's cookbook recipe will document the pattern.

## Intended Next Step

Step 7 of B16: demonstrator `Vm<i16, u16, f32>` and cookbook recipe documenting the `pub type NarrowVm<'a, 'arena> = Vm<'a, 'arena, i16, u16, f32>` pattern. Awaiting operator prompt.
