# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T22. B2.4 inference reach extension and B3 first-class closures as function arguments.
**Status**: Complete. Both items execute end to end.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 470 tests pass workspace-wide. 402 keleusma unit (2 new across B2.4 and B3 follow-ons), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session delivered the B2.4 inference reach extension and the B3 first-class closures-as-arguments slice.

B2.4 inference reach extension. The monomorphization pass's `infer_arg_type` helper now handles more expression shapes when inferring concrete types for generic call arguments. New cases include direct function call returns (resolved through a function-return-type map), tuple and array literals (recursive inference of element types), cast expressions (using the target type), enum variants (using the enum name), and if/match expressions (using the first branch's tail expression). Generic call sites whose arguments use these shapes specialize correctly without requiring intermediate let bindings.

Pruning policy refinement. The previous strict prune dropped every generic function from the program output, which was too aggressive: closure-typed arguments cannot be inferred to a concrete type, so monomorphization leaves the receiving function generic. Without retention, the runtime would fail with "undefined function" at the call site. The policy now drops only generic functions whose specializations were generated. Generics with no specializations are retained because they execute correctly through runtime tag dispatch on `Value` tags. This is the safe default that supports first-class closure arguments and other opaque-type call sites.

B3 first-class closures as function arguments. With the pruning policy adjusted, a generic function `fn apply<F>(f: F, x: i64) -> i64 { f(x) }` now compiles and runs. The compiler resolves the parameter `f` as a local and emits `Op::CallIndirect` for `f(x)`. The closure passed as the argument flows through the call frame as a `Value::Func` and dispatches at the receiving function's call site. `examples/closure_as_arg.rs` demonstrates `apply(g, 41)` returning 42.

## Tests

Two new typecheck tests.

- `monomorphize_inference_through_function_call`. Generic call site uses a function call as an argument; inference resolves the call's return type and specializes correctly.
- `closure_passed_as_argument`. A generic function takes a closure as an argument and invokes it from the body through indirect dispatch.

One new example, `examples/closure_as_arg.rs`, demonstrates end-to-end execution.

## Trade-offs and Properties

The inference reach extension's coverage matches the typical generic call patterns in idiomatic code. Field access on a struct local would also benefit but requires threading struct field-type tables through the inference helper; deferred to a future iteration.

The pruning policy relaxation accepts some redundancy: a generic function may have one or more specializations and ALSO remain in the program as a polymorphic chunk if at least one call site could not be specialized. This adds a small amount of dead code in mixed cases but ensures correctness for all call patterns. A future cleanup pass could detect when ALL call sites of a generic function were specialized and prune the original at that point.

The first-class closure path uses the existing `Value::Func` representation for both argument-passed closures and locally-bound closures. There is no distinction at runtime, which is the correct behavior. The receiving function treats the parameter as a callable through `Op::CallIndirect`.

## Changes Made

### Source

- **`src/monomorphize.rs`**. `infer_arg_type` extended to handle Expr::Call, Cast, EnumVariant, TupleLiteral, ArrayLiteral, If, and Match expressions. New `fn_returns: BTreeMap<String, TypeExpr>` parameter threaded through `rewrite_block`, `rewrite_stmt`, `rewrite_iterable`, `rewrite_expr`, and `infer_arg_type`. Pruning policy changed from "drop all generics" to "drop only generics with at least one specialization".
- **`src/typecheck.rs`**. Two new unit tests.
- **`examples/closure_as_arg.rs`** (new). End-to-end demonstration of a closure passed as a function argument and invoked from the receiving function.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2.4 entry updated to record the inference reach extension. B3 entry updated to mark first-class closures as function arguments resolved.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T22.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None of the originally listed items remain. P1 through P10 fully resolved. B1 resolved. B2 fully resolved with monomorphization MVP, follow-ons, and inference reach. B3 fully resolved with environment capture and first-class arguments.

The remaining items.

- B2.4 generic struct and enum monomorphization. Estimated 3 to 5 hours.
- B3 capture by reference semantics. Currently capture is by value.
- B2.4 polymorphic recursion cycle detection. Currently bounded by SPECIALIZATION_LIMIT.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

The remaining items are smaller refinements rather than core features. Generic struct and enum monomorphization is the most substantial; capture-by-reference is rarely needed in practice; cycle detection is a quality improvement over the existing bound.

Await human prompt before proceeding.

## Session Context

This long session closed the named B2.4 and B3 follow-on items. The compile pipeline now handles a wider range of generic call patterns through extended argument-type inference, and closures pass through function call sites as first-class values.
