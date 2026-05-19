# keleusma-rtos example

> Draft cooperative-scheduling microkernel where every task is a Keleusma `loop main` script. Verified end-to-end on the STM32N6570-DK on 2026-05-18. Standalone Rust crate under `examples/rtos/`; intentionally not a member of the parent workspace because its bare-metal dependencies (embassy git pins, defmt, cortex-m-rt, an embedded heap allocator) are heavy and orthogonal to the parent crate's normal build.

Operator documentation is in [`MANUAL.md`](MANUAL.md). Architectural rationale and roadmap are in [`SPEC.md`](SPEC.md).

## Quick start

All commands below run from inside `examples/rtos/`.

Std demonstrator (development host):

```bash
cd examples/rtos
cargo run --release --bin three-task-std
```

Bare-metal demonstrator (STM32N6570-DK, BOOT0 in dev position, ST-LINK V3-EC attached):

```bash
cd examples/rtos
cargo run --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform
```

Bare-metal library build (smoke test without flashing):

```bash
cd examples/rtos
cargo build --target thumbv8m.main-none-eabihf --lib \
    --no-default-features --features stm32n6570dk-platform
```

## What works

- Three cooperative tasks (LED blinker, sensor poller, heartbeat) dispatch from a single kernel loop.
- The std platform produces simulated GPIO output, a simulated triangular-wave sensor, monotonic time, and stdout logging.
- The N6 platform produces real GPIO toggles on PG10, monotonic time through `embassy_time::Instant`, async sleep through `embassy_time::Timer::at`, and defmt RTT logging. ADC is stubbed to `0` pending phase 4.
- The kernel core is `no_std + alloc`. Std and embedded platforms are feature-gated behind `std-platform` and `stm32n6570dk-platform` respectively.
- `Platform::sleep_until` is async; `Kernel::run` is `async fn`. The std demonstrator drives the kernel through a minimal `block_on`; the N6 demonstrator drives it under the embassy executor.
- DSL natives validate resource indices against `PlatformResources` and return a script-side `Status` enum. The LED task pattern-matches the status to demonstrate the protocol; the prelude declares the enums once.
- Verified on hardware. Boot banner, kernel-construction trace, and four heartbeat log lines at 5-second intervals over 15 seconds of capture (see `MANUAL.md` § 9 and § 7 for the timeline).

## Architecture

```
+-----------------------------------------------------------+
|  Three Keleusma `loop main` tasks (scripts)               |
|  - scripts/led.kel        (toggles GPIO 13)               |
|  - scripts/sensor.kel     (polls channel 0, alarms >1000) |
|  - scripts/heartbeat.kel  (logs every 5 s)                |
|  All three see scripts/prelude.kel, prepended at compile  |
|  time, which declares Status and StatusErrorCode.         |
+-----------------------------------------------------------+
|  Native function surface (src/natives.rs)                 |
|  - host::clock_now  -> Word                               |
|  - host::log(text)                                        |
|  - host::gpio_set(pin, high) -> Status                    |
|  - host::sensor_read(channel) -> Word     (legacy)        |
|  - host::adc_read(channel) -> (Status, Word)              |
|  - host::usart_write / usart_read                         |
|  - host::spi_write / spi_read                             |
|  - host::i2c_write / i2c_read                             |
|  - host::{gpio_pin_count, sensor_channel_count,           |
|           uart_count, spi_count, i2c_count, timer_count}  |
+-----------------------------------------------------------+
|  Kernel core (src/kernel.rs)                              |
|  - Kernel<P: Platform>                                    |
|  - Task table; cooperative dispatch loop                  |
|  - Reads yielded `(reason, payload)`, schedules next      |
|    dispatch, sleeps platform when no task ready           |
+-----------------------------------------------------------+
|  Platform trait (src/platform/mod.rs)                     |
|  - now_ms, sleep_until, log, gpio_set, sensor_read        |
|  - RESOURCES (PlatformResources), NAME                    |
|  - Optional: usart_*, spi_*, i2c_*, adc_read              |
+-----------------------------------------------------------+
|  Per-platform implementations (one file per platform,     |
|  feature-gated):                                          |
|  - src/platform/std.rs           StdPlatform              |
|  - src/platform/stm32n6570_dk.rs Stm32N6570DkPlatform     |
+-----------------------------------------------------------+
|  Shared kernel construction (src/setup.rs)                |
|  - three_task_kernel<P>() and                             |
|    three_task_kernel_with_arena_capacity<P>(cap)          |
|  - Prepends scripts/prelude.kel via include_str!          |
+-----------------------------------------------------------+
|  Entry-point binaries (one per platform):                 |
|  - src/bin/three_task_std.rs                              |
|    Drives Kernel::run through a minimal block_on against  |
|    StdPlatform.                                           |
|  - src/bin/three_task_n6.rs                               |
|    Embassy executor entry; installs PG10; drives the      |
|    kernel against Stm32N6570DkPlatform.                   |
+-----------------------------------------------------------+
```

## Files

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Module root; feature-gated re-exports. |
| `src/platform/mod.rs` | The `Platform` trait and `PlatformResources` struct. Feature-gates per-platform sub-modules. |
| `src/platform/std.rs` | The std-backed `StdPlatform` implementation. Gated on `std-platform`. |
| `src/platform/stm32n6570_dk.rs` | `Stm32N6570DkPlatform` backed by `embassy-stm32`. Gated on `stm32n6570dk-platform`. Pin 13 → PG10. |
| `src/kernel.rs` | `Kernel<P>`, `Task`, scheduler dispatch loop. |
| `src/natives.rs` | Native function registration for tasks. `Status` helpers and DSL natives validated against `PlatformResources`. |
| `src/setup.rs` | Shared kernel-construction code. Prepends `scripts/prelude.kel` to every task source. |
| `src/bin/three_task_std.rs` | Thin host-side entry point. |
| `src/bin/three_task_n6.rs` | Bare-metal entry point. `#![no_std] #![no_main]`. Installs the `embedded_alloc::LlffHeap` global allocator (wrapped to handle zero-byte requests) and drives the kernel under the embassy executor. |
| `build.rs` | Emits embedded link arguments (`--nmagic`, `-Tlink.x`, `-Tdefmt.x`) only when `CARGO_CFG_TARGET_OS == "none"`. |
| `memory.x` | AXISRAM2 map for the embedded build. 640 KB FLASH at 0x34100000, 384 KB RAM at 0x341A0000. |
| `.cargo/config.toml` | Target-scoped `runner = "probe-rs run --chip STM32N657"` for `thumbv8m.main-none-eabihf`. |
| `rust-toolchain.toml` | Toolchain pin (Rust 1.92) with the `thumbv8m.main-none-eabihf` target. |
| `scripts/prelude.kel` | Shared script-side prelude. Declares `Status`, `StatusErrorCode`, and the `use` lines for the host natives. |
| `scripts/led.kel` | LED blinker task. Pattern-matches the `Status` returned by `host::gpio_set`. |
| `scripts/sensor.kel` | Sensor poller task. |
| `scripts/heartbeat.kel` | Heartbeat task. |
| `MANUAL.md` | Operator manual: hardware setup, build matrix, platform protocol, porting guide, troubleshooting. |

## See also

- [`MANUAL.md`](MANUAL.md) — hardware setup, build matrix, platform protocol, porting guide, defmt log interpretation, troubleshooting.
- [`SPEC.md`](SPEC.md) — architectural rationale and long-term roadmap (the original RTOS microkernel specification).
- [`../../docs/README.md`](../../docs/README.md) — the parent project's documentation knowledge graph.
- [`../../README.md`](../../README.md) — the parent Keleusma crate's README.
