# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.0-M3 completion. Hot swap API and host interoperability layer.
**Status**: Complete

## Verification

**Command**: `cargo test && cargo clippy --tests -- -D warnings && cargo fmt --check`
**Result**: 238 tests pass. Zero clippy warnings. Format clean. Six new hot swap tests cover same-schema preserved, new-schema replaced, size mismatch rejected, no-data module, swap at reset starts new module, and rollback to prior version.

## Summary

Completed the remaining V0.0-M3 items by adding the host interoperability layer for the data segment, the hot swap API, and end-to-end integration tests. Documented the concurrency contract that follows from Rust borrow semantics. The slot-based `Vec<Value>` interface was chosen over a `repr(C)` struct mapping to avoid unsafe code and to keep the runtime consistent with the rest of the VM. Schema mismatch detection at swap time is by size check plus host attestation. Hash comparison and structural type checking against a schema descriptor were deferred to a later phase.

## Changes Made

### Source Code

- **src/vm.rs**: Added `replace_module(new_module, initial_data)` performing verification, size validation, and atomic replacement of the module and data segment with frame and stack reset. Added `data_len()` accessor for hosts that need the slot count without inspecting the module. Added six integration tests covering same-schema preserved, new-schema replaced, size mismatch rejected, no-data module, swap at reset starts new module, and rollback to prior version.

### Knowledge Graph

- **docs/architecture/EXECUTION_MODEL.md**: Added Host Interoperability Layer subsection describing the slot-based `Vec<Value>` interface and the Vm public API. Added concurrency note that Rust borrow semantics enforce single ownership.
- **docs/architecture/COMPILATION_PIPELINE.md**: Updated Typical Host Usage to show data segment initialization and the hot swap pattern.
- **docs/reference/GLOSSARY.md**: Added `replace_module` entry.
- **docs/decisions/RESOLVED.md**: Added R29 recording the slot-based interoperability decision and the size-check-plus-attestation schema mismatch policy.
- **docs/decisions/PRIORITY.md**: P6 marked resolved as R29.
- **docs/process/TASKLOG.md**: V0.0-M3 milestone marked complete. Active milestone is none, ready for V0.1 planning.

## Unaddressed Concerns

1. **Schema hash and structural type checking are deferred.** R29 selects the simplest schema mismatch detection mechanism, namely host attestation plus size check. Stronger mechanisms remain open. Tool qualification under any safety standard would require at least structural type checking against a schema descriptor recorded in the module. This is a candidate item for V0.1 if certification work is pursued.
2. **Dialogue type compatibility across swaps is not checked.** Dialogue types are erased at the bytecode level. The VM cannot detect a swap that violates the invariant. The host bears this responsibility. Future work could record dialogue type metadata in the module and check it at `replace_module` time.
3. **Data segment values are `Value` enums, not packed memory.** The `.data` analogy is conceptual. The runtime backing is a `Vec<Value>` rather than a contiguous memory region matching a Rust struct layout. For hosts that need true `repr(C)` interop, a marshalling step is required. Adding direct memory interop is a candidate item for a later phase, conditional on the certification path.
4. **Native function access to the data segment is indirect.** Native functions cannot directly read or write the data segment. The script must read a value via `GetData`, pass it to the native, receive a result, and write back via `SetData`. This is acceptable for primitive fields. Composite field access through native functions requires the same marshalling discipline as host integration.
5. **The hot swap mechanism has no formal correctness proof.** Tool qualification would require a formal specification of the swap semantics and a machine-checked proof that the swap preserves the four guarantees. This is consistent with the certification gap analysis in RELATED_WORK Section 7.

## Intended Next Step

V0.0 is complete. The natural next milestone is V0.1 planning. Candidate items include the following.

A. Type checker implementation (P1). Currently the compiler produces bytecode without type checking or name resolution validation.

B. For-in over arbitrary expressions (P2). Range and array iteration are supported. Iteration over other expressions is specified but not implemented.

C. Error recovery model (P3). The current implementation halts on error. A recovery model would allow the host to resume execution at a defined point.

D. Schema descriptor metadata for stronger schema mismatch detection (deferred from R29).

E. Soundness proof for the structural verifier or for the hot swap mechanism. This is a substantial research undertaking.

Recommend selecting one of A, B, or C for V0.1, deferring D and E to a later phase. Type checker (A) provides the largest immediate quality improvement and is the standard expectation for a production scripting language.

Await human prompt before proceeding.

## Session Context

The data segment feasibility analysis stalled in a prior session. The current session resumed and completed it. The full sequence in this session was as follows.

1. Surveyed the project state and identified the in-progress data segment work.
2. Resolved four originally ambiguous specification points and the singular block decision through clarifying dialog.
3. Conducted the research and documentation formalization pass adding RELATED_WORK Section 8 and adopting the four-section memory model analogy.
4. Brought the source code into conformance with the specification through the singular block check, fixed-size field type validation, and verifier bounds check.
5. Added the host interoperability layer and hot swap API, completing V0.0-M3.

Two commits were made for items 3 and 4. A third commit will follow for item 5.
