# Keleusma RTOS Microkernel Specification

> Working spec for a Keleusma-driven cooperative real-time microkernel. Not part of the shipped V0.x roadmap. Tracked under `tmp/` until promoted to `docs/`.

## 1. Motivation

The Keleusma verifier's WCET and WCMU bounds, the `loop main` cooperative yield model, and the conservative-verification stance compose into a coherent kernel design that does not exist in shipping form today. The combination "total cooperative scheduling" gives an RTOS the response-latency guarantee of a pre-emptive kernel without the per-task kernel stacks, atomic critical sections, priority inheritance machinery, and interrupt-during-syscall complexity that preemption normally drags in.

The shipped rogue example demonstrates the host-script architecture at consumer-software scale. This spec proposes the same architecture applied to a safety-critical embedded RTOS niche.

## 2. Goals and non-goals

**Goals.**

- A minimal trusted Rust microkernel that boots, schedules cooperative tasks, services interrupts, and dispatches a Keleusma virtual machine per task.
- Per-task logic expressed as `loop main` Keleusma scripts with verifier-proven WCET and WCMU bounds.
- Inversion of control between the kernel core and the platform: a single trait abstracts every platform-specific operation. Porting the kernel to a new target is a matter of writing a new trait implementation.
- Two reference implementations of the platform trait: an `std`-backed implementation for development and CI, and an `embassy`-backed implementation for embedded targets (Cortex-M, RISC-V, Xtensa).
- A demonstrator example with three or four cooperative tasks producing an observable scheduling timeline.

**Non-goals.**

- Formal verification of the kernel implementation. The verifier's correctness in design is load-bearing for safety arguments; verifying the implementation itself is downstream work analogous to the seL4 verification effort.
- A general-purpose operating system. This is an RTOS for fixed-task-count systems with bounded resources.
- Preemption. The cooperative-plus-total combination is the load-bearing design choice.
- A drop-in replacement for FreeRTOS, Zephyr, or RTIC. This is a fresh architecture, not a port of an existing API.
- Multi-core scheduling beyond "one VM per core, IPC through shared memory". Tightly-coupled threaded workloads are out of scope.
- Dynamic task creation. The task graph is fixed at link time. Future iterations could relax this.

## 3. Architecture

### 3.1 Layered structure

```
+-----------------------------------------------------------+
|  Application tasks (Keleusma loop main scripts)           |
|  - One script per logical task                            |
|  - WCET and WCMU bounds proved at compile time            |
+-----------------------------------------------------------+
|  Native function surface (Rust)                           |
|  - host::clock_now, host::sleep_until, host::log, etc.    |
|  - One implementation per native; trait-dispatched        |
+-----------------------------------------------------------+
|  Kernel core (Rust, generic over the platform trait)      |
|  - Scheduler: cooperative dispatch on next-wakeup         |
|  - VM pool: one Keleusma Vm per task                      |
|  - Interrupt entry/exit (cooperative kernel doesn't       |
|    preempt user tasks, but real ISRs run)                 |
+-----------------------------------------------------------+
|  Platform trait (the inversion point)                     |
|  - Clock, sleep, log, GPIO, sensor, etc.                  |
|  - Implementations: StdPlatform, EmbassyPlatform<HAL>     |
+-----------------------------------------------------------+
|  Concrete platform                                        |
|  - std + tokio (development host)                         |
|  - embassy-stm32, embassy-nrf, embassy-rp, etc.           |
+-----------------------------------------------------------+
```

### 3.2 Trusted computing base

The trusted portion is the kernel core plus the chosen platform implementation plus the Keleusma VM. Task scripts are outside the trust boundary; the verifier proves their safety properties at compile time.

Estimated sizes:

| Component | LOC |
|-----------|-----|
| Kernel core (scheduler, VM pool, interrupt entry) | ~600 |
| Platform trait | ~100 |
| Std platform implementation | ~300 |
| Embassy platform implementation | ~400 (depends on target HAL) |
| Native function registry | ~300 |
| Keleusma VM (existing crate) | ~6000 |
| **Trusted Rust total (per target)** | **~7400-7700** |

Below ten thousand lines for the full kernel, dominated by the existing Keleusma VM. Comparable to FreeRTOS's nine thousand lines of C. seL4 lands in a similar size class with its verification artifacts being much larger than the kernel itself.

### 3.3 Task model

A task is one Keleusma `loop main` script. Each task owns a virtual machine and an arena. The script's `fn main` parameter list and return type form the task's communication boundary with the kernel.

The conventional task signature:

```keleusma
data state {
    // Per-task private state preserved across yields.
}

loop main(wakeup_reason: Word) -> (Word, Word) {
    // ... read inputs, run logic, write outputs ...
    let next_wakeup_ms = state.last_run + state.period_ms;
    let _ = yield (NEXT_WAKEUP, next_wakeup_ms);
    (0, 0)
}
```

The yielded tuple is `(reason, payload)`. The kernel reads it to decide when to resume the task.

Yield reasons (initial set):

| Reason code | Name | Payload |
|------|------|---------|
| 0 | Wait | Sleep until the given monotonic time in milliseconds. |
| 1 | EventWait | Block until the given event is signalled. Payload is the event id. |
| 2 | Yield | Yield without a wakeup condition; scheduler picks the next ready task. |

Future codes can add timeouts on event waits, priority hints, and similar refinements.

## 4. The platform trait

The trait is the inversion point. The kernel core depends on this trait and nothing else from the platform. A new target is one trait implementation.

```rust
/// The platform abstraction. Implementations provide a clock,
/// async sleep, logging, and a small I/O surface. The kernel
/// core depends on this trait alone and is otherwise platform-
/// independent.
///
/// Implementations are typically zero-sized or hold only static
/// state. The kernel calls trait methods through a generic type
/// parameter to avoid dynamic dispatch.
pub trait Platform: Send + Sync + 'static {
    /// Monotonic time since boot, in milliseconds. Must not
    /// wrap during the kernel's expected lifetime.
    fn now_ms() -> u64;

    /// Sleep cooperatively until the absolute monotonic time
    /// `at_ms` has passed. The async signature is what permits
    /// the embassy implementation; the std implementation can
    /// use `tokio::time::sleep_until` or a poll loop.
    fn sleep_until(at_ms: u64) -> impl core::future::Future<Output = ()>;

    /// Emit a log line. On std targets this routes to stdout
    /// via println!. On embedded targets this routes to defmt,
    /// rtt-target, or a UART driver.
    fn log(line: &str);

    /// Set a GPIO-like output. Implementations decide how to
    /// map `pin` onto their hardware. Simulated platforms can
    /// print "GPIO N -> H/L".
    fn gpio_set(pin: u8, high: bool);

    /// Read an analogue or simulated sensor channel. Returns
    /// a sixteen-bit unsigned value.
    fn sensor_read(channel: u8) -> u16;

    /// Signal a kernel event by id. Used by ISRs to wake
    /// event-waiting tasks. On std, a software signal.
    fn signal_event(event_id: u8);

    /// Yield cooperatively to the platform's executor. The std
    /// implementation may be a no-op or a tokio yield; the
    /// embassy implementation lets other futures run.
    fn yield_now() -> impl core::future::Future<Output = ()>;
}
```

The async signature on `sleep_until` and `yield_now` is the embassy concession. Embassy's `Timer::at(...).await` and `yield_now().await` are the natural fit. The std implementation wraps `tokio::time::sleep_until` or a simple busy-poll-with-yield.

Async-trait crate is not used; the `impl Future` return type in trait methods is a stable feature since Rust 1.75.

### 4.1 Why a trait rather than `cfg` switches

Conditional compilation works but does not enforce the abstraction. With a trait, a kernel-core function cannot accidentally call a platform-specific API because the trait surface is the only platform-visible surface. The kernel core compiles against the trait, and the platform is a generic parameter. The compiler enforces the boundary.

## 5. Reference implementations

### 5.1 Std backend

```rust
pub struct StdPlatform;

impl Platform for StdPlatform {
    fn now_ms() -> u64 {
        // Time since process start.
        START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
    }

    async fn sleep_until(at_ms: u64) {
        let now = Self::now_ms();
        if at_ms > now {
            tokio::time::sleep(std::time::Duration::from_millis(at_ms - now)).await;
        }
    }

    fn log(line: &str) {
        println!("{}", line);
    }

    fn gpio_set(pin: u8, high: bool) {
        println!("[gpio {}] -> {}", pin, if high { "H" } else { "L" });
    }

    fn sensor_read(channel: u8) -> u16 {
        // Simulated sensor: triangular wave on channel 0,
        // constant on channel 1.
        match channel {
            0 => triangular_wave_at(Self::now_ms()),
            _ => 512,
        }
    }

    fn signal_event(event_id: u8) {
        EVENT_BUS.signal(event_id);
    }

    async fn yield_now() {
        tokio::task::yield_now().await;
    }
}
```

Approximate size: 300 LOC including the simulated sensors, the event bus, and the static initialisers.

### 5.2 Embassy backend (Cortex-M reference)

```rust
pub struct EmbassyPlatform<H: embassy_hal_internal::Peripheral>; // sketch

impl<H> Platform for EmbassyPlatform<H> {
    fn now_ms() -> u64 {
        embassy_time::Instant::now().as_millis()
    }

    async fn sleep_until(at_ms: u64) {
        embassy_time::Timer::at(embassy_time::Instant::from_millis(at_ms)).await;
    }

    fn log(line: &str) {
        defmt::info!("{}", line);
    }

    fn gpio_set(pin: u8, high: bool) {
        // Look up the pin in the platform's GPIO registry and
        // drive it. The registry is initialised at boot.
        GPIO_REGISTRY.lock().drive(pin, high);
    }

    fn sensor_read(channel: u8) -> u16 {
        ADC.lock().read_blocking(channel)
    }

    fn signal_event(event_id: u8) {
        EVENT_BUS.signal(event_id);
    }

    async fn yield_now() {
        embassy_futures::yield_now().await;
    }
}
```

Approximate size: 400 LOC including the GPIO registry, the ADC wrapper, and the event-bus implementation. Specific HAL crate (`embassy-stm32`, `embassy-nrf`, `embassy-rp`) selected via Cargo features.

A port to a new microcontroller is a new `EmbassyPlatform<NewHal>` impl plus the GPIO/ADC mapping. The kernel core does not change.

## 6. The kernel core

### 6.1 Scheduler

Cooperative, sleep-until-driven, monotonic-clock based.

```rust
pub struct Kernel<P: Platform> {
    tasks: heapless::Vec<Task, MAX_TASKS>,
    _phantom: PhantomData<P>,
}

struct Task {
    vm: Vm<'static, 'static>,
    name: &'static str,
    next_wakeup_ms: u64,
    state: TaskState,
}

enum TaskState {
    Ready,
    SleepingUntil(u64),
    WaitingFor(u8), // event id
    Finished,
}

impl<P: Platform> Kernel<P> {
    pub async fn run(&mut self) -> ! {
        loop {
            // 1. Pick the next runnable task.
            let now = P::now_ms();
            let candidate = self.tasks.iter_mut()
                .filter(|t| matches!(t.state, TaskState::Ready))
                .next();
            let task = match candidate {
                Some(t) => t,
                None => {
                    // No ready task. Sleep until the earliest wakeup.
                    let next_wake = self.earliest_wakeup();
                    P::sleep_until(next_wake).await;
                    self.refresh_ready();
                    continue;
                }
            };
            // 2. Dispatch the task.
            let result = task.vm.resume(Value::Int(0));
            // 3. Read its yielded payload.
            match result {
                Ok(VmState::Yielded(Value::Tuple(t))) if t.len() == 2 => {
                    let reason = extract_int(&t[0]);
                    let payload = extract_int(&t[1]);
                    task.state = match reason {
                        0 => TaskState::SleepingUntil(payload as u64),
                        1 => TaskState::WaitingFor(payload as u8),
                        2 => TaskState::Ready,
                        _ => TaskState::Finished,
                    };
                }
                Ok(VmState::Reset) => {
                    // loop main wrapped; resume normally next round.
                    task.state = TaskState::Ready;
                }
                _ => task.state = TaskState::Finished,
            }
        }
    }
}
```

The scheduler is approximately 300 lines of straight-line code. There are no locks because cooperative scheduling means the kernel runs exclusively between dispatches. ISRs that signal events use `core::sync::atomic` and `signal_event`; the dispatcher refreshes the ready set on each iteration.

### 6.2 Worst-case latency claim

For task `T` with WCET-to-yield `W_T` cycles, the worst-case dispatch latency from scheduler-tick to `T`-runs is:

```
latency(T) = scheduler_overhead + max(W_other for other tasks ready at tick)
```

Every `W_other` is a verifier-proven bound. The kernel's `scheduler_overhead` is a small constant measurable on the platform. Therefore `latency(T)` is bounded by a known constant for every task, certifiable.

This is the load-bearing property. Preemptive RTOSes need additional verification artifacts to make a similar claim (priority inversion protocols, atomic critical sections, etc.). The cooperative + total combination gets the same result for free from the verifier.

### 6.3 Interrupt handling

ISRs run in interrupt context, do minimum work (read hardware register, signal an event), and exit. The kernel dispatcher picks up the signalled event on its next iteration.

ISR responsibilities:

- Acknowledge the interrupt at the hardware level.
- Update any non-blocking shared state (atomic counters, queue tails).
- Call `P::signal_event(event_id)` to wake event-waiting tasks.
- Return.

ISRs do not call into the Keleusma VM. The VM runs only in the dispatch loop. This is what keeps the trust boundary clean.

## 7. Native function surface

Tasks call host natives to interact with the platform. The natives are thin wrappers around `Platform` methods.

| Native | Signature | Semantics |
|--------|-----------|-----------|
| `host::clock_now() -> Word` | nullary | Returns `P::now_ms()`. |
| `host::log(message: Text)` | unary | Calls `P::log`. |
| `host::gpio_set(pin: Word, high: Word)` | binary | Calls `P::gpio_set`. |
| `host::sensor_read(channel: Word) -> Word` | unary | Calls `P::sensor_read`. |
| `host::wait_for_event(event_id: Word)` | (unary, yields) | Yields with reason 1, payload `event_id`. |

The yield-style natives (`wait_for_event`) do not call back into the kernel; they emit a yielded tuple that the kernel reads. This keeps the dispatch shape uniform.

## 8. Demonstrator example

The reference example, under `examples/rtos/`, has three cooperative tasks.

| Task | Period | Behaviour |
|------|--------|-----------|
| LED toggle | 500 ms | Flips GPIO 13. State counter increments. |
| Sensor poll | 100 ms | Reads channel 0, logs when above threshold. |
| Heartbeat | 5000 ms | Logs `"system OK uptime={}ms"`. |

Expected output, condensed:

```
[t=    0] kernel: launching 3 tasks
[t=    0] kernel: WCET led=1240c sensor=890c heartbeat=320c
[t=    0] kernel: worst-case latency floor: ~1240 cycles + dispatch
[t=    0] led: gpio 13 -> H, next wakeup t=500
[t=    0] sensor: ch0=512, next wakeup t=100
[t=    0] heartbeat: system OK uptime=0ms, next wakeup t=5000
[t=  100] sensor: ch0=523, next wakeup t=200
[t=  200] sensor: ch0=587, next wakeup t=300
[t=  300] sensor: ch0=672, ABOVE threshold!, next wakeup t=400
[t=  400] sensor: ch0=781, ABOVE threshold!, next wakeup t=500
[t=  500] led: gpio 13 -> L, next wakeup t=1000
[t=  500] sensor: ch0=890, ABOVE threshold!, next wakeup t=600
...
```

The verifier-proven WCETs printed at boot are the certification evidence. The timeline below is the qualitative correctness evidence.

### 8.1 Test plan

- Unit tests on the scheduler dispatch logic with a mock platform.
- Tests on each task script's WCET coming out of the verifier with an asserted value.
- Integration test running the std platform for one simulated second, asserting the expected number of dispatches per task.
- A QEMU-Cortex-M run as a CI step, asserting the same timeline.

## 9. Build and feature gating

### 9.1 Crate layout

```
keleusma-rtos/
├── Cargo.toml
├── src/
│   ├── lib.rs             # Kernel<P>, Task, scheduler
│   ├── platform.rs        # The Platform trait
│   └── natives.rs         # Native function registry
├── platforms/
│   ├── std/               # StdPlatform
│   └── embassy/           # EmbassyPlatform
│       ├── cortex-m/
│       ├── stm32/
│       ├── nrf/
│       └── rp/
└── examples/
    └── three-task/
        ├── main.rs        # Generic over Platform
        └── scripts/
            ├── led.kel
            ├── sensor.kel
            └── heartbeat.kel
```

### 9.2 Feature flags

| Feature | Pulls in |
|---------|----------|
| `std` | StdPlatform, tokio runtime |
| `embassy-stm32` | EmbassyPlatform for STM32 |
| `embassy-nrf` | EmbassyPlatform for nRF |
| `embassy-rp` | EmbassyPlatform for RP2040 |
| `defmt` | defmt logging in embassy implementations |

The `keleusma` crate must remain `no_std + alloc` for embassy targets. Any drift in the underlying crate that introduces a `std` dependency is a release blocker for this kernel work.

### 9.3 Build matrix

| Target | Toolchain | Output | CI |
|--------|-----------|--------|----|
| Host (Linux/macOS) | stable | `examples/three-task` binary | always |
| `thumbv7em-none-eabihf` | stable | `examples/three-task` ELF for STM32 | always (cross-compile) |
| QEMU lm3s6965evb | stable | runs the ELF | always |
| Physical STM32F4 dev board | stable | flash and observe | manual |

## 10. Open questions

1. **`no_std + alloc` cleanliness of the existing Keleusma runtime.** The crate-level claim is in the docs. The bundled examples are hosted (SDL3 dependency). The first concrete work is exercising the bare-metal compile path and patching any std drift. Estimated effort to discover: half a day. Estimated effort to fix if any: half a day to one day.

2. **Heap allocator on bare metal.** Keleusma needs `alloc`. Embassy projects typically use `embedded-alloc` with a linker-allocated heap region. The kernel must initialise the allocator before the first VM is built. Documented as a one-line `#[global_allocator]` setup in the embassy example.

3. **Async cancellation semantics.** If a task yields with a `sleep_until` and an event fires that would interest it, does the sleep cancel? Initial answer: no. The task observes the event on its next wakeup. Future iterations can add explicit cancellation.

4. **Static task table vs. dynamic registration.** Initial design assumes the task table is built at kernel-construction time and not modified afterwards. This matches RTOS conventions. Dynamic registration is future work.

5. **Inter-task communication.** Initial design has no shared memory between tasks beyond the platform's event signals. A message-passing primitive (bounded queue per task) is the natural extension and probably belongs in V2.

6. **Failure handling.** If a task's WCET budget is exceeded at runtime (which should be impossible given the verifier, but the platform may have slower-than-expected cycles), what happens? Initial answer: log and continue, on the assumption that the cycle-budget margin is generous. Hard real-time deployments would want a stricter response, configurable.

7. **The kernel's own WCET.** The scheduler is straight-line code, but it iterates the task array. The WCET is `O(N)` in task count. For fixed task counts this is constant. Documented.

## 11. Target hardware: STM32N6570-DK

The available physical target is the STMicroelectronics STM32N6570-DK discovery kit. Key device characteristics relevant to this project.

| Property | Value |
|----------|-------|
| MCU | STM32N657X0 (Cortex-M55 main core plus Cortex-M0+ for low-power, NPU for inference) |
| Architecture | Armv8-M Mainline with TrustZone-M and the M-Profile Vector Extension |
| Clock | Up to 800 MHz on the M55 |
| RAM | 4.2 MB internal SRAM |
| Flash | External octal-flash module on the dev board (no on-chip flash on the N6 family) |
| Debug | ST-LINK V3-EC integrated on the board |
| Power | USB-C power and debug |

The Cortex-M55 is the most capable Cortex-M class available. The kernel design above does not depend on any of its advanced features (MVE, TrustZone-M, NPU); a future iteration could explore using TrustZone-M to harden the kernel/script boundary, but the V1 design stays platform-portable.

Embassy support for the STM32N6 family. `embassy-stm32` covers the STM32 family broadly. The N6 series is recent (2024); at time of writing it is on the active-development path but is not in `embassy-stm32`'s default supported list. The first concrete bare-metal task is to verify which STM32 features the platform implementation can rely on (timer, UART for logging, GPIO, the integrated ST-LINK virtual COM port) and whether the existing embassy support is sufficient or whether the implementation needs a board-specific HAL layer beneath the embassy abstractions.

The N6 is both the development target and the showcase target. The operator does not have a second physical board for proxy validation. The mitigation is to lean harder on the two non-hardware stages of the pipeline.

- **Hosted std target.** The first three or four days of work happen entirely on the developer's machine through the `StdPlatform` implementation. The architecture, the scheduler, the native function surface, and the script-side conventions are all validated here without any hardware in the loop. Roughly 80% of the architectural bugs are findable at this stage.
- **QEMU Cortex-M55.** The `qemu-system-arm -M mps3-an547` machine emulates the Cortex-M55 with the Armv8-M Mainline profile that the N6 uses. This is the right proxy target absent a second physical board. QEMU runs the same ELF that a physical board would, exercises the same `embassy-cortex-m-rt` boot path, and surfaces bare-metal-specific bugs (allocator setup, panic handler, vector table) without touching hardware. Approximately 15% of bugs are findable here. The remaining 5% are board-specific issues that only appear with the N6's actual peripherals, clock tree, and memory map.

The cost of having only the N6 is that the embassy port has to happen against the N6's actual peripheral surface from the start. There is no opportunity to validate against a known-good `embassy-stm32` part first. If `embassy-stm32` does not yet support the N6 family at the level needed, the work expands to include either contributing N6 support back to the embassy project or writing a minimal board-specific HAL against `cortex-m-rt` directly with manual peripheral access.

### 11.1 Firmware backup before hardware work

Before flashing any custom firmware to the STM32N6570-DK, the operator wants a way to recover the factory state. ST ships the board with demonstration firmware showcasing the integrated NPU (the STM32N6 Model Zoo AI inference demo). Operators commonly want this back later for hardware validation, vendor support, or as a sanity check that the board itself works.

**The naive dump approach does not work on this board.** Confirmed empirically with STM32CubeProgrammer v2.22 against the N6570-DK Rev B. The expected commands run cleanly through connect and external-loader upload, but the loader's `Init` step fails with `Data read failed` regardless of BOOT0 position. Debug Authentication discovery (`debugauth=2`) hangs at the "Writing magic number" stage. The chip's security state machine is blocking flash reads from the debugger.

This is the N6's safety-critical-chip behaviour working as ST designed it. The factory firmware is protected and the chip refuses to expose its flash contents to a debugger without credentials we do not have. The same gate protects the NPU model data, which lives in the same external octal flash region.

**The substitute is to download ST's published factory image rather than dump from the board.** ST distributes the demonstration firmware as a re-flashable binary alongside the STM32CubeN6 firmware package.

1. Visit the product page at <https://www.st.com/en/microcontrollers-microprocessors/stm32n6570-dk.html>.
2. In the "Tools & Software" section, download the demonstration firmware. It is typically inside `en.x-cube-ai-n6.zip` or distributed as a standalone `STM32N6570-DK_Demo_FW_Vx.y.z.bin`.
3. Verify the SHA-256 against ST's published hash on the download page.
4. Store the file outside the repository (binary, vendor-distributable, not a source artifact). A safe location is `~/firmware_backups/STM32N6570-DK/factory.bin` or equivalent.
5. The STM32CubeN6 firmware package also ships flash-back scripts under `Projects/STM32N6570-DK/` that drive the CLI with the correct external loader, address mapping, and security-state-aware reset sequence. Use those scripts to restore factory state when needed rather than rolling our own.

The published image is more authoritative than a dump anyway because ST distributes it with a hash and signature.

**For the spec's phase 5 deliverable.** The factory firmware backup task changes from "dump the running firmware to a file" to "download the ST-published image and store its hash". The recovery procedure becomes "run the STM32CubeN6 flash-back script", not "flash back the dump". The end state is the same: the operator has a recovery path if the board needs to return to factory state. The means differ for this particular board.

## 12. Roadmap

The roadmap reflects that the STM32N6570-DK is the only physical target available. The validation pipeline relies on hosted std and QEMU before touching hardware.

| Phase | Effort | Output |
|-------|--------|--------|
| 1. Spec finalisation | 1 day | This document promoted to `docs/architecture/`. |
| 2. Std platform plus three-task demonstrator | 3-5 days | `examples/rtos/` builds and runs on the host. Manual chapter. Cookbook recipe. |
| 3. Bare-metal compile path validation | 1-2 days | `keleusma` crate compiles for `thumbv8m.main-none-eabihf` (the N6's Cortex-M55 target). Std drift patched if found. |
| 4. QEMU Cortex-M55 demonstrator | 3-5 days | The same `examples/rtos/` binary runs under `qemu-system-arm -M mps3-an547`. Bare-metal-specific bugs (allocator, panic handler, vector table) surface here. |
| 5. STM32N6570-DK factory firmware backup | 0.5 day | Download ST's published factory demo binary from the product page; verify hash; store outside the repo. The chip's security gate blocks debugger-driven dumps; the substitute is ST's distribution. See section 11.1. |
| 6. embassy-stm32 N6 support investigation | 2-5 days | Determine whether `embassy-stm32` supports the N6 family at the level needed. If yes, proceed with the standard pattern. If no, either contribute support back or write a minimal board HAL against `cortex-m-rt`. The size of this phase depends on what the investigation finds. |
| 7. Embassy platform implementation for the N6 | 5-10 days | `EmbassyPlatform<Stm32N6>` running on the dev board. Factory firmware re-flashable if anything goes wrong. |
| 8. Manual chapter and cookbook recipe | 1 day | `docs/guide/RTOS.md` published; the cookbook gains a "Total cooperative scheduling" recipe. |

Total to "shippable demonstrator running on the N6": approximately three to four weeks of focused work for one engineer.

Total to "manual and cookbook only, no real example": one week.

Total to "hosted std demonstrator only, no bare-metal": one week, including the manual chapter and cookbook recipe.

### 12.1 Decision points along the way

Two checkpoints during the work will decide whether to continue or adjust scope.

**After phase 4 (QEMU runs).** If QEMU works cleanly, the architecture is validated and the path to N6 is mostly board bring-up. If QEMU exposes architectural issues, fix them before touching hardware. The std-target work in phase 2 catches most architectural bugs, but QEMU catches the bare-metal-specific ones.

**After phase 6 (embassy support investigation).** If `embassy-stm32` does not yet support the N6, the operator should decide whether to invest in either contributing support back to the embassy project (longer, higher value) or writing a minimal board-specific HAL (shorter, lower value, harder to maintain). The decision depends on whether the rest of the operator's roadmap includes more embassy work or whether this is a one-off.

### 12.2 Risk register

Specific risks ordered by likelihood, given the single-hardware constraint.

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `embassy-stm32` does not yet support the N6 at the level needed | Medium-high | Phase 6 explicitly investigates before committing to embassy. Fallback is board HAL via `cortex-m-rt`. |
| QEMU mps3-an547 has divergent behaviour from real N6 hardware | Medium | Validate the same binary on QEMU and on hardware; any differences are debugging signal. |
| `keleusma` runtime has unannounced `std` dependencies | Low-medium | Phase 3 surfaces these early. Patching is small if found. |
| The N6's external octal flash needs board-specific setup before code can run | Medium | The ST-LINK V3-EC provides bootloader access. The factory firmware backup (phase 5) ensures recovery is always possible. |
| Cortex-M55 specific features (MVE, TrustZone-M) cause issues if accidentally relied on | Low | The platform implementation explicitly stays on the M-Mainline subset that is broadly supported. |

## 13. Decision log placeholders

The following decisions are recorded for future reference once the work begins. Each should land as a `docs/decisions/` entry.

- DXX. Cooperative scheduling chosen over pre-emptive scheduling. Rationale: WCET-bound total semantics give the same response-latency guarantee without the kernel-stack and critical-section costs.
- DXX. Platform inversion through a Rust trait rather than `cfg` switches. Rationale: compiler-enforced abstraction boundary; testable in isolation.
- DXX. Embassy chosen as the primary embedded backend. Rationale: async-first model fits cooperative scheduling; broad target support; active ecosystem.
- DXX. Heap allocator required (cannot be eliminated). Rationale: Keleusma's runtime needs `alloc`. Bounded heap consumption is provable through WCMU.

## 14. Relationship to other docs

When this spec is promoted out of `tmp/`, it lands at `docs/architecture/RTOS_MICROKERNEL.md` with cross-links to:

- `docs/guide/COOKBOOK.md` for the "total cooperative scheduling" recipe.
- `docs/guide/RTOS.md` for the user-facing manual.
- `docs/architecture/LANGUAGE_DESIGN.md` for the WCET/WCMU bound origin.
- `docs/architecture/EXECUTION_MODEL.md` for the loop main resume model.
- `docs/decisions/RESOLVED.md` for the architectural decisions listed above.

---

End of working spec.
