# keleusma-rtos Operator Manual

> A draft cooperative-scheduling microkernel where every task is a Keleusma `loop main` script. Verified on the STM32N6570-DK on 2026-05-18. Standalone Rust crate at `examples/rtos/`; intentionally not a member of the parent workspace.

This manual covers hardware setup, the build matrix, the platform abstraction, the script-side protocol, porting to a new board, the memory budget, and troubleshooting. The [`README.md`](README.md) in the same directory carries a one-page overview and the quick-start commands. The architectural rationale and roadmap live in [`SPEC.md`](SPEC.md).

## Table of contents

1. [Hardware requirements](#1-hardware-requirements)
2. [Build matrix](#2-build-matrix)
3. [The platform trait](#3-the-platform-trait)
4. [The Status protocol](#4-the-status-protocol)
5. [Data partitioning: shared, private, and const](#5-data-partitioning-shared-private-and-const)
6. [Porting to a new board](#6-porting-to-a-new-board)
7. [Memory budget](#7-memory-budget)
8. [Defmt log interpretation](#8-defmt-log-interpretation)
9. [Troubleshooting](#9-troubleshooting)
10. [Verified behaviour](#10-verified-behaviour)
11. [Roadmap](#11-roadmap)

## 1. Hardware requirements

### Std demonstrator

No hardware. Builds and runs on the developer's host machine through the system Rust toolchain. Useful for kernel-logic iteration without flashing.

### Bare-metal demonstrator (STM32N6570-DK)

The reference embedded target. The implementation is wired against the on-board green user LED (PG10).

| Item | Notes |
|------|-------|
| Board | STMicroelectronics STM32N6570-DK Discovery Kit. The N6 family runs a Cortex-M55 plus a Neural-ART NPU; the kernel uses the Cortex-M55 core only. |
| Cable | USB-C cable to the board's ST-LINK V3-EC port (the USB-C connector closest to the LCD). The ST-LINK V3-EC provides flashing, debugging, and the defmt RTT channel. |
| Boot configuration | BOOT0 and BOOT1 in the development position (silkscreened on the board near the boot switches). The development position lets probe-rs take control of the chip and load the application into AXISRAM2. The factory position runs the boot ROM from external flash instead, which bypasses probe-rs. |
| Flasher | `probe-rs` 0.30+ on the host. Install with `cargo install probe-rs-tools`. The integrated ST-LINK V3-EC is discovered automatically by chip name `STM32N657`. |
| Optional | A second USB-C cable plugged into the user USB-C port if the demonstrator is extended with a USB device. The current demonstrator does not use it. |

### Probe-rs chip-target alias

The bundled `.cargo/config.toml` declares:

```toml
[target.thumbv8m.main-none-eabihf]
runner = "probe-rs run --chip STM32N657"
```

This is target-scoped so `cargo run --bin three-task-std` (host target) continues to use the default exec, while `cargo run --target thumbv8m.main-none-eabihf --bin three-task-n6` flashes the board. Edit the chip identifier if you adapt the kernel to a different N6 variant.

## 2. Build matrix

All commands run from inside `examples/rtos/`.

### Cargo features that control image size and trust

The microkernel offers two orthogonal cargo features that propagate into the parent crate to gate the source-to-bytecode pipeline and the load-time verifier respectively.

| Feature | Effect when on |
|---------|-----------------|
| `keleusma-compile` | The runtime image carries the lexer, parser, type checker, monomorphizer, and compiler. Task scripts are tokenised and compiled at boot. |
| `keleusma-verify` | The runtime image carries the structural verifier and the resource-bounds check. `Vm::new` runs them at load and rejects modules that violate the contract. |

Both are in the default feature set. Disabling either trades flash for an explicit trust shift; disabling both yields the smallest runtime image.

When `keleusma-compile` is off, `build.rs` invokes the parent crate's compile pipeline at host build time (through a `[build-dependencies]` entry) and emits one `OUT_DIR/<name>.kel.bin` per task script. The runtime then loads through `Module::from_bytes` on `include_bytes!` constants.

When `keleusma-verify` is off, `Vm::new` skips the structural verifier and the resource-bounds check. The host is then attesting that an equivalent verification ran at the artefact-ingestion step. The bytecode-format invariants the VM relies on for memory safety are guaranteed by the producer rather than checked at load.

Measured `.text` size on the bare-metal binary for each useful combination on the STM32N6570-DK target.

| Combination | `.text` | Notes |
|-------------|--------:|-------|
| `keleusma-compile` + `keleusma-verify` (default) | 622 KB | Source compiled at boot, verified at load. Boot to scheduler around 215 milliseconds. |
| `keleusma-verify` only | 160 KB | Precompiled bytecode, verified at load. Boot to scheduler around 43 milliseconds. |
| Neither | 140 KB | Precompiled bytecode, trust-loaded. Boot to scheduler around 39 milliseconds. Smallest image. |

The table grew slightly from the prior pass when two new demonstrator tasks were added: `event_listener` (waits on event id 1, exercises the ISR-to-task wake-up pattern) and `faulty` (triggers `DivisionByZero` every fifth iteration to exercise the supervised-restart policy). The added compiled chunks plus the kernel's pending-event queue, the per-task WCET budget and restart counter fields, and the platform watchdog hook together account for roughly 3 KB.

Several savings landed across the V0.2 closing pass. The `text` and `floats` surface features are both disabled on the runtime keleusma dependency; task scripts use only integer and fixed-point arithmetic. Diagnostic logging routes through the `host::log_event(code, data)` native rather than `host::log(text)`, including kernel-emitted events that previously went through `format!("{:?}", vmerror)` and pulled in the full float formatter chain (`flt2dec`, `CACHED_POW10`, `__divdf3`, `__adddf3`, char `escape_debug_ext`). Each kernel event has its own numeric discriminant declared in `src/natives.rs` and a matching format-string arm in each platform's `log_event` implementation. With `floats` off the `Value::Float` and `ConstValue::Float` variants are compiled out, the `Op::IntToFloat` and `Op::FloatToInt` arms degrade to `VmError::InvalidBytecode` (the variants stay defined to preserve wire-format stability), the VM's binary-arith float branch is conditionally compiled out, and the soft-float `compiler_builtins` routines drop entirely. The release profile sets `panic = "abort"` to drop unwinding tables. Two embassy-stm32 features (`exti`, `unstable-pac`) are dropped because the kernel does not exercise them.

Cumulative reduction against the pre-pass baseline:

| Combination | Baseline | Current | Delta |
|-------------|---------:|--------:|------:|
| `keleusma-compile` + `keleusma-verify` | 614 KB | 621 KB | +7 KB |
| `keleusma-verify` only | 211 KB | 160 KB | **-51 KB** |
| Neither | 192 KB | 140 KB | **-52 KB** |

The full-pipeline mode is essentially unchanged because the source compiler (lexer, parser, type checker, monomorphizer) dominates that image; the savings concentrate in the embedded production modes.

The combination `keleusma-compile` without `keleusma-verify` is technically allowed but rarely useful, because the compiler-emitted bytecode then carries 0 in the WCET and WCMU header fields and the runtime has no analysis to populate them either.

### Std demonstrator

Source mode (default):

```bash
cargo run --release --bin three-task-std
```

Prints the boot banner and the three task log lines to stdout. The LED task's `gpio_set` calls log to stdout (`[gpio 13] -> H/L`); the sensor task logs the alarm crossing each time the simulated triangular wave goes above the threshold; the heartbeat task logs `system OK` every five seconds. Ctrl-C to stop.

Precompiled-bytecode mode on the host:

```bash
cargo run --release --bin three-task-std \
    --no-default-features --features std-platform,keleusma-verify
```

Same behaviour, but the runtime image does not include the compile pipeline. Useful for measuring the size or boot-time difference on the host before deploying the same configuration to the bare-metal target.

### Bare-metal demonstrator (STM32N6570-DK)

Default features for the platform plus the source compile and verifier:

```bash
cargo run --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform,keleusma-compile,keleusma-verify
```

Precompiled bytecode with the verifier (recommended for production):

```bash
cargo run --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform,keleusma-verify
```

Precompiled bytecode under trust (smallest image):

```bash
cargo run --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform
```

The runner setting in `.cargo/config.toml` invokes `probe-rs run --chip STM32N657` against the built ELF. probe-rs erases AXISRAM2, loads the image, opens an RTT channel, and streams defmt records back to the terminal. Reset or detach the probe to stop.

### Bare-metal library smoke test (no flashing)

```bash
cargo build --target thumbv8m.main-none-eabihf --lib \
    --no-default-features --features stm32n6570dk-platform
```

Builds the kernel core and the `Stm32N6570DkPlatform` impl as a library against the embassy stack. Useful for CI and for catching trait-shape regressions in `embassy-stm32`'s upstream changes without needing a flasher.

### Tests

```bash
cargo test --release
```

Runs the unit tests, including the two that pin `status_ok` and `status_err(code)` to the exact `Value::Enum` shapes the host emits. Tests run against the std build only; the embedded targets are validated through the cross-compile build above.

### Lint

```bash
cargo clippy --release -- -D warnings
cargo clippy --target thumbv8m.main-none-eabihf --release \
    --bin three-task-n6 --no-default-features \
    --features stm32n6570dk-platform -- -D warnings
```

Both targets pass clippy clean.

## 3. The platform trait

The kernel core is parameterised over a `Platform: 'static` trait declared in `src/platform/mod.rs`. The trait surface is:

| Item | Shape | Purpose |
|------|-------|---------|
| `const RESOURCES: PlatformResources` | Associated constant. | Static description of GPIO pin count, sensor channel count, and UART/SPI/I2C/timer counts. Read at boot for the banner; read at every native call for index validation. |
| `const NAME: &'static str` | Associated constant. | Human-readable platform name. Appears in the banner. Examples: `"std-host"`, `"stm32n6570-dk"`. |
| `fn now_ms() -> u64` | Required. | Monotonic time since boot, in milliseconds. Must not wrap during expected lifetime. |
| `fn sleep_until(at_ms: u64) -> impl Future<Output = ()>` | Required, async-shape. | Sleep cooperatively until the absolute monotonic time `at_ms`. The std impl blocks the calling thread inside the future's poll; the embassy impl awaits `embassy_time::Timer::at`. Kernel await sites do not change across platforms. |
| `fn log(line: &str)` | Required. | Emit a log line. Std uses `println!`; the N6 uses `defmt::info!` over RTT. |
| `fn gpio_set(pin: u8, high: bool)` | Required. | Drive a GPIO output. The natives layer pre-validates `pin < RESOURCES.gpio_pin_count`. |
| `fn sensor_read(channel: u8) -> u16` | Required. | Read an analogue or simulated sensor channel. The natives layer pre-validates `channel < RESOURCES.sensor_channel_count`. |
| `fn usart_write`, `usart_read`, `spi_write`, `spi_read`, `i2c_write`, `i2c_read`, `adc_read` | Provided with default bodies. | Default to no-op / 0 so platforms with `_count == 0` satisfy the trait without per-platform code. Override on platforms that wire the corresponding peripherals. |

### What the natives layer guarantees

Every DSL native (`src/natives.rs`) validates its index arguments against `PlatformResources` before forwarding to the platform method. The platform's method bodies may therefore assume the index is in range. Out-of-range indices return `Status::Err(StatusErrorCode::Invalid…)` to the script; they never reach hardware.

This decoupling means new platforms only need to populate `RESOURCES` correctly; the validation logic is the same across all platforms.

## 4. The Status protocol

Scripts and the host's native layer share two enums declared in `scripts/prelude.kel`:

```keleusma
enum Status {
    Ok = 0,
    Err(Word) = 1,            // payload is a StatusErrorCode discriminant
}

enum StatusErrorCode {
    // Discriminant 0 is reserved; a Status cast to Word yields 0 for Ok.
    InvalidPin = 1,
    InvalidChannel = 2,
    InvalidController = 3,
    InvalidAddress = 4,
    NotSupported = 5,
    Busy = 6,
    Timeout = 7,
    BadArgument = 8,
}
```

The prelude is prepended to every task source at compile time by `setup::build_module`. Tasks see the enums and the relevant `use host::…` declarations without copying them into each script.

### How natives return Status

Write natives return `Status` directly. Read natives return `(Status, Word)` so a successful read carries the value and a failed read carries the error code with a zero value placeholder. The host helpers `status_ok()` and `status_err(StatusErrorCode)` in `src/natives.rs` construct the exact `Value::Enum` shapes that the script-side `match` arms recognise.

### Idiomatic script-side usage

```keleusma
const data ev {
    gpio_fail: Word = 2,
}

match host::gpio_set(13, state.on) {
    Status::Ok => (),
    Status::Err(code) => host::log_event(ev.gpio_fail, code),
};
```

The compiler emits an `Op::IsEnum(enum_const, variant_const)` chain that pattern-matches against the type-name and variant strings. Native-constructed `Value::Enum` values participate in the same dispatch.

Script-side logging routes through `host::log_event(code, data)` rather than `host::log(text)`. The task scripts compile without the `text` surface feature, which removes the lexer, parser, and runtime support for string literals from the flash image. A per-event format string lives on the host side in each `Platform::log_event` implementation, and the script and host agree on the numeric event discriminants by convention. The constants in `src/natives.rs` (`EV_HEARTBEAT_OK`, `EV_LED_GPIO_FAIL`, `EV_SENSOR_ABOVE`) document the current set.

### Tuple-returning natives

```keleusma
const data ev {
    adc_ok: Word = 4,
    adc_fail: Word = 5,
}

let (status, value) = host::adc_read(0);
match status {
    Status::Ok => host::log_event(ev.adc_ok, value),
    Status::Err(code) => host::log_event(ev.adc_fail, code),
};
```

## 5. Data partitioning: shared, private, and const

Keleusma V0.2 partitions a script's persistent data into three classes. Each class has a different host visibility, a different storage location, and a different lifecycle. The microkernel's heartbeat task demonstrates all three.

### Shared data

```keleusma
data state {
    count: Word,
}
```

Equivalent to bare `data` in earlier versions. Shared data is host-visible through `Vm::set_data(slot, value)` and `Vm::get_data(slot)`. Storage is owned by the Vm. Survives RESET. The host populates initial values before `Vm::call`; the script reads and writes the same slots through `state.field` syntax.

### Private data

```keleusma
private data state {
    counter: Word,
}
```

Private data lives in the arena's persistent region. The host has no API access; `Vm::set_data` and `Vm::get_data` reject private slot indices. The script reads and writes through `state.field` exactly like shared data. Survives RESET. The host must size the arena's persistent capacity before constructing the VM:

```rust
arena.resize_persistent(vm::required_persistent_capacity_for(&module))?;
let vm = Vm::new(module, &arena)?;
```

The compiler rejects modules whose private data is never written; the diagnostic recommends `const data` as the rewrite.

### Const data

```keleusma
const data cfg {
    period_ms: Word = 5000,
}
```

Compile-time constants. Field reads compile to constant loads in the per-chunk constant pool; field writes are compile errors. No runtime data-segment slot is allocated. Supports scalar primitives (`Word`, `Byte`, `Float`, `Bool`, `Text`, unit) and composite tuple and array literals. The heartbeat task uses `const data cfg { period_ms: Word = 5000 }` so the period is baked into the bytecode rather than supplied by the host.

### Choosing the right class

| Need | Class |
|------|-------|
| Host wants to read or write the value at runtime | shared |
| Script wants persistent storage hidden from the host | private |
| Value is fixed at compile time and never changes | const |

The compiler enforces these rules. Mixing classes is permitted (one block of each visibility per module under R28).

## 5.5. Numeric overflow handling (demonstrated by the heartbeat task)

The heartbeat task's counter increment uses the V0.2 numeric overflow construct to saturate on `Word::MAX` rather than wrap on a hypothetical long-running deployment whose mission outlives the i64 range. The construct dispatches on the arithmetic outcome and selects a saturation value when overflow or underflow occurs.

````
state.count = state.count + 1 {
    overflow => saturate_max,
    underflow => saturate_min,
    ok(v) => v,
};
````

The `saturate_max` and `saturate_min` keywords resolve to `Word::MAX` and `Word::MIN` respectively. The `ok(v) => v` arm passes the successful sum through unchanged. The construct is supported for `+`, `-`, `*`, `/`, `%`, and unary `-` on Word operands; each must cover `ok`, `overflow`, and `underflow` exactly once (the pipe pattern `overflow|underflow => shared_body` combines two outcomes).

The construct compiles to a checked-arithmetic opcode (`Op::CheckedAdd`, `Op::CheckedSub`, `Op::CheckedMul`, `Op::CheckedNeg`, or for division and modulo the regular opcode plus a stamped zero flag) followed by a flag-based dispatch through an If/Else block. The runtime cost per construction is the arithmetic opcode plus the dispatch (one local store, one local load, one compare, one branch); no host-side cycle counting is required because the bound is statically known.

For probe and embedded deployments where saturation rather than wrapping is the correct failure mode for accumulators, the construct removes a class of silent-arithmetic bugs without imposing the overhead of dynamic checks at every site.

## 5.6. Other V0.2 language features

The remaining V0.2 surface extensions (newtype declarations, refinement-type predicates, information-flow labels) are not yet adopted by the microkernel demonstrator but compose naturally with the patterns shown above. The reference documentation is in [`docs/design/GRAMMAR.md`](../../docs/design/GRAMMAR.md) Section 7.5 and [`docs/architecture/LANGUAGE_DESIGN.md`](../../docs/architecture/LANGUAGE_DESIGN.md) Section "Surface Extensions Added in V0.2".

Worked patterns operators may want to adopt:

- **Newtype for time-precision discipline.** `newtype LocalProperMs = Word; newtype OriginFrameMs = Word;` plus host natives that produce each separately makes accidental cross-frame arithmetic a type error.
- **Refinement types for input validation.** `newtype Percent = Word where in_range_0_100;` traps at the construction site rather than at downstream use, localising the failure to the point of input rather than the point of damage.
- **Information-flow labels for telemetry separation.** `Word@MissionSecret` on sensitive sensor channels and `Word@Open` on transmittable telemetry, with explicit `declassify` operators marking the disclosure audit points.

## 6. Porting to a new board

The three-layer split (kernel core, platform impl, entry binary) makes the port mechanical. The kernel core does not change.

### Steps

1. **Add `src/platform/<name>.rs`.** Declare a unit struct `pub struct <Name>Platform;` and implement `Platform` for it. Populate `RESOURCES` with the board's peripheral count, `NAME` with a short identifier, and the five required methods. Optional methods (`usart_*`, `spi_*`, `i2c_*`, `adc_read`) have default bodies; override the ones the board actually wires. See `src/platform/stm32n6570_dk.rs` for a concrete reference.
2. **Feature-gate the sub-module in `src/platform/mod.rs`.** Add the matching `#[cfg(feature = "...")]` blocks that declare the sub-module and re-export the marker type at the platform module root.
3. **Declare the feature in `Cargo.toml`.** Add `<name>-platform = ["dep:embassy-<board>", "dep:cortex-m", ...]`. Mirror the dependency set used by `stm32n6570dk-platform` for embassy-backed boards; trim to what the board actually needs.
4. **Decide on peripheral storage.** Trait methods are associated functions (no `&self`), so peripheral handles need static storage. The N6 impl uses `critical_section::Mutex<RefCell<Option<...>>>` plus a one-shot `install(handles)` function called once from the binary's `main`. Adapt this pattern: declare statics for each handle the platform owns, install them at boot, read them inside the trait methods.
5. **Add a binary `src/bin/three_task_<name>.rs`** following the shape of `three_task_n6.rs`. Set the `[[bin]]` entry in `Cargo.toml` with the matching `required-features`. Initialise the heap, call `embassy_<board>::init`, install peripherals, and call `three_task_kernel_with_arena_capacity::<NamePlatform>(arena_capacity)`. Tune `HEAP_SIZE` and `TASK_ARENA_CAPACITY` against the board's RAM.
6. **Add `memory.x` if the board has its own memory map.** Conventional Cortex-M boards with on-chip flash can share a single `memory.x`; boards with unusual layouts (the N6 is one such — it has no on-chip flash) need a board-specific map. `build.rs` already emits the embedded link arguments only when `CARGO_CFG_TARGET_OS == "none"`, so the std demonstrator continues to link cleanly.
7. **Verify.** Build the library against the new feature combination as a smoke test, then build the binary and flash. Run for at least fifteen seconds of defmt RTT capture and confirm the heartbeat log fires three times.

### Multi-pin wiring

`gpio_set(pin: u8, high: bool)` takes a pin index but the platform decides which underlying handle to drive. The N6 impl maps pin 13 to PG10 and warns on any other index. A more elaborate impl can map a range of indices to a table of `Output<'static>` handles installed via `install`. The natives layer only validates `pin < RESOURCES.gpio_pin_count`; the platform's pin-to-handle resolution is its own responsibility.

## 7. Memory budget

### N6 layout

The bundled `memory.x` allocates the AXISRAM2 region (1024 KB at `0x34100000`):

| Region | Origin | Length | Purpose |
|--------|--------|-------:|---------|
| FLASH  | `0x34100000` | 640 KB | The Keleusma runtime image (lexer, parser, type checker, monomorphizer, compiler, VM, verifier) plus the kernel core, platform impl, embassy stack, defmt, and the entry binary. Current usage is ~622 KB under the full-pipeline default; precompiled bytecode modes use 140-160 KB, leaving 480-500 KB free for user code and NPU weights. |
| RAM    | `0x341A0000` | 384 KB | The global heap (320 KB), other `.bss` (transient), stack, and embassy executor state. The three per-task arenas are leaked into the heap. |

The N6 has no on-chip flash. The boot ROM enables AXISRAM2 regardless of BOOT0 position, and probe-rs loads the application into it directly. The map fills AXISRAM2 entirely; future iterations may slim the FLASH image by shipping precompiled bytecode and stripping the compile-time pipeline.

### Heap sizing

`HEAP_SIZE = 320 KB` in `three_task_n6.rs` is sized to cover:

- Three leaked task arenas: `3 * TASK_ARENA_CAPACITY = 48 KB`.
- The compile-pipeline transient state for each task (tokens, AST, bytecode, constant pool, monomorphization): tens of KB per task, freed when the per-task `build_module` returns. Peak demand happens during the third task's build, when two prior arenas already occupy 32 KB and the compile pipeline runs against a third script.
- Margin for `linked_list_allocator` fragmentation: the LlffHeap is a first-fit linked-list allocator and grows holes between freed transients. A practical margin of ~50% of the working set has been adequate for the three demonstrator scripts.

If you add more tasks, expect `HEAP_SIZE` to grow super-linearly with the task count because of fragmentation. A more compact allocator (TLSF) or precompiled bytecode loading would reduce both peak working set and fragmentation.

### Arena sizing

`TASK_ARENA_CAPACITY = 16 KB` covers the runtime working set of the three demonstrator scripts. The arena holds:

- The Vm's operand stack (`Vec<Value, BottomHandle>` rooted in the arena's bottom).
- The Vm's call-frame table (likewise rooted in the bottom).
- Any `KStr` allocations the script makes through arena-aware natives. The demonstrator natives do not allocate `KStr`.

If you migrate scripts that build large temporary arrays, raise `TASK_ARENA_CAPACITY` and the heap accordingly. The std demonstrator uses `DEFAULT_ARENA_CAPACITY = 64 KB` per task because heap is abundant.

### Zero-byte allocator wrapper

`linked_list_allocator` (under `embedded-alloc::LlffHeap`) returns an allocation error for zero-byte layouts. Rust's `alloc` crate treats that as OOM and panics with `memory allocation of 0 bytes failed`. The Rust allocator contract permits returning a dangling well-aligned pointer for zero-byte requests, and most callers (`Vec::with_capacity(0)`, `RawVec::shrink_to(0)`) expect that behaviour.

The N6 binary therefore wraps `LlffHeap` in a `ZeroSizeOk` adapter that intercepts zero-byte allocations on every `GlobalAlloc` entry point (`alloc`, `alloc_zeroed`, `realloc`, `dealloc`) and returns `layout.align() as *mut u8` rather than reaching the underlying allocator. The wrapper is a few dozen lines and adds no measurable runtime cost.

## 8. Defmt log interpretation

A clean run on the N6 looks like this:

```
0.000030 [INFO ] === Keleusma RTOS three-task demonstrator (N6) ===
0.000122 [INFO ] Platform: stm32n6570-dk (gpio_pin_count=256, sensor_channel_count=16)
0.000518 [INFO ] Tasks: led (500ms), sensor (100ms), heartbeat (5000ms)
0.214965 [INFO ] kernel: scheduler entering loop
0.217620 [INFO ] heartbeat: system OK
5.218658 [INFO ] heartbeat: system OK
10.219635 [INFO ] heartbeat: system OK
15.220611 [INFO ] heartbeat: system OK
```

Reading the timeline:

- Lines at t≈0 are the boot banner. The first three are direct `defmt::info!` calls from `three_task_n6.rs`; they describe the platform and the task periods.
- The `kernel: scheduler entering loop` line at t≈215 ms reports the end of kernel construction. The 215 ms is the wall-clock cost of compiling three Keleusma scripts on the Cortex-M55 — tokenisation, parsing, type checking, monomorphisation, bytecode generation, and VM setup for each.
- The first `heartbeat: system OK` at t≈218 ms is the heartbeat task's first dispatch. It logs immediately after `kernel.run().await` enters the scheduler loop.
- Subsequent heartbeats at 5.218 s, 10.219 s, 15.220 s confirm the scheduler is dispatching the heartbeat task on its 5000 ms cadence. Drift below 2 ms across 15 seconds (the heartbeat is scheduled 5000 ms after its previous wake) is the embassy time driver's resolution.

What is **not** logged:

- LED toggles. `host::gpio_set` does not log; the LED's behaviour is observable only on the board. PG10 should be visibly toggling at 2 Hz (high for 500 ms, low for 500 ms).
- Sensor reads. The sensor task calls `host::sensor_read`, which the N6 impl currently stubs to `0`. The "above threshold" log only fires when the reading exceeds 1000, so the sensor task on the N6 produces no log lines during normal operation.

To capture more, add `defmt::info!` calls inside `Stm32N6570DkPlatform::gpio_set` or extend the LED script to call `host::log_event` on each toggle. The host owns the format string, so adding a new event takes three coordinated edits: define a new `EV_*` constant in `src/natives.rs`, add a matching arm in each platform's `Platform::log_event` implementation, and call `host::log_event(new_code, data)` from the script.

## 9. Troubleshooting

### `SwdApWdataError` warnings during flash

```
WARN probe_rs::probe::stlink: send_jtag_command 242 failed: SwdApWdataError
```

Harmless. probe-rs retried a few SWD writes during initial chip attach. The flash still completes. Common on first connection after a power cycle.

### `memory allocation of N bytes failed` panic

The global allocator returned null for an allocation of `N` bytes:

- **N > 0**: The heap is exhausted. Either raise `HEAP_SIZE` and the RAM region in `memory.x`, or shrink `TASK_ARENA_CAPACITY`, or reduce the number of tasks. Check the size against the board's available SRAM.
- **N == 0**: The wrapper around `LlffHeap` was bypassed. Verify `static HEAP: ZeroSizeOk<Heap>` is the `#[global_allocator]` (not a bare `Heap`). The wrapper handles `alloc`, `alloc_zeroed`, `realloc`, and `dealloc`; if you change it, keep all four overridden so the zero-byte guard always wins.

### Stack overflow / undefined instruction in unexpected places

Most likely the stack ran into the heap. The stack lives at the top of RAM and grows downward; the heap (`HEAP_MEM`) lives in `.bss`. If `HEAP_SIZE` plus other `.bss` exceeds RAM minus stack reserve, the two collide. Cortex-M55 raises a usage fault on the resulting bad access. Either grow RAM in `memory.x` or shrink `HEAP_SIZE`.

### BOOT0 in factory position

probe-rs cannot take control of the chip. Symptoms: `probe-rs run` times out, or the chip stays in factory firmware. Move BOOT0 to the development position (silkscreened). Factory firmware can be restored later through STM32CubeProgrammer with the published ST image; the development position does not erase or alter factory firmware in external flash.

### `cargo run` builds but probe-rs not found

`cargo install probe-rs-tools` from a host with `cargo` available. The runner is invoked by name from `.cargo/config.toml`, so `probe-rs` must be on `PATH`.

### Embassy panic about HSE / MSI / PLL

```
panicked at 'When the HSE is used as cpu/system bus clock or clock source for any PLL, it is not allowed to be disabled'
```

The bare-metal binary tried to reconfigure a clock source that's still in use. The default `embassy_stm32::init(Default::default())` configuration should not trip this; the panic implies a custom `Config` that disables a clock in use. Revert to `Default::default()` and add custom clock setup only after the demonstrator runs end-to-end.

## 10. Verified behaviour

Verified on 2026-05-18 against an STM32N6570-DK.

| Check | Result |
|-------|--------|
| Std demonstrator runs end-to-end with the expected timeline (LED 500 ms, sensor ~100 ms, heartbeat 5000 ms). | Pass. |
| Bare-metal `--lib` build for `thumbv8m.main-none-eabihf` succeeds with `--features stm32n6570dk-platform`. | Pass. |
| Bare-metal binary build succeeds. | Pass. text 614 KB, bss 329 KB; fits the declared 640 KB FLASH + 384 KB RAM. |
| Bare-metal binary flashes and runs on the N6. | Pass. Boot banner, kernel-construction trace, scheduler entry at t≈215 ms, heartbeat at t≈218 ms and every 5000 ms thereafter. Captured 15+ seconds with four heartbeat ticks. |
| `cargo test --release`. | Pass. Two unit tests on the `Status` helpers. |
| `cargo clippy --release -- -D warnings`. | Clean. |
| `cargo clippy --target thumbv8m.main-none-eabihf --release --bin three-task-n6 --no-default-features --features stm32n6570dk-platform -- -D warnings`. | Clean. |

The LED on the board is presumed to be toggling at 2 Hz; verification of the physical edge requires visual inspection at the bench.

## 11. Roadmap

Items left for follow-up work. None are blockers for the current state of the demonstrator.

| Item | Notes |
|------|-------|
| ADC wiring on the N6 | `sensor_read` returns `0` on the N6 (stub). Phase 4 wires `ADC1` channel 0 through `embassy-stm32`'s ADC driver and removes the stub. The natives layer is already in place; only the platform method body changes. |
| Pin map extension | The N6 wires only pin 13 (PG10). A pin-to-handle table installed at boot would expand the addressable pin set. The natives layer accepts any index below `RESOURCES.gpio_pin_count` (256). |
| WCET banner | The spec calls for the verifier-bounded per-task WCET to be printed at boot as certification evidence. `keleusma::vm::auto_arena_capacity_for` and related hooks expose the data; wiring them into the boot banner is a small follow-up. |
| Event bus | The `YieldReason::WaitForEvent` code is accepted but parks the task indefinitely. A kernel-side event bus and a `host::wait_for_event` native would close the loop. |
| Hot reload | The rogue example's F5 reload pattern applies cleanly; not wired here. |
| Precompiled bytecode | Loading rkyv-archived bytecode at boot instead of compiling source on-chip would cut the 215 ms kernel-construction time and shrink the FLASH image. |
| Priorities and deadlines | First-ready-wins is the only scheduling policy. Priority and deadline-monotonic policies are documented as future work in the spec. |

## See also

- [`README.md`](README.md) — overview, file table, quick-start commands.
- [`SPEC.md`](SPEC.md) — the architectural rationale, the three-layer split, the conservative-verification stance, and the long-term roadmap.
- [`../../docs/README.md`](../../docs/README.md) — the parent project's documentation knowledge graph.
- [`../../README.md`](../../README.md) — the parent Keleusma crate's README.
