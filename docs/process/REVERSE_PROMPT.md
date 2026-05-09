# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T5. P2 local type tracking for for-in struct field access.
**Status**: Complete. P2 fully resolved for practical cases.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 368 tests pass workspace-wide. 323 keleusma unit including 2 new for the deferred for-in cases, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The deferred P2 case is now resolved. The compiler tracks local variable types and the for-in iteration bound resolves through identifier locals.

`Local` struct gained a `ty: Option<TypeExpr>` field. Let bindings record their declared annotation or inferred type. Parameters record their declared type at function entry. The `static_for_in_length` helper consults the type of identifier expressions through the local table, in addition to function returns and data block fields.

Type inference covers a narrow set of patterns sufficient for the for-in optimization.

- Struct construction. `Type { ... }` has type `Type`.
- Function call. The function's declared return type.
- Identifier. The local's recorded type.
- Field access. The struct or data field's declared type.
- Array literal with elements of inferable type.
- Literal value. The corresponding primitive or unit type.

The two new tests cover the previously deferred paths.

`for_in_over_struct_field_from_local_passes_strict_verify`. A program declares `struct Box { items: [i64; 3] }` and constructs `let b = Box { items: [..] }`. The for-in loop `for x in b.items` resolves through `b`'s type to the struct's field type and emits a `Const(3)` end bound that the verifier accepts.

`for_in_over_param_array_passes_strict_verify`. A function `sum_n(arr: [i64; 4])` iterates over its parameter. The parameter's declared type is recorded on the local and consulted by the for-in optimizer.

P2 is now fully resolved for the practical cases. Two cases remain deferred and are documented as future enhancement.

## Deferred Future Enhancements

- Nested array access. `for x in matrix[0]` where `matrix` is `[[T; N]; M]`. The result type of `[]` indexing is not yet inferred. The fix is to extend `infer_expr_type` to handle `Expr::ArrayIndex` by extracting the element type from the indexed array's type.
- Match expression results. `for x in match cond { ... => arr1, _ => arr2 }`. Match arms' result type tracking through inference is not yet implemented.

## Changes Made

### Source

- **`src/compiler.rs`**: `Local` struct gains `ty: Option<TypeExpr>` field. New `declare_local_typed` and `local_type` helpers. New `infer_expr_type` free function that infers a type from a narrow set of expression patterns. `compile_let_pattern_typed` records the declared or inferred type on the resulting local. `bind_param_pattern` accepts and records the parameter's declared type. `static_for_in_length` consults `Expr::Ident` through the local table. `struct_name_of` resolves identifiers through local types in addition to data block names.
- **`src/vm.rs`**: Two new tests covering the previously deferred for-in cases.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**: P2 marked fully resolved. The newly handled cases are listed alongside the previous ones. Out-of-scope cases are documented.
- **`docs/process/TASKLOG.md`**: V0.1-M3-T5 row added. New history row.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The local type tracking is bounded to what the for-in optimization needs. It is not a full type checker (P1 covers that). The compiler-side type info is computed lazily during compilation, with each let binding inferring once at its declaration site and parameters recording their declared type.

The infer_expr_type helper handles a narrow set of patterns. When a pattern is not recognized, the helper returns `None` and the local's type remains unset. Downstream, `static_for_in_length` falls back to `Op::Len` for sources whose length is not statically known. The compiler still emits valid bytecode for these cases; the strict-mode verifier rejects loops without an extractable bound.

Inference is shallow rather than recursive in some places. For example, `let x = some_func_returning_complex_type()` records the function's return type, but if the program then does `let y = x.field.subfield`, the chain is fully recursive through `infer_expr_type`. The let-time inference visits patterns recursively and produces correct types for the cases supported.

## Unaddressed Concerns

1. **Nested array access in for-in source.** Documented above as future enhancement.

2. **Match expression results.** Documented above.

3. **Type-tracked compilation more broadly.** The compiler now has more type information than before but does not unify with the type checker's pass. A future iteration could expose typecheck's computed types to the compiler to remove the duplication. Not blocking.

4. **Multiheaded function parameter types.** When a function has multiple heads (multiheaded dispatch), parameters are bound through the dispatch path rather than directly. The current implementation tracks types only on the simple-binding path. Multiheaded parameters will not get type tracking until the dispatch path is updated. Acceptable for the for-in use case because multiheaded functions typically have non-trivial patterns rather than simple variable bindings.

## Intended Next Step

Three paths.

A. Pivot to P7 follow-on. Operand stack and DynStr arena migration. Closes the bounded-memory guarantee end to end. Substantial refactor.

B. Publish keleusma main crate to crates.io now that P1, P2, P3, and P10 are fully resolved.

C. Continue with backlog items such as B7 (bidirectional errors through yield) which couples with the resolved P3.

Recommend B if external visibility is the priority. Recommend A if the bounded-memory guarantee is load-bearing for upcoming use cases.

Await human prompt before proceeding.

## Session Context

This long session resolved P10 across all phases, P1 with pipeline integration, P3 (explicit error recovery), P2 fully (typed for-in cases plus the deferred local field access case). The remaining open priority item is P7 follow-on. The session has produced extensive infrastructure for the embedded distribution story and the safety-critical positioning.
