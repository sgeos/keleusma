# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T11. P7 fully resolved. Operand stack arena migration and `Value::KStr` via the `ConstValue` and `Value` split.
**Status**: Complete. P7 items 1 through 8 are resolved.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 412 tests pass workspace-wide. 344 keleusma unit, 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration (7 new), 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session completed the deferred P7 items 7 and 8.

Item 7. Operand stack arena migration. The operand stack and call-frame stack are now `allocator_api2::vec::Vec<T, BottomHandle<'arena>>` instead of globally-allocated `Vec<T>`. The arena's bottom region holds the stacks across iterations. The arena's top region holds dynamic strings and other scratch. The `Vm` distinguishes between two reset paths.

- Top-only reset for `Op::Reset`. Invalidates dynamic strings and clears the top region while preserving the operand stack and frames. The new `Arena::reset_top_unchecked` is the underlying primitive. Used between stream iterations where local state must persist.
- Full reset for error recovery and hot swap. Drops the arena-backed stacks, advances the epoch, and clears both ends. The stacks are recreated as zero-capacity instances that allocate fresh on first push. The discipline that no allocator-bound collection holds storage at the moment of bottom-region reset is documented in the safety comment on `full_reset_arena_internal`.

The semantic change is observable. Bottom-region usage no longer drops to zero at `Op::Reset` because the operand stack is bottom-allocated and survives the reset. The existing test that asserted `bottom_used == 0` after `Op::Reset` was updated to assert `top_used == 0` and to observe the epoch advance through a `KString` handle.

Item 8. `Value::KStr` integration. `ConstValue` is the new compile-time-constant type that participates in the rkyv archive. `Value` is the runtime type and adds the `KStr(KString)` variant alongside the existing `DynStr(String)`. The compiler emits `ConstValue` into the constant pool through `ConstValue::try_from_value` at the boundary, which rejects `DynStr` and `KStr` because they cannot be compile-time constants. The runtime lifts archived constants into `Value` through `Value::from_const_archived`, the inverse direction. `Chunk.constants` is now `Vec<ConstValue>`. `value_from_archived` is now a thin wrapper over `Value::from_const_archived` that operates on `ArchivedConstValue`.

The `Value::KStr` variant carries a lifetime-free `KString` (which is `ArenaHandle<str>`). Resolution goes through `Value::as_str_with_arena(&arena)`, returning `Some(&str)` on success or `Err(Stale)` if the arena has been reset since the handle was issued. `Value::contains_dynstr` treats both `DynStr` and `KStr` as dynamic for the cross-yield prohibition. `Value` no longer derives Archive. PartialEq for `Value::KStr` compares captured handles by epoch identity, so two `KStr` values with the same content but different epochs are not equal. Hosts that want content equality go through `as_str_with_arena`.

## Trade-offs and Properties

Bounded memory holds end to end for the operand stack and for dynamic strings allocated through `KString`. The remaining unbounded path is `Value::DynStr(String)`, which uses the global allocator. The `to_string` native still emits `DynStr` because the current native ABI does not thread the arena through. Threading the arena through native invocations would let `to_string` produce `Value::KStr` and is a separate enhancement.

The `KString` equality discipline is identity-based, not content-based. Two `KString` handles compare equal only if they share the same epoch (and, by construction, the same arena allocation). This avoids requiring an arena borrow inside `PartialEq`, which the trait does not allow. The cost is that `Value::KStr` cannot be compared against an unrelated `KString` of the same content without an explicit arena resolution. Test code that needs content equality uses `as_str_with_arena`.

The constant-pool boundary is one-directional. `ConstValue::try_from_value` lowers a `Value` to `ConstValue` if and only if the value is a compile-time constant. The reverse direction `ConstValue::into_value` is total. Runtime values cannot become compile-time constants because that would require serializing handles, which the design refuses for soundness reasons.

The reset discipline is documented and tested. `Arena::reset_top_unchecked` is the primitive for between-iteration resets that preserve bottom-region collections. `Arena::reset_unchecked` clears both ends and is the primitive for full reset. Both advance the epoch. The `Vm` is the sole caller of these unsafe primitives in the runtime.

## Tests

Seven new tests in `tests/kstring_boundary.rs` cover the `Value::KStr` boundary surface.

- `value_kstr_type_name_is_kstr`. Surface check.
- `value_kstr_resolves_through_arena`. Round trip.
- `value_kstr_returns_stale_after_reset`. Stale detection through `as_str_with_arena`.
- `value_kstr_counts_as_dynstr_for_cross_yield_prohibition`. Cross-yield discipline.
- `value_kstr_inside_tuple_is_detected`. Recursive detection.
- `value_kstr_equality_uses_epoch_identity`. The PartialEq contract.
- `value_as_str_returns_none_for_kstr_without_arena`. The non-arena accessor returns None for `KStr`.

The previously failing `vm_arena_reset_at_op_reset` test was rewritten to reflect the new semantics: top-only reset at `Op::Reset` with the stack and frames preserved. The test now allocates a `KString` from the arena, observes the epoch advance, and confirms the handle becomes stale.

## Changes Made

### Source

- **`keleusma-arena/src/lib.rs`**. New `Arena::reset_top_unchecked` method that clears the top region and advances the epoch through a shared reference, leaving the bottom region intact. Documented as the primitive for between-iteration resets where bottom-region allocator-bound collections persist.
- **`src/bytecode.rs`**. New `ConstValue` enum that mirrors `Value` for compile-time constants only. Carries the rkyv `Archive` derive and serializes faithfully. Variants: `Unit, Bool, Int, Float, StaticStr, Tuple, Array, Struct, Enum, None`. `Value` no longer derives `Archive` and gains a new `KStr(KString)` variant. New `Value::from_const_archived` lifts an archived constant into a runtime value. New `Value::as_str_with_arena` resolves any string variant including `KStr` against an arena. `Value::contains_dynstr` extended to recognize `KStr`. `ConstValue::try_from_value` lowers `Value` to `ConstValue` for the constant subset. `ConstValue::into_value` is the inverse total lift. `Value::PartialEq` extended to compare `KStr` by epoch identity.
- **`src/vm.rs`**. `Vm.stack` and `Vm.frames` are now `allocator_api2::vec::Vec<T, BottomHandle<'arena>>` through the new `StackVec` type alias. `reset_arena_internal` is the top-only reset, used by `Op::Reset`. `full_reset_arena_internal` is the new full reset that drops and recreates the stacks before clearing both ends. `chunk_const_str` updated to match `ArchivedConstValue::StaticStr` only. The previous `vm_arena_reset_at_op_reset` test rewritten to assert the top-only semantics through `KString` staleness and epoch advance.
- **`src/compiler.rs`**. `add_constant` now converts `Value` to `ConstValue` at the boundary through `ConstValue::try_from_value`. Runtime-only variants in compile-time positions panic.
- **`src/verify.rs`**. Updated to import `ConstValue`. Pattern matches on the constant pool now match `ConstValue::Int` instead of `Value::Int`. Test fixtures updated.
- **`src/utility_natives.rs`**. `to_string` native gains a `KStr` arm that returns a placeholder string. Threading the arena through the native ABI to resolve `KStr` is a separate enhancement.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**. P7 entry rewritten as resolved across all eight items. R34, R39, and R40 are the design records.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T11.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None for P7. The bounded-memory guarantee holds end to end for the operand stack and for arena-backed dynamic strings.

The remaining engineering improvement is threading the arena through the native ABI so that natives like `to_string` can produce `Value::KStr` instead of `Value::DynStr`. This is a marshall-layer enhancement, not a P7 item.

Resolved priorities to date. P1, P2, P3, P4, P5, P6, P7, P8, P9, P10.

## Intended Next Step

P7 closure complete. Recommend either of two paths.

A. Native ABI extension. Thread the arena through the native invocation context so that natives can produce `Value::KStr`. Updates the `NativeFn` signature, the marshall layer, and `register_native_*` helpers. Delivers full bounded-memory for native-produced strings. Estimated one to two hours.

B. Publish keleusma main crate to crates.io now that P1 through P10 are fully resolved. Cuts a v0.1 release that includes the boundary type and the host-owned arena.

Recommend A if the bounded-memory guarantee for natives is load-bearing for upcoming use cases. Recommend B if external visibility is the priority.

Await human prompt before proceeding.

## Session Context

This long session resolved P7 across all eight items. Earlier in the session the type checker gaps 11 through 14 were closed and an integration test for three-level-nested for-in loops with match expressions was added. The host-owned arena and `KString` boundary type landed first. Then operand stack migration. Then the `ConstValue`/`Value` split that allowed `Value::KStr` to coexist with rkyv. The bounded-memory guarantee now holds end to end for the operand stack and arena-allocated strings.
