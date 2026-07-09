# Chapter 38. Calibrated WCET and Cost Models

## Goal

By the end of this chapter you will understand how the worst-case
execution time is costed, and how a host obtains a figure calibrated to
its hardware.

## The cost model

The WCET analysis of Chapter 20 counts the work on the longest path
between two yields. To turn an opcode count into a cycle figure, it
consults a cost model: a table assigning a pipelined-cycle cost to each
opcode, plus the size of a value slot.

The runtime ships `NOMINAL_COST_MODEL`. Its costs are estimates, not
measurements: one cycle for data movement, two for arithmetic, three for
division, five for composite construction, ten for a function call. The
nominal model is sound for comparing two programs on one platform, but
its numbers are not the cycle counts of any specific processor.

## A measured model

A host that needs WCET figures calibrated to a real deployment processor
uses a measured cost model. The `keleusma-bench` workspace member
benchmarks each opcode on a target and emits a generated
`MEASURED_COST_MODEL` fragment. The host includes the fragment for its
target and passes the model to the `_with_cost_model` variant of the
WCET API:

```rust
include!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/measured_cost_models/aarch64_apple_darwin.rs"));

use keleusma::verify::wcet_stream_iteration_with_cost_model;

let cycles = wcet_stream_iteration_with_cost_model(chunk, &MEASURED_COST_MODEL, &[])?;
```

The `keleusma-bench` crate documents the capture workflow for generating
a fragment for a new target.

## Native function attestation

The WCET and WCMU analyses cost a native function call as zero by
default, because the verifier cannot see inside the host's Rust code. A
host that needs a sound bound declares per-native costs before the
analysis runs:

```rust
vm.set_native_bounds("math::sin", 25, 0)?;
vm.set_native_bounds("text::upper", 100, 256)?;
```

The first number is the worst-case pipelined-cycle cost of the native,
the second is its worst-case arena heap allocation in bytes. These are
the host's promise. The verifier accepts the declared values without
independently measuring them, so the host bears responsibility for their
accuracy, established by measurement or by bounded analysis of the native.

## What the figure means

The bound the analysis produces is in pipelined cycles, a measure of
work. Converting it to wall-clock seconds on a deployment platform
requires a platform-specific factor that accounts for cache and pipeline
stalls and the clock period. The language guarantees the pipelined-cycle
bound. The host attests to the conversion factor for its hardware. This
division of responsibility is the same one Chapter 20 described, seen
here from the host's side.

## What you now know

- A cost model assigns pipelined-cycle costs to opcodes; the analysis
  uses it to produce a WCET figure.
- `NOMINAL_COST_MODEL` is unmeasured and good for relative comparison; a
  `MEASURED_COST_MODEL` from `keleusma-bench` is calibrated to a target.
- `set_native_bounds` attests the cost of a native function so the
  analysis can account for it.
- The bound is in pipelined cycles; converting to wall-clock time is the
  host's responsibility.

The next chapter walks a complete host end to end.
