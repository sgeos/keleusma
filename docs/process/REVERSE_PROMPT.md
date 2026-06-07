# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-06
**Status**: B28 P4 is merged into `feat-flat-memory-model`, pushed, and the sub-feature branch pruned. The B32 arena bytes-builder was prototyped, judged obsolete, and reverted (its commit was unpushed and is dropped; the `BACKLOG.md` B32 entry carries an obsolete banner with the rationale). A new sub-feature branch `feat-flat-memory-refs` is cut for B28 P3. No P3 code has landed yet; a design decision is open (below).

## Why B32 was reverted

B32 specified a stateful, bounds-checked builder for writing into arena memory incrementally. The actual flat-byte consumer does not write incrementally: `GenericValue::try_pack_flat` assembles the whole body in a `Vec<u8>` with the `byte_size` known up front, and `FlatComposite::in_arena` migrates it in one shot via `alloc_top_bytes` + `copy_nonoverlapping` + `ArenaHandle::from_raw_parts`, exactly as `KString::alloc` does. The existing `alloc_top_bytes` (a writable `NonNull<[u8]>`) plus the epoch-stamped `ArenaHandle` already cover the need. The builder had no consumer, so it was dropped.

## P3 scope and the open design decision

P3 makes reference-typed fields (`Text`, `Opaque`) flat-eligible so a composite holding them uses the flat byte body instead of the boxed `Vec<Value>` fallback. The current state is sound: such composites fall back to boxed, which is correct but heap-resident rather than arena-resident.

The map (from `src/value_layout.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/compiler.rs`):
- `ScalarKind::Text` (reserved `2 * word_bytes`) and `ScalarKind::Opaque` (reserved `word_bytes`) already exist; `read_scalar_le`/`write_scalar_le` panic on them with "handled in B28 P3".
- `flat_scalar_kind` (type side) and `flat_tuple_scalar_kind` (value side) exclude `Text` and `Opaque` today.

**Critical soundness constraint.** A reference field cannot be packed into the flat body by storing its pointer directly. `Value::Opaque` holds `Arc<dyn HostOpaque>`, which is (a) a 16-byte fat pointer (data plus vtable), so it does not fit the reserved `word_bytes` slot, and (b) `Drop`-bearing, so writing its raw pointer into arena bytes that a `RESET` reclaims without running `Drop` would leak or double-free the refcount. `Value::StaticStr` similarly owns a heap `String`, and `KStr`'s `ArenaHandle<str>` is 24 bytes, larger than the reserved 16. The only sound representation is an index handle into a VM-side registry, with the owning `Arc`/`String` held in the registry and the flat body storing a small POD index. This is the B33 mechanism; it is genuinely necessary here, unlike B32. The construction and access paths must intern and resolve through the VM registry, which also means the static `try_pack_flat` choke point cannot pack a reference field without VM cooperation.

This is a multi-day, soundness-critical change touching the `Value` representation, the construction and access handlers, RESET semantics, the yield boundary, and the marshall layer.

## Intended next step

Confirm the P3 approach and scope before building, then implement the minimal sound first slice. The open question is which reference kind to take first and how far to scope the registry (ephemeral-only versus ephemeral plus persistent for `private data` fields).
