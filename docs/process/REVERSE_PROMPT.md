# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T34 Code deduplication pass.
**Status**: Complete. Three concrete consolidations land. All 506 workspace tests pass; clippy and format clean.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 506 tests pass workspace-wide. No test count change; consolidation is behavior-preserving.
- Format clean.
- Clippy clean.

## Summary

This pass identifies and removes three concrete duplications in the runtime crate. Each is a behavior-preserving refactor verified by the existing test suite.

### Span literal repetition

`Span` is a four-field record (`start`, `end`, `line`, `column`). Constructions of the all-zero "synthetic" span appeared in `target.rs` as repeated four-field struct literals. The struct now derives `Default`, and the literals are replaced with `Span::default()`. Future code that needs a synthetic span no longer needs to repeat the field initializers, and the choice of zero-initialization is codified once.

### Native string-helper consolidation

Eight native functions in `utility_natives.rs` (`to_string`, `to_string_with_ctx`, `length`, `length_with_ctx`, `concat`, `concat_with_ctx`, `slice`, `slice_with_ctx`, plus `println`) shared four code shapes that were copy-pasted with small variations:

1. The argument-count check.
2. The string-from-Value extraction, with two variants depending on whether arena context was available.
3. The i64-from-Value extraction with a typed error message.
4. The string-result wrapping in either `Value::DynStr` or `Value::KStr` depending on arena presence.

Four new helpers consolidate these patterns:

- `check_arity(name, expected, args)` returns `Err` with a uniform message when the count does not match.
- `read_string_arg(arena: Option<&Arena>, v)` extracts an owned `String`, optionally resolving arena-backed `Value::KStr` through the supplied arena. The single function now covers both the no-arena and with-arena paths through the `Option`.
- `read_i64_arg(name, arg_label, v)` extracts an `i64` with a uniform typed error.
- `finalize_string_result(name, arena, out)` wraps a produced `String` in either `Value::DynStr` or `Value::KStr`, choosing based on the arena presence and producing a uniform allocation-failure message.

The `native_concat` / `native_concat_with_ctx` pair are now thin wrappers over `concat_impl(arena: Option<&Arena>, args)`, and the `native_slice` / `native_slice_with_ctx` pair are thin wrappers over `slice_impl`. The shared logic lives once. Other natives (`to_string`, `length`, `println`) adopted `check_arity` for the argument-count check.

### Decode-cache helper signature

`decode_all_ops` previously accepted a borrowed `AlignedVec<8>`, so the owned-bytecode constructor and the borrowed-bytecode constructor could not share the same call. The signature now accepts a plain `&[u8]`, which both paths can pass: `Vm::construct` passes `aligned.as_slice()` from its newly serialized bytes; `Vm::view_bytes_zero_copy` passes the borrowed bytes directly. The borrowed path previously inlined the iteration loop with the same logic as the helper; that duplication is gone.

## What was deferred

A more ambitious abstraction would extract the AST walker pattern that recurs across `monomorphize.rs`, `compiler.rs::hoist_*`, and `target.rs::check_*_against_target`. Each of these has a structural recursion through `Block`, `Stmt`, `Expr`, and `Iterable` that could share a `MutVisitor` trait with default-implemented walk methods. The refactor was not undertaken in this session because:

1. The three walker families track different per-pass state (locals tables, specialization caches, validation context). A clean trait-based abstraction would require restructuring each pass's state into a struct and migrating the parameter-threading style to method dispatch.
2. The risk of subtle behavior change is non-trivial. The walker functions in monomorphize alone are about 800 lines spread across three families; any structural reorganization would need extensive cross-checking.
3. The existing duplication is structural (the recursion shape is the same) but not behavioral (each pass acts differently at each node). The Rust idiom for this case is the visitor trait, but the cost of introducing the abstraction must be weighed against the cost of maintaining the explicit walks. The current code is straightforward to read and modify per pass.

The visitor abstraction remains available as future work if a fourth walker family lands or if the existing families need significant changes that would benefit from shared infrastructure. It is recorded as a tracked refinement rather than a closed gap.

## Net Size Change

| File | Before | After | Delta |
|---|---|---|---|
| `src/utility_natives.rs` | 696 | 638 | -58 |
| `src/vm.rs` | 3748 | 3736 | -12 |
| `src/target.rs` | 538 | 523 | -15 |
| `src/token.rs` | small | 143 | +1 (Default derive) |

Total: approximately 84 lines removed. Modest but representative of the duplication that was actually present. The remaining duplication is in walker structures whose unification is deferred per the analysis above.

## Trade-offs and Properties

The native-helper consolidation merged the arena-context dichotomy that the code previously expressed as separate functions into a single `Option<&Arena>` parameter. Callers that always have an arena (the with-ctx path) pass `Some(arena)`; callers that do not (the non-context path) pass `None`. The runtime cost is one branch per call, which is negligible for native functions whose dominant cost is their logic.

The `Option<&Arena>` pattern in `read_string_arg` may invite the question of whether to mask `KStr` resolution failures uniformly or to surface different errors depending on the arena presence. The implementation distinguishes: missing arena context produces an error pointing at the registration mismatch (the host registered a string-producing native through the no-context path but supplied a `KStr` argument); a stale handle in an arena-aware call produces the standard "stale (arena reset since allocation)" error. The two error messages remain distinguishable.

The `decode_all_ops` signature change from `&AlignedVec<8>` to `&[u8]` is a strict generalization. Any caller that previously passed an `AlignedVec` continues to work via `aligned.as_slice()`. Callers with borrowed bytes can use the helper directly.

## Changes Made

### Source

- **`src/token.rs`**. `Span` derives `Default`.
- **`src/target.rs`**. Replaced four-field `Span` literals with `Span::default()`.
- **`src/utility_natives.rs`**. New helpers `check_arity`, `read_string_arg`, `read_i64_arg`, `finalize_string_result`. Removed previous helpers `string_view_no_arena` and `string_view_with_arena` in favor of the single `read_string_arg`. Native pairs `concat`/`concat_with_ctx` and `slice`/`slice_with_ctx` consolidated to `*_impl` shared bodies. Other natives adopted `check_arity`.
- **`src/vm.rs`**. `decode_all_ops` signature changed from `&AlignedVec<8>` to `&[u8]`. Borrowed-bytecode constructor inlined logic removed; both call sites use the helper.

### Knowledge Graph

- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T34.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The visitor-trait refactor for AST walkers across monomorphize, compiler::hoist, and target validation is recorded as a tracked refinement. It is not in any backlog entry yet; if pursued it would warrant a new entry describing the abstraction surface and the migration plan.

The named V0.1 work continues to be closed. The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session focused on duplication of the small, explicit kind: repeated literals, near-identical function bodies, and a helper whose signature could not be shared between two callers. The visitor-trait refactor for AST walkers is the larger remaining duplication; it is documented but deferred because the cost of introducing the abstraction exceeds the cost of the present explicit walks.
