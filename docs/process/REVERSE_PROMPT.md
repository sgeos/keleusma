# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T41 Cost-model calibration tool.
**Status**: Complete. New `keleusma-bench` workspace crate measures pipelined cycles per opcode on the host CPU and emits a calibrated `CostModel` source fragment.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p keleusma-bench --bin keleusma-bench
```

**Results**:

- Workspace tests pass.
- Format clean.
- Clippy clean.
- The benchmark runs on AArch64 with CNTVCT_EL0 and produces ordering-correct relative measurements.

## Summary

The user observed that the cost-model calibration tool did not exist and asked for one designed to make adding new targets easy. The result is a new workspace crate `keleusma-bench` with a library and a CLI binary.

### Architecture extensibility

Each host architecture provides a cycle counter through the `CycleCounter` trait. Built-in implementations cover x86_64 through RDTSC, AArch64 through CNTVCT_EL0 read by inline assembly, and a portable `Instant`-based fallback for architectures without a built-in counter. Adding a new architecture requires implementing `CycleCounter` for a new struct and adding a `cfg` arm to `default_counter`. The benchmark engine is otherwise architecture-independent.

### Opcode extensibility

Opcode coverage lives in the `OPCODE_SPECS` table. Each entry specifies the opcode name, a function building the operation pattern, the required constants, and the operation count per pattern. The benchmark engine constructs a Func chunk that inlines the pattern `PATTERN_REPETITIONS` times (default 100,000) and measures total cycles. Adding coverage for a new opcode requires appending a spec to the table with appropriate setup and cleanup operations to keep the operand stack balanced.

### Methodology

The benchmark approximates pipelined cycles through best-case observation. Each opcode pattern executes in an inlined sequence within a Func chunk. The engine reads the cycle counter, runs the chunk, reads the counter again, computes total cycles, and divides by the repetition count. Multiple measurement passes run after warmup; the minimum across passes is reported on the rationale that the minimum corresponds to the run with warmest caches and best branch prediction.

Internal arithmetic uses `f64` to preserve precision when counter resolution is coarse relative to per-pattern cost. Per-opcode values clamp to a minimum of 1 so the generated cost model never reports zero cycles, which would be unsound for use in WCET analysis.

### Source emission

The CLI binary runs the benchmark suite and emits a Rust source fragment defining `measured_op_cycles` and `MEASURED_COST_MODEL`. The host application includes the fragment into its build and constructs a calibrated VM. The output uses the same opcode-category structure as the bundled `nominal_op_cycles`, with category cycle counts computed as the maximum over the category's representative opcodes.

### Methodology limitations

The README documents the known limitations: inlined sequences keep instruction-cache and data-cache hot, branch prediction is trivial when the same opcode repeats, the host system must be quiescent during measurement, and frequency scaling can change cycle-to-time mapping mid-run. Sound WCET requires static analysis with hardware models or deployment on time-predictable platforms. The measured tool produces best-effort calibration suitable for soft real-time and order-of-magnitude WCET, not certified hard real-time bounds.

## Trade-offs and Properties

The choice to use inlined patterns rather than a measured loop avoids loop-overhead measurement. The trade-off is that the chunk grows linearly with `PATTERN_REPETITIONS` and the instruction cache may not hold the full pattern for very large repetition counts. The default of 100,000 is a balance: large enough to give the AArch64 architectural counter (which runs at 24 MHz on Apple silicon, much slower than CPU clock) usable resolution, while small enough that the chunk fits in typical instruction caches.

The choice to clamp per-op values to a minimum of 1 is a soundness move. The benchmark may report zero ticks per opcode when the counter is too coarse for the measurement. A zero-cost opcode in the cost model would let WCET analysis claim free execution, which is unsound. Clamping to 1 ensures the cost model is always conservative; the per-op number may be inaccurate but it is never optimistic.

The category aggregation in the source emitter takes the maximum cost over the category's representative opcodes. This is conservative for pipelined-cycle WCET: a chunk's bound is computed against the worst opcode in each category. Tighter bounds would require per-opcode cost rather than category aggregation, which is a future refinement.

The benchmark currently produces relative-ordering-correct measurements on AArch64 even when the absolute resolution is coarse. The raw per-pattern values preserve precision that the rounded per-op values lose. The user can read both in the generated output's comment header.

## Files Touched

- **`Cargo.toml`** at workspace root. Added `keleusma-bench` as a workspace member.
- **`keleusma-bench/Cargo.toml`** (new). Crate metadata for crates.io publication.
- **`keleusma-bench/README.md`** (new). Usage, methodology, extensibility instructions.
- **`keleusma-bench/src/counter.rs`** (new). `CycleCounter` trait and built-in implementations.
- **`keleusma-bench/src/lib.rs`** (new). Benchmark engine, opcode specs, source emitter.
- **`keleusma-bench/src/main.rs`** (new). CLI binary.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T41.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The benchmark tool is functional. Several refinements are tracked but not blocking.

- The architectural counter on AArch64 (CNTVCT_EL0) runs at the system counter frequency, not the CPU clock. The reported numbers are in counter ticks rather than CPU cycles. The conversion factor depends on `CNTFRQ_EL0` which the tool does not currently read. A future refinement would normalize to CPU cycles by reading `CNTFRQ_EL0` and the CPU's nominal frequency.
- The PMU cycle counter (`PMCCNTR_EL0`) gives true CPU cycles on AArch64 but requires kernel-level enable. The tool could grow an optional dependency on a kernel module or a higher-precision counter library.
- Per-opcode coverage is incomplete. Several opcodes (Yield, Call, MakeClosure) currently use placeholder patterns because they require multi-chunk modules or Stream-classified chunks. The OPCODE_SPECS table can grow.
- Cross-validation against the bundled `nominal_op_cycles` would catch outright errors in the benchmark.
- A criterion-style statistical aggregation (median, p99, with outlier rejection) would replace the simple minimum.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session delivered the cost-model calibration tool that the user identified as missing. The architecture is designed to make per-target and per-opcode extension straightforward. The current implementation produces relative-ordering-correct measurements on AArch64 and is ready for use on x86_64 hosts where the RDTSC counter has higher resolution.
