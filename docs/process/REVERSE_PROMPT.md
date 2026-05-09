# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T18. B2.3 method call dispatch and B2.4 monomorphization MVP.
**Status**: Complete. Method calls dispatch through impl-defined functions for concrete receivers, and monomorphization specializes generic functions per concrete type so generic-receiver method calls resolve through the specialized chunk.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 460 tests pass workspace-wide. 392 keleusma unit (3 new for method dispatch and monomorphization), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session closed B2.3 method call dispatch and landed the B2.4 monomorphization MVP.

B2.3 method call dispatch. New `Expr::MethodCall` variant in the AST. The parser distinguishes field access from method call by looking ahead for `(` after `expr.name`. The type checker resolves the receiver type, takes its head (`i64`, `Pair`, etc.), and looks up the registered impl method `Trait::Head::method` in `ctx.functions`. The compiler's `MethodCall` arm uses `infer_expr_type` plus a new `type_expr_head` helper to resolve the receiver's head and search the function map for any entry ending with `::Head::method`. The receiver is implicitly the first argument; arity checks adjust accordingly. Impl method bodies are now also type-checked under their mangled names so parameter and return type lookups resolve correctly.

B2.4 monomorphization MVP. New `src/monomorphize.rs` module exposes a `monomorphize` pass that runs between type checking and compilation in the `compile` entry point. The pass walks call sites of generic functions, infers concrete type arguments from literal arguments and identifiers with declared types, and generates specialized `FunctionDef` instances per `(function, type_args)` pair through type substitution in the parameter list, return type, and body. Specialization names use double-underscore mangling such as `use_doubler__i64`. After specialization, the compile pipeline re-runs the type checker so specialized bodies benefit from concrete-type method resolution. Generic functions whose specialization was generated are dropped from the output, leaving only the specialized chunks.

End-to-end demonstration. `examples/monomorphize_generic_method.rs` compiles and executes `fn use_doubler<T: Doubler>(x: T) -> i64 { x.double() }` where the body's method call resolves only after monomorphization specializes `use_doubler` for `T = i64`. The result is 42 from `use_doubler(21)`. The receiver-style call `21.double()` still works directly without monomorphization because the receiver type is concrete at the call site.

## Tests

Three new unit tests in `src/typecheck.rs`.

- `method_call_resolves_to_impl`. The minimal case: concrete-receiver method dispatch resolves to the impl-defined function.
- `method_call_unknown_method_rejected`. A method that no impl provides is rejected with a clear error.
- `monomorphize_generic_method_dispatch`. The end-to-end case: a generic function with a `T: Trait` bound calls a trait method on its parameter, and monomorphization specializes the function so the method call resolves.

Two new examples.

- `examples/method_call.rs`. Concrete-receiver dispatch (`21.double()` returns 42).
- `examples/monomorphize_generic_method.rs`. Monomorphization-driven dispatch.

## Trade-offs and Properties

The monomorphization MVP handles the common case but leaves several follow-ons.

- Generic structs and enums. The pass specializes only generic functions. Generic struct construction and field access continue to use runtime tag dispatch. Specialization of structs would require emitting per-instantiation copies of struct templates and rewriting field offsets accordingly.
- Inference reach. The MVP infers concrete type arguments from literal arguments and locally-declared identifiers. Type arguments that flow through chains of function calls or through generic-receiver method results are not yet handled and the call site is left generic.
- Polymorphic recursion guard. If a generic function calls itself with type arguments derived from its own type parameters, the pass would loop indefinitely. The MVP relies on the call graph being finite and the test suite not exercising polymorphic recursion. A guard against unbounded specialization should be added before the feature is considered production-ready.
- Pruning unused generic functions. The MVP retains generic functions that did not get specialized; they remain as dead code that the compiler emits but the runtime never enters.

The B2.3 method dispatch landed without surface syntax for `Trait::method(x)`-style explicit dispatch through Keleusma's path syntax. Calls go through either receiver-style `x.method(args)` resolved at compile time, or are routed through the mangled name by monomorphization for generic-body calls. The two paths cover the common cases.

## Changes Made

### Source

- **`src/ast.rs`**. New `Expr::MethodCall` variant.
- **`src/parser.rs`**. Postfix expression parser distinguishes field access from method call by looking ahead for `(` after `expr.name`.
- **`src/typecheck.rs`**. `MethodCall` arm in `type_of_expr`. Impl method bodies type-checked under mangled names. Three new unit tests.
- **`src/compiler.rs`**. `MethodCall` arm in `compile_expr`. New `type_expr_head` helper. Function group set extended to include impl methods under mangled names. Compile pipeline runs `monomorphize()` between type check and compilation, then re-runs type check on the monomorphized program.
- **`src/monomorphize.rs`** (new). Module exposes `monomorphize(Program) -> Program`. Implements call-site rewriting, type substitution in expressions and statements, specialization generation, and dead-generic pruning.
- **`src/lib.rs`**. New `monomorphize` module declaration.
- **`examples/method_call.rs`** (new). Demonstrates concrete-receiver method dispatch.
- **`examples/monomorphize_generic_method.rs`** (new). Demonstrates monomorphization-driven dispatch.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2.4 entry rewritten as MVP-landed with remaining-work list.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T18.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None. P1 through P10 fully resolved. B1 resolved. B2 resolved for declarations, bound enforcement, impl signature validation, method dispatch, and monomorphization MVP.

The remaining items.

- B2.4 follow-on. Generic structs and enums monomorphization. Inference reach extension. Polymorphic recursion guard. Pruning of unused generic functions. Estimated 5 to 10 hours total across these.
- B3 closures and anonymous functions. Independent feature. Estimated 6 to 10 hours.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

The natural next step is B3 closures and anonymous functions, which is an independent feature with clear scope. Alternatively the B2.4 follow-on tightens the monomorphization pass.

Await human prompt before proceeding.

## Session Context

This session closed B2.3 (method call dispatch through receiver-style syntax) and delivered the B2.4 monomorphization MVP. Generic functions with concrete-arg call sites are now specialized at compile time, and method calls inside generic function bodies resolve to the impl's mangled function through the specialized version. The example `examples/monomorphize_generic_method.rs` demonstrates the full path end to end.
