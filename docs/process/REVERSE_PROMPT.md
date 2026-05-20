# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: B16 step 9 lifts `stddsl::Text` and `stddsl::Shell` to be generic over `(W, A, F)`. All four standard library bundles are now parametric: `Math` and `Audio` over `(W, A)` with `F` pinned to `f64`; `Text` and `Shell` over `(W, A, F)` with no pinning. The utility-natives and shell-natives modules are generic over the runtime's word and float types, with integer payload bridging through `Word::to_i64` and `Word::from_i64_wrap`. B16 is fully closed from a runtime-correctness standpoint.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Lift `stddsl::Text` and `stddsl::Shell` to be generic. | `src/utility_natives.rs` rewritten with every native function and helper quantifying over `<W: Word, F: Float>`. Pattern arms switch from `Value::` to `GenericValue::`; integer payload formatting bridges through `W::to_i64` so any narrow word type renders the same numeric output as the default i64; length values from `length` wrap through `W::from_i64_wrap` so they fit the runtime's word width. `register_utility_natives<'a, 'arena, W: Word, A: Address, F: Float>` takes `&mut GenericVm<W, A, F>` and passes generic function pointers to `register_native_with_ctx` and `register_native`. `src/stddsl/shell.rs` lifted the same way for `getenv`, `has_env`, `run`, `run_checked`, and `exit`; the exit-code argument bridges through `W::to_i64` for the `std::process::exit(code as i32)` call site, and the `(exit_code, stdout)` tuple wraps the exit code through `W::from_i64_wrap`. `src/stddsl/mod.rs` updated to impl `Library<W, A, F>` for `Text` and `Shell` universally; the inner `text::register` quantifies the same way. A new integration test in `tests/narrow_vm.rs` (gated on the `text` feature) registers `stddsl::Text` on `GenericVm<i16, u16, f64>` and confirms `length("hello")` returns `5_i16`. |

## Verification matrix

```bash
cargo test -p keleusma --lib                                                    # 736 lib tests pass
cargo test --workspace                                                          # all workspace tests pass
cargo test --workspace --features text                                          # 10 narrow_vm tests pass (was 9; +1 Text lift)
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
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved (nine steps complete) |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |

## Notes

- All four `stddsl` bundles (`Math`, `Audio`, `Text`, `Shell`) are now registrable on narrow runtimes through the `register_library` entry point. `Math` and `Audio` retain the `F = f64` constraint because their inner closures pin `f64`; a host running an f32 runtime would silently truncate constants through `Float::from_f64` if those bundles were lifted further. `Text` and `Shell` have no float surface and so quantify over `F` without restriction.
- The `truncate_int` workaround retained in `Op::Add` and `binary_arith` remains intentional, documented backward-compat scaffolding. The load-time width check (step 8) means this path now only fires when a wide Vm runs narrow bytecode (the supported direction).
- Address-bound runtime opcodes that would consume the `A` parameter for more than the load-time width check remain a future enhancement; no concrete consumer yet. The `A` parameter is no longer purely-phantom because of step 8 but does not yet participate in any opcode's dispatch.
- The `RUNTIME_*_BITS_LOG2` global constants in `bytecode.rs` remain at 6 (i64) as the binary's bytecode-level upper bound. They could be reduced for a binary build that wants to exclude i64 bytecode entirely; that is a build-configuration question rather than a runtime gap.

## Intended Next Step

Awaiting operator prompt. B16 is closed end-to-end across runtime, marshall, standard-library bundles, and demonstrator/cookbook documentation. The next development action belongs to the operator's selection from B13, B14, B15, or a new directive.
