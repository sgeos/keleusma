# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T40 Document WCET in pipelined cycles.
**Status**: Complete. Documentation-only change. The unit terminology shifts from "nominal cycles" to "pipelined cycles" with explicit definition, caveats for actual cycles and wall-clock time, and the calibration-factor framing for practical deployment.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 513 tests pass workspace-wide. No code changes.
- Format clean.
- Clippy clean.

## Summary

The user committed to documenting WCET in pipelined cycles, with explicit caveats for actual cycles and wall-clock time, and the framing that the language proves an order-of-magnitude-correct bound that hosts convert to deployed WCET through a platform-specific scalar.

### Pipelined cycles

A pipelined cycle is a CPU cycle in which the host's pipeline operates at steady-state throughput. The cycle assumes warm instruction and data caches, correctly predicted branches, and no contention on the memory bus from other cores or peripheral DMA. The pipelined-cycle metric is what CPU optimization tables, including Agner Fog's instruction tables and the Intel Optimization Reference Manual, call "throughput" or "reciprocal throughput" per instruction. The metric is observable, reproducible, and measurable through standard benchmarking with warm caches and a stable predictor.

### Industry terminology adopted

The user's prose used "constant multiplier" for the platform-specific scalar that converts the pipelined-cycle bound to deployed wall-clock WCET. The WCET literature term is **calibration factor** or equivalently **dilation factor**. Both terms are industry-recognized. The documentation uses calibration factor as primary with dilation factor noted as a synonym.

The user's prose used "order of magnitude" for the precision claim. This is acceptable as written, but the documentation pairs it with the more specific framing that the bound is sound for the abstract pipelined-cycle metric and that the conversion to wall-clock time involves a deployment-validated scalar.

The Java Optimized Processor [WC5] is referenced as the canonical example of a time-predictable platform where the calibration factor approaches unity by hardware design.

### Documentation structure

`LANGUAGE_DESIGN.md` WCET section restructured with five subsections:

1. **Units.** Defines pipelined cycles and bytes with the warm-cache, predicted-branch, contention-free assumption set.
2. **What the language guarantees.** The verifier proves a definitive pipelined-cycle bound. The bound is sound for the abstract metric. The language does not guarantee wall-clock time or actual cycles; both gaps are the host's responsibility to characterize.
3. **Caveats for actual cycles.** Stalls from cache misses, mispredictions, and contention. Typically within a small constant factor for quiescent deployments, larger and more variable for contended ones.
4. **Caveats for wall-clock time.** Clock period and frequency scaling. Time-predictable platforms reduce the gap toward unity.
5. **Bounded order-of-magnitude WCET.** The calibration-factor framing. For many practical applications including audio engines, game scripts, and embedded controllers, the pipelined-cycle bound multiplied by a measured calibration factor is sufficient.

### Source-level updates

`bytecode.rs` documentation for `CostModel`, `NOMINAL_COST_MODEL`, `nominal_op_cycles`, and `Op::cost` all updated to use pipelined-cycle terminology consistently. The bundled-values caveat retained: the values are unmeasured estimates suitable for relative ordering on a single platform; measured pipelined-cycle tables are the deployment-validation upgrade path.

`README.md` WCET feature bullet expanded to mention pipelined cycles, the calibration factor, and the cross-reference to the canonical WCET section.

## Trade-offs and Properties

The shift from "nominal cycles" to "pipelined cycles" is a pure terminology improvement. The numeric values in the bundled cost model are unchanged. The implementation behavior is unchanged. What changes is what the documentation tells readers about what the analysis actually delivers.

"Nominal" was technically accurate but read as aspirational; readers would reasonably ask what the values would be if validated. "Pipelined cycles" is precise about what the metric is, points at industry-recognized definitions, and is honest about what is and is not validated. A reader who has done CPU optimization recognizes the term immediately and understands the assumption set.

The calibration-factor framing matches how real-time systems engineers actually deploy software with WCET concerns. The language proves the abstract bound. The host validates the calibration factor for its specific platform and deployment configuration. The product of the two is the wall-clock WCET, with the host attesting to the soundness of the calibration factor through its own validation process. This division of responsibility is the right place to draw the abstraction boundary because the calibration factor depends on host platform, host operating environment, and host certification process, none of which the language can determine unilaterally.

## Files Touched

- **`docs/architecture/LANGUAGE_DESIGN.md`**. WCET section restructured with five subsections covering units, guarantees, caveats for actual cycles, caveats for wall-clock time, and the bounded order-of-magnitude framing.
- **`src/bytecode.rs`**. `CostModel`, `NOMINAL_COST_MODEL`, `nominal_op_cycles`, and `Op::cost` documentation updated to use pipelined-cycle terminology.
- **`README.md`**. WCMU and WCET feature bullet expanded.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T40.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The unit conventions are now documented precisely. The internal threading of `CostModel` through `module_wcmu` remains the immediate follow-on. A measured-cycle benchmarking tool that emits a calibrated `op_cycles` function would be the next step beyond that.

## Intended Next Step

Quota is at the threshold. Subsequent sessions can take up internal cost-model threading, the measured-cycle benchmarking tool, or other backlog items.

## Session Context

This session improved the documentation of the language's WCET unit. The conceptual contract was already correct; the terminology now matches industry conventions and makes the calibration-factor approach to deployment explicit. Hosts reading the documentation now have a precise framing for what the language guarantees, what it does not, and how to convert the analyzed bound to deployed wall-clock WCET.
