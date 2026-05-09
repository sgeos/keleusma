# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T20. B3 closure runtime end to end.
**Status**: Complete. Closures execute end to end without environment capture. The capture follow-on is documented as next-session work.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 467 tests pass workspace-wide. 399 keleusma unit (2 new for closure round trips), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session delivered the B3 closure runtime end to end. Closures execute against the existing VM through new instructions and a hoisting pass.

Runtime representation. New `Value::Func(u16)` variant carrying the chunk index of the closure body. Two new instructions: `Op::PushFunc(u16)` produces a `Func` value; `Op::CallIndirect(u8)` pops `arg_count` arguments plus the `Func` value from the operand stack and invokes the referenced chunk through the standard call-frame mechanism.

Closure hoisting. After type checking and monomorphization, the compile pipeline runs a hoist pass that walks every function and impl method body. Each `Expr::Closure` is replaced with an `Expr::Ident { name: "__closure_<n>" }` reference and a fresh `FunctionDef` is appended to the program with the same parameters, return type, and body. The synthetic functions receive chunk indices like ordinary user-defined functions.

Identifier resolution. The compiler now resolves `Expr::Ident` against the function map after locals. An unbound name that matches a function name emits `Op::PushFunc(idx)`, producing a `Func` value at runtime that can flow through locals and into indirect calls.

Indirect call resolution. The compiler resolves `Expr::Call { name, args }` to indirect dispatch when `name` is a local. It emits `GetLocal(slot)` followed by the arguments and `Op::CallIndirect(n)`. The type checker accepts the same shape and returns a fresh type variable.

Wire format. `BYTECODE_VERSION` bumped to 6 to reflect the new opcode discriminants.

End-to-end demonstration. `examples/closure_basic.rs` compiles and executes `let f = |x: i64| x + 1; f(41)` returning 42.

## Tests

Two new typecheck round-trip tests.

- `closure_executes_end_to_end`. The minimal case: a single-parameter closure stored in a local and invoked.
- `closure_no_param_callable`. The nullary-closure case.

One new example, `examples/closure_basic.rs`, demonstrates end-to-end execution.

## Trade-offs and Properties

The MVP omits environment capture. Closures that reference outer-scope variables fail at compile time with an "undefined variable" error. The fix introduces a captured environment that the closure value carries alongside the chunk index. The runtime binds captured values as additional implicit parameters at invocation. The capture mechanism is the largest remaining piece of B3.

The MVP supports closures stored in locals. Passing a closure as an argument to another function and invoking it from the called function requires the typecheck to flow function types through call signatures. The current minimum admits direct local-stored closures.

The wire-format bump (`BYTECODE_VERSION = 6`) is a deliberate breaking change. Bytecode produced under earlier versions must be recompiled. The golden bytes test was updated to pin the new wire format.

## Changes Made

### Source

- **`src/bytecode.rs`**. New `Value::Func(u16)` variant with `PartialEq`, `type_name`, and `try_from_value`/`render` arms. New `Op::PushFunc(u16)` and `Op::CallIndirect(u8)` opcodes with cost, stack-shrink, and archived converters. `BYTECODE_VERSION` bumped to 6.
- **`src/vm.rs`**. New runtime arms for `Op::PushFunc` (push `Value::Func`) and `Op::CallIndirect` (pop args plus `Func` value, invoke chunk).
- **`src/compiler.rs`**. New `hoist_closures` pass between monomorphization and compilation. New `hoist_in_block`, `hoist_in_stmt`, `hoist_in_expr` recursive helpers. `Expr::Ident` arm extended to resolve against the function map after locals. `compile_call` arm extended to emit indirect dispatch when the call name is a local. `Expr::Closure` arm rejects with an internal-only error since the hoist pass should run before compile.
- **`src/typecheck.rs`**. `Expr::Call` arm short-circuits to a fresh type when `name` resolves to a local, accepting indirect-call call sites. Two new closure round-trip tests.
- **`src/utility_natives.rs`**. `render_value_to_string` arm for `Value::Func`.
- **`examples/closure_basic.rs`** (new). End-to-end demonstration.
- Golden bytes test updated for `BYTECODE_VERSION = 6`.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B3 entry rewritten as resolved (without environment capture). Capture and first-class closures-as-arguments documented as remaining work.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T20.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None of the originally listed items remain. P1 through P10 fully resolved. B1 resolved. B2 fully resolved with monomorphization MVP and follow-ons. B3 resolved without environment capture.

The remaining items.

- B3 closure environment capture. Estimated 4 to 6 hours.
- B3 first-class closures as function arguments. Estimated 2 to 4 hours.
- B2.4 generic struct and enum monomorphization. Estimated 3 to 5 hours.
- B2.4 inference reach extension for type arguments flowing through call chains. Estimated 2 to 4 hours.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

The natural next step is the closure environment capture, which closes the closure loop fully and enables idiomatic higher-order programming. Alternatively, the B2.4 follow-on for generic structs and enums tightens the generics story.

Await human prompt before proceeding.

## Session Context

This long session closed the B3 closure runtime end to end: new opcodes, runtime support, compiler hoisting, type-checker indirect-call recognition, and a demonstration that a closure stored in a local invokes correctly. Environment capture and closures-as-arguments are documented as remaining follow-on work.
