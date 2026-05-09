# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T2. P1 type checker wired into compile pipeline.
**Status**: Complete. P1 is now resolved.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 360 tests pass workspace-wide. 315 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The static type checker is now invoked from `compile`. Type errors are surfaced as `CompileError` before bytecode emission. The end-to-end pipeline is parse → type check → compile.

The integration required four small changes.

1. New `Literal::Unit` variant on the AST so the unit value `()` has a distinct representation rather than overloading `Literal::Int(0)`.
2. The parser now emits `Literal::Unit` for the `()` literal.
3. The compiler emits `Op::PushUnit` for `Literal::Unit` in expression and pattern contexts.
4. The type checker recognizes `Literal::Unit` as `Type::Unit`.

`compile` now begins with a call to `typecheck::check(program)` and converts any returned `TypeError` into a `CompileError` with the same span and a `type error: ` prefix on the message.

Five tests that previously relied on the lax pre-check behavior were updated to declare the types they reference.

- `compile_enum_variant` and `eval_enum_variant` now include `enum Color { Red, Green, Blue }`.
- `compile_struct_init` and `eval_struct_init_and_field` now include `struct Point { x: i64, y: i64 }`.
- `compile_tuple_literal` now declares its return type as `(i64, i64, i64)` instead of `()`.

## Changes Made

### Source

- **`src/ast.rs`**: New `Literal::Unit` variant.
- **`src/parser.rs`**: `()` literal produces `Literal::Unit` instead of `Literal::Int(0)`.
- **`src/compiler.rs`**: `compile` invokes `typecheck::check` and propagates errors as `CompileError`. Two match arms in expression and pattern compilation handle `Literal::Unit` by emitting `Op::PushUnit`. Three test programs updated.
- **`src/typecheck.rs`**: One match arm in `type_of_expr` returns `Type::Unit` for `Literal::Unit`.
- **`src/vm.rs`**: Two test programs updated.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**: P1 marked resolved with strikethrough.
- **`docs/process/TASKLOG.md`**: V0.1-M3-T2 row added marking integration complete. New history row.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The `Literal::Unit` variant adds one match arm to every site that pattern-matches on `Literal`. The cascade was three sites: parser, expression compiler, pattern compiler, and type checker. All updated.

The integration is conservative. The type checker's lenient handling of unknown function names (treating them as natives that return `Type::Unknown`) means the integration does not reject existing programs that call native functions through `use` declarations. Native function call types remain unchecked at compile time and are detected at runtime.

The five updated tests are now more realistic. Programs that reference struct or enum types without declaring them were programmer error in any case. The type checker correctly catches these as undeclared-type or unknown-variant errors.

## Unaddressed Concerns

1. **Pattern type checking against the scrutinee.** Match arms accept any pattern shape regardless of the scrutinee's static type. The runtime continues to surface mismatches.

2. **Match arm exhaustiveness.** Not checked. The runtime continues to surface non-matching cases through the `NoMatch` error.

3. **Native function call types.** Not type-checked at compile time. Bringing them in requires either a richer `use` declaration that includes signatures or a static registry mapped at compile time.

4. **Hindley-Milner inference (B1).** Not yet implemented. The MVP checker uses local inference for unannotated `let` bindings only.

## Intended Next Step

Three paths.

A. Pivot to P3 (error recovery model). Defining recovery semantics for runtime errors closes the safety-critical positioning alongside the now-resolved P1.

B. Pivot to P7 follow-on (operand stack and DynStr arena migration). Closes the bounded-memory guarantee end to end.

C. Publish the keleusma main crate to crates.io now that P1 and P10 are resolved.

Recommend A or B based on which constituency is the priority. Recommend C if external visibility matters.

Await human prompt before proceeding.

## Session Context

This session resolved P10 across all phases (rkyv format, in-place validation, archive converters, full Vm refactor with Vm<'a>, true zero-copy execution, include_bytes example), landed P1 as a standalone pass, and now integrated P1 into the compile pipeline. The end-to-end pipeline now performs parse, type check, and bytecode compile in sequence with type errors surfaced as compile errors. Both P1 and P10 are resolved.
