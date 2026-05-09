# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T24. B2.4 generic enum specialization, polymorphic recursion cycle detection refinement, and B3 capture-by-reference disposition.
**Status**: Complete. The remaining named B2.4 and B3 follow-on items are now closed. The last B2.4 item (generic enum specialization) and the cycle detection refinement land in this session, and the capture-by-reference question is closed as not applicable to Keleusma's surface.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 472 tests pass workspace-wide. 404 keleusma unit (1 new for enum specialization), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session closes the remaining named B2.4 and B3 follow-on items.

### Generic enum specialization

The new `specialize_enums` pass runs in `monomorphize` after `specialize_structs` and mirrors that pass for `Expr::EnumVariant` whose target enum has type parameters. For each variant construction site, the pass walks the enum's declared variant payload types looking for `TypeExpr::Named(tp_name, ..)` patterns whose name matches a type parameter, infers the concrete type from the corresponding payload value via the existing `infer_arg_type` helper, emits a specialized `EnumDef` with payload types substituted, and rewrites the `EnumVariant`'s `enum_name` to the mangled form. Subsequent compilation sees the specialized enum as a regular non-generic enum, closing the same compile-time inference gap for enum-payload method dispatch that the struct pass closes for fields.

### Polymorphic recursion cycle detection

The fixed-point loop now bounds two ways. The existing global `SPECIALIZATION_LIMIT` caps the total number of specializations across the entire program. The new `PER_FUNCTION_LIMIT` caps the number of specializations any single generic function may produce, which is the structural signature of polymorphic recursion. The loop tracks per-function counts in a `BTreeMap`, updates them after each `rewrite_block` call by recovering the origin function name from the `origin__type_args` mangling, and exits early when any single function exceeds its bound. When the per-function bound trips, the remaining work is left unspecialized; subsequent compilation surfaces the truncation through the bytecode chunk count limit, which produces a clearer error path than infinite expansion.

### Capture-by-reference disposition

Capture by reference is not meaningful in Keleusma's pure-functional surface. The language's `let` bindings are immutable by design. There is no surface assignment operator that mutates a previously bound local, which means a captured local cannot diverge from the captured snapshot regardless of whether the capture is by value or by reference. The only mutable mechanism is the data segment, which is accessed through `data.field` and `data.field = expr` syntax that is independent of closure capture. A closure that wants to mutate shared state therefore reads and writes data segment slots directly. Capture by reference would only matter in a language with mutable locals, which Keleusma intentionally does not have. The item is therefore closed as not applicable rather than deferred.

## Tests

One new typecheck test, `monomorphize_enum_specialization_round_trip`, covers a `Maybe<T>` round trip through construction with a concrete payload value and a `match` over the result.

## Trade-offs and Properties

The enum specialization pass uses positional matching on payload types just as the struct pass uses positional matching on field types. For each declared type parameter, the pass searches for the first variant payload field whose declared type expression is `Named(tp_name)` and infers from the corresponding argument's value. More complex cases such as a payload field of `Pair<T, U>` (type parameter nested inside another generic) are not handled and the call site is left generic, mirroring the struct pass's treatment of nested generic field types.

The original generic enum declaration is retained alongside the specialization. This is the safe default consistent with the struct pass: callers that constructed the enum in ways the pass could not analyze still see the generic declaration. Future cleanup could prune declarations whose every construction was specialized.

The per-function specialization limit is conservative. Legitimate uses of polymorphic recursion that converge on a finite set of types remain admissible; only true unbounded recursion through type-argument growth trips the limit. The bound is set to 64 specializations per function, which is generous for hand-written code while remaining within practical bytecode chunk count limits.

## Changes Made

### Source

- **`src/monomorphize.rs`**. New `specialize_enums` pass invoked from `monomorphize` after `specialize_structs`. New helpers `specialize_enum`, `rewrite_enum_variants_block`, `rewrite_enum_variants_stmt`, `rewrite_enum_variants_expr` walk the program and rewrite enum variant construction sites. New `PER_FUNCTION_LIMIT` constant and `per_fn_counts` `BTreeMap` track per-function specialization counts within the existing fixed-point loop.
- **`src/typecheck.rs`**. New unit test `monomorphize_enum_specialization_round_trip`.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2.4 entry updated to record generic enum specialization and the polymorphic recursion cycle detection refinement as resolved. B3 entry updated to record capture-by-reference disposition as not applicable.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T24.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None of the originally named B2.4 or B3 items remain unresolved. The named work for generics, traits, monomorphization, and closures is structurally complete.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding. Subsequent work is open-ended; candidates from `BACKLOG.md` include further wire-format optimizations under B11 (per-op decode), additional native-side ergonomics, or work outside the previously named B2 and B3 scopes.

## Session Context

This session closed the remaining named B2.4 and B3 follow-on items. The compile pipeline now specializes generic functions, generic structs, and generic enums per concrete instantiation, and the monomorphization fixed-point loop is guarded against polymorphic recursion both globally and per-function. The capture-by-reference question is closed as not applicable because the surface language has no mutable locals.
