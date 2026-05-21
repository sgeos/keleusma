# keleusma-bench

Cost-model calibration tool for Keleusma. Measures pipelined cycles per opcode on a host CPU and emits a `CostModel` implementation that the Keleusma runtime can use for WCET analysis on that host.

## Purpose

Keleusma reports WCET in pipelined cycles per the unit conventions documented in `docs/architecture/LANGUAGE_DESIGN.md`. The bundled `NOMINAL_COST_MODEL` carries unmeasured estimates suitable for relative ordering of programs. A deployment that needs accurate per-host pipelined-cycle bounds runs `keleusma-bench` on the target hardware and replaces the nominal model with the measured output.

The tool does not produce wall-clock-time bounds. Wall-clock time requires the platform-specific calibration factor that maps pipelined cycles to actual cycles to wall-clock seconds. The host establishes the calibration factor through its own deployment validation. See the WCET section of `LANGUAGE_DESIGN.md` for the full unit framing.

## Usage

```sh
cargo run --release --bin keleusma-bench -- --output measured_cost_model.rs
```

The output file is a Rust source fragment that the host can `include!` into its build, exposing a `measured_op_cycles` function and a `MEASURED_COST_MODEL` constant. Replace the nominal model in your VM construction:

```rust
use keleusma::{CostModel, VALUE_SLOT_SIZE_BYTES};

include!("measured_cost_model.rs");

let model = CostModel {
    value_slot_bytes: VALUE_SLOT_SIZE_BYTES,
    op_cycles: measured_op_cycles,
};
```

## Pre-generated fragments

This crate also carries pre-generated cost-model fragments for common host architectures under [`measured_cost_models/`](./measured_cost_models/). Hosts running on those architectures can `include!` the committed fragment without regenerating. See [`measured_cost_models/README.md`](./measured_cost_models/README.md) for the list and usage details.

## Methodology notes

The benchmark constructs Func chunks whose opcode stream inlines the target pattern many times and times them end-to-end. The reported `cycles_per_op` is `ceil(cycles_per_pattern)`, not `ceil(cycles_per_pattern / ops_per_pattern)`. Dividing by the pattern's op count would distribute the measured cost across all ops in the pattern and collapse below-counter-tick fractional values to one cycle uniformly, losing the relative ordering between opcode categories. Using the per-pattern value directly preserves the ordering at the cost of overstating per-op cost in absolute terms; for WCET this is conservative.

The per-pattern value is *scaled to CPU cycles* before being reported. Raw counter deltas are in counter ticks, which differs from CPU cycles on counters that run below CPU clock speed (e.g., AArch64 CNTVCT_EL0 at 24 MHz on Apple Silicon, where one tick is approximately 134 CPU cycles on a 3.228 GHz core). The scaling depends on the counter's `cpu_cycles_per_count` method, which uses an assumed CPU clock. The assumption is documented in `src/counter.rs` (`DEFAULT_ASSUMED_CPU_HZ`) and is overridable per-host via the `KELEUSMA_BENCH_CPU_HZ` environment variable. Without this scaling, the reported "cycles" would be counter ticks, not CPU cycles, and the model would be off by approximately two orders of magnitude on Apple Silicon.

Categories whose opcodes cannot be exercised in isolation by this bench (`Yield` requires a Stream chunk; `Call` requires a multi-chunk module) fall back to a *scaled* nominal value: `nominal_op_cycles(op) * max_measured_to_nominal_ratio`, where the ratio is the maximum across measured categories. This keeps the units consistent across the emitted model (all entries are CPU cycles) and is conservative for the unmeasured categories. Without this scaling the fallback would mix nominal relative-weight units with measured CPU-cycle units, which would be incoherent.

## Adding a New Target Architecture

Each host architecture provides its own cycle counter. The `CycleCounter` trait abstracts the read primitive. Built-in implementations cover x86_64 (RDTSC), AArch64 (CNTVCT_EL0), Cortex-M (DWT_CYCCNT under no_std), and a portable `Instant`-based fallback under `std`. To add a new architecture:

1. Implement `CycleCounter` for the new architecture in `src/counter.rs`. The implementation reads the architecture's cycle-counter register and returns a `u64`. Implement `cpu_cycles_per_count` to convert from the counter's tick rate to CPU cycles, and `frequency_hz` to report the counter's nominal tick rate.
2. Add a `cfg` gate to `default_counter` selecting the new implementation when compiled for that target. For embedded targets where the counter requires runtime parameters (such as a CPU clock supplied by the bootloader), the embedded binary may construct the counter directly rather than going through `default_counter`.
3. Run the existing benchmark suite on the new architecture and verify the output is reasonable.

The benchmark engine and the source emitter are architecture-independent. Only the counter primitive needs per-architecture work.

## Embedded targets and the N6-DK

The crate is `no_std + alloc`-compatible with `default-features = false`. The Cortex-M DWT_CYCCNT counter is available under `target_arch = "arm"`. An embedded bench binary lives at `examples/rtos/src/bin/bench_n6.rs` and demonstrates the pattern: boot embassy, enable DWT_CYCCNT, run [`BenchConfig::embedded_default`] (1,000 repetitions to fit in device RAM), emit each `Measurement` via defmt RTT, halt. The host-side runner captures the defmt log and feeds it to `keleusma-bench --from-log` to produce the fragment.

See [`measured_cost_models/README.md`](./measured_cost_models/README.md) for the full N6 capture workflow.

## Adding a New Opcode

When the `Op` enum gains a new variant, the benchmark suite must learn how to exercise that opcode in isolation. Add an entry to the `OPCODE_SPECS` table in `src/lib.rs` with:

- The `Op` variant constructor.
- A setup pattern that prepares the operand stack with appropriate operands.
- A cleanup pattern that consumes the opcode's outputs to keep the stack balanced.
- Any constants the opcode references through the constant pool.

The benchmark engine uses the spec to construct a chunk with N inlined copies of the pattern, divides observed cycles by N, and reports the per-opcode pipelined-cycle cost.

## Methodology

The benchmark approximates pipelined-cycle cost through best-case observation. Each opcode is exercised N times in an inlined sequence. The total cycle count is read with a high-resolution architectural counter (RDTSC on x86_64, CNTVCT_EL0 on AArch64). The minimum cycle count over multiple runs is the reported value, on the rationale that the minimum corresponds to the run with warmest caches and best branch prediction, which is the closest observable approximation of pipelined cycles.

The methodology has known limitations.

- Inlined sequences keep instruction-cache and data-cache hot. Realistic workloads may have more cache pressure, which the measurement underestimates.
- Branch prediction is trivial when the same opcode repeats. Realistic dispatch loops have varying opcodes, which the measurement underestimates.
- The host system must be quiescent during measurement. Background processes can perturb readings.
- Frequency scaling can change cycle-to-time mapping mid-run. Disable frequency scaling or pin the benchmark to a fixed frequency for repeatable results.

These limitations are inherent to benchmark-based measurement. Sound WCET requires static analysis with hardware models (aiT, Bound-T) or deployment on time-predictable platforms (JOP, ARM Cortex-R with timing analysis). The measured tool produces best-effort calibration suitable for soft real-time and order-of-magnitude WCET, not certified hard real-time bounds.

## License

0BSD. Same as Keleusma.
