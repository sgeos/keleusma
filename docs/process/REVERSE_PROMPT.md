# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-02
**Status**: B28 P2 tuple flat activation complete end to end on sub-feature branch `feat-flat-memory-tuple`, two commits beyond the feature branch `feat-flat-memory-model`. The full default test suite is green. The branch is ready to merge into `feat-flat-memory-model` once you have reviewed the host-reflection decision below.

## What landed this session

Tuples whose fields are transitively scalar (non-reference, non-float) now use the flat byte body end to end. The two commits:

- `b57c307` re-spec, behaviour preserving. `Op::GetTupleField(u8)` became `Op::GetTupleField(TupleField)`, where `TupleField` is `Flat { offset, kind }` or `Boxed { index }`. Wire codec, cost and slot arms, and round-trip tests updated. The compiler still emitted boxed and the VM still built boxed, so behaviour was unchanged; this isolated the operand and wire churn.
- `5baa8fe` activation. Construction, access, and every reader now agree on the representation per tuple type, which equality relies on.

Key pieces of the activation:

- One construction choke point. `GenericValue::tuple_with_widths` decides flat or boxed from the element kinds and packs little-endian at given widths. `tuple()` delegates at runtime widths. The VM `NewTuple` packs at module widths. `from_const_archived` and `ConstValue::into_value` thread widths so constant tuples match. Host marshalling builds through the same path. This uniformity is what keeps a tuple type a single representation everywhere it is built.
- Access baking. The compiler bakes offset and kind per element when the tuple type is flat-eligible, threaded into all four emission sites including `compile_pattern_test`, which receives an ephemeral compile-time type record. `infer_expr_type` gained accurate-or-none inference for the checked-arithmetic construct so a let-destructure of its scalar-tuple result bakes flat access rather than faulting.
- Float exclusion. Float fields keep the boxed body for now, because the flat body compares by raw bytes, which would change the plus-zero, minus-zero, and NaN semantics of tuple equality. Revisit with a kind-aware equality before flattening floats.

## The host-reflection decision

A pure-bytes flat tuple cannot be read by generic Rust code that lacks the element layout. The marshalling boundary was reshaped to read flat bodies through the Rust element types. The remaining raw readers, meaning code that matched `Value::Tuple` and called `.elements()`, were converted to the typed marshalling path. You chose to push through this rather than defer the runtime representation to V0.4.

One genuine limit remains and is accepted as interim. `format_value` in the command-line frontend is a typeless display path with no static type at runtime, so it cannot decode a flat tuple element-wise. It renders a flat tuple as a placeholder noting the byte length. REPL and `println` display of a transitively-scalar tuple is therefore degraded until either the return type is threaded into the formatter or the V0.4 native backend bakes display. This is documented inline at the call site.

## Verification

- `cargo test` green across all suites: lib 1070, marshall, arena, zero-copy, bench, and the 53 rogue-script tests.
- `cargo clippy --tests -- -D warnings` and `cargo clippy --tests --all-features -- -D warnings` clean. `cargo fmt --check` clean for the B28 files.
- Tests under `--no-default-features` and `--all-features` show no failures.

## Concurrent external activity observed

During this session the working tree was modified concurrently by activity outside this task: `src/typecheck.rs`, `src/verify.rs`, and several untracked probe and proof-of-concept test files appeared and disappeared, carrying information-flow-control label-laundering probes and array and call underflow proofs of concept. Some of those probes are order-dependent or rely on shared global state and fail when run in isolation. None of this is part of B28 and none was authored here. The B28 commits were made by explicit path so they contain only the seven flat-tuple files and none of that separate work. If those probes are yours in progress, they are untouched in the working tree.

## Recommended next step

Merge `feat-flat-memory-tuple` into `feat-flat-memory-model` once the reflection decision above is acceptable. Then continue P2 by replicating the flat representation for `Array`, `Struct`, and `Enum`, and by flattening nested composites inline through the recursive layout. References (`Text`, `Opaque`) and the boxed fallback remain the scaffold until P3 makes them fixed-size handles, after which the dual representation is removed.

## Reference

- `docs/decisions/BACKLOG.md` B28 is the authoritative design and plan.
- `src/flat_value.rs` holds `FlatComposite`; `src/value_layout.rs` and `src/layout_pass.rs` are the compile-time layout, never carried on a value.
- `src/bytecode.rs` `tuple_with_widths` and `flat_tuple_scalar_kind` are the construction choke point and the kind predicate; the compiler's `type_flat_scalar_kind` and `tuple_field_access` are the type-side mirror.
- `docs/roadmap/V0_4_0_NATIVE_CODEGEN.md` carries the deferred flat-machine ISA redesign.
