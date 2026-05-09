# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T16. B2.2 follow-on and B2.3 traits + bounds.
**Status**: Complete. Generic types compose with field/parameter/return positions, and trait declarations with bound enforcement at call sites are functional. Method dispatch and impl-method-vs-trait validation are deferred.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 454 tests pass workspace-wide. 386 keleusma unit (8 new for B2.2 follow-on and B2.3), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session delivered the B2.2 follow-on (`TypeExpr::Named` carries generic arguments) and B2.3 trait declarations with bound enforcement.

B2.2 follow-on. The previous session's generic structs and enums could not be referenced as field, parameter, or return types because `TypeExpr::Named` only carried a name. Extended to `TypeExpr::Named(String, Vec<TypeExpr>, Span)`. The parser accepts `Cell<T>` syntax in any type position. The type checker resolves generic arguments through `from_expr_with_params` so they participate in unification. Pattern matching on a generic enum payload binds the variable to the per-instance instantiation correctly.

B2.3 traits and bounds. Surface syntax `trait Name<T> { fn method(args) -> ret; }` and `impl Trait for Type { method definitions }`. New `Trait` and `Impl` keywords. New `TraitDef`, `ImplBlock`, and `TraitMethodSig` AST nodes. `TypeParam` carries `bounds: Vec<String>` populated from `<T: Trait1 + Trait2>` syntax with the `+` separator for multiple bounds. `FnSig` records `type_param_bounds` parallel to `type_param_vars`.

Bound enforcement at call sites. `instantiate_sig` now returns the per-call fresh type variables alongside the instantiated parameter and return types. After argument unification records the substitution, each bounded fresh variable is resolved through the active substitution and the resulting head type checks against the trait `impls` registry. Types lacking an impl are rejected with a precise error.

Method dispatch deferred. Impl method bodies are parsed and stored but not yet wired through the compiler. Receiver-style calls `x.method(args)` resolving to the impl for `x`'s type are next-session work. The bound enforcement prevents incorrect calls at compile time; the method invocation itself awaits the dispatch implementation.

## Tests

Eight new unit tests in `src/typecheck.rs`.

B2.2 follow-on (2):

- `generic_struct_pattern_match_on_enum`. Pattern matching on `Maybe<T>::Just(x)` binds `x` to the instantiated payload type and the match expression returns the correct type.
- `generic_struct_referenced_by_field_type`. A generic struct used as a field type inside another struct (`struct Wrap<T> { inner: Cell<T> }`) parses and type-checks.

B2.3 (6):

- `trait_declaration_parses_and_typechecks`. Minimal trait, impl, and bounded function compile and run.
- `trait_bound_satisfied_by_impl`. Bound is admitted when the impl exists.
- `trait_bound_unsatisfied_rejects_call`. Bound is rejected when the impl does not exist for the call's argument type.
- `unbounded_type_param_admits_any_type`. Without bounds, any concrete argument is accepted.
- `multiple_trait_bounds_on_one_param`. `T: A + B` admits a type that implements both.
- `missing_one_of_multiple_bounds_rejected`. `T: A + B` rejects a type that implements only one.

One new example, `examples/generic_match.rs`, demonstrates pattern matching on a generic enum and nested generic structs running end to end.

## Trade-offs and Properties

The B2.3 implementation enforces bounds at call sites but does not yet dispatch trait methods at runtime. This means a function `fn use_tag<T: Tag>(x: T) -> i64` can be type-checked and called with a constrained `T`, but if its body invokes `x.tag()`, the call would not yet resolve to the impl-defined method. The current value of the trait machinery is the static type-level guarantee; the runtime dispatch is the next layer.

Multiple bounds are stored as a flat `Vec<String>`. A future enhancement would deduplicate or canonicalize the order, but the current code admits duplicates without harm.

The `type_head_name` helper canonicalizes `Type` to its impl-key string (`i64`, `Pair`, etc.). Tuples, arrays, and options receive uniform names (`tuple`, `array`, `Option`) which means an impl of `Tag for tuple` would match any tuple regardless of arity. This is a coarse approximation; future work refines it once method dispatch lands.

## Changes Made

### Source

- **`src/token.rs`**. New `Trait` and `Impl` keyword tokens.
- **`src/ast.rs`**. `TypeExpr::Named` carries `Vec<TypeExpr>` of generic arguments. New `TraitDef`, `ImplBlock`, and `TraitMethodSig` types. `TypeParam` carries `bounds: Vec<String>`. `Program` carries `traits` and `impls` fields.
- **`src/parser.rs`**. New `parse_trait_def` and `parse_impl_block`. `parse_type_param` parses optional bounds (`: Trait1 + Trait2`). `parse_optional_type_params` shared between functions, structs, and enums. Generic-argument parsing in named type positions.
- **`src/typecheck.rs`**. New `traits` and `impls` maps in `Ctx`. Pass 1d registers trait declarations and impl blocks. `FnSig` carries `type_param_bounds`. `instantiate_sig` returns the per-call fresh variables. Call-site bound enforcement after unification. New `type_head_name` helper. `from_expr_with_params` resolves named generic arguments through the type-param mapping.
- **`src/compiler.rs`**. Pattern updates for the extended `TypeExpr::Named` shape.
- **`examples/generic_match.rs`**. End-to-end demonstration of pattern matching on generic enums and nested generic structs.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2 entry rewritten to record functions, structs, enums, traits, and bounds as resolved, with method dispatch and monomorphization documented as remaining work.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T16.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None. P1 through P10 fully resolved. B1 resolved. B2 resolved for declarations and bound enforcement.

The remaining items the user listed in the most recent prompt.

- Trait method dispatch. Wires `x.method(args)` through the compiler and runtime so impl-defined methods can actually be invoked. Pairs naturally with the just-landed bound enforcement. Estimated 4 to 6 hours.
- Compile-time monomorphization for performance. Specializes each generic chunk per (function or type, type_args) pair and elides the runtime tag-dispatch cost. Estimated 4 to 8 hours.
- B3 closures and anonymous functions. Independent of the generics track. Adds closure syntax, environment capture in the VM, and a closure type representation. Estimated 6 to 10 hours.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

The natural next session is trait method dispatch, which closes the trait loop end to end. Then compile-time monomorphization for performance. B3 closures can land in any session as an independent feature.

Await human prompt before proceeding.

## Session Context

This long session resolved the B2.2 follow-on (`TypeExpr::Named` generic arguments) and the B2.3 trait declaration and bound enforcement slice. The type checker now performs Hindley-Milner inference, supports generic functions, structs, and enums with per-instance type arguments, and validates trait bounds at call sites against an impl registry. The runtime continues to dispatch polymorphically on `Value` tags. Trait method dispatch and compile-time monomorphization remain as next-session work.
