# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T17. B2.3 deferred (impl method registration and signature validation) plus B2.4 monomorphization design.
**Status**: Complete for B2.3 deferred work. B2.4 monomorphization is documented as a four-phase design with implementation deferred to a focused future session.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 457 tests pass workspace-wide. 389 keleusma unit (3 new for impl signature validation), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session delivered the B2.3 deferred work and recorded a clear B2.4 monomorphization design. The implementation of monomorphization itself is deferred to a focused future session because it touches the compiler and runtime substantially.

B2.3 deferred. Impl methods register in `ctx.functions` under the mangled name `Trait::TypeHead::method`. The compiler folds impl methods into the function group set under their mangled names so they emit as regular bytecode chunks. Pass 1e validates impl method signatures against the trait declaration: each impl method must match a trait method by name, the impl block's trait reference must resolve to a declared trait, and the parameter arity must agree. Three new typecheck tests cover the rejection paths for unknown traits, unknown methods, and arity mismatches.

B2.4 design. The four-phase plan in `docs/decisions/BACKLOG.md`:

- Phase 1. Call-graph traversal from `main` records each unique `(function, type_args)` pair encountered.
- Phase 2. Specialization generation clones the function body and substitutes the abstract type-parameter variables with the concrete types throughout. The specialization name suffixes the original with the canonical encoding of the type args.
- Phase 3. Trait method resolution within specializations rewrites every use of a trait method on a known-concrete type to the impl's mangled name (`Trait::TypeHead::method`).
- Phase 4. Output emits only the monomorphic specializations. Generic functions are dropped.

Method call surface syntax `x.method(args)` folds into B2.4 because monomorphization-rewriting subsumes the explicit dispatch. After monomorphization, the receiver type is concrete and the call rewrites directly.

## Tests

Three new unit tests in `src/typecheck.rs`.

- `impl_method_with_extra_method_rejected`. An impl method that does not appear in the trait declaration is rejected.
- `impl_method_arity_mismatch_rejected`. An impl method whose parameter count differs from the trait declaration is rejected.
- `impl_for_unknown_trait_rejected`. An impl referencing an undeclared trait is rejected.

## Trade-offs and Properties

The B2.3 deferred work landed without surface syntax for receiver-style method calls (`x.method(args)`). The mangled-name calls registered in the function map are not currently parseable because the parser's path syntax requires upper-case path segments and primitives like `i64` are lower-case. This is a deliberate scoping choice: monomorphization is the cleanest path to method dispatch, and once it lands, generic call sites rewrite to the mangled names automatically without needing receiver syntax to be parseable.

Impl signature validation in this session is structural (arity and name) rather than full type compatibility. Full validation would unify impl parameter types against the trait's declared types under a `Self = impl_for_type` substitution. This is recorded as next-session work but does not block typical use cases.

The B2.4 monomorphization design records the work in four phases. The implementation is non-trivial because it cascades through compiler and runtime: rewriting calls in compiled chunks, generating specialized chunks, possibly emitting type-arg-specialized struct field offsets. The session budget did not allow for a clean implementation that would not regress existing tests.

## Changes Made

### Source

- **`src/typecheck.rs`**. Pass 1d extended to register impl methods under mangled names. New Pass 1e validates impl signatures against trait declarations. Pass 2 also checks impl method bodies as ordinary functions. Three new tests.
- **`src/compiler.rs`**. Function groups now include impl methods under their mangled names through synthetic FunctionDef clones. The original impl methods are not emitted as their declared names.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B2 entry rewritten to record impl method registration and signature validation as resolved. B2.4 monomorphization promoted to a top-level entry with a detailed four-phase design.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T17.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None. P1 through P10 fully resolved. B1 resolved. B2 resolved for declarations, bound enforcement, and impl signature validation.

The remaining items the user listed.

- B2.4 compile-time monomorphization. Designed and ready for implementation. Estimated 4 to 8 hours for generic functions plus 2 to 4 hours for trait method resolution within specializations.
- B3 closures and anonymous functions. Independent feature. Estimated 6 to 10 hours.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Three reasonable directions.

A. B2.4 monomorphization implementation. Pairs with the just-landed impl method registration to deliver end-to-end trait method dispatch.

B. B3 closures and anonymous functions. Independent feature. Adds environment capture in the VM.

C. Publish the workspace to crates.io.

Await human prompt before proceeding.

## Session Context

This session completed B2.3 deferred work (impl method registration in compiler and typecheck plus signature validation) and recorded a comprehensive design for B2.4 monomorphization. The type checker now correctly rejects malformed impls. The compiler emits impl methods as chunks ready for direct invocation by mangled name. Monomorphization implementation is deferred because it touches both compiler and runtime substantially.
