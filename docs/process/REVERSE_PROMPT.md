# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T28. B2.4 inference reach for `MethodCall`, `UnaryOp`, and `BinOp`.
**Status**: Complete. Three remaining inference gaps closed. Each gap was a real failure where a generic call argument's type was not inferred and the program failed to compile.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 482 tests pass workspace-wide. 415 keleusma unit (3 new), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

A probe identified three inference shapes that previously failed compilation when used as generic call arguments:

- A method-call return value such as `use_doubler((21).double())`.
- A unary-negation such as `use_doubler(-n)` where `n: i64`.
- An arithmetic binary operation such as `use_doubler(n + 11)`.

Each failed because `monomorphize::infer_arg_type` returned `None` for the corresponding `Expr` variant, which caused the call site to remain generic. After re-typecheck against the unspecialized body, the receiver type was abstract and the method dispatch could not resolve, surfacing as `type T has no method <name>`.

The three arms now resolve as follows:

- `Expr::MethodCall` looks up the impl method's declared return type under a `<head>::<method>` mangled key in `fn_returns`. The map is populated at the top of `monomorphize` from `program.impls` alongside top-level functions, using a new `type_head_for_impl` helper that mirrors the compiler's existing `type_expr_head` convention. The mangling is intentionally distinct from the compiler's `Trait::<head>::<method>` chunk-folding mangling so the two namespaces stay disjoint.
- `Expr::UnaryOp` recurses on the operand for `Neg` and returns `Prim(Bool)` for `Not`.
- `Expr::BinOp` recurses on the left operand for arithmetic operators (`Add`, `Sub`, `Mul`, `Div`, `Mod`) and returns `Prim(Bool)` for comparison and logical operators (`Eq`, `NotEq`, `Lt`, `Gt`, `LtEq`, `GtEq`, `And`, `Or`).

These extensions remain consistent with the WCET goal because they only widen the static type information available at monomorphize time. They do not introduce any new runtime mechanism, indirect dispatch path, or unbounded execution shape.

## Tests

Three new typecheck tests, each exercising the full compile pipeline through `compile_src`:

- `monomorphize_inference_through_method_call`
- `monomorphize_inference_through_unary_op`
- `monomorphize_inference_through_bin_op`

## Trade-offs and Properties

The choice to fold impl method returns into `fn_returns` under a single namespace, rather than threading a separate map through the rewrite chain, kept the diff narrow. The mangled key `<head>::<method>` cannot collide with a top-level function name because Keleusma identifiers cannot contain `::`.

The arithmetic-binop arm uses the left operand's type as the result type. The type checker's existing same-type unification means the right operand has an equal type at type-check time, so taking the left is correct under the type checker's contract. If the operands' types are not unified, the receiver's method dispatch will fail to resolve at re-typecheck and the program will be rejected, which is the desired behavior.

The unary-not arm returns `Bool` rather than recursing on the operand. Keleusma uses `Not` for logical negation only (no bitwise `!`), so the result is always boolean.

The method-call return type returned by `fn_returns.get(<head>::<method>)` is the declared return type from the impl method's signature. If the trait declares the method generically (which is not currently expressible because trait method signatures are concrete), the substitution would need to propagate; this is not a real case in the present language.

## Changes Made

### Source

- **`src/monomorphize.rs`**. `monomorphize` now folds impl method returns into `fn_returns` under `<head>::<method>` keys, using a new `type_head_for_impl` helper. `infer_arg_type` gains arms for `Expr::UnaryOp`, `Expr::BinOp`, and `Expr::MethodCall`.
- **`src/typecheck.rs`**. Three new tests exercising the inference reach extensions through the full compile pipeline.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2.4 entry's "Inference reach extension" paragraph extended with the new shapes.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T28.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The named B1, B2.2, B2.3, B2.4, and B3 work has now been exhaustively exercised. Inference reach covers literals, identifiers, function-call returns, method-call returns, unary and binary operators, casts, enum variants, struct constructions, tuple and array literals, if and match arms, field access, tuple-index, and array-index. The closure subsystem covers first-class arguments, environment capture, transitive nested capture, and recursive let-bound closures, with the WCET-incompatible recursive case rejected by the safe verifier.

Known approximations that remain documented but not addressed in this session, consistent with the user's reminder that certain features must remain out of scope:

- The WCMU and WCET analyses do not follow `Op::CallIndirect` targets. Programs that construct unbounded recursion through indirect dispatch over non-recursive closures (such as `apply(apply, x)`) are admissible despite being unbounded at runtime.
- A recursion-depth attestation API for recursive closures would re-admit them under a host-declared bound; not implemented.
- Block expressions `{ ... }` are not currently parseable as primary expressions, so a closure constructed inside an immediately-evaluated block is rejected at parse time. Parser-level concern, not in current scope.
- The B1 `Type::Unknown` sentinel is retained as a permissive transitional anchor for runtime-only dispatch positions. Removing it would require declaring native function signatures. Type-system tightening rather than WCET concern.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding. Subsequent work falls outside the named B1, B2.2, B2.3, B2.4, and B3 scope, or pertains to the documented WCET refinements and B1 sentinel cleanup.

## Session Context

This session closed the residual inference gaps in B2.4 that surfaced through a comprehensive probe of common expression shapes used as generic call arguments. The inference reach is now uniform across the practical surface of expressions whose result types can be statically determined.
