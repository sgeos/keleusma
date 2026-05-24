# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-23
**Status**: B28 P2 reverted; B28 reframed as a pure runtime refactor against the unchanged V0.2.0 ISA. The seven consolidated composite opcodes I added in the prior P2 commit are removed; the `Value::Composite` variant is removed; the wire format, the opcode ID table, the VM op handlers, the bench cost model, and the docs are all restored to their pre-P2 state. 930 lib tests passing (the post-P1 count). The B28 phased plan has been rewritten to migrate composite Value internal storage from `Vec<Value>` to flat bytes through a sequence of per-type phases (P2 tuples, P3 arrays, P4 structs, P5 enums) followed by arena integration (P6), WCMU correction (P7), marshalling preservation (P8), and closure (P9).

## Summary of work since the last reverse-prompt update

### ISA review

Reviewed `docs/spec/INSTRUCTION_SET.md`, `docs/spec/STRUCTURAL_ISA.md`, `docs/spec/WIRE_FORMAT.md`, and `docs/architecture/EXECUTION_MODEL.md`. The V0.2.0 ISA carries 69 opcodes (numeric IDs 0-68) and the docs reference that count consistently. The composite opcodes (`NewTuple`, `NewArray`, `NewStruct`, `NewEnum`, `GetField`, `GetTupleField`, `GetEnumField`, `GetIndex`, `Len`) carry sufficient information for the runtime to build per-chunk layout caches and operate on flat-byte composite bodies without ISA changes:

- `Op::NewTuple(count)` consumes `count` operand-stack values; the runtime infers each value's byte size from its discriminant and packs into a flat buffer.
- `Op::NewStruct(template_idx)` looks up the struct template's byte layout (computed once at chunk-load time from the existing wire-format struct templates).
- Field-access opcodes use the cache to resolve offsets at dispatch time.

The data-section opcodes (`GetData`, `SetData`, `GetDataIndexed`, `SetDataIndexed`, `BoundsCheck`) read or write whole-slot values; for composite-typed slots the runtime returns the flat-byte buffer wrapped in the existing `Value::Tuple`/`Array`/`Struct`/`Enum` variant. The host marshalling layer translates at the API boundary.

The B28 work is entirely runtime-side. No ISA changes. No wire format changes. No opcode additions or retirements. `BYTECODE_VERSION` stays at 1.

### Revert

Removed from the prior P2 commit:

- `src/bytecode.rs`: the seven Op variants (`AllocTransient`, `WriteScalarAt`, `ReadScalarAt`, `WriteCompositeAt`, `ReadCompositeAt`, `ReadDataField`, `WriteDataField`); the `Value::Composite(Vec<u8>)` variant; the `use crate::value_layout::ScalarKind;` import; the cost and stack-effect cases for the new opcodes.
- `src/value_layout.rs`: the `to_u8` and `from_u8` methods on `ScalarKind` (only used by the retired wire-format paths).
- `src/wire_format.rs`: opcode IDs 69-75 from the `OPCODE_ID_TABLE`; the matching cases in `opcode_id_of` and `encode_op` and `decode_op`; the `WireFormatError::InvalidScalarKind` variant; the eighteen P2-specific tests.
- `src/vm.rs`: the seven op handlers; the `write_scalar_into_bytes`, `read_scalar_from_bytes`, `check_offset`, `word_bytes_for`, `float_bytes_for` helpers; the thirteen P2 execution tests.
- `keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs`: the new opcode cost entries.

`ScalarKind::Text` and `ScalarKind::Opaque` remain on `ScalarKind` because the layout pass (`src/layout_pass.rs`, landed in P1) uses them to compute byte sizes for `Text` and `Opaque` types appearing in source-level code.

### B28 redesign

Rewrote the B28 entry in `docs/decisions/BACKLOG.md`. The pure runtime refactor framing:

- The ISA does not change. The V0.2.0 opcode set carries sufficient information.
- The wire format does not change. Existing V0.2.x bytecode loads under the post-B28 runtime without modification.
- The composite Value internal payload changes from `Vec<Value>` to flat bytes plus a layout reference. The layout cache is built at chunk-load time from struct templates and type declarations carried in the existing wire format.
- Composite bodies move from the global heap to the arena's top ephemeral head alongside `KString` bodies. Mark-based reclamation at scope boundaries; RESET at `loop main()` closing brace clears the head.
- WCMU calculation in the verifier produces precise byte sums from the layout cache; the V0.2.0 `Vec`/`String` over-approximations disappear.

Phased plan rewritten to nine phases (P0 done, P1 done, P2-P9 remaining):

| Phase | Scope | Effort |
|-------|-------|--------|
| P0 | LayoutDescriptor and FlatComposite infrastructure | Complete (`45df5bf`) |
| P1 | Compile-time layout pass | Complete (`0fc5950`) |
| P2 | Migrate `Value::Tuple` internal storage to flat bytes plus layout reference | 4-5 days |
| P3 | Migrate `Value::Array` | 3-4 days |
| P4 | Migrate `Value::Struct` with per-chunk struct-template layout cache | 5-7 days |
| P5 | Migrate `Value::Enum` | 3-4 days |
| P6 | Move composite bodies to arena's top ephemeral head | 5-7 days |
| P7 | WCMU correction in verifier | 3-4 days |
| P8 | Native marshalling preservation across the representation change | 3-4 days |
| P9 | Hot-code-swap migration, documentation, B28 closure | 3-4 days |

Total estimated effort: four to six weeks for the remaining phases.

### B29 reframed

B29 (strippable debug opcodes) was previously framed as the carrier of `DataSlotAnnotation`, an opcode B28 needed for runtime data-section layout. Under B28's pure runtime refactor framing the runtime computes layouts from existing wire-format metadata; no `DataSlotAnnotation` opcode is needed. B29 is reframed as an independent framework for development aids (variable names, source spans, breakpoints, IFC label audit trails). The B29 catalogue removed the `DataSlotAnnotation` row; the cross-reference to B28 was clarified; the implementation cost table updated to remove the B28-load-bearing line.

### Documentation consistency

`docs/spec/INSTRUCTION_SET.md` claims "The instruction set contains 69 opcodes." After my P2 work the count was 76; after the revert it is 69 again, matching the doc. `docs/spec/WIRE_FORMAT.md` says "opcode values 0-68 are valid, values 69-127 are reserved" which is now consistent. `docs/architecture/EXECUTION_MODEL.md` says "65 of 69 opcodes carry their operand inline in the 4-byte record" which is also consistent. No doc edits were needed.

## Verification

- `cargo test --workspace`: 930 lib tests passing. Total tests across the workspace: 1082.
- `cargo test -p keleusma --no-default-features --features compile,verify`: 806 tests pass (floats-gated tests skipped as expected).
- `cargo test -p keleusma --all-features`: 909 tests pass.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

## Open questions

None at the design layer. The pure-runtime-refactor framing is internally consistent and grounded in verified facts about the V0.2.0 ISA's information content.

## Recommended next step

Begin the revised P2: migrate `Value::Tuple` internal storage from `Vec<GenericValue>` to flat bytes plus a layout reference.

P2 scope:

1. Define a new `Value::Tuple` payload that holds an `Arc<TupleLayout>` (the per-tuple-type layout) plus a `Vec<u8>` flat-byte buffer. The `Arc<TupleLayout>` lets multiple tuple values of the same type share one layout allocation.
2. Update `Op::NewTuple(count)` op handler to pop `count` values, infer each value's byte size from its discriminant, build the layout, allocate the byte buffer, and pack the values' bytes inline.
3. Update `Op::GetTupleField(index)` op handler to read the indexed field's bytes through the layout's offset table.
4. Update `materialise_kstrings` and equality paths for the new representation.
5. Native marshalling: ensure the public `Value::Tuple` API constructors and accessors continue to work through the marshall module.
6. Tests: update the existing tuple-related tests to assert byte-level behaviour where it matters; the rest should continue passing.

P2 effort: 4-5 days.

Awaiting operator confirmation to proceed.

## Reference

- `docs/decisions/BACKLOG.md` B28 (revised), B29 (revised).
- `src/value_layout.rs` defines `LayoutDescriptor` and `ScalarKind` (now without `to_u8`/`from_u8`).
- `src/flat_value.rs` defines `FlatComposite` and the scalar byte helpers (still useful for P6 onwards).
- `src/layout_pass.rs` defines `LayoutContext` and `LayoutError`.
- `docs/spec/INSTRUCTION_SET.md` and `docs/spec/WIRE_FORMAT.md` describe the unchanged V0.2.0 ISA.
