# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.0-M6 partial completion. Arena allocator foundation.
**Status**: Partial. Arena infrastructure in place. Operand stack and dynamic string arena migration tracked as P7 follow-on.

## Verification

**Command**: `cargo test && cargo clippy --tests --all-targets -- -D warnings && cargo fmt --check`
**Result**: 286 tests pass (269 unit + 17 integration), up from 272. Zero clippy warnings. Format clean. Eleven new arena unit tests cover initial state, stack and heap allocation, alignment, exhaustion, stack-heap meet detection, reset, integration with `allocator_api2::vec::Vec`, dual-region usage, zero-capacity, and zero-size layout. Three new VM tests cover default arena capacity, configurable capacity, and arena reset at `Op::Reset`.

## Summary

Implemented the arena allocator foundation specified in R32. Added `allocator-api2` 0.4 as a stable polyfill dependency. Created `src/arena.rs` with the `Arena` type owning a fixed-size `Box<[u8]>` backing buffer and tracking two bump pointers via `Cell<usize>`. Defined `StackHandle` and `HeapHandle` types implementing `allocator_api2::Allocator`. Wired the arena into `Vm` with a configurable default capacity of 65536 bytes. The arena is reset at `Op::Reset` and at `replace_module`. Added R34 recording the implementation. Updated `EXECUTION_MODEL.md`, `GLOSSARY.md`, `PRIORITY.md`, `RESOLVED.md`, `TASKLOG.md`, and `CLAUDE.md`.

## Changes Made

### Source Code

- **Cargo.toml**: Added `allocator-api2 = { version = "0.4", default-features = false, features = ["alloc"] }`. The crate is no_std-compatible and serves as a stable polyfill of the unstable `core::alloc::Allocator` trait.
- **src/arena.rs**: New module. Contains the `Arena` type with `Box<[u8]>` backing buffer and two `Cell<usize>` bump pointers. Stack region grows from offset zero. Heap region grows down from buffer length. Alignment-aware allocation. Reset method. Eleven unit tests.
- **src/lib.rs**: Added `pub mod arena` and re-exports `Arena`, `StackHandle`, `HeapHandle` at the crate root.
- **src/vm.rs**: Added `arena` field to `Vm` struct. Added `DEFAULT_ARENA_CAPACITY` constant of 65536 bytes. Added `Vm::new_with_arena_capacity` constructor for host-configurable arena size. Added `arena()` and `arena_mut()` accessors. The `Op::Reset` handler now calls `arena.reset()`. The `replace_module` method also resets the arena. Three new tests verify the integration.

### Knowledge Graph

- **docs/decisions/RESOLVED.md**: Added R34 recording the arena allocator implementation, including the rationale for the two-handle design and the deferral of the deeper integration work.
- **docs/decisions/PRIORITY.md**: Updated P7 to mark the foundation as complete and to enumerate the remaining work, namely the operand stack and dynamic string arena migration.
- **docs/architecture/EXECUTION_MODEL.md**: Added an Arena Implementation subsection describing the concrete `Arena` type and its handles.
- **docs/reference/GLOSSARY.md**: Updated the Dual-end arena entry. Added a StackHandle / HeapHandle entry.
- **docs/process/TASKLOG.md**: V0.0-M6 partial completion recorded with task breakdown.
- **CLAUDE.md**: Updated repository structure to include `src/arena.rs`. Updated technology stack to mention `allocator-api2`. Test count updated to 286.

## Unaddressed Concerns

1. **Deeper arena integration is not yet done.** The operand stack continues to use `alloc::Vec<Value>`, namely the global allocator. The dynamic string storage `Value::DynStr(String)` continues to use `alloc::String`. The arena exists and is reset on schedule, but its principal use today is host-supplied native functions that wish to allocate scratch buffers. Migrating the operand stack and dynamic string storage to use the arena requires propagating an arena lifetime parameter through the `Vm` struct and through `Value`. This is a substantial refactor and is tracked as P7 follow-on work. The visible behavior of the runtime is unchanged because Rust drop semantics continue to enforce the arena lifetime.

2. **WCMU instrumentation remains pending.** The fifth guarantee specified in R31 is documented but not enforced. The host-attestation surface for native functions does not yet include WCMU declarations. Tracked as P8.

3. **Arena exhaustion produces an `AllocError` from the `allocator_api2::Allocator` trait.** The Keleusma VM does not yet have a bytecode-level path that surfaces this error to the host. When the operand stack and dynamic string storage are migrated to the arena, a new `VmError::ArenaExhausted` variant will need to be added and the runtime allocation sites will need to map `AllocError` to it. Currently, only host code calling the arena handles directly observes the error.

4. **Default arena capacity is hard-coded to 65536 bytes.** The host can override via `Vm::new_with_arena_capacity`. The auto-arena-sizing path described in R31 and P8 is not yet implemented because it depends on WCMU computation.

## Intended Next Step

Two paths forward.

A. Continue with V0.0-M6 by tackling the operand stack and dynamic string arena migration (P7 follow-on items 5 and 6). This delivers the full arena story for the runtime, with the operand stack as `allocator_api2::vec::Vec<Value, StackHandle>` and dynamic strings as a custom `DynStr` storage backed by `allocator_api2::vec::Vec<u8, HeapHandle>`. Substantial refactor due to lifetime parameter cascade.

B. Pivot to WCMU instrumentation (P8) before the deeper arena integration. The WCMU analysis is independent of the storage representation and parallels the existing WCET analysis closely. Adding `Op::memory()` and `wcmu_stream_iteration()` is well-scoped work.

Recommend B because it closes the fifth-guarantee gap with smaller blast radius. Path A then becomes the V0.0-M7 milestone.

Await human prompt before proceeding.

## Session Context

This session executed across multiple milestones. V0.0-M3 completed the data segment with hot swap. V0.0-M4 added static marshalling. V0.0-M5 partial introduced the two-string-type discipline. V0.0-M6 partial added the arena allocator foundation. Eight commits accumulated during the session, all on main.
