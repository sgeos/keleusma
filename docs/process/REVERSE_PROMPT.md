# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T31. B11 per-op decode optimization.
**Status**: Complete. Hot dispatch loop now reads decoded ops from a per-chunk cache populated once at construction.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 497 tests pass workspace-wide. 429 keleusma unit, 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The hot dispatch loop previously called `op_from_archived(&chunk.ops[ip])` per instruction fetch. That function performed a discriminant match across the archived `Op` form and produced a small payload copy. For hot loops the cost compounds with the iteration count.

The VM now caches a per-chunk `Vec<Op>` populated at construction. The hot path reads through `decoded_ops[chunk_idx][ip]`, which is a constant-time slice index. The previous per-fetch decoding is performed once at construction and at every `replace_module`.

Implementation:

- New `decoded_ops: Vec<Vec<Op>>` field on `Vm` indexed as `decoded_ops[chunk_idx][ip]`.
- New `decode_all_ops` helper walks the archived module's chunks and decodes every op into the cache.
- `Vm::construct` (owned bytecode path) populates the cache via `decode_all_ops`.
- `Vm::view_bytes_zero_copy` (borrowed bytecode path) populates the cache inline because its archived view comes from a borrowed slice rather than a copied `AlignedVec`.
- `Vm::replace_module` re-decodes for the new module after the bytecode swap.
- `Vm::chunk_op` simplified to a direct slice index. The archived form is no longer consulted on the hot path.

Trade-offs.

- Cost: one heap allocation per chunk at construction, proportional to the program's total op count. The `Op` type is `Copy` (a small enum with payload), so the per-op storage is small.
- Zero-copy contract: constants and string data continue to be read on demand from the archived form. Only the op slice is materialized eagerly. The wire-format zero-copy benefit for constant pools, string data, and metadata is preserved.
- One-shot scripts: cost is roughly equal to the previous per-fetch decoding (the same number of decodings happen, just amortized differently).
- Hot-loop scripts: per-iteration savings compound with iteration count.

Option B from the original B11 entry (specialized dispatch tables for a small set of hot opcodes) was not pursued. The simpler cache approach removes the per-fetch decode cost without the codegen complexity, and benchmark-driven workload analysis would be needed to identify which opcodes are hot enough to merit specialization. Option A is the conservative win.

## Tests

No new tests added. The existing 429 keleusma unit tests cover the whole execution loop across many opcodes, which collectively verify the decode cache is correct. A failure in `op_from_archived` would surface as an op-mismatch in any test that exercises the relevant opcode; the test surface is broad enough that adding a dedicated decode-cache test would be redundant.

## Trade-offs and Properties

The decoded cache is owned by the VM and lives for the VM's lifetime. Memory cost is bounded by the program's total op count and is fixed at construction or hot-swap time; there is no growth at runtime. The cache survives `Op::Reset` since the bytecode does not change at reset; only the operand stack and arena top region are cleared.

The borrowed-bytecode zero-copy constructor `Vm::view_bytes_zero_copy` was previously the path that minimized memory overhead at construction. It now allocates the decoded cache, increasing its memory footprint. The trade-off is justified: the zero-copy constructor is intended for hot-loop dispatch where the per-iteration savings dominate the per-construction cost. A future refinement could expose a constructor flag to opt out of the cache for hosts that genuinely need minimum-construction-overhead, but no such use case has been observed.

The cache is reconstructed at `replace_module`. The cost is the same as construction. Hot swap is rare relative to dispatch, so this is negligible.

## Changes Made

### Source

- **`src/vm.rs`**. New `Vm::decoded_ops` field. New module-level `decode_all_ops` helper. `Vm::construct`, `Vm::view_bytes_zero_copy`, and `Vm::replace_module` populate the cache. `Vm::chunk_op` reads from the cache instead of decoding on demand.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B11 marked resolved with the option-A implementation documented.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T31.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The named B11 work is closed. The remaining open BACKLOG items are smaller refinements or items deferred earlier in V0.1-M3:

- Recursion-depth attestation API for recursive closures (a refinement over the safe-constructor rejection of `Op::MakeRecursiveClosure`).
- `Op::CallIndirect` flow analysis to tighten WCET bounds for non-recursive indirect dispatch.
- Removing the `Type::Unknown` sentinel from B1 (requires declaring native function signatures).
- f-string finer-grained span attribution.
- Block expressions as primary parsing form.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session optimized the hot dispatch loop by caching decoded ops at VM construction. The change is structurally simple, preserves the zero-copy contract for constant data, and removes a per-fetch overhead that compounded across hot loops. No tests were added because the existing 429 unit tests cover the dispatch loop comprehensively.
