# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-23
**Status**: B28 P2 complete. Seven consolidated composite opcodes added with wire-format encoding, VM op handlers, and a new `Value::Composite(Vec<u8>)` runtime variant. Old composite opcodes coexist with the new ones; both code paths are functional. 948 lib tests passing (up from 930 after P1).

## Summary of work since the last reverse-prompt update

### New opcodes

Seven new variants on `Op`:

- `AllocTransient(u16)`: bump-allocate the requested byte size, push a `Value::Composite` handle.
- `WriteScalarAt(u16, ScalarKind)`: pop scalar, write into top-of-stack composite at offset; composite stays.
- `ReadScalarAt(u16, ScalarKind)`: pop composite, push the scalar at offset.
- `WriteCompositeAt(u16, u16)`: pop nested composite, copy bytes into parent at offset; parent stays.
- `ReadCompositeAt(u16, u16)`: pop parent, push a copy of bytes at offset with byte_size.
- `ReadDataField(u16, u16, ScalarKind)`: P2 stub; P3 emission and P4 arena integration land it.
- `WriteDataField(u16, u16, ScalarKind)`: P2 stub.

### Wire format

Opcode IDs 69-75 assigned. `WriteScalarAt` and `ReadScalarAt` carry inline u16+u8 operands. `WriteCompositeAt` and `ReadCompositeAt` use the existing pool u16+u16 shape. `ReadDataField` and `WriteDataField` use the existing pool u16+u16+u8 shape. `ScalarKind::to_u8` and `ScalarKind::from_u8` provide stable wire serialisation. `WireFormatError::InvalidScalarKind(u8)` covers corrupted or feature-mismatched tags. `BYTECODE_VERSION` stays at 1.

### Runtime

`Value::Composite(Vec<u8>)` is the new variant on `GenericValue`. The bytes are the composite's flat representation; the layout descriptor is not carried in the value (the compiler emits offsets and kinds so the runtime treats the bytes as opaque). For B28 P2 the bytes are heap-allocated; P4 onwards moves them to the arena's top ephemeral head.

Helper free functions `write_scalar_into_bytes::<W, F>(bytes, offset, kind, value)` and `read_scalar_from_bytes::<W, F>(bytes, offset, kind)` perform the scalar conversion. They are parametric over `Word` and `Float`, supporting the bundled `i64`/`f64` case; narrow word widths sign-extend through the `i64` round-trip; narrow float widths (`f32`) and the Text/Opaque scalar paths surface `InvalidBytecode` until subsequent phases.

The op handlers for `AllocTransient`, `WriteScalarAt`, `ReadScalarAt`, `WriteCompositeAt`, and `ReadCompositeAt` are fully implemented. `ReadDataField` and `WriteDataField` return `VmError::InvalidBytecode` for P2; P3 wires the compiler emission, and P4 wires the data-section arena integration.

### Old opcodes coexist

The V0.2.0 composite opcodes (`NewTuple`, `NewArray`, `NewStruct`, `NewEnum`, `GetField`, `GetTupleField`, `GetEnumField`, `SetField`, `GetData`, `SetData`) are untouched. They continue to work. P5 will retire them once compiler emission has migrated to the new opcodes.

## Verification

- `cargo test --workspace`: 948 lib tests passing (up from 930 after P1). 18 new tests across wire-format roundtrips and VM execution.
- `cargo test -p keleusma --no-default-features --features compile,verify`: 823 tests pass (floats-gated tests skipped as expected).
- `cargo test -p keleusma --all-features`: 927 tests pass.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

## Open questions

None. P2 establishes the ISA surface and the runtime semantics; the compiler emission and arena integration follow in P3 and P4.

## Recommended next step

Begin P3: compiler emission migrates to the new opcodes.

P3 scope:

1. Integrate `LayoutContext` from P1 into the compile pipeline. Add a layout-pass step between monomorphisation and emission that builds a per-type layout table from the post-monomorphisation struct and enum definitions.
2. Compiler emission migrates composite construction sites to emit `AllocTransient(byte_size)` plus a sequence of `WriteScalarAt(offset, kind)` opcodes. The byte_size and offsets come from the layout pass.
3. Field-access sites migrate to emit `ReadScalarAt(offset, kind)` or `ReadCompositeAt(offset, byte_size)` based on whether the field is a scalar or a nested composite.
4. Data-section access sites migrate to emit `ReadDataField(slot, offset, kind)` or `WriteDataField(slot, offset, kind)`. The VM's data-section storage remains the current V0.2.0 shape for P3; arena integration lands in P4.
5. Both code paths exist in parallel: a feature flag or a compile-time switch chooses between the old and new emission. Default: old emission until P3 is fully validated, then switch.
6. WCMU calculation may need updates because byte costs now reflect the flat-byte representation. P7 owns the formal WCMU correction; P3 may surface temporary WCMU regressions in golden numbers that get fixed in P7.

P3 effort: 5-7 days. The complexity is concentrated in the compiler's emission logic, which currently emits the old composite opcodes from many sites.

## Reference

- `src/bytecode.rs` defines the new `Op` variants and `Value::Composite`.
- `src/wire_format.rs` carries the encoding and decoding for opcodes 69-75.
- `src/vm.rs` carries the op handlers and the `write_scalar_into_bytes` / `read_scalar_from_bytes` helpers.
- `src/value_layout.rs` carries `ScalarKind::to_u8` and `ScalarKind::from_u8`.
- `src/layout_pass.rs` from P1 produces the `LayoutDescriptor` instances that P3 will consume during emission.
