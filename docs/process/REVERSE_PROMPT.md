# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-06
**Status**: B28 P4 is merged, pushed, pruned. B32 was reverted and marked obsolete. B28 P3 is in progress on `feat-flat-memory-refs` (pushed): opaque (host-reference) fields are now flat-eligible in struct and enum composites, end to end through construction, access, and equality. Three P3 commits have landed green: the registry foundation with drop-at-RESET (`6c3f168`), pointer-identity dedup interning (`f542bc7`), and flat opaque struct/enum fields (`baf711f`).

## Why B32 was reverted

B32 specified a stateful, bounds-checked builder for writing into arena memory incrementally. The actual flat-byte consumer does not write incrementally: `GenericValue::try_pack_flat` assembles the whole body in a `Vec<u8>` with the `byte_size` known up front, and `FlatComposite::in_arena` migrates it in one shot via `alloc_top_bytes` + `copy_nonoverlapping` + `ArenaHandle::from_raw_parts`, exactly as `KString::alloc` does. The existing `alloc_top_bytes` (a writable `NonNull<[u8]>`) plus the epoch-stamped `ArenaHandle` already cover the need. The builder had no consumer, so it was dropped.

## P3 scope and the open design decision

P3 makes reference-typed fields (`Text`, `Opaque`) flat-eligible so a composite holding them uses the flat byte body instead of the boxed `Vec<Value>` fallback. The current state is sound: such composites fall back to boxed, which is correct but heap-resident rather than arena-resident.

The map (from `src/value_layout.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/compiler.rs`):
- `ScalarKind::Text` (reserved `2 * word_bytes`) and `ScalarKind::Opaque` (reserved `word_bytes`) already exist; `read_scalar_le`/`write_scalar_le` panic on them with "handled in B28 P3".
- `flat_scalar_kind` (type side) and `flat_tuple_scalar_kind` (value side) exclude `Text` and `Opaque` today.

**Critical soundness constraint.** A reference field cannot be packed into the flat body by storing its pointer directly. `Value::Opaque` holds `Arc<dyn HostOpaque>`, which is (a) a 16-byte fat pointer (data plus vtable), so it does not fit the reserved `word_bytes` slot, and (b) `Drop`-bearing, so writing its raw pointer into arena bytes that a `RESET` reclaims without running `Drop` would leak or double-free the refcount. `Value::StaticStr` similarly owns a heap `String`, and `KStr`'s `ArenaHandle<str>` is 24 bytes, larger than the reserved 16. The only sound representation is an index handle into a VM-side registry, with the owning `Arc`/`String` held in the registry and the flat body storing a small POD index. This is the B33 mechanism; it is genuinely necessary here, unlike B32. The construction and access paths must intern and resolve through the VM registry, which also means the static `try_pack_flat` choke point cannot pack a reference field without VM cooperation.

This is a multi-day, soundness-critical change touching the `Value` representation, the construction and access handlers, RESET semantics, the yield boundary, and the marshall layer.

## P3 design (settled with the operator)

- A reference field stores a `word_bytes` index into a VM-side registry, not the pointer. `Value::Opaque(Arc<dyn HostOpaque>)` cannot be packed directly (16-byte fat pointer, `Drop`-bearing), and `KString`/`StaticStr` do not fit the reserved slot either; an index does.
- The registry is `ephemeral_opaques: Vec<Arc<dyn HostOpaque>>` on the VM. `RESET` clears it so `Drop` runs and refcounts decrement (the operator's "run `Drop` as part of `RESET`"). The clear lives in the VM (`Op::Reset` and `full_reset_arena_internal`, both `&mut self`), preserving the arena's POD-only contract. A later slice adds a persistent registry for opaques in `private data` that must survive `RESET`.
- The typeless-boundary problem is solved host-side: at a yield or native boundary the VM hands the host flat bytes, and the host's `#[derive(KeleusmaType)]` already knows the layout and decodes them through `from_flat_bytes` (the existing P2 flat-marshalling path, extended to reference fields). The marshalling layer resolves an opaque field's index through the registry as it decodes. No VM-side field-walker and no runtime layout table.
- Construction interns each `Value::Opaque(arc)` into the registry and packs the index word; access reads the index word at the compiler-baked `ScalarKind::Opaque` offset and resolves it back to `Value::Opaque(Arc::clone(...))`.

## What landed and what is scoped out

Opaque is flat in struct and enum fields only. Tuples and arrays keep boxing an opaque element: their access form is recovered by the compiler's lightweight `infer_expr_type`, which cannot recover an opaque element type from a native call, so a value-driven flat tuple would disagree with its boxed-baked access. Structs and enums use the named type for access, which is reliable. Construction interns opaque fields on the flat path; access resolves the index; equality is correct via pointer-identity dedup.

The opaque fallback in `LayoutContext` (post-type-check, a bare unknown `Named` is opaque) excludes the built-in `Option`, which the enum-variant lowering recovers as a bare `Named("Option")` that must stay boxed. The regression this caught (flat `Option` bodies) is fixed and covered by the existing `option_*` tests.

## Boundary analysis (corrected)

Two boundaries I had listed turn out to be moot or non-existent, which significantly narrows the remaining opaque work:

- **Persistent `data`: impossible, not a regression.** The compiler rejects opaque types (and any struct or enum transitively containing one) in data-segment fields (`compiler.rs`, "opaque types are not yet admissible in data segment fields"). An opaque-bearing composite therefore cannot reach a persistent slot, so the ephemeral registry never dangles into persistent state. No persistent registry is needed for opaque.
- **Host decode via derive: never existed.** There is no `KeleusmaType` impl for opaque (`Arc<dyn HostOpaque>` or a host opaque type), so a `#[derive(KeleusmaType)]` type could never have an opaque field, before or after P3. Decoding an opaque field from a flat body host-side is a new feature (opaque marshalling), not a P3 regression.

What remains is the **yield of a whole opaque-bearing composite for manual host inspection**: pre-P3 the host received a boxed struct with `Value::Opaque(Arc)`; post-P3 it receives a flat byte body whose opaque field is a registry index. Resolving it at the yield boundary needs the compile-time type (the yield op does not carry it) or opaque marshalling support. This is the one genuine limitation, and it is the typeless-flat-composite display limitation already documented for scalar fields, now extended to opaque.

## Text representation (operator-decided, blueprint locked)

The opaque half of P3 is complete and verified for struct and enum fields. Text is the next reference kind, and the operator chose a representation distinct from opaque: a flat `Text` field is a **two-word `(data_ptr, len)` reference directly into arena-resident string bytes** — no registry, no index, no extra tracking, because a string is not `Drop`-bearing like an `Arc`. The existing two-word `ScalarKind::Text` slot is exactly right, so no size change. Staleness is covered by the composite's own epoch-checked `resolve`: the string and the composite share the ephemeral arena lifetime, so if the composite resolves current the string is valid, and both are reclaimed together at `RESET`. A dynamic string that outlives its epoch resolves to a clean error, not undefined behaviour, so strings (static always, dynamic while epoch-valid) cross yield safely with no special guard.

Implementation blueprint:

- **`KString`/`ArenaHandle`**: add `raw_parts(&self) -> (usize, usize)` returning `(data_ptr as usize, len)` from the handle's `NonNull<str>` metadata (no arena deref), and rely on the existing `ArenaHandle::from_raw_parts` to rebuild a handle from `(ptr, len, epoch)`.
- **`value_layout`**: `flat_scalar_kind(Text) -> Some(Text)`; size stays two words. Update the `Opaque`-style eligibility test for `Text`.
- **Packer (`bytecode.rs`)**: `flat_tuple_scalar_kind(KStr) -> Some(Text)`; `write_scalar_le(KStr)` writes the two `raw_parts` words; `StaticStr` stays `None` and must be converted to `KStr` first. `read_scalar_le`'s `Text` arm cannot run (no epoch), so the `Text` read happens in the VM (below).
- **VM construct (flat struct/enum path)**: before packing, convert each `StaticStr` to an arena-resident `KStr` via `KString::alloc(arena, s)` (the "heap-allocated string" exception); `KStr` values stay. This sits alongside the existing opaque-to-index substitution. Tuples and arrays keep boxing `Text` (add a `Text` exclusion to the value-driven flat decision, and bake boxed in `tuple_field_access`/`array_elem_operand`), because their access form is recovered by lightweight inference.
- **VM access (`read_flat_scalar`)**: for `ScalarKind::Text`, read `(ptr, len)`, rebuild `KString` via `from_raw_parts` against `arena.epoch()` (valid because the composite just resolved current), and push `Value::KStr`.
- **Equality**: becomes pointer-based for `Text` fields (same arena allocation compares equal), consistent with the existing `KStr` handle equality; document this as the one semantic consequence. `StaticStr` content equality across distinct flat composites does not hold (each gets its own arena copy), which is the deliberate trade for the no-tracking representation.
- **Tests**: a flat struct `Text` field round-trips a static and a dynamic string through construction and access; a `KStr`-backed field crosses a yield and resolves while epoch-valid and errors cleanly after `RESET`.

This is a substantial unsafe slice (raw pointer pack/unpack), so it lands as its own green commit and is not rushed onto the shared branch.

## Other follow-ups

Opaque marshalling (a `KeleusmaType` path for opaque values) and resolving a whole opaque-bearing composite yielded for manual host inspection are a separate later feature, not part of P3's core. P5 (hot-swap migration, documentation, decision closure) remains after the reference fields.
