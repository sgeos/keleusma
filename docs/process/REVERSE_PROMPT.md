# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T12. Native ABI arena context, float width log2, B9 documentation, BACKLOG hygiene.
**Status**: Complete. All requested follow-on items addressed.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 415 tests pass workspace-wide. 347 keleusma unit (3 new for arena-aware natives), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session addressed all engineering gaps adjacent to recent P7 work and all future-work items that interact with P10, except the `keleusma-arena` registry version which the user deferred. Document hygiene also landed.

Native ABI extension. The new `NativeCtx<'a>` type carries a borrow of the host-owned arena. The `Vm` constructs a fresh `NativeCtx` at every `Op::CallNative` dispatch and passes it to the native. The native function type is now `for<'a> Fn(&NativeCtx<'a>, &[Value]) -> Result<Value, VmError>`. The legacy registration paths (`register_native`, `register_native_closure`, `register_fn`, `register_fn_fallible`) are unchanged in their public signature; they wrap the supplied function so it ignores the context. The new `register_native_with_ctx` and `register_native_with_ctx_closure` variants pass the context through.

Arena-aware utility natives. The `register_utility_natives_with_ctx` companion to `register_utility_natives` registers `to_string` and `length` through the new ABI. `to_string` allocates a `KString` from the arena and returns `Value::KStr` for the bounded-memory path. `length` resolves `Value::KStr` arguments through the arena before counting characters. The non-arena `length` errors on `Value::KStr` with a clear message pointing the caller to the ctx-aware registration. The legacy `register_utility_natives` remains for hosts that prefer `Value::DynStr` outputs through the global allocator.

Float width log2 in the wire format. The bytecode header gains a `float_bits_log2` byte at offset 12 (previously reserved). The `Module` struct gains a `float_bits_log2: u8` field. The runtime defines `RUNTIME_FLOAT_BITS_LOG2 = 6` for f64. The verifier admits bytecode whose `float_bits_log2` is at most the runtime's, paralleling the existing word and address size discipline. A new `LoadError::FloatSizeMismatch` variant surfaces a width mismatch with a precise error message. Bytecode version bumped from 4 to 5. The golden bytes test updated. Future portability work tracked under B10 will use this field to gate narrower or wider float support.

B9 hot update of yielded static strings. Resolved structurally and documented. `Value::from_const_archived` materializes archived `StaticStr` constants into owned `String` values at the moment they are pushed onto the operand stack. Yielded values that contain a `Value::StaticStr` therefore hold owned heap data that is independent of the bytecode buffer. A hot update through `Vm::replace_module` does not affect the host's retained yield value because the string bytes were already copied out at the lift boundary. The BACKLOG entry is now marked resolved.

Per-op decode optimization. Recorded as backlog item B11. The current zero-copy execution path reads each instruction through `op_from_archived(&chunk.ops[ip])`, performing a discriminant match per fetch. Two candidate optimizations are documented (cached `Vec<Op>` per chunk, specialized dispatch table for hot opcodes). Deferred until profiling identifies the dispatch as a hot path on real workloads.

BACKLOG hygiene. The duplicate `B5` heading was renamed to `B5b. Static string discipline extensions` to remove the conflict with the resolved structural-verification entry. Three stale `Open` rows in TASKLOG were marked complete with their resolving session IDs.

## Tests

Three new unit tests in `src/utility_natives.rs` cover the arena-aware natives.

- `to_string_with_ctx_int_returns_kstr`. Confirms `to_string(42)` returns `Value::KStr` with the expected resolved contents.
- `to_string_with_ctx_string_returns_kstr`. Confirms string-typed input flows through the boundary.
- `length_with_ctx_string_counts_chars`. Confirms `length` resolves `KStr` arguments through the arena.

The existing 412 tests continue to pass after the wire format bump and the marshall layer update.

## Changes Made

### Source

- **`keleusma-arena/src/lib.rs`**. No new public surface this session; the prior `reset_top_unchecked` carries the arena-side semantic for `Op::Reset`.
- **`src/bytecode.rs`**. `Module` gains `float_bits_log2: u8`. `BYTECODE_VERSION` bumped to 5. New `RUNTIME_FLOAT_BITS_LOG2` constant. New `LoadError::FloatSizeMismatch` variant with display formatting. `to_bytes` writes the new byte; `from_bytes` and `access_bytes` validate it. Header documentation updated.
- **`src/vm.rs`**. New `NativeCtx<'a>` public type with a single field `arena: &'a Arena`. Native function type updated to `for<'a> Fn(&NativeCtx<'a>, &[Value]) -> Result<Value, VmError>`. New `register_native_with_ctx` and `register_native_with_ctx_closure` methods. The dispatch in `Op::CallNative` constructs a `NativeCtx` per call. The legacy `register_native`, `register_native_closure`, `register_fn`, and `register_fn_fallible` wrap the supplied function. Golden bytes test updated for the new wire format.
- **`src/marshall.rs`**. `BoxedNativeFn` retyped to accept `&NativeCtx<'_>`. The `IntoNativeFn` and `IntoFallibleNativeFn` macros wrap the inner Rust function with a closure that ignores the context. Test helpers gain a small `ctx` builder.
- **`src/utility_natives.rs`**. Refactored `native_to_string` and `native_length` to share a `render_value_to_string` helper that optionally takes an arena reference for `Value::KStr` resolution. New `native_to_string_with_ctx` and `native_length_with_ctx` use the ctx-aware ABI. New `register_utility_natives_with_ctx` registers the arena-aware variants. Three new tests cover the arena-aware path.
- **`src/lib.rs`**. Re-exports `NativeCtx` from `vm`.
- **`src/compiler.rs`**, **`src/verify.rs`**, **`src/vm.rs`** test fixtures. `Module` literals gain `float_bits_log2: RUNTIME_FLOAT_BITS_LOG2`.
- **`examples/zero_copy_demo.kel.bin`**. Regenerated against `BYTECODE_VERSION = 5`.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**. P7 entry extended with item 9 covering the native ABI extension. The closing paragraph updated to record the bounded-memory end-to-end status.
- **`docs/decisions/BACKLOG.md`**. Duplicate `B5` heading renamed to `B5b. Static string discipline extensions`. `B9` marked resolved structurally. New `B11. Per-op decode optimization for zero-copy execution` records the deferred dispatch optimization.
- **`docs/process/TASKLOG.md`**. Three stale `Open` rows in V0.0-M6 marked complete with their resolving session IDs. New row for V0.1-M3-T12.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Trade-offs and Properties

The native ABI extension is backwards compatible. Existing `register_native`, `register_fn`, and `register_fn_fallible` callers compile and run unchanged. Hosts that want bounded-memory dynamic strings opt into the new ABI through `register_native_with_ctx` or `register_utility_natives_with_ctx`. The cost is a small wrapping closure for legacy registrations and a per-dispatch `NativeCtx` construction (one pointer copy).

The float width field is forward-looking. The runtime currently rejects narrower or wider floats because no masking is implemented. When narrower-float support lands, the verifier will admit narrower bytecode and the runtime will mask through a sign-extending cast similar to the integer path. The wire format change is a one-time bump from version 4 to 5.

The `Value::KStr` discipline propagates correctly through the cross-yield prohibition. `Value::contains_dynstr` returns true for both `DynStr` and `KStr`, so attempts to yield a value that contains a `KStr` fail at the yield boundary as they do for `DynStr`. This preserves the soundness contract from R31.

The B9 resolution is structural rather than design-level. The resolution is tied to the `Value::from_const_archived` lift that always copies into owned `String`. Future zero-copy yield paths that retain `&ArchivedString` references in `Value` would re-introduce the concern; the BACKLOG entry documents the alternative host-responsibility model.

## Remaining Open Priorities

None. P1 through P10 fully resolved.

The remaining open engineering item the user explicitly deferred is the `keleusma-arena` registry version. The registry currently hosts v0.1.0; the local crate has new APIs (`epoch`, `ArenaHandle`, `KString`, `reset_top_unchecked`, `force_reset_epoch`) that need a v0.2 publication before the main `keleusma` crate can be published to crates.io with a registry-only dependency. This is documentation and release process work, not a design or implementation gap.

## Intended Next Step

The user has not specified the next step. Two reasonable directions.

A. Publish keleusma-arena v0.2 and then keleusma v0.1 to crates.io. Cuts an external release that includes the boundary type, host-owned arena, and arena-aware native ABI.

B. Continue language work toward V0.2 milestones once those are scoped.

Await human prompt before proceeding.

## Session Context

This long session resolved P7 across all nine items including the native ABI extension that closes the bounded-memory loop end to end for native-produced dynamic strings. The wire format gained a `float_bits_log2` byte for future portability. B9 was resolved structurally and B11 was opened to track per-op decode optimization. BACKLOG hygiene removed the duplicate B5. TASKLOG was synced with three previously stale rows now marked complete.
