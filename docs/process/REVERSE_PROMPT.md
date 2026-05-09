# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T25. B2.4 inference reach extension to field access, B2 stale doc cleanup, and B3 nested-closure transitive capture.
**Status**: Complete. The remaining named B2 and B3 follow-ons that surfaced after the prior session are closed. Field-access inference and nested-closure transitive capture both required real implementation work, while the B2 backlog entry was outdated documentation that has been corrected.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 475 tests pass workspace-wide. 407 keleusma unit (3 new), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session closed three follow-on items:

### B2.4 inference reach extension to field access

`monomorphize::infer_arg_type` previously returned `None` for `Expr::FieldAccess` because no struct table was available. The pass now builds a struct table at the top of `monomorphize()` and threads it through `rewrite_block`, `rewrite_stmt`, `rewrite_expr`, and `rewrite_iterable`. The `FieldAccess` arm of `infer_arg_type` resolves the object's nominal type, looks up the struct's declared field type, and applies per-instance type-argument substitution when concrete type arguments are available on the receiver. Abstract field types (those whose declared type is exactly one of the struct's type parameters and the receiver carries no type arguments) are guarded against erroneous propagation: returning such an abstract type as the inferred argument would cause the call site to specialize against a type variable rather than a concrete type.

The motivating case `let h = Holder { value: 21 }; use_doubler(h.value)` where `Holder` is a non-generic struct with `value: i64` and `use_doubler<T: Doubler>` requires a concrete `T` now compiles end to end.

### B2 stale doc cleanup

The B2 backlog entry listed "Method call surface syntax (`x.method(args)`)" as remaining work. That syntax landed in V0.1-M3-T18 with `Expr::MethodCall`. The entry has been corrected to record the syntax as implemented and to remove the stale "remaining work" bullet.

### B3 nested-closure transitive capture

`compiler::collect_free_in_expr` previously treated `Expr::ClosureRef` as a leaf, so an inner closure that captured an outer-function local could not transitively propagate that capture through an enclosing closure. The motivating failure was `let base = 100; let outer = |x| { let inner = |y| base + x + y; inner(3) }; outer(7)`, which produced a "captures `base` which is not a local in the enclosing scope" compile error because `outer`'s synthetic chunk did not receive `base` as a captured implicit parameter.

The `ClosureRef` arm of the free-variable collector now treats each entry of the inner closure's `captures` list as a free variable of the enclosing expression unless it is bound by the enclosing scope's parameters. The outer closure's hoisted chunk therefore gains the transitively captured name as an additional implicit parameter, and at the inner closure's construction site that name is in scope and is captured normally through `Op::MakeClosure`.

## Tests

Three new typecheck tests:

- `monomorphize_inference_through_field_access` covers the field-access inference round trip.
- `closure_nested_inside_closure_typechecks` covers the basic nested-closure type check.
- `closure_nested_capturing_outer_local_typechecks` covers the transitive-capture case.

One new example, `examples/closure_nested.rs`, demonstrates end-to-end execution.

## Trade-offs and Properties

The struct table threading is invasive at the call-site level (every recursive call in the rewrite chain gains an extra argument) but conceptually narrow. The `infer_arg_type` function continues to take an `Option` for the struct table so callers in the `specialize_structs` and `specialize_enums` chains, which do not need field-access inference, can pass `None` without owning a struct table.

The transitive-capture propagation is conservative in that it adds every inner-capture name to the enclosing scope's free-variable list. If an inner closure's capture is in fact bound by the enclosing scope (for example, the inner closure captures the outer closure's parameter), the outer's `bound` set filters it out. The propagation does not deduplicate against names already in `out` beyond the existing `!out.contains(...)` check, which keeps the logic uniform with the rest of the collector.

## Changes Made

### Source

- **`src/monomorphize.rs`**. Struct table built at the top of `monomorphize()` and threaded through `rewrite_block`, `rewrite_stmt`, `rewrite_expr`, `rewrite_iterable`. `infer_arg_type` signature extended with `Option<&BTreeMap<String, StructDef>>`. `FieldAccess` arm resolves field type with per-instance substitution and abstract-type guard.
- **`src/compiler.rs`**. `collect_free_in_expr`'s `ClosureRef` arm now propagates inner-capture names to the enclosing scope's free-variable list, separated from the unreachable `Closure` arm.
- **`src/typecheck.rs`**. Three new unit tests.
- **`examples/closure_nested.rs`** (new). End-to-end demonstration.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2 entry corrected to remove the stale method-call-syntax item. B2.4 entry updated with field-access inference and the abstract-type guard. B3 entry updated with the nested-closure transitive-capture mechanism.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T25.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None of the originally named B2.2, B2.3, B2.4, or B3 items remain unresolved. Subsequent sessions can address items outside this scope, such as B11 (per-op decode optimization for zero-copy execution) or B5b (variable-cost string operations).

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding. Subsequent work is open-ended.

## Session Context

This session closed the residual follow-on items in B2.2, B2.3, B2.4, and B3 that surfaced after the prior session: a stale doc entry, a real inference-reach gap for field access, and a real transitive-capture gap for nested closures. The compile pipeline now specializes generic calls whose arguments include field access expressions, and closures can transitively capture from any enclosing scope through any number of intermediate closures.
