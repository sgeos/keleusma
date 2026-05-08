# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.0-M6 completion. Arena extracted to standalone keleusma-arena crate.
**Status**: Complete. Auto-arena sizing and call-graph WCMU integration tracked as P8 follow-on for V0.0-M7.

## Verification

**Command**: `cargo test --workspace && cargo clippy --workspace --tests --all-targets -- -D warnings && cargo fmt --check`
**Result**: 300 tests pass across the workspace (266 keleusma unit + 17 keleusma integration + 17 keleusma-arena). Zero clippy warnings. Format clean.

## Summary

Extracted the dual-end bump-allocated arena into a standalone workspace crate named `keleusma-arena`. The crate is positioned as a general-purpose embedded arena allocator with the tagline "Simple and boring memory allocator for exciting applications." The differentiation from `bumpalo` rests on fixed-size storage, fail-fast allocation, dual-end discipline, generic `Budget` contract, and `core`-only operation without `alloc`. The keleusma runtime continues to use the arena through a thin dependency, with backwards-compatible aliases preserving the old `StackHandle` and `HeapHandle` names at the runtime crate root.

## Changes Made

### New Crate

- **keleusma-arena/Cargo.toml**: Workspace member. `default-features = ["alloc"]`. `alloc` feature enables `Arena::with_capacity` and the corresponding `allocator-api2` collection types. Disable for `core`-only targets.
- **keleusma-arena/src/lib.rs**: New crate root. `Arena` type with three constructors. `BottomHandle` and `TopHandle` with `allocator_api2::Allocator` impls. `Budget` and `fits_budget` for generic budget contract. `BottomMark` and `TopMark` for LIFO discipline. Unsafe `rewind_bottom`, `rewind_top`, `reset_bottom`, `reset_top`. Peak watermark tracking with `bottom_peak`, `top_peak`, `clear_peaks`. Seventeen unit tests covering all of the above.
- **keleusma-arena/README.md**: Standalone documentation including quick start, static-buffer use, collection integration, budget contract, mark and rewind, observability, comparison with `bumpalo`, features, and crate family.

### Workspace

- **Cargo.toml**: Added `keleusma-arena` as workspace member.

### Keleusma Runtime

- **src/arena.rs**: Removed.
- **src/lib.rs**: Removed `pub mod arena`. Added `keleusma-arena` re-exports for `Arena`, `BottomHandle`, `Budget`, `TopHandle`. Added backwards-compatible aliases `StackHandle = BottomHandle` and `HeapHandle = TopHandle` at the crate root.
- **src/vm.rs**: Renamed `crate::arena::Arena` to `keleusma_arena::Arena`. Renamed `stack_used`, `heap_used`, `stack_handle`, `heap_handle` to `bottom_used`, `top_used`, `bottom_handle`, `top_handle` to match the new arena names.
- **src/verify.rs**: Added `budget_for_stream(chunk)` adapter that produces a `keleusma_arena::Budget` from the WCMU analysis. Updated `verify_resource_bounds` to use the arena's `fits_budget` for the admissibility check. Error message updated to reference "WCMU budget" rather than "WCMU bound."
- **Cargo.toml**: Added `keleusma-arena` path dependency with `alloc` feature.

### Knowledge Graph

- **docs/decisions/RESOLVED.md**: Added R36 recording the extraction, the API changes, the new surface, the generic budget contract design, and the tagline.
- **docs/reference/GLOSSARY.md**: Updated Stack/HeapHandle entry to BottomHandle/TopHandle. Added Budget and BottomMark/TopMark entries.
- **docs/process/TASKLOG.md**: V0.0-M6 marked complete. History updated with the extraction.
- **CLAUDE.md**: Updated repository structure to include `keleusma-arena/`. Updated technology stack and test count to 300.

## Unaddressed Concerns

1. **Auto-arena sizing remains pending.** P8 follow-on. The host configures arena capacity manually. Future iteration can compute the WCMU sum at module load and size the arena automatically.

2. **Call-graph WCMU integration remains pending.** P8 follow-on. The current analysis treats `Call` and `CallNative` instructions as locally consuming their argument slots without including transitive contributions. Variable-iteration loops are treated as single iteration. Both warrant a coordinated improvement.

3. **The standalone arena crate does not yet have published versions.** This is a local workspace member. Publishing to crates.io requires a separate decision and release process. The crate metadata is ready for publishing when desired.

4. **`from_buffer_unchecked` accepts arbitrary lifetimes through unsafe code.** The user is responsible for ensuring the buffer outlives the arena. A typed-lifetime variant `Arena<'a>` would express this in the type system but cascades through downstream code. The unsafe escape hatch is the V1 compromise. A typed-lifetime variant is admissible as a future addition if demand emerges.

5. **Operand stack and DynStr arena migration in keleusma runtime remains pending.** P7 follow-on. The new arena crate is now ready to support this work. The runtime continues to use the global allocator for operand stack and `String`-backed `DynStr`. Migrating these to use `BottomHandle` and `TopHandle` is iterative refactor work tracked separately.

## Intended Next Step

Three paths.

A. V0.0-M7 implementing P7 follow-on items, namely operand stack and DynStr arena migration in the keleusma runtime. The new arena crate provides the substrate.

B. V0.0-M7 implementing P8 follow-on items, namely auto-arena sizing and call-graph WCMU integration.

C. Pivot to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

Recommend A because it tightens the relationship between the documented design and the runtime implementation. Path B is also defensible if the certification posture is the priority.

Await human prompt before proceeding.

## Session Context

The session has executed across V0.0-M3 (data segment), V0.0-M4 (static marshalling), V0.0-M5 (two-string-type discipline), and V0.0-M6 (arena allocator, WCMU instrumentation, and arena crate extraction). The arena work concludes with a standalone reusable crate that sits at the foundation of the runtime. Twelve commits have accumulated on this branch. The five guarantees of Keleusma now have concrete implementation backing for totality, productivity, bounded-step, bounded-memory, and safe swapping, with the analysis limitations noted above.
