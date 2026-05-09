# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T4. P2 for-in over typed expressions.
**Status**: Complete for typed cases. P2 resolved with one deferred case.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 366 tests pass workspace-wide. 321 keleusma unit including 3 new for typed for-in cases, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The compiler now infers the static array length of for-in source expressions from typed expressions and emits a `Const(N)` end bound. The strict-mode WCMU verifier accepts this pattern, removing the previous restriction that for-in worked only over array literals.

The cases now admissible under strict-mode WCMU.

- Array literal. `for x in [1, 2, 3]`. Length from element count.
- Let-bound array literal. `let arr = [1, 2, 3]; for x in arr`. Length traced through the local alias chain to the originating `NewArray`. This case worked before and remains admissible.
- Function return. `for x in make()` where `make` returns `[T; N]`. Length from the declared return type.
- Data segment field. `for x in ctx.items` where `items` is declared `[T; N]`. Length from the data block declaration.

Implementation. A new `TypeInfo` struct is collected in `compile()` from the AST. It maps struct names to field types, function names to return types, and data block names to field types. The `FuncCompiler::static_for_in_length` helper consults `TypeInfo` to extract the iteration bound when the for-in source is a function call, a field access on a data block, or an array literal. When the length is known the compiler emits `Const(N), SetLocal(end_slot)` rather than the previous `GetLocal(arr), Len, SetLocal(end_slot)` pattern. The strict-mode WCMU verifier (R38) accepts the constant-bound pattern through its existing `trace_const_set_local` analysis.

## Deferred Case

For-in over struct field access from a local variable.

```
struct Box { items: [i64; 3] }
fn main() -> i64 {
    let b = Box { items: [1, 2, 3] };
    let n = 0;
    for x in b.items { /* ... */ }
    n
}
```

This case is rejected by the strict-mode WCMU verifier. The compiler does not track the type of local variable `b`. Without that type information, the `b.items` field access cannot be resolved to a typed array. The fix is to add local variable type tracking in the compiler, which the type checker (P1) already has the information for. The type checker runs before the compiler and discards its computed types. Plumbing the type info from `typecheck` into the compiler so that `let` bindings carry their declared or inferred type would close this case. This is recorded as future enhancement work.

## Changes Made

### Source

- **`src/compiler.rs`**: New `TypeInfo` struct with structs, function returns, and data field types. `FuncCompiler` gains a `type_info` field and two helper methods `static_for_in_length` and `struct_name_of`. New free function `array_length_of_type` extracts the length from `TypeExpr::Array`. `compile()` builds `TypeInfo` from the program AST and passes it through `compile_function_group` to `FuncCompiler::new`. `compile_for(Iterable::Expr)` now consults `static_for_in_length` and emits `Const(N)` when known. The existing test `compile_for_in_array` was updated to assert the new pattern (no `Op::Len` for array literals).
- **`src/vm.rs`**: Three new tests. `for_in_over_function_return_passes_strict_verify` confirms a typed function return path verifies cleanly. `for_in_over_data_segment_field_passes_strict_verify` confirms data segment field access verifies. `for_in_over_array_literal_runs` confirms end-to-end execution with the new pattern.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**: P2 marked resolved for typed cases. The deferred struct-field-from-local case is documented as future work.
- **`docs/process/TASKLOG.md`**: V0.1-M3-T4 row added. New history row.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The change is local to the compiler. The verifier and the runtime are unchanged. The wire format is unchanged. The `Const(N)` pattern that the compiler emits is identical to the canonical for-range pattern that the verifier already recognizes, so no verifier changes were needed.

The compiler now traces type information from the AST during compilation. This duplicates some work the type checker does. A future iteration could unify the two by exposing the type checker's computed context to the compiler. The duplication is bounded for now because the compiler only consults a small subset of type information (struct field types, function return types, data field types) used for the for-in optimization.

The fall-back to `Op::Len` is preserved for cases where the length is not statically known. This means the compiler still emits valid bytecode for sources whose lengths are runtime-determined. The strict-mode verifier rejects these, but the runtime would execute them correctly if the verifier were configured leniently.

## Unaddressed Concerns

1. **Struct field access from a local variable.** Not yet handled. Documented as future enhancement that requires local type tracking.

2. **Nested array access.** Cases like `for x in matrix[0]` where `matrix` is `[[T; N]; M]` are also not handled. Same root cause as the local struct field case.

3. **Type-tracked compilation more broadly.** The `TypeInfo` struct holds only the subset needed for P2. A richer compiler-side type context could enable additional optimizations and tighter verification, but that work is admissible as a separate refactor.

## Intended Next Step

Three paths.

A. Pivot to P7 follow-on (operand stack and DynStr arena migration). Closes the bounded-memory guarantee end to end.

B. Publish keleusma main crate to crates.io now that P1, P2, P3, and P10 are resolved.

C. Tackle the deferred for-in case by adding local type tracking in the compiler.

Recommend B if external visibility is the priority. Recommend A if the bounded-memory guarantee is load-bearing for upcoming use cases. Recommend C if completing for-in coverage is the priority.

Await human prompt before proceeding.

## Session Context

This long session resolved P10 across all phases (rkyv format, in-place validation, archive converters, full Vm refactor with Vm<'a>, true zero-copy execution, include_bytes example), P1 (type checker pass plus pipeline integration), P3 (explicit error recovery via reset_after_error), and now P2 for typed for-in cases. The remaining open priority items are P7 follow-on and the deferred for-in case.
