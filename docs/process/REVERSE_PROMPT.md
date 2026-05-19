# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: Cooperative Real-Time Operating System (RTOS) microkernel example added at `examples/rtos/`, verified end-to-end on the STM32N6570-DK board. Operator manual and architectural specification ship alongside the example crate. Documentation knowledge graph and parent README updated with links to the new example.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Resolve the zero-byte allocation panic during heartbeat-task construction on the STM32N6570-DK. | Three-pronged fix. First, the `memory.x` layout was rebalanced. Flash region shrunk from 768 KB to 640 KB, the random-access memory (RAM) region grew from 256 KB to 384 KB, filling Advanced eXtensible Interface Static Random-Access Memory 2 (AXISRAM2) differently. Second, the global heap size grew from 224 KB to 320 KB. Third, a new `setup::build_task_with_arena_capacity` and the matching `three_task_kernel_with_arena_capacity` factory let the bare-metal binary size each task arena at 16 KB rather than the host default of 64 KB. The combination eliminated the fragmentation that previously caused a third 64 KB arena allocation to fail in a 224 KB heap. The `ZeroSizeOk` wrapper around `LlffHeap` was retained and extended to cover `alloc`, `alloc_zeroed`, `realloc`, and `dealloc` as defence in depth. |
| Attempt to flash hardware and verify the microkernel runs end-to-end. | The STM32N6570-DK was flashed three times in this session through `probe-rs run --chip STM32N657`. Each run produced the boot banner, the kernel-construction trace, scheduler entry at t≈215 milliseconds (ms), and four heartbeat log lines at t≈218, 5218, 10219, 15220 ms. The operator independently confirmed the on-board green light-emitting diode (LED) on PG10 toggling. |
| Bring the example into proper shape for distribution with comments and a manual. | A separate operator manual was written at `examples/rtos/MANUAL.md`. The `README.md` was trimmed to a one-page overview with quick-start commands and a file table. Source comments were audited across the script files (`led.kel`, `sensor.kel`, `heartbeat.kel`), the bare-metal entry binary, and the platform implementation. The `ZeroSizeOk` allocator wrapper has its rationale documented inline. |
| Migrate the scaffold from `tmp/` to a permanent location and link the documentation from the knowledge graph. | The crate moved to `examples/rtos/`. The architectural specification migrated from `tmp/RTOS_MICROKERNEL_SPEC.md` into the example crate as `examples/rtos/SPEC.md`. The crate stays a standalone (non-workspace-member) Rust crate because its embassy git pins, defmt, cortex-m-rt, and embedded heap allocator are heavy and orthogonal to the parent's normal build. New rows in `docs/README.md` point to the example README, MANUAL, and SPEC. A pointer block at the top of `docs/extras/README.md` explains the colocated companion documents. The parent `README.md` gained an RTOS subsection under Examples with the quick-start commands. `CHANGELOG.md` has an Unreleased Added entry. `CLAUDE.md` was updated to show the new repository structure and to document the detached crate convention. The originals in `tmp/` were removed. |

## Verification matrix

```bash
cargo test --release --features text                                          # 575 lib tests, all pass; 17 integration; 53 rogue scripts; doctests
cargo clippy --workspace --tests --features text -- -D warnings               # clean
(cd examples/rtos && cargo build --release --bin three-task-std)              # clean
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 --no-default-features \
    --features stm32n6570dk-platform)                                         # clean
(cd examples/rtos && cargo test --release)                                    # 2 unit tests pass (status_ok, status_err)
(cd examples/rtos && cargo clippy --release -- -D warnings)                   # clean
(cd examples/rtos && cargo clippy --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 --no-default-features \
    --features stm32n6570dk-platform -- -D warnings)                          # clean
```

Hardware verification on the STM32N6570-DK on 2026-05-18.

```
0.000030 [INFO ] === Keleusma RTOS three-task demonstrator (N6) ===
0.000122 [INFO ] Platform: stm32n6570-dk (gpio_pin_count=256, sensor_channel_count=16)
0.000518 [INFO ] Tasks: led (500ms), sensor (100ms), heartbeat (5000ms)
0.216278 [INFO ] kernel: scheduler entering loop
0.218933 [INFO ] heartbeat: system OK
5.219665 [INFO ] heartbeat: system OK
10.220611 [INFO ] heartbeat: system OK
15.221618 [INFO ] heartbeat: system OK
```

## Quick-start commands

```bash
# Standard library (std) demonstrator on the development host
cd examples/rtos && cargo run --release --bin three-task-std

# STM32N6570-DK demonstrator, with the board attached and BOOT0 in
# the development position
cd examples/rtos && cargo run --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform

# Bare-metal library smoke test that requires no flashing
cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf --lib \
    --no-default-features --features stm32n6570dk-platform
```

## Notes

- The `examples/rtos/` crate is intentionally not a member of the parent workspace. Adding it would force every parent `cargo build` invocation to resolve the embassy git dependencies. The detached layout matches the precedent set by `tmp/embassy_hello_n6/` and lets the parent's normal development loop stay fast.
- A local `.gitignore` inside `examples/rtos/` excludes `target/` and `Cargo.lock` from the parent repository.
- Operator manual at `examples/rtos/MANUAL.md` carries hardware setup, build matrix, platform-trait protocol, Status enum protocol, porting guide, memory budget, defmt log interpretation, troubleshooting, and roadmap.
- Architectural rationale at `examples/rtos/SPEC.md` is the original RTOS microkernel specification, now living next to the implementation.

## Intended Next Step

Awaiting operator prompt. Candidate next moves.

1. **Analogue-to-Digital Converter (ADC) wiring on the STM32N6570-DK.** The `sensor_read` method on the bare-metal platform currently returns zero. Wiring `ADC1` channel zero through the embassy ADC driver removes the stub and lets the sensor task react to real readings.
2. **Pin map extension.** Only PG10 is currently wired on the N6. A handle table installed at boot would expand the addressable pin set to match the 256-pin count published in `PlatformResources`.
3. **Worst-Case Execution Time (WCET) banner at boot.** The specification calls for the verifier-bounded per-task WCET to be printed at boot as certification evidence. Wiring `keleusma::vm::auto_arena_capacity_for` into the boot banner is a small follow-up.
4. **Event bus.** The `WaitForEvent` yield reason is accepted but parks the task indefinitely. A kernel-side event bus and a `host::wait_for_event` native would close the loop.
5. **Precompiled bytecode loading.** Compiling source on-chip costs 215 ms at boot. Loading rkyv-archived bytecode at boot instead would cut both that startup cost and the flash image size.
