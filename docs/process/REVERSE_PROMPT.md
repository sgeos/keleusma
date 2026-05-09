# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T26. B2.4 inference reach to TupleIndex and ArrayIndex, and B3 recursive closures via let-binding self-reference.
**Status**: Complete. Recursive closures land with full runtime support (new opcode, runtime self-push, BYTECODE_VERSION bump). Two more inference shapes (tuple-index, array-index) close the remaining low-effort B2.4 inference gaps.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 479 tests pass workspace-wide. 411 keleusma unit (4 new), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session closed two follow-on items:

### B2.4 inference reach to TupleIndex and ArrayIndex

A probe revealed that the existing tests for inference shapes used `check_src` which only runs the type-check pass. The type checker is permissive enough to accept generic call sites whose argument types it cannot resolve, deferring the error to monomorphization or compilation. Tests therefore passed even when monomorphization could not specialize the call. A new `compile_src` helper exercises the full compile pipeline so a missing inference path produces an actionable test failure.

With the more rigorous test, two real inference gaps surfaced. `Expr::TupleIndex` and `Expr::ArrayIndex` returned `None` from `infer_arg_type`. Both are now resolved: tuple-index reads the element type from the inferred tuple's element list, and array-index reads the element type from the inferred array type. New tests exercise each shape end to end through the compile pipeline.

The existing field-access test was also refactored to use `compile_src` so it actually verifies monomorphization rather than just type-check.

### B3 recursive closures via let-binding

The form `let f = |...| ... f(...) ...` declares a closure whose body references its own let-binding name. Previously this failed with "undefined function `f`" at type-check.

The new flow works as follows:

- The type checker registers a fresh type variable for the let-binding name before walking the closure value, so the body's self-reference type-checks rather than failing as undefined.
- The hoist pass detects the pattern in `Stmt::Let` when the value is `Expr::Closure` and the binding name appears in the body's free variables. The synthetic chunk's parameter list is laid out as `(other_captures..., self_param, explicit_params...)` where `self_param` carries the binding name. Other captures are populated normally, and the resulting `Expr::ClosureRef` carries `recursive = true`.
- The compiler emits the new `Op::MakeRecursiveClosure(chunk_idx, n_captures)` for recursive `ClosureRef`s. This produces a `Value::Func { recursive: true, ... }`. The compiler also accepts captures that resolve to top-level functions or already-hoisted synthetic chunks via `Op::PushFunc`, which is needed for nested closures whose inner capture is an outer closure's binding name.
- The runtime extends `Op::CallIndirect`. When the popped `Value::Func` has `recursive == true`, the VM pushes the closure value itself onto the operand stack between the env values and the explicit arguments, populating the synthetic chunk's self parameter. References to the binding name inside the body resolve to the local that holds the closure and dispatch through indirect call.
- Recursive closures coexist with regular captures. The synthetic chunk receives `(captures..., self, explicit_params...)`. The existing transitive capture mechanism continues to work because the recursive marker only affects how `CallIndirect` handles the self slot.

`BYTECODE_VERSION` bumped to `7`. The golden bytes test was updated accordingly.

End-to-end demonstration: `examples/closure_recursive.rs` computes `fact(5) == 120`.

## Tests

Four new typecheck tests:

- `monomorphize_inference_through_tuple_index` covers the tuple-index inference case end to end through the compile pipeline.
- `monomorphize_inference_through_array_index` covers the array-index inference case.
- `recursive_closure_typechecks` covers the basic recursive closure pattern.
- `recursive_closure_with_capture_typechecks` covers a recursive closure that also captures an outer-function local.

The existing `monomorphize_inference_through_field_access` test was refactored to use `compile_src` rather than `check_src`.

One new example: `examples/closure_recursive.rs`.

## Trade-offs and Properties

The recursive closure implementation requires runtime support (a new opcode and a `Value::Func` flag). The alternative (a body rewrite that threads the closure value as an explicit argument at every recursive call) avoids the runtime change but requires synthetic transformations across the body. The runtime approach is simpler structurally and concentrates the change in the dispatch path.

The `recursive` flag on `Value::Func` adds a single boolean per first-class function value at runtime. The cost is negligible for an `enum` Value with multiple existing fields. The flag is `false` for plain function references produced by `Op::PushFunc` and for non-recursive closures produced by `Op::MakeClosure`, so existing code continues to behave identically.

The hoist pass's detection of self-reference is conservative: it only fires when the let-binding's pattern is a single `Pattern::Variable` and the value is exactly an `Expr::Closure`. Indirect cases (a closure assigned to a destructured pattern, or a closure passed through other expressions) are not detected and produce the original "undefined function" error. This is acceptable because the canonical pattern covers the practical use cases.

The type checker's pre-registration of a fresh type variable is similarly scoped. The variable is later overwritten by the actual closure type when `bind_pattern` runs at the end of let-checking. Within the closure body, the binding's type is the fresh variable, so call sites unify against it but cannot fully resolve the closure's signature. The compile pipeline's monomorphization and indirect-call dispatch then handle the runtime resolution.

## Changes Made

### Source

- **`src/bytecode.rs`**. New `Op::MakeRecursiveClosure(u16, u8)` variant. New `recursive: bool` field on `Value::Func`. WCMU cost, stack growth/shrink, and `op_from_archived` updated for the new variant. `BYTECODE_VERSION` bumped to `7`.
- **`src/vm.rs`**. New `Op::MakeRecursiveClosure` execution arm. `Op::CallIndirect` extended: when the popped `Value::Func` is recursive, the VM pushes the closure value itself between env values and explicit arguments. Golden bytes test updated for version 7.
- **`src/utility_natives.rs`**. `render_value_to_string` recognizes the recursive flag and renders accordingly.
- **`src/ast.rs`**. `Expr::ClosureRef` gained `recursive: bool`.
- **`src/typecheck.rs`**. `Stmt::Let` arm now pre-registers a fresh type variable for the binding when the value is a closure and the pattern is a simple variable. Three new tests covering tuple-index, array-index, and recursive closure cases. New `compile_src` helper. Existing inference test refactored to use `compile_src`.
- **`src/compiler.rs`**. New `hoist_let_bound_closure` helper that produces recursive `ClosureRef`s. The compiler's `ClosureRef` arm emits `Op::MakeRecursiveClosure` when the closure is recursive and falls back to top-level function references via `Op::PushFunc` when a capture name resolves to a function rather than a local.
- **`src/monomorphize.rs`**. `infer_arg_type` `Expr::TupleIndex` and `Expr::ArrayIndex` arms now return inferred element types. The `subst_in_expr` arm for `ClosureRef` propagates the new `recursive` field.
- **`examples/closure_recursive.rs`** (new). End-to-end demonstration of `fact(5) == 120`.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2.4 entry updated with the tuple-index and array-index inference cases. B3 entry updated with the recursive-closure mechanism, including the runtime contract and bytecode version bump.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T26.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None of the originally named B2.2, B2.3, B2.4, or B3 items remain unresolved. The closure subsystem now supports first-class arguments, environment capture, transitive nested capture, and recursive let-bound closures. The monomorphization pass resolves type arguments through literals, identifiers, function-call returns, casts, enum variants, struct constructions, tuple and array literals, if and match arms, field access, tuple-index, and array-index expressions.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding. Subsequent work falls outside the previously named B2.2, B2.3, B2.4, and B3 scope. Candidates include B11 (per-op decode optimization) or B5b (variable-cost string operations) from the backlog.

## Session Context

This session closed the residual TupleIndex/ArrayIndex inference gaps in B2.4 and added full recursive closure support to B3. The bytecode wire format advanced to version 7. The closure subsystem is now feature-complete for the canonical functional patterns (recursion, capture, nesting, first-class arguments).
