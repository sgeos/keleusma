# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T1. P1 standalone type checker.
**Status**: Complete as a standalone pass. Pipeline integration deferred until parser refactor.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 360 tests pass workspace-wide. 315 keleusma unit including 13 new typecheck tests, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

A static type checker for Keleusma source programs is now in place at `src/typecheck.rs`. The pass is callable as `typecheck::check(&program)` after parse and before bytecode emission.

Coverage.

- Function call argument count and argument types against parameter declarations.
- Function return expression type against declared return type.
- Let binding type against the value's type when annotation is present.
- Let bindings without annotation infer from the right-hand side.
- Arithmetic and comparison operations have type-compatible operands.
- Logical operators require bool operands.
- Field access references defined fields on the operand type.
- Struct construction provides defined fields with the right types.
- Tuple index in range. Array index of i64.
- Cast operations are between admissible types, namely i64 to f64 and back.
- Identifier references resolve to known locals or function names.
- If-else branch type agreement.
- For-range bound types. For-in element type extraction.
- Enum variant existence and payload arity and types.

Architecture. The checker uses a two-pass design. Pass one collects type definitions, struct field signatures, enum variant payload signatures, data block field types, and function signatures. Pass two checks each function body against its declared signature. The internal `Type` enum is independent of the `TypeExpr` AST node so the checker can reason about types without surface-syntax detail.

Out of scope and deferred.

- Hindley-Milner inference (B1).
- Detailed pattern type checking against the scrutinee. Match arms accept any pattern. The runtime detects mismatches.
- Match arm exhaustiveness.
- Native function call types. Natives are registered at runtime through `Vm::register_*`.
- Yielded value types. The dialogue type is not yet tracked.

## Pipeline integration is deferred

The natural next step is to invoke the checker from `compile`. This is blocked by a parser quirk. The unit literal `()` is currently represented as `Literal::Int(0)` rather than as a unit value. The compiler emits `Op::PushUnit` for this case through pattern matching on the literal, but the type checker would surface spurious i64-versus-Unit mismatches for unit-returning functions.

Follow-up sequence to integrate the checker.

1. Add `Literal::Unit` to the AST, or change the parser to produce `TupleLiteral` with empty elements for `()`.
2. Update the compiler to handle the new representation by emitting `Op::PushUnit`.
3. Update the type checker to recognize the new representation as `Type::Unit`.
4. Invoke `typecheck::check` from `compile` and convert errors to `CompileError`.
5. Update any existing test programs that relied on lax behavior.

The current commit deliberately stops short of step 1 to keep the scope bounded. The standalone checker is useful as a host-callable verification step in its own right.

## Changes Made

### Source

- **`src/typecheck.rs`**: New file. ~850 lines. Type enum, TypeError, Ctx, check entry point, and a comprehensive set of unit tests for the coverage matrix.
- **`src/lib.rs`**: New `pub mod typecheck;` declaration.
- **`src/compiler.rs`**: Documentation note explaining why the type checker is not yet wired into `compile`. Hosts can call `typecheck::check(program)` themselves before `compile`.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**: P1 entry expanded. Coverage list, deferred items, and the integration follow-up steps documented.
- **`docs/process/TASKLOG.md`**: V0.1-M3-T1 row added marking the standalone pass complete. New history row.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The checker uses `BTreeMap` from the `alloc` crate rather than `HashMap` so it remains compatible with the `no_std + alloc` posture.

`Type::Unknown` is a sentinel used when type information cannot be determined without inference. Treated as compatible with anything in this MVP pass. Enables the checker to stop short of full inference while still detecting concrete mismatches. Future iterations can tighten the rules as inference is added.

Native function calls return `Type::Unknown` because the checker has no compile-time view of native signatures. This is lenient by design. Bringing native types into the checker requires either a `use` declaration that includes signatures or a static registry mapped at compile time. Both are larger changes than this MVP.

The two-pass design lets type definitions reference one another in any order. Functions can call any other function in the program regardless of declaration order.

## Unaddressed Concerns

1. **Pipeline integration.** Documented above. The standalone checker is callable but not invoked by `compile`. Hosts that want compile-time checking call it directly.

2. **Pattern type checking.** Match arms bind variables but do not check the pattern shape against the scrutinee type. Runtime continues to surface mismatches.

3. **Match exhaustiveness.** Not checked. The runtime continues to surface non-matching enum cases through the `NoMatch` error.

4. **Native function types.** Not type-checked at compile time. Bringing them in requires either a richer `use` declaration or a compile-time registry.

5. **Generic and parametric types.** Not supported. Tracked as B1 (HM inference) and B2 (traits or generic type parameters).

## Intended Next Step

Three paths.

A. Integrate the checker into the compile pipeline. Requires the small parser refactor for the unit literal. Then existing test programs that depend on lax behavior need adjustment.

B. Pivot to P3 error recovery model. Defining recovery semantics for runtime errors closes the safety-critical positioning alongside P1.

C. Pivot to P7 follow-on. Operand stack and DynStr arena migration. Closes the bounded-memory guarantee end to end.

Recommend A if compile-time type errors are the priority. Recommend B if defining error recovery semantics matters more. Recommend C if the bounded-memory guarantee is the priority.

Await human prompt before proceeding.

## Session Context

Long session that closed out P10 across all phases (rkyv format, in-place validation, archive converters, full Vm refactor with Vm<'a>, true zero-copy execution, include_bytes example) and now landed P1 as a standalone pass. The type checker covers the core surface of Keleusma's static type system. Pipeline integration is the next step and depends on a small parser change.
