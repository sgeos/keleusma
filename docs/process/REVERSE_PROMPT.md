# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T21. B3 closure environment capture.
**Status**: Complete. Closures execute end to end with environment capture.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 469 tests pass workspace-wide. 400 keleusma unit (1 new for closure capture), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

This session delivered closure environment capture, completing the B3 closure feature.

Runtime. `Value::Func` extended from a plain `u16` chunk index to a struct with `chunk_idx: u16` and `env: Vec<Value>`. Plain function-name references continue to produce `Func` values with empty `env` through `Op::PushFunc`. Closures with captured outer-scope locals produce `Func` values with non-empty `env` through the new `Op::MakeClosure(chunk_idx, n_captures)` instruction, which pops `n_captures` values from the operand stack and stores them in declaration order. `Op::CallIndirect` was updated to push env values back onto the operand stack as implicit arguments before the explicit ones, increasing the called chunk's argument count by `env.len()`.

Hoist pass. New free-variable collection walks closure bodies and produces a list of identifiers referenced but not bound by the closure's own parameters. The list is filtered to drop names declared as natives via `use` declarations and names qualified with `::`. The remaining names become both the synthetic function's prepended parameters and the captures recorded in the new `Expr::ClosureRef { name, captures, span }` AST variant.

Compiler. The `ClosureRef` arm emits `GetLocal(slot)` for capture names that resolve as locals and `PushFunc(idx)` for capture names that resolve as top-level functions. Then it emits `MakeClosure(synth_idx, n)` when there are captures or `PushFunc(synth_idx)` when there are none.

Type checker. `Expr::ClosureRef` produces a fresh type variable. The previously added local-call short-circuit continues to handle indirect-call call sites.

End-to-end. `examples/closure_capture.rs` compiles and executes `let n: i64 = 10; let f = |x: i64| x + n; f(5)` returning 15. The hoisted synthetic function has parameters `(n, x)`. The construction site emits `GetLocal(n)` followed by `MakeClosure(synth, 1)`. The invocation site emits `GetLocal(f)`, the explicit args, and `CallIndirect(1)`. The runtime extracts `env = [n_value]` and pushes both the env value and the explicit arg before invoking the synthetic chunk.

## Tests

One new typecheck test, `closure_captures_outer_local`. The full test suite continues to pass against the new wire format.

One new example, `examples/closure_capture.rs`, demonstrates end-to-end execution.

## Trade-offs and Properties

The capture is by value: the closure stores a copy of each captured value at creation time. Subsequent mutation of the original variable does not propagate to the closure. This matches the canonical capture-by-value semantics and avoids the complications of shared mutable state across closures.

Captures are determined statically at hoist time. Names referenced as Call heads are also subject to capture analysis, which means closures that call top-level functions by name capture those functions as `Func` values. This is correct but slightly slower than direct calls because the called function goes through an indirect call inside the synthetic chunk's body. Future optimization could detect when a captured name is a top-level function and resolve the call directly.

The native filter at hoist time uses two rules: drop names that match a `use` declaration's import name, and drop names with `::`. The first catches use-declared natives; the second catches qualified paths such as trait methods. This is a heuristic but covers the common cases.

The wire format bump (`BYTECODE_VERSION = 6`) absorbs the new opcodes (`PushFunc`, `MakeClosure`, `CallIndirect`).

## Changes Made

### Source

- **`src/bytecode.rs`**. `Value::Func` extended to `{ chunk_idx, env }`. New `Op::MakeClosure(u16, u8)` opcode with cost, stack-shrink, stack-growth, and archived converter arms.
- **`src/vm.rs`**. `Op::MakeClosure` runtime arm pops captures and pushes a closure value. `Op::CallIndirect` runtime arm extracts the env from the popped `Func` value and pushes env values as implicit arguments before the explicit ones, with the chunk's parameter count adjusted accordingly.
- **`src/utility_natives.rs`**. `render_value_to_string` arm updated for the new `Func` shape: `<fn:idx>` for empty env, `<closure:idx/n>` for non-empty.
- **`src/ast.rs`**. New `Expr::ClosureRef { name, captures, span }` variant. `Expr::span` extended.
- **`src/compiler.rs`**. New `collect_pattern_names`, `collect_free_in_block`, `collect_free_in_stmt`, `collect_free_in_expr` helpers for hoist-time free-variable analysis. `hoist_closures` threads a `BTreeSet<String>` of native names through the pass and filters captures. `hoist_in_expr` for `Expr::Closure` now collects free variables, prepends them as synthetic-function parameters, and replaces with `Expr::ClosureRef`. New `Expr::ClosureRef` arm in `compile_expr` emits captures and either `MakeClosure` or `PushFunc`.
- **`src/typecheck.rs`**. `Expr::ClosureRef` returns a fresh type variable. New unit test `closure_captures_outer_local`.
- **`src/monomorphize.rs`**. `subst_in_expr` and `rewrite_expr` arms for `Expr::ClosureRef` (no-op pass-through).
- **`examples/closure_capture.rs`** (new). End-to-end demonstration.
- Golden bytes test updated for the new wire format.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B3 entry rewritten as resolved with capture. First-class closures as function arguments tracked as remaining work.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T21.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

None of the originally listed items remain. P1 through P10 fully resolved. B1 resolved. B2 fully resolved with monomorphization MVP and follow-ons. B3 resolved with environment capture.

The remaining items.

- B3 first-class closures as function arguments. Estimated 2 to 4 hours.
- B2.4 generic struct and enum monomorphization. Estimated 3 to 5 hours.
- B2.4 inference reach extension for type arguments flowing through call chains. Estimated 2 to 4 hours.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Idiomatic higher-order programming patterns now work for the common cases. The next natural step is first-class closures as function arguments, which enables closures to be passed to higher-order functions like `map` or `filter`. Alternatively, the B2.4 follow-on for generic structs and enums tightens the generics story.

Await human prompt before proceeding.

## Session Context

This long session delivered the final piece of B3 closures: environment capture. Closures now compose with idiomatic outer-scope variable use, executing end to end through the same `Op::CallIndirect` mechanism that handled the no-capture case. Free-variable analysis at hoist time, capture emission at the construction site, and env push-back at invocation form the complete capture pipeline.
