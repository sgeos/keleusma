# Changelog

All notable changes to `keleusma-bench` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-21

First publicly released line. V0.1.x circulated as a pre-release alongside the parent `keleusma` crate. The crate version is locked one-to-one with the major-minor of `keleusma`.

### Added

- Cost-model calibration tool. Measures pipelined cycles per opcode on a host CPU and emits a `CostModel` source fragment that the Keleusma runtime can `include!` for WCET analysis on that host.
- `CycleCounter` trait abstracting per-architecture cycle counters. Built-in implementations: `Rdtsc` on `x86_64` (Time Stamp Counter), `CntvctEl0` on `aarch64` (generic timer virtual count register), `DwtCycCnt` on Cortex-M with the DWT cycle counter, and a portable `InstantCounter` fallback on platforms without a dedicated cycle counter.
- `cpu_cycles_per_count` and `frequency_hz` methods on `CycleCounter`. Per-counter scaling factor that converts counter ticks to CPU pipeline cycles. The factor is necessary because some counters (`CNTVCT_EL0` on Apple silicon, for example) tick at a fixed system frequency that is decoupled from the CPU clock.
- `BenchConfig` struct configuring measurement parameters. Fields include `iterations`, `warmup_iterations`, `arena_capacity` (for measurements that allocate through the arena), and target-host CPU frequency overrides. The `embedded_default` constructor returns parameters suitable for Cortex-M targets with a small arena budget.
- `keleusma-bench` command-line binary. Supports `--cpu-hz <hz>` to override the assumed CPU frequency, `--output <path>` to specify the emitted Rust source fragment path, and several measurement-tuning flags. The binary is gated behind the `std` feature so the crate remains `no_std + alloc`-compatible when consumed by bare-metal bench harnesses.
- `MEASURED_COST_MODEL` constant in the emitted output. A `CostModel` value populated from the measured per-opcode cycle counts, suitable for direct use through the runtime's `_with_cost_model` verifier API.
- `measured_op_cycles` function in the emitted output. Returns the measured pipelined cycle count for a given `Op` discriminant, used by the emitted `MEASURED_COST_MODEL`.

### Cargo features

- `default = ["std", "floats"]`. Enables the host CLI and pulls the `floats` feature on the keleusma runtime so the bench measures the full opcode surface including float ops.
- `std`. Gates the host-side CLI binary and the environment-variable override for the CPU clock. Without it, the crate is `no_std + alloc`-compatible.
- `floats`. Passthrough to `keleusma/floats`.

### Notes

- The tool does not produce wall-clock-time bounds. Wall-clock time requires the platform-specific calibration factor that maps pipelined cycles to wall-clock seconds. The host establishes the calibration factor through its own deployment validation. See the WCET section of `LANGUAGE_DESIGN.md` for the full unit framing.
- The crate is consumed by the Cortex-M bench binary at `examples/rtos/src/bin/bench_n6.rs` in `no_std + alloc` mode through `default-features = false`. The host CLI re-enables features through the default `std + floats` feature.

### Licensed

- BSD Zero Clause License (`0BSD`).
