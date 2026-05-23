# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-23
**Status**: B28 P0 complete. Two new parallel-infrastructure modules (`src/value_layout.rs`, `src/flat_value.rs`) establish the foundation for the V0.2.x runtime composite-Value representation refactor. No existing runtime path is touched; the modules are dormant until subsequent phases wire them in. 907 lib tests passing (up from 868). All workspace tests green across the default, default-minus-floats, and all-features feature matrices. Clippy and rustfmt clean.

## Summary of work since the last reverse-prompt update

### `src/value_layout.rs`

`ScalarKind` is a `Copy` enum tagging the fixed-size primitive types Keleusma admits in composite positions: `Unit`, `Bool`, `Byte`, `Int`, `Fixed`, and `Float` (the last gated behind the `floats` feature). `ScalarKind::size_in_bytes(word_bytes, float_bytes)` returns the byte size of each scalar under the supplied word and float widths.

`LayoutDescriptor` is the structural shape of any composite Keleusma type. Variants: `Scalar(ScalarKind)`, `Tuple(Vec<LayoutDescriptor>)`, `Array { element, count }`, `Struct { type_name, fields }`, `Enum { type_name, variants }`. The descriptor stores no width information; sizes and offsets are computed on demand from the supplied word and float byte widths. This keeps the descriptor independent of the parametric `Word` and `Float` type parameters and aligned with the `Target` cross-architecture portability model.

Methods on `LayoutDescriptor`: `size_in_bytes`, `field_offset`, `field_layout`, `struct_field_offset`. Tuple field offset is the sum of preceding element sizes. Struct field offset is the sum of preceding field sizes in declaration order. Array field offset is index times element size. Enum size is one byte (discriminant) plus the largest variant's payload size.

Twenty-two unit tests cover scalar sizes under varied widths, tuple/array/struct/enum size formulas, field offsets, and edge cases (empty composites, single-byte word widths, narrow words, mixed-field tuples, nested tuples, all-unit-variant enums, no-variant enums).

### `src/flat_value.rs`

Byte-level read and write helpers for the bundled runtime case (`Word = i64`, `Float = f64`): `write_bool`, `read_bool`, `write_byte`, `read_byte`, `write_i64`, `read_i64`, `write_f64`, `read_f64`. All helpers are little-endian. The f64 helpers are gated behind the `floats` feature. The parametric runtime case (other word and float widths) is deferred to later B28 phases.

`FlatComposite` pairs a `Vec<u8>` with an `Arc<LayoutDescriptor>`. Construction (`FlatComposite::new`) zero-initialises the byte buffer to the layout's declared size. The descriptor is held behind `Arc` so multiple composite values of the same type share one descriptor allocation.

Seventeen unit tests cover scalar round-trips (bool true/false/non-zero, byte at offset, i64 boundary values including `i64::MIN`/`i64::MAX`/zero, f64 boundary values including `f64::MIN`/`f64::MAX`/`f64::EPSILON`/`f64::INFINITY`/`f64::NEG_INFINITY`/NaN), little-endian byte ordering, offset-safe writes, and FlatComposite construction with tuple/array/struct/mixed-field layouts.

### Module registration

`src/lib.rs` declares both modules as top-level `pub mod`. Module documentation explicitly states that they are parallel infrastructure not yet consumed by any runtime path, with forward pointers to subsequent B28 phases.

## Verification

- `cargo test --workspace`: 907 lib tests passing (up from 868), plus the workspace test suites all green. Total tests across the workspace: 1059.
- `cargo test -p keleusma --no-default-features --features compile,verify`: 784 tests pass (floats-gated tests skipped as expected).
- `cargo test -p keleusma --all-features`: 886 tests pass.
- `cargo clippy --workspace --tests -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

## Open questions

None. P0 is parallel infrastructure with no integration into existing runtime paths. The next phase (P1) makes the integration decisions.

## Recommended next step

P1: migrate `Value::Tuple` from `Vec<GenericValue>` to a flat-byte payload using the P0 foundation.

P1 scope:

1. Define a new internal representation for `Value::Tuple` that holds an `Arc<FlatComposite>` (or equivalent) rather than `Vec<GenericValue>`. The public `Value` enum surface visible at the host API must remain stable; the change is to the internal payload only.
2. Update `Op::NewTuple` to materialise a tuple as a `FlatComposite` instance, writing each operand-stack value into the corresponding byte offset.
3. Update `Op::GetIndex` (when the indexed value is a tuple) to read the byte offset from the layout and decode the scalar to a `Value` for the operand stack.
4. Update the `materialise_kstrings` and equality paths for the new representation.
5. Update tuple-related tests to assert byte-level layout where appropriate.
6. WCMU calculation for `Op::NewTuple` shifts: the per-op byte cost reflects the flat-byte size rather than the `Vec` overhead the V0.2.0 verifier assumed. Update the golden WCMU numbers in the tests that pin them.
7. Native marshalling preservation: the `KeleusmaType` derive's tuple-marshalling path needs to convert between the host's `(A, B, C)` tuple and the flat-byte representation. The derive's macro-generated code should remain unchanged; the conversion happens inside the marshall module.

P1 effort estimate: 3-4 days (matches the original phased plan). The risk is concentrated in the WCMU number shifts and the tuple-marshalling preservation; the rest is mechanical.

The wire-format extension stays for P5 (the chunk-local `debug_pool` field and the `DataSlotAnnotation` opcode). P1 through P4 are pure runtime refactors that change the internal Value representation without touching the bytecode.

## Reference

- `src/value_layout.rs` defines `ScalarKind` and `LayoutDescriptor`.
- `src/flat_value.rs` defines the scalar helpers and `FlatComposite`.
- B28 entry in `docs/decisions/BACKLOG.md` covers the phased implementation plan.
- B29 entry in `docs/decisions/BACKLOG.md` covers the strippable debug opcodes that P5 will land alongside the wire-format extension.
