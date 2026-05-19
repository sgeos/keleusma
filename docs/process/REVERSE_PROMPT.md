# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: V0.2 agent-driven deferred-items pass complete. Five items closed across the parent crate, the RTOS microkernel, and the documentation. Target-scaled `Fixed` defaults are threaded through the compile pipeline. The microkernel disables the `text` feature and routes script logging through a numeric `host::log_event` native. Two embassy-stm32 features are dropped. The top-level README gains a feature-matrix table. 613 lib tests pass workspace-wide.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Reduce microkernel flash size; disable `text`; log through registered natives. | The microkernel runtime keleusma dependency drops `text`. A new `host::log_event(code: Word, data: Word)` native replaces `host::log(text)` and forwards to a new `Platform::log_event(code: u32, data: i64)` method with platform-side per-event format strings (std `println!`, N6 `defmt::info!`). The script and host agree on numeric discriminants through `EV_HEARTBEAT_OK`, `EV_LED_GPIO_FAIL`, and `EV_SENSOR_ABOVE` constants in `src/natives.rs`. `register_utility_natives` is no longer called. Two embassy-stm32 features (`exti`, `unstable-pac`) are dropped because the kernel does not use them. Bare-metal `.text`: 180 KB trust-load (was 192, -12 KB), 199 KB precompile-plus-verify (was 211, -12 KB). |
| Target-scaled `Fixed` defaults for sub-64-bit targets. | New `Target::fixed_default_frac_bits()` helper returns the lower half of the target's word width (Q31.32 on the 64-bit host, Q15.16 on a 32-bit target, Q7.8 on a 16-bit target, Q3.4 on an 8-bit target). New `check_with_target` entry point on the type checker threads the value through `Ctx::fixed_default_frac_bits` and the unified resolver `Ctx::resolve_type`/`Ctx::resolve_type_with_params`. New `normalize_fixed_defaults` AST pass in the compiler rewrites every `PrimType::Fixed(None)` to `PrimType::Fixed(Some(target_frac))` before the type checker observes the program, so the compiler downstream reads the resolved immediate at the `Op::WordToFixed`, `Op::FixedToWord`, `Op::FixedMul`, and `Op::FixedDiv` emission sites without falling back to the host default. Two new tests cover the lattice (`fixed_default_frac_bits_scales_with_target_word_width`) and the end-to-end opcode-immediate change under cross-compilation (`fixed_default_changes_when_targeting_embedded_16`). |
| Top-level README feature matrix. | The `Features` section gained a `### Cargo features` subsection with a three-row table covering `compile`, `verify`, and `text`. Each row names what the feature adds and when a host typically drops it to save flash. The subsection references the microkernel's flash-size table as a concrete data point. |
| Microkernel documentation sync (MANUAL.md, SPEC.md, README.md). | MANUAL.md's flash-size table is regenerated with the new numbers. The idiomatic-script-usage and tuple-returning-natives examples in section 4 are rewritten to use `host::log_event(code, data)` with per-script `const data ev { ... }` discriminant tables. SPEC.md's native-surface table replaces `host::log(message: Text)` with `host::log_event(code: Word, data: Word)`. README.md's diagram updates the natives list. |
| Phase 8 follow-on documentation: per-yield dataflow refinement note. | The Memory Model section of `LANGUAGE_DESIGN.md` was updated in the previous session with the refinement description; no further edits were required in this pass. |

## Verification matrix

```bash
cargo test --workspace --features text                                         # 613 lib + 17+17+3+53+37+6+7 integration tests pass
cargo clippy --workspace --tests --features text -- -D warnings                # clean
cargo fmt --all                                                                # idempotent

# Microkernel std demonstrator (host).
(cd examples/rtos && cargo run --release --bin three-task-std)                 # heartbeat / led / sensor events visible

# Microkernel bare-metal flash size (STM32N6570-DK).
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform)                    # 180 KB .text
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform,keleusma-verify)    # 199 KB .text
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform,keleusma-compile,keleusma-verify)  # 622 KB .text
```

## Notes

- The 613 lib-test figure is the runtime crate's lib test count: prior 611 from Phase 8, +2 new tests for target-scaled `Fixed` defaults (lattice plus end-to-end opcode emission). All workspace tests pass, including microkernel kernel-side tests.
- The `Platform::log_event` default body is a no-op, so platforms that do not surface script logging continue to satisfy the trait without further implementation. The std and N6 platforms both implement the method.
- The `normalize_fixed_defaults` pass walks every place a `TypeExpr` can appear in the Program: function parameter and return types, struct field types, enum variant arguments, data field types, let-binding annotations, cast targets, closure parameter and return types, and impl-method signatures. Composite type expressions (`Tuple`, `Array`, `Option`, `Named<…>`) recurse into their components.
- The kernel-side `Platform::log(&str)` method is retained for host-emitted diagnostics (`kernel.rs` uses it for task scheduling errors and VM error reports through `format!`). The split between `log` and `log_event` keeps the script-side surface free of arbitrary strings while preserving the host's ability to emit rich diagnostic text from Rust.
- Source-level hardware verification is not part of this pass; the std demonstrator runs correctly and produces the expected heartbeat, GPIO, and sensor-above event lines. The N6 binary has been size-measured but not flash-tested in this session.

## Intended Next Step

Awaiting operator prompt. The remaining deferred items are operator-only or warrant a design discussion before implementation.

1. **Hardware verification on STM32N6570-DK.** The N6 binary builds in all three modes and is size-measured; an operator-driven probe-rs flash run would confirm that the `log_event`-based scripts produce the same RTT timeline as the previous `host::log` form. Last verified configuration on hardware was 2026-05-18 (under the prior `text`-enabled scripts).
2. **Operator action: Phase 8 release tag.** With these deferred items closed, V0.2 is feature-complete relative to the original Phase 0 spec. Operator decides whether to cut `v0.2.0` now or batch with further point-release work.
3. **Open follow-ons** that require discussion before implementation:
   - Bare `Option::None` in function returns (type inference tightening).
   - Native function signature declarations (removing `Type::Unknown` sentinel; B1).
   - `Op::CallIndirect` flow analysis to admit non-recursive closures (B3).
   - Schema hash or structural checking for hot swap (P6 follow-on).
   - Halt vs soft error category field on `VmError` (P3 follow-on).
   - Per-op decode cache or JIT for hot paths (P10 follow-on).
