# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.0-M6 substantial completion. WCMU instrumentation and verification.
**Status**: Substantial. WCMU analysis and module-load verification in place. Auto-arena sizing and call-graph integration deferred.

## Verification

**Command**: `cargo test && cargo clippy --tests --all-targets -- -D warnings && cargo fmt --check`
**Result**: 294 tests pass (277 unit + 17 integration), up from 286. Zero clippy warnings. Format clean. Eight new tests cover simple WCMU computation, branching, NewStruct heap, NewArray heap, non-stream rejection, resource bounds pass, oversized rejection, and skip-non-stream.

## Summary

Implemented the fifth Keleusma guarantee (R31). Added per-instruction memory cost methods on `Op`, namely `stack_growth`, `stack_shrink`, and `heap_alloc`. Added `wcmu_stream_iteration` in `src/verify.rs` paralleling the existing WCET analysis. Added `verify_resource_bounds` that checks the WCMU sum against the arena capacity. Wired the verification into `Vm::new`, `Vm::new_with_arena_capacity`, and `Vm::replace_module`. Widened the native function attestation API with WCET and WCMU bound fields and a `Vm::set_native_bounds` setter. Documented the implementation as R35 and updated the priority list, knowledge graph, and instruction set reference.

## Changes Made

### Source Code

- **src/bytecode.rs**: Added `VALUE_SLOT_SIZE_BYTES` constant (32 bytes on the modern 64-bit target). Added `Op::stack_growth()`, `Op::stack_shrink()`, and `Op::heap_alloc(chunk)` methods returning slot counts and bytes respectively.
- **src/verify.rs**: Added `McuResult` internal struct tracking peak stack, end-of-region stack delta, and heap total. Added `wcmu_region` and `wcmu_subregion` walking the block-structured CFG with appropriate aggregation rules. Added `wcmu_stream_iteration(chunk)` returning `(stack_wcmu_bytes, heap_wcmu_bytes)`. Added `verify_resource_bounds(module, arena_capacity)` checking the WCMU sum against the configured arena. Eight new tests.
- **src/vm.rs**: Added `wcet` and `wcmu_bytes` fields to `NativeEntry` initialized to defaults (`DEFAULT_NATIVE_WCET = 10`, `DEFAULT_NATIVE_WCMU_BYTES = 0`). Added `Vm::set_native_bounds(name, wcet, wcmu)` setter. Wired `verify_resource_bounds` into `Vm::new_with_arena_capacity` and `Vm::replace_module`.

### Knowledge Graph

- **docs/decisions/RESOLVED.md**: Added R35 recording the WCMU implementation, the per-instruction methods, the aggregation rules, the host attestation widening, and the module-load enforcement. Documented current limitations (single-iteration loop, no transitive call analysis).
- **docs/decisions/PRIORITY.md**: Updated P8 to reflect substantial completion. Auto-arena sizing and call-graph WCMU integration remain as follow-on items.
- **docs/architecture/EXECUTION_MODEL.md**: Updated WCMU paragraph to reference `wcmu_stream_iteration` and `verify_resource_bounds` and to note the limitations.
- **docs/reference/GLOSSARY.md**: Updated WCMU entry. Added `Op::stack_growth, Op::stack_shrink, Op::heap_alloc` entry. Added Native attestation entry. Added `verify_resource_bounds` entry.
- **docs/reference/INSTRUCTION_SET.md**: Added WCMU Cost Tables section with stack growth, stack shrink, and heap allocation per instruction.
- **docs/process/TASKLOG.md**: V0.0-M6 substantially complete. Active milestone none, ready for V0.0-M7 or V0.1.

## Unaddressed Concerns

1. **Auto-arena sizing is not implemented.** The host configures arena capacity at `Vm::new_with_arena_capacity`. A future iteration could compute the WCMU sum at module load and size the arena automatically. This is one of the remaining P8 items and is well-scoped follow-on work.

2. **Call-graph WCMU integration is not implemented.** The current analysis treats `Call` and `CallNative` instructions as locally consuming their argument slots and producing one return value, but does not include the transitive stack and heap effects of the called function. This is sound for programs without function calls but is an underestimate for programs that do call helper functions or natives. The WCET analysis has the same limitation. Both warrant a coordinated improvement that walks the call graph bottom-up.

3. **Variable-iteration loops are treated as one iteration.** The Keleusma surface language requires bounded for-range loops, but the bytecode does not encode the iteration count visibly. A pass that analyzes the loop structure to extract the iteration bound is needed for sound WCMU and WCET in the presence of loops. This too is shared with the existing WCET limitation.

4. **The default `VALUE_SLOT_SIZE_BYTES` of 32 may be over-conservative.** The actual `core::mem::size_of::<Value>()` on the modern 64-bit target is implementation-dependent. The choice of 32 ensures soundness even if the runtime representation grows, but produces tighter bounds when it underestimates. A future iteration could derive the constant from `size_of` directly with a const assertion.

5. **The deeper arena integration of operand stack and DynStr remains open.** Tracked as P7 follow-on. The WCMU analysis already accounts for both regions on the assumption that they will be arena-resident. Once the integration is done, the analysis becomes load-bearing for runtime soundness rather than an aspirational bound.

## Intended Next Step

Three paths forward.

A. V0.0-M7 implementing P8 follow-on items, namely auto-arena sizing and call-graph WCMU integration. Both are well-scoped extensions of the existing analysis.

B. V0.0-M7 implementing P7 follow-on items, namely operand stack and DynStr arena migration. Substantial refactor due to lifetime parameter cascade through `Vm` and `Value`.

C. Pivot to V0.1 candidates. Type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

Path A delivers more on the certification posture in less time. Path B closes the gap between the documented arena lifetime and the runtime mechanism. Path C opens new feature areas for the language layer.

Await human prompt before proceeding.

## Session Context

This long session has executed across V0.0-M3 (data segment), V0.0-M4 (static marshalling), V0.0-M5 (two-string-type discipline), and V0.0-M6 (arena allocator and WCMU instrumentation). Ten commits have accumulated. The five guarantees of Keleusma now have concrete implementation backing for totality, productivity, bounded-step, bounded-memory, and safe swapping, with the limitations on the static analyses noted above. Additional work was requested on positioning the Keleusma arena allocator as a general-purpose embedded arena differentiated from bumpalo. That positioning will be addressed in the next response.
