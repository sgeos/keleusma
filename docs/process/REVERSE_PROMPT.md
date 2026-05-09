# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T23. B2.4 generic struct specialization.
**Status**: Complete. Generic struct specialization closes the named B2.4 gap. The remaining items are generic enum specialization and capture-by-reference, both smaller follow-ons.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 471 tests pass workspace-wide. 403 keleusma unit (1 new for struct method dispatch), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session delivered generic struct specialization, closing the named B2.4 monomorphization gap.

The new `specialize_structs` pass runs in `monomorphize` after function specialization. It walks the program for `Expr::StructInit` whose target struct has type parameters. For each, the pass infers the struct's type arguments by matching declared field types against provided field values' types using the same `infer_arg_type` helper used for function-call inference. When all type arguments can be inferred, the pass emits a specialized `StructDef` with field types substituted by the concrete types and rewrites the `StructInit`'s name to a mangled form like `Cell__i64`.

After specialization, the compiler sees the specialized struct as a regular non-generic struct. The compile-time field-type inference resolves the field's concrete type, which means method dispatch on field-typed receivers works correctly. The motivating case `c.value.double()` where `c: Cell<i64>` now compiles end to end, dispatching to `Doubler::i64::double` through the specialized field's concrete type.

## Tests

One new typecheck test, `monomorphize_struct_field_method_dispatch`, covers the round trip.

One new example, `examples/struct_method_dispatch.rs`, demonstrates end-to-end execution.

## Trade-offs and Properties

The specialization pass uses positional matching to infer type arguments: for each declared type parameter, find the first declared field whose type expression is `Named(tp_name)` and infer from the corresponding field value. This handles the common case where a type parameter appears directly as a field type. More complex cases such as `field: Pair<T, U>` (type param nested inside another generic) are not yet handled and the call site is left generic.

The original generic struct declaration is retained alongside the specialization. This is the safe default: callers that constructed the struct in ways the pass could not analyze still see the generic declaration. Future cleanup could prune declarations whose every construction was specialized.

Generic enums are NOT yet specialized in this session. Method dispatch on enum payload types would benefit from a similar pass and is documented as the remaining piece of monomorphization.

## Changes Made

### Source

- **`src/monomorphize.rs`**. New `specialize_structs` pass invoked from `monomorphize` after function specialization. New `mangle_struct`, `specialize_struct`, `rewrite_struct_inits_block`, `rewrite_struct_inits_stmt`, `rewrite_struct_inits_expr` helpers walk the program and rewrite struct construction sites.
- **`src/typecheck.rs`**. New unit test `monomorphize_struct_field_method_dispatch`.
- **`examples/struct_method_dispatch.rs`** (new). End-to-end demonstration.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2.4 entry updated to record generic struct specialization as resolved. Remaining work narrowed to generic enums and the polymorphic recursion cycle detection refinement.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T23.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None of the originally named items remain unresolved.

The remaining items.

- B2.4 generic enum specialization. Estimated 1 to 2 hours.
- B2.4 polymorphic recursion cycle detection (refinement over the existing SPECIALIZATION_LIMIT bound).
- B3 capture by reference semantics (the current capture is by value).

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

The remaining items are smaller refinements rather than core features. Generic enum specialization is the most natural follow-on to today's work because it uses the same pattern as struct specialization.

Await human prompt before proceeding.

## Session Context

This session closed the B2.4 generic struct specialization deferred item. The compile pipeline now specializes both generic functions and generic structs per concrete instantiation, enabling compile-time method dispatch on generic struct field types.
