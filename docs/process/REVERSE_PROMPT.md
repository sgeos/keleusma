# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-23
**Status**: B28 P1 complete. Compile-time layout pass landed in new `src/layout_pass.rs` module. `LayoutContext::layout_for` recursively walks AST type expressions and produces `LayoutDescriptor` byte-layout descriptors. `ScalarKind` gained `Text` and `Opaque` variants. 930 lib tests passing (up from 907). All workspace test suites green across default, default-minus-floats, and all-features matrices. Clippy and rustfmt clean. No pipeline integration yet; P2 wires the pass into op emission.

## Summary of work since the last reverse-prompt update

### `src/layout_pass.rs`

`LayoutContext<'a>` borrows the struct and enum tables for the duration of layout computation. Constructor takes `word_bytes` and `float_bytes` matching the target descriptor.

`LayoutContext::layout_for(&TypeExpr) -> Result<LayoutDescriptor, LayoutError>` recursively computes layouts:

- `Unit` → `Scalar(Unit)`
- `Prim(PrimType, _)` → `Scalar(kind_for_prim)` where `Byte → Byte`, `Word → Int`, `Fixed(_) → Fixed`, `Float → Float`, `Bool → Bool`, `Text → Text`.
- `Tuple(elems, _)` → `Tuple(layout_for each elem)`
- `Array(elem, count, _)` → `Array { element, count as usize }` with `InvalidArraySize` if count is negative
- `Option(inner, _)` → `Enum { type_name: "Option", variants: [(None, []), (Some, [layout_for inner])] }`
- `Labelled(inner, _, _)` → transparently descends to `layout_for(inner)`
- `NegativeLabelled(inner, _, _)` → transparently descends to `layout_for(inner)`
- `Named(name, args, _)`:
  - If `args` is non-empty → `UnresolvedGeneric` (post-monomorphization input expected)
  - Look up in struct table → `Struct { type_name, fields: [(name, layout_for type_expr)] }`
  - Look up in enum table → `Enum { type_name, variants: [(name, [layout_for each payload])] }`
  - Otherwise → `UnknownType`

`LayoutError`: `UnknownType(String)`, `UnresolvedGeneric(String)`, `InvalidArraySize(i64)`, `UnsupportedType(String)`.

Convenience: `LayoutContext::size_in_bytes(&TypeExpr) -> Result<usize, LayoutError>` returns the total byte size in one call.

### `src/value_layout.rs` extensions

`ScalarKind` gained two new variants to cover types that did not previously have a scalar representation:

- `Text` (2 word bytes): static-string offset plus length, or arena handle plus epoch. The runtime distinguishes the underlying representation; the layout pass treats them uniformly.
- `Opaque` (1 word byte): `Arc<dyn HostOpaque>` pointer.

`ScalarKind::size_in_bytes` updated to return `2 * word_bytes` for `Text` and `word_bytes` for `Opaque`.

### Module registration

`src/lib.rs` declares `layout_pass` as a top-level `pub mod` gated behind the `compile` feature (parallel to `compiler`, `monomorphize`, etc.). Module docs explicitly note that P1's deliverable is callable infrastructure, not pipeline integration.

## Verification

- `cargo test --workspace`: 930 lib tests passing (up from 907). 23 new tests across the layout_pass module and the two new ScalarKind variants.
- `cargo test -p keleusma --no-default-features --features compile,verify`: 806 tests pass (floats-gated tests skipped as expected).
- `cargo test -p keleusma --all-features`: 909 tests pass.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

## Open questions

None. P1 is callable parallel infrastructure with clear inputs and outputs.

## Recommended next step

Begin P2: define the new consolidated opcode set and implement op handlers.

P2 scope:

1. New opcode variants in `Op` enum: `AllocTransient(byte_size: u16)`, `WriteScalarAt(offset: u16, kind: ScalarKind)`, `ReadScalarAt(offset: u16, kind: ScalarKind)`, `WriteCompositeAt(offset: u16, byte_size: u16)`, `ReadCompositeAt(offset: u16, byte_size: u16)`, `ReadDataField(slot: u16, offset: u16, kind: ScalarKind)`, `WriteDataField(slot: u16, offset: u16, kind: ScalarKind)`.
2. Wire-format encoding for the new opcodes (numeric IDs, operand serialisation).
3. Op handlers in `src/vm.rs` for each new opcode. The handlers operate on a new internal composite-value representation that holds a transient-region handle plus byte size.
4. Old composite opcodes (`NewTuple`, `NewArray`, `NewStruct`, `NewEnum`, `GetField`, `GetTupleField`, `GetEnumField`, `SetField`, `GetData`, `SetData`) marked deprecated but still functional. Both code paths exist in parallel; tests pass under both.
5. New transient-region handle type that wraps an arena `TopHandle<'arena>` with a byte size and a layout (or layout reference). Living alongside the existing `Value::Tuple(Vec<...>)` etc.
6. Unit tests covering each new opcode in isolation (allocate, write scalar, read scalar, write composite, read composite, read data field, write data field).

P2 estimated effort: 5-7 days. P2 is the most invasive phase because the new op handlers must integrate with the arena and the operand stack.

The wire format extension under P2 includes the new opcode numeric encodings. `BYTECODE_VERSION` stays at 1.

## Reference

- `src/layout_pass.rs` defines `LayoutContext` and `LayoutError`.
- `src/value_layout.rs` defines `LayoutDescriptor` and `ScalarKind` (now with `Text` and `Opaque`).
- `src/flat_value.rs` defines the scalar byte helpers and `FlatComposite`.
- B28 entry in `docs/decisions/BACKLOG.md` covers the revised phased implementation plan.
- B29 entry in `docs/decisions/BACKLOG.md` covers the strippable debug opcodes that P6 will land alongside the chunk-local `debug_pool` field.
