# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T14. B1 deferred work and B2 generic functions.
**Status**: Complete. Generic functions are supported end to end. B1 deferred substitution-apply landed. Remaining B2 work (generic structs/enums, trait bounds, monomorphization) is documented as future work.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 441 tests pass workspace-wide. 373 keleusma unit (8 new for B1 deferred and B2 generics), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session resolved B1 deferred work and added B2 generic functions to Keleusma.

B1 deferred. The substitution-application pass landed at end of `check_function`. Locals and the function's `FnSig` are resolved to their inferred types after the body is checked. A per-function snapshot rolls back type variables so cross-function checking remains independent. The `Type::Unknown` sentinel is retained as the permissive transitional anchor for runtime-only dispatch positions such as native function results without declared signatures.

B2 generic functions. Surface syntax `fn name<T, U>(args) -> ret { body }` is parsed and represented in the AST through a new `TypeParam` type and a `type_params: Vec<TypeParam>` field on `FunctionDef`. The lexer reuses the existing `Lt` and `Gt` tokens. The type checker records the abstract `Type::Var` allocated per type parameter in `FnSig::type_param_vars`. Call sites use the new `instantiate_sig` helper to substitute abstract variables with fresh per-call variables before unifying with actual arguments. Two distinct call sites of the same generic function instantiate independently, so the same generic function flows through different concrete types in the same module.

Compilation and runtime. Keleusma's `Value` enum is runtime-tagged. Bytecode operations dispatch on the tag, so a generic chunk that flows values through unchanged works for any concrete type without compile-time monomorphization. The `examples/generic_identity.rs` demonstrates `fn id<T>(x: T) -> T { x }` compiling and executing end to end.

The compile-time monomorphization pass that was originally scoped turned out to be a performance optimization rather than a correctness requirement for Keleusma's design. The runtime-tagged `Value` enum naturally accommodates polymorphic dispatch. Specializing chunks per type instantiation would elide the runtime tag dispatch cost but is not necessary for the language to support generic functions.

## Tests

Eight new unit tests cover the new functionality.

Five parser tests in `src/parser.rs`.

- `parse_fn_with_single_type_param`. `fn id<T>(x: T) -> T { x }` parses with one type parameter.
- `parse_fn_with_multiple_type_params`. `fn pair<T, U>(...)` parses with two.
- `parse_fn_with_trailing_comma_in_type_params`. Trailing comma admitted.
- `parse_fn_empty_type_params_accepted`. The trivial empty list is accepted.
- The existing `parse_fn_definition` test extended to assert `type_params.len() == 0` for non-generic functions.

Four typecheck tests in `src/typecheck.rs`.

- `generic_identity_function_typechecks`. The minimal generic function passes type checking with a concrete call site.
- `generic_function_called_with_two_types_separately`. Two distinct call sites instantiate the type parameter independently.
- `generic_function_with_two_type_params`. Multiple type parameters work.
- `generic_function_arity_mismatch_rejected`. Arity errors still surface for generic functions.

One example program at `examples/generic_identity.rs` demonstrates end-to-end compilation and execution.

## Trade-offs and Properties

The B1 substitution-application pass is silent on unresolved type variables. Reporting them as inference failures would break the permissive treatment of native function results, which currently allow field access and other operations without static signatures. Moving to strict reporting would require declaring native signatures, which is recorded as future work.

The B2 design relies on runtime tag dispatch for polymorphism. Operations that constrain `T` to a specific shape (such as arithmetic that requires numeric `T`) currently fail at runtime with a `VmError::TypeError` rather than at compile time. Static enforcement would require trait bounds, which are tracked separately under future B2 follow-on work.

The session did not implement compile-time monomorphization. The implementation revealed that the existing runtime-polymorphic dispatch handles generic functions naturally. Monomorphization is documented as a future optimization for performance.

## Changes Made

### Source

- **`src/ast.rs`**. New `TypeParam` struct with `name` and `span`. `FunctionDef` gains `type_params: Vec<TypeParam>`.
- **`src/parser.rs`**. `parse_function_def` accepts an optional `<T, U>` block after the function name. New `parse_type_param` helper. Five parser tests added.
- **`src/typecheck.rs`**. The B1 substitution-application pass at end of `check_function`. New `instantiate_sig` helper for generic call-site instantiation. New `Type::from_expr_with_params` resolves type-parameter names against a mapping. `FnSig` gains `type_params: Vec<String>` and `type_param_vars: Vec<Type>` fields. Pass 1c constructs generic signatures with fresh abstract variables per type parameter. Four typecheck tests added.
- **`examples/generic_identity.rs`**. Demonstration of end-to-end execution of `fn id<T>(x: T) -> T { x }`.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B1 entry rewritten as resolved. B2 entry rewritten as resolved-for-functions with remaining work documented.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T14.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None. P1 through P10 fully resolved. B1 resolved. B2 resolved for generic functions.

The remaining future work tracked under B2 follow-on.

- Generic struct and enum declarations.
- Trait declarations and trait bounds for static enforcement of type-parameter constraints.
- Compile-time monomorphization for performance.

The `keleusma-arena` registry version is still v0.1.0 and the local crate has new APIs. Publishing the main `keleusma` crate to crates.io requires either bumping `keleusma-arena` to v0.2 first or accepting the path dependency.

## Intended Next Step

Three reasonable directions.

A. B2 follow-on for generic structs and enums. The same machinery used for generic functions extends naturally; the work is in parser, AST, and typecheck. Estimated 4 to 6 hours.

B. Trait declarations and bounds. Adds static enforcement to currently runtime-checked constraints. Larger feature; estimated 8 to 12 hours.

C. Publish the workspace to crates.io.

Await human prompt before proceeding.

## Session Context

This long session resolved P7 across all nine items, B9, the float width portability prep, B1 (foundation and deferred work), and B2 (generic functions). The type checker now performs Hindley-Milner inference and supports generic function signatures. The runtime polymorphism of `Value` makes monomorphization optional rather than required.
