# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-08
**Status**: B28 P4 is merged, pushed, pruned. B32 was reverted and marked obsolete. B28 P3 (opaque and Text reference fields flat in struct and enum composites, plus the host-boundary decode of both via `RefContext`/`Vm::decode`) is substantially implemented on `feat-flat-memory-refs`. The operator has now locked the open design questions; this document records those decisions and the implementation work they imply. Remaining B28 work is the P3 follow-ups recorded below plus P5 (hot-swap migration, documentation, decision closure).

## Decisions locked (operator, 2026-06-08)

Three questions were open. The operator resolved all three. The prior multi-option analysis in this file is superseded by these decisions and retained only as history at the end.

### 1. Compiler bakes access type from the type-checked program

There is no need for the compiler to "statically recover" an access type, and no fundamental reason a tuple or array reference element must be boxed. The program is fully type-checked before lowering, so the compiler has every access site's type and should bake the access operand from it. The current value-driven boxing of reference elements in tuples and arrays is an artefact of the compiler's lightweight `infer_expr_type` returning `None` at some sites, not a limit of the model. **Decision:** the compiler tracks whatever type information it needs at lowering and bakes the flat access operand directly, so tuple and array reference elements become flat like struct and enum fields. The value-driven boxing fallback is then removed.

### 2. Compiler bakes enum equality over the used bytes

The compiler knows each variant's used byte count `N` (discriminant plus the active variant's payload). **Decision:** enum equality is compiled to compare exactly the used bytes (field-wise or used-prefix), with `N` baked into the bytecode at compile time and discarded afterward. The runtime then needs no per-variant size table and never reads padding slack, so the slack zero-fill is removed. This replaces the current typeless whole-body byte comparison for enums.

### 3. Text is a two-word in-body handle; the arena supplies the epoch

The flat `Text` field stays a **two-word `(ptr, len)` handle** in the composite body. The epoch is **not** stored in the field and the slot is **not** widened to three words. Instead, anything read out of the arena is wrapped with the epoch, reconstituting the de-facto three-part handle (`KString` = `(ptr, len, epoch)`) that the runtime already uses for a bare dynamic string. **Decision:** no representation change. The only implementation fix is that extraction must reattach the **originating composite's** epoch, not the current arena epoch, so a read after a `RESET` resolves to a clean `Stale` outcome rather than a dangling dereference. A composite that transitively contains `Text` inherits the same string flow restrictions as a bare dynamic string (cross-yield prohibition, data-segment exclusion), enforced by the type checker descending through field and variant-payload types. This decision is recorded in the spec at `docs/spec/TYPE_SYSTEM.md` ("Strings inside composites (B28 P3)").

## Implementation work implied by the decisions

Ordered by tractability and safety, per the operator's "tractability is number one" directive.

1. **Text epoch sourcing (safety). Done.** A flat `Text` field is read by reattaching the **originating composite's** epoch rather than the current arena epoch. `FlatComposite::Inline` now carries that epoch (`Inline { bytes, epoch }`); `to_inline` captures the `Arena` handle's epoch on materialisation, `from_bytes_with_epoch` propagates it to an extracted nested child, and `FlatComposite::ref_epoch` exposes it. `read_flat_scalar` takes a `ref_epoch` argument and `RefContext` carries a `ref_epoch` field; `Vm::decode` sets it from the value's `flat_ref_epoch`, while the native path keeps the current epoch (the argument is read synchronously before any `RESET`). The pinning test `tests/flat_text_stale.rs` yields a flat `Text` struct, resumes to the `RESET`, overwrites the reclaimed region, and asserts the stale decode returns a clean error rather than the overwritten bytes; it failed before the fix (`Ok("XXXX…")`) and passes after. Verified green: default lib (1101) plus the flat/opaque integration tests, clippy with no warnings, and `--all-features` narrow-word lib (1108). No representation change to the two-word `Text` slot.
2. **Transitive cross-yield restriction (compiler). Done.** The operator's rule: static text and any container of it may cross the yield boundary; dynamic text and any container of it may not. A flat (struct/enum) `Text` field is always dynamic (a literal is copied into the arena at construction), and the runtime `contains_dynstr` walk cannot see it inside flat bytes, so the **compiler** rejects yielding any value whose layout transitively contains a flat `Text` field (`layout_has_flat_text` over the yielded type's `LayoutDescriptor`, checked at `Expr::Yield`). A direct `Text` in a tuple, array, or `Option` is boxed and, with a bare `Text`, stays governed by the existing runtime check (static admitted, dynamic rejected); the walk descends through those containers to find a flat-text struct or enum below. `tests/flat_text_yield.rs` covers struct, enum, transitive-nesting rejection and the bare-static-string / no-Text allowances. Consequence: a struct or enum with a `Text` field cannot be yielded even with a literal, because the representation makes that text dynamic; admitting static-text composites would need a rodata-referencing flat `Text` field, recorded as a future enhancement. WCET/WCMU unaffected (a compile-time rejection). Verified green: lib (1101), flat/opaque/marshall integration (3/2/6/27), clippy clean. The old `tests/flat_text_stale.rs` yield-staleness test is removed because that path is now a compile error; item 1's epoch reattachment remains as defense in depth for the synchronous native and return reads.
3. **Compiler-baked access from type-checked types (point 1).** Bake the flat access operand for tuple and array reference elements from the type-checked type and remove the value-driven boxing fallback.
4. **Field-wise enum equality (point 2).** Bake used-byte enum equality and remove the slack zero-fill.
5. **Arena-allocator for residual storage.** Any composite storage still on the global heap (`FlatComposite::Inline`, the boxed bodies) is a WCMU-bound conformance gap, since bounded worst-case memory usage is the value proposition. `keleusma-arena` already exposes `BottomHandle`/`TopHandle` as `allocator_api2::Allocator` impls, so `allocator_api2::vec::Vec<T, TopHandle>` is the path to move that storage into the arena. This is the eventual closure, not urgent relative to the safety items.

## State of the implemented P3 surface

- Opaque is flat in struct and enum fields (construct interns into `ephemeral_opaques`, deduped by `Arc::ptr_eq`; access resolves the index; equality is pointer-identity; the registry is cleared at `RESET` so `Drop` runs). Tuples and arrays box opaque elements today; item 3 above flattens them.
- Text is flat in struct and enum fields (construct copies a `StaticStr` into the arena and packs `(ptr, len)`; access rebuilds a `KStr`). Narrow-word builds keep `Text` boxed because the field stores a host pointer. The epoch-sourcing fix (item 1) and the transitive flow restriction (item 2) are the corrections this turn records.
- Host-boundary decode of both is implemented across struct, enum, nested container, and native-argument paths via a `RefContext` threaded through `from_value_ctx`/`from_flat_bytes_ctx` and the `Vm::decode` helper. `impl KeleusmaType for String` copies the string out to an owned `String`; `impl KeleusmaType for Arc<dyn HostOpaque>` resolves the index through `ctx.opaques`.
- The host → script return path (`into_value` for a reference-bearing composite) routes through `struct_with_widths` with no arena, so a host-built composite comes back boxed rather than flat. Making it flat requires an `into_value_ctx(self, ctx)` mirroring `from_value_ctx`, threading the arena the native wrapper already holds. Tracked as part of the access/representation cleanup.

## History (superseded analysis)

The earlier entries below proposed a three-word `(ptr, len, epoch)` slot and weighed it against a one-word registry index. Both are superseded by decision 3: the two-word handle stays and the arena supplies the epoch. The boundary analysis remains accurate: persistent `data` storage of a reference-bearing composite is compiler-rejected (so the ephemeral registry never dangles into persistent state), and there was no pre-existing host decode of `Text`/opaque to regress. B32 was reverted because the flat-byte consumer assembles a whole body with a known `byte_size` and migrates it in one shot via `alloc_top_bytes`, so the incremental bounds-checked builder had no consumer.
