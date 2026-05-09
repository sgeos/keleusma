# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T19. B2.4 follow-on, B2.3 follow-on, B3 closures (parser, AST, type-check).
**Status**: Complete for the slice committed. Closure runtime support is deferred to a focused future session.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 465 tests pass workspace-wide. 397 keleusma unit (5 new across B2.3, B2.4, B3), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session closed the named B2.x follow-on items and started B3 closures with a parser and type-check slice. Closure runtime support is documented as next-session work.

B2.4 polymorphic recursion guard. The monomorphization pass now bounds the number of specializations through `SPECIALIZATION_LIMIT = 1024`. Programs that would expand specializations unboundedly through polymorphic recursion break out of the fixed-point loop with the partial set rather than entering an infinite loop. The bound is generous; legitimate programs reach a fixed point well below it.

B2.4 prune unspecialized generics. After the specialization pass, every function with non-empty `type_params` is dropped from the program output. Call sites that should have been monomorphized but were not surface as compile-time errors rather than emitting dead-code chunks. The previous behavior retained generic functions whose specializations were generated but left the rest as silent dead code.

B2.3 full impl-vs-trait signature validation. Pass 1e in the type checker validates each impl method's parameter types and return type against the trait declaration's structurally, not just arity. The check resolves both sides through `Type::from_expr` against the same `ctx.types` registry and rejects mismatches with precise error messages. Two new typecheck tests cover parameter type and return type rejection.

B3 closures. Surface syntax `|args| body` and `|args| -> ret { body }` is parsed. The lexer distinguishes the bare `|` token (`Bar`) from the pipeline operator `|>` (`Pipe`). The AST carries `Expr::Closure { params, return_type, body, span }`. The type checker walks the closure body in a fresh parameter scope where each parameter binds to a fresh type variable or its declared type. The closure expression itself produces a fresh type variable; first-class function types are tracked under future B3 follow-on work. Monomorphization recurses through closure bodies during substitution and call-site rewriting.

The runtime side of B3 is deferred. The compiler rejects closure expressions at compile time with a clear "closure runtime is not yet implemented" message. The syntax surface is stable while the implementation evolves.

## Tests

Five new unit tests this session.

- `impl_method_param_type_mismatch_rejected`. Trait declares `fn double(x: i64) -> i64`; impl supplies `fn double(x: bool) -> i64`. Rejected.
- `impl_method_return_type_mismatch_rejected`. Trait declares `-> i64`; impl returns `bool`. Rejected.
- `parse_closure_no_params_no_body`. `|| 42` parses with empty params and a single-tail-expression block.
- `parse_closure_with_one_param`. `|x: i64| x + 1` parses with one declared param.
- `parse_closure_with_block_body`. `|x: i64| -> i64 { x * 2 }` parses with explicit return type and a brace block.

## Trade-offs and Properties

The closure runtime is the substantive remaining piece. Without it, closures are parser-only constructs. Adding the runtime requires a new `Value::Func(u16)` variant, an indirect-call op, environment capture, and resolution of `f(args)` where `f` is a closure-typed local. Estimated 6 to 10 hours of focused work.

The polymorphic recursion guard uses a hard limit rather than a static analysis of cycle structure. The hard limit is simple, predictable, and adequate for the practical case where bounded recursion produces a small specialization set. A future enhancement could detect cycles in the call graph and reject before allocating specializations.

The prune-generics pass is now strict: any retained call to a generic function name surfaces as a compile error during the post-monomorphization compilation. This is a deliberate trade-off; previously the silent dead-code emission masked monomorphization gaps that the user could not see without reading the bytecode.

## Changes Made

### Source

- **`src/token.rs`**. New `Bar` token for the bare `|`.
- **`src/lexer.rs`**. Bare `|` lexes to `Bar`; the previous "unexpected `|`" error path is removed. The `error_bare_pipe` test renamed and updated to `bare_pipe_is_bar`.
- **`src/ast.rs`**. New `Expr::Closure { params, return_type, body, span }` variant. `Expr::span` extended to cover it.
- **`src/parser.rs`**. Closure literal parsing in `parse_primary_expr`. Three new parser tests.
- **`src/typecheck.rs`**. Pass 1e validates impl method parameter and return types structurally against the trait declaration. Closure expressions type-check by walking the body in a fresh parameter scope. Two new typecheck tests for impl signature validation.
- **`src/compiler.rs`**. Closure expressions reject at compile time with a clear deferred-runtime error.
- **`src/monomorphize.rs`**. `SPECIALIZATION_LIMIT = 1024` bounds the fixed-point loop. Generic functions are dropped from the program output after specialization. Substitution and call-site rewriting recurse through closure bodies.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B3 entry rewritten to record the parser, AST, and type-check slice as resolved, with the runtime work documented.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T19.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None. P1 through P10 fully resolved. B1 resolved. B2 fully resolved with monomorphization MVP and follow-ons. B3 parser, AST, and type-check landed.

The remaining items.

- B3 closure runtime. Estimated 6 to 10 hours.
- B2.4 follow-on. Generic structs and enums monomorphization. Estimated 3 to 5 hours.
- B2.4 follow-on. Inference reach extension for type arguments flowing through call chains. Estimated 2 to 4 hours.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

The natural next step is the B3 closure runtime, which closes the closure loop end to end and enables higher-order programming patterns. Alternatively, finishing the B2.4 follow-on for generic structs and enums tightens the generics story.

Await human prompt before proceeding.

## Session Context

This session closed all named B2.x follow-on items (polymorphic recursion guard, prune unspecialized generics, full impl-vs-trait signature validation) and delivered the B3 closure parser, AST, and type-check slice. Closure runtime support is deferred to a focused future session and documented with a clear implementation guide.
