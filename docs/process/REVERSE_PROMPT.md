# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T35 Visitor trait refactor for AST walkers.
**Status**: Complete. The structural recursion that was previously copy-pasted across six walker families now lives once in two visitor traits with default-implemented walk methods. Six passes migrated. 506 tests still pass.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 506 tests pass workspace-wide. Behavior preserved.
- Format clean.
- Clippy clean.

## Summary

The prior session deferred the visitor-trait refactor as a tracked refinement. This session executes it. A new `src/visitor.rs` module defines `MutVisitor` and `Visitor` traits with default-implemented `walk_block`, `walk_stmt`, `walk_expr`, `walk_iterable` methods that handle structural recursion. Each pass implements the trait via a state struct and overrides only the `visit_*` hooks it needs. The trait pattern is the standard Rust idiom for AST walkers (used by syn, swc, rustc).

### Trait shape

The two traits are symmetric except for mutability:

- `MutVisitor` operates on `&mut` references and is suitable for transformation passes (rewriting, hoisting).
- `Visitor` operates on `&` references and is suitable for inspection passes (free-variable analysis, validation).

Both traits expose:

- `visit_block`, `visit_stmt`, `visit_expr`, `visit_iterable` as the override hooks.
- `walk_block`, `walk_stmt`, `walk_expr`, `walk_iterable` as the default-implemented structural recursion.

Each `visit_*` default calls the corresponding `walk_*`. A pass that does nothing structural produces a no-op traversal. A pass overrides `visit_*` to insert its logic at that node kind, optionally calling `self.walk_*` to recurse before or after, depending on whether the pass needs pre-order or post-order semantics.

### Migrated passes

Six passes are migrated:

1. **`target::TargetChecker`** (immutable). Replaces `check_block_against_target`, `_stmt`, `_expr`. The visitor stores a `first_error: Option<CompileError>` and short-circuits subsequent visits once an error is recorded.
2. **`compiler::FreeVarCollector`** (immutable). Replaces `collect_free_in_block`, `_stmt`, `_expr`. The visitor manages the `bound: BTreeSet<String>` scope by saving and restoring around `visit_block`, `Stmt::For` body, and `Match` arm bodies.
3. **`compiler::ClosureHoister`** (mutable). Replaces `hoist_in_block`, `_stmt`, `_expr`. The visitor still routes the `Stmt::Let` recursive-closure case through a dedicated `hoist_let_bound_closure` method on the struct, but `Expr::Closure` uses the visitor's `walk_expr` for body recursion before transforming the node into an `Expr::ClosureRef`.
4. **`monomorphize::EnumSpecializer`** (mutable). Replaces `rewrite_enum_variants_block`, `_stmt`, `_expr`.
5. **`monomorphize::StructSpecializer`** (mutable). Replaces `rewrite_struct_inits_block`, `_stmt`, `_expr`.
6. **`monomorphize::CallSpecializer`** (mutable). Replaces `rewrite_block`, `_stmt`, `_expr`, `_iterable`.

Each monomorphize visitor stores the per-pass state (locals, specs cache, new specializations output) and the read-only context (generics map or generic-types map, fn_returns, struct_table). The `Stmt::Let` arm overrides update the locals map after walking the value; the `Expr::Call`, `Expr::StructInit`, or `Expr::EnumVariant` arms perform the specialization after walking the children.

### Net Size Change

| File | Before | After | Delta |
|---|---|---|---|
| `src/monomorphize.rs` | 2156 | 1204 | -952 (-44%) |
| `src/compiler.rs` | 2805 | 2664 | -141 |
| `src/target.rs` | 538 | 453 | -85 |
| `src/visitor.rs` | new | 305 | +305 |

Total reduction across the four files: 873 lines. The largest gain is in `monomorphize.rs` because it had three near-identical 800-line walker families collapsed into three small visitor structs that share the common recursion through the trait's default `walk_*` methods.

## Trade-offs and Properties

### What got cleaner

The structural recursion through the AST is no longer copy-pasted six times. The single source of truth is the `walk_*` defaults in `visitor.rs`. When the AST grows a new variant, the new variant only needs handling in one place per trait (the `walk_*` default), not in every pass.

Each pass's per-node logic now lives in a focused `visit_*` override that does not mix structural recursion with transformation logic. The override decides pre-order or post-order through the placement of `self.walk_*(node)` relative to its own logic, which makes the order explicit at each call site rather than implicit in a hand-written recursion.

### What got marginally less clean

The state-bag pattern for each pass introduces struct boilerplate that the parameter-threading style did not have. Each pass now has a struct definition with field declarations and an `impl MutVisitor for X` block. For a one-method pass this is more boilerplate than the equivalent parameter-thread. However, the size advantage of skipping repeated structural recursion dominates beyond the smallest passes, and the larger passes (`CallSpecializer` with seven state fields) make the struct pattern more readable than threading seven parameters.

### Behavior preservation

All 506 tests pass without modification. The migrations are mechanical replacements of the structural recursion code, with the per-node logic preserved verbatim. The state-management pattern in `FreeVarCollector` (saving and restoring the `bound` set across scope boundaries) replicates exactly what the original did via cloning.

### Why this was worth doing

The prior session's small wins (Span::default, native helper consolidation, decode_all_ops signature) saved about 84 lines. This session saves 873 lines while consolidating the most duplicated pattern in the codebase. The visitor abstraction also positions future passes to be added without touching existing pass implementations.

## Files Touched

- **`src/visitor.rs`** (new). `MutVisitor` and `Visitor` traits with default `walk_*` methods covering Block, Stmt, Expr, Iterable.
- **`src/lib.rs`**. New `pub mod visitor` declaration.
- **`src/target.rs`**. New `TargetChecker` struct + `Visitor` impl. Removed `check_block_against_target`, `_stmt`, `_expr`.
- **`src/compiler.rs`**. New `FreeVarCollector` struct + `Visitor` impl, replaces `collect_free_in_block`, `_stmt`, `_expr`. New `ClosureHoister` struct + `MutVisitor` impl, replaces `hoist_in_block`, `_stmt`, `_expr`. The `hoist_closures` entry-point updated to construct a `ClosureHoister` and drive it.
- **`src/monomorphize.rs`**. New `EnumSpecializer`, `StructSpecializer`, `CallSpecializer` structs each with `MutVisitor` impl. The corresponding old walker families removed.

### Knowledge Graph

- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T35.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The visitor refactor is complete and at quiescence. The named V0.1 work continues to be closed. The `keleusma-arena` registry version is still v0.1.0.

The `Type::Unknown` sentinel, indirect-dispatch flow analysis, recursion-depth attestation API, finer-grained f-string spans, and block-as-primary parser changes remain documented as future refinements.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session executed the deferred visitor-trait refactor. The result is a 873-line reduction across four files with the structural-recursion duplication consolidated into a single source of truth. All 506 workspace tests pass. The pattern is now available for any future pass that needs to walk the AST.
