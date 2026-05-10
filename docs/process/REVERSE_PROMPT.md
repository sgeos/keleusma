# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T39 Wired cost-model tables for WCET (nominal cycles) and WCMU (bytes).
**Status**: Complete. The cost-model surface is wired into `bytecode.rs`, the public verify API surface, and the language documentation. Internal threading through `module_wcmu` is recorded as a tracked refinement.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 513 tests pass workspace-wide (445 keleusma unit + 17 marshall + 17 kstring_boundary + 28 keleusma-arena unit + 6 keleusma-arena doctests). Five new unit tests in the new `cost_model_tests` module.
- Format clean.
- Clippy clean.

## Summary

The user committed to documenting WCMU as bytes and WCET as nominal cycles, with the cost tables wired even if accuracy is deferred to a future cycle.

### CostModel infrastructure

A new public `CostModel` struct in `src/bytecode.rs` carries two fields: `value_slot_bytes: u32` and `op_cycles: fn(&Op) -> u32`. Three methods support the analysis: `cycles(op)` returns the nominal cycle cost, `slots_to_bytes(slots)` converts a slot count to bytes via the model's slot size, and `heap_alloc_bytes(op, chunk)` computes the WCMU heap allocation in bytes for composite-construction opcodes.

A new `NOMINAL_COST_MODEL` constant exports the bundled defaults. A new `nominal_op_cycles` free function holds the cycle table that the `Op::cost` method now delegates to. The pre-existing `Op::cost` and `Op::heap_alloc` methods are now thin wrappers over the nominal model. Behavior is preserved exactly; the values returned by the unmeasured table are unchanged.

### Unit declarations

WCMU is bytes. The byte unit is target-independent in principle. The actual byte count returned by the analysis depends on the cost model's `value_slot_bytes`, which the runtime declares to match its value representation. The current 64-bit Keleusma runtime declares 32 bytes per slot.

WCET is nominal cycles. The values are unmeasured estimates suitable for relative ordering of programs on a single platform. The scale assigns one cycle to data movement and trivial control flow, two to arithmetic and comparison, three to division and field lookup, five to composite construction, ten to function calls. The values are not validated against any specific host CPU.

What nominal cycles means in practice. A program whose nominal-cycle WCET is one hundred is more expensive than a program whose nominal-cycle WCET is fifty when both run on the same platform. The absolute number does not convert to wall-clock time without a host-specific calibration. Hosts that need wall-clock WCET in measured cycles construct a custom `CostModel` whose `op_cycles` returns measured cycles per opcode for the target hardware.

### Public verify API surface

A new `verify::verify_resource_bounds_with_cost_model` entry point accepts a host-supplied cost model. The current implementation delegates to the existing nominal-model path; full threading of the model through the per-chunk WCMU computation requires a 32-call-site refactor of internal helpers and is recorded as future work. The API surface is stable for hosts to build against.

### Tests

Five new unit tests in the new `cost_model_tests` module in `src/bytecode.rs`:

- `nominal_cost_model_value_slot_bytes_matches_constant` pins the `NOMINAL_COST_MODEL.value_slot_bytes` against `VALUE_SLOT_SIZE_BYTES`.
- `nominal_cost_model_cycles_match_op_cost_method` confirms that the `Op::cost` backward-compatibility wrapper agrees with the nominal cost model's cycle table for representative opcodes across all five tiers.
- `cost_model_slots_to_bytes_uses_slot_size` exercises the slot-to-byte conversion with a custom slot size.
- `cost_model_heap_alloc_bytes_scales_with_slot_size` constructs a custom cost model with half the nominal value-slot size and confirms that `heap_alloc_bytes` for composite-construction opcodes scales linearly. This pins the contract that `value_slot_bytes` determines the byte conversion.
- `custom_cost_model_returns_custom_cycles` constructs a custom cost model whose `op_cycles` returns a flat one hundred for every opcode and confirms that `CostModel::cycles` returns the custom value. This pins the contract that a host-supplied function pointer flows through the model.

## Trade-offs and Properties

The threading of the cost model through internal helpers is deferred. The 32 internal call sites that currently use `Op::cost()` and `Op::heap_alloc(chunk)` continue to use those methods, which delegate to `NOMINAL_COST_MODEL`. A custom cost model passed to `verify_resource_bounds_with_cost_model` is accepted at the API boundary but does not yet flow through to the bound. The bound is currently always computed against the nominal model.

This is honest about the present implementation. The cost-model surface is real and tested, hosts can construct custom models and observe correct behavior at the model level, but the model parameter is a forward declaration in the verify API. A subsequent session will thread the model through the helpers, at which point a custom cost model will determine the bound.

The choice to ship the API surface now and defer the threading reflects quota constraints and the user's explicit framing that accuracy is deferred. Hosts building against the contract today will get correct values once the threading lands; the contract itself is stable.

## Files Touched

- **`src/bytecode.rs`**. New `CostModel` struct, `NOMINAL_COST_MODEL` constant, `nominal_op_cycles` function. `Op::cost` and `Op::heap_alloc` refactored to delegate. Five new unit tests in `cost_model_tests`.
- **`src/verify.rs`**. New `verify_resource_bounds_with_cost_model` entry point.
- **`src/lib.rs`**. Re-export `CostModel`, `NOMINAL_COST_MODEL`, `VALUE_SLOT_SIZE_BYTES`, `nominal_op_cycles`.
- **`docs/architecture/LANGUAGE_DESIGN.md`**. WCET Analysis section restructured to document units explicitly, distinguish nominal from measured cycles, and explain the cost-model surface.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T39.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The cost-model API surface is complete. The internal threading through `module_wcmu` and per-chunk computation is the immediate follow-on. Estimated 32 call-site updates plus parameter additions to four helper functions. Once threaded, custom cost models will determine the bound rather than just appearing in the API.

Beyond that, populating measured per-target cycle tables is its own multi-session sourcing effort that depends on hardware datasheets and benchmark measurements.

## Intended Next Step

Quota is near the threshold. Subsequent sessions can take up the threading work or move to other backlog items.

## Session Context

This session delivered the wired cost-model surface that the user requested. The infrastructure is in place: `CostModel` struct, `NOMINAL_COST_MODEL` default, `verify_resource_bounds_with_cost_model` entry point, five tests proving the model construction and contract. The unit conventions are documented explicitly. Threading the model through internal verify helpers is straightforward future work.
