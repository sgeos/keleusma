# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T29. B5b string discipline extensions and B6 f-string interpolation.
**Status**: Complete. Both features land with explicit WCET trade-offs documented.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 495 tests pass workspace-wide. 427 keleusma unit (12 new), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

The user explicitly opted into B5b and B6 work despite the prior backlog stance that "Keleusma is not a value-add for string work." This session implements both with WCET-aware framing.

### B5b. Concatenation and slicing as utility natives

Two new natives in `src/utility_natives.rs`, each in two variants:

- `concat(s1: String, s2: String) -> String` returns the catenation. The non-context variant returns `Value::DynStr`. The context variant `concat_with_ctx` resolves `Value::KStr` operands through the supplied arena and returns `Value::KStr` allocated in the arena's top region.
- `slice(s: String, start: i64, end: i64) -> String` returns the substring at the given character indices. Indices are Unicode code-point counts, matching the existing `length` semantics. Out-of-range indices return a `NativeError`. The context variant `slice_with_ctx` follows the same arena pattern as `concat_with_ctx`.

Three helpers factor the shared logic. `string_view_no_arena` projects a `Value` to `&str`, rejecting `Value::KStr`. `string_view_with_arena` resolves `Value::KStr` through the arena. `slice_chars` extracts the code-point range with bounds checking. Both natives register through `register_utility_natives` and `register_utility_natives_with_ctx`.

### B6. f-string interpolation

f-strings land as a lex-time desugaring. The lexer recognizes `f"..."` ahead of regular identifier lexing. Inside the body, `{...}` markers delimit interpolated expressions. The lexer scans the body, collects alternating literal and interpolation parts via a new `FStringPart` enum, and emits a desugared token stream:

- Empty f-string produces `StringLit("")`.
- Literal-only produces the bare `StringLit`.
- Single interpolation produces `to_string(<expr>)`.
- Mixed produces a left-associative chain of `concat` calls.

The interpolated expression is recursively tokenized through the public `tokenize` entry point; the trailing `Eof` is dropped at splice. Lex errors inside an interpolation propagate to the outer call. The lexer uses a new `pending: VecDeque<Token>` buffer so the multi-token desugared stream can be returned through the standard `next_token` interface. Escapes `\{` and `\}` produce literal braces. Newlines inside f-strings or interpolations are rejected. Unmatched `}` is rejected.

The desugaring depends on the runtime registration of `to_string` and `concat`. Programs using f-strings must register the corresponding natives at host setup time. The compile pipeline does not detect missing registrations until VM construction.

## WCET and WCMU Considerations

Both features produce dynamic strings whose worst-case output length is bounded by input lengths but is not a compile-time constant. The current verifier treats native allocations as the per-native attestation supplied through `Vm::set_native_bounds`. Hosts that rely on `verify_resource_bounds` for real-time embedding must declare heap bounds for `concat` and `slice` (and any other string-producing native they register) before constructing the VM through the safe constructor. Without an attestation, the analysis treats the native as zero-cost, which is unsound for unbounded inputs.

This is consistent with the existing contract for native attestation and does not introduce a new soundness gap. The trade-off is documented in `BACKLOG.md` for both B5b and B6 entries.

## Tests

Twelve new unit tests in `src/utility_natives.rs::tests`:

- B5b natives: `concat_two_static_strings`, `concat_static_with_dynamic`, `slice_basic`, `slice_full_range`, `slice_empty_range`, `concat_with_ctx_returns_kstr`, `slice_with_ctx_returns_kstr`.
- B6 f-strings: `fstring_no_interpolation`, `fstring_single_interp`, `fstring_mixed_interp`, `fstring_multiple_interps`, `fstring_escaped_braces`.

One new example: `examples/string_ops.rs` exercises the combined feature end to end. The program builds an interpolated greeting, slices its head, and concatenates a suffix to demonstrate the natives and the f-string desugaring path returning the expected value.

## Trade-offs and Properties

The lexer-side desugaring is invisible to the parser. Once the f-string body is split and folded into the token stream for `concat`/`to_string` calls, the parser produces a regular `Expr::Call` AST. This keeps the parser, type checker, monomorphizer, and compiler unchanged.

The recursive use of `tokenize` for interpolation expressions reuses the public lexer entry point. There is no new exposed API. The Eof token in the recursive lex output is dropped before splicing. The synthesized tokens carry the f-string's outer span as a single source location, so error messages from later passes point at the f-string rather than at character offsets inside it. This is acceptable for a first iteration; finer-grained span attribution is recorded as a future refinement.

`slice` uses code-point indices rather than byte offsets. This matches `length` and prevents multi-byte UTF-8 sequences from being split. The trade-off is that slicing an N-character string is O(N) due to the code-point traversal. For real-time embedding this cost contributes to the native's WCET attestation.

The non-context-aware `concat` and `slice` reject `Value::KStr` operands because no arena is available for resolution. Hosts mixing arena and non-arena strings must use the context-aware variants. This is consistent with the existing `length` contract.

## Changes Made

### Source

- **`src/utility_natives.rs`**. New `native_concat`, `native_concat_with_ctx`, `native_slice`, `native_slice_with_ctx` natives. Helpers `string_view_no_arena`, `string_view_with_arena`, `slice_chars`. Both `register_utility_natives` and `register_utility_natives_with_ctx` updated. Twelve new unit tests.
- **`src/lexer.rs`**. New `FStringPart` enum, `emit_fstring_desugar` helper, `Lexer::pending` buffer drained by `next_token`, `Lexer::lex_fstring` body scanner, recognizer for `f"` prefix.
- **`examples/string_ops.rs`** (new). End-to-end demonstration.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B5b and B6 both marked resolved with the new mechanisms documented. WCET implications recorded for hosts that rely on `verify_resource_bounds`.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T29.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The string subsystem is now feature-complete for the agreed scope: catenation, slicing, and interpolation. Format specifiers (`{x:.2}` and similar) are explicitly out of scope; hosts that want them can register additional natives.

Known limitations and future refinements:

- f-string spans collapse to the outer literal's span, so error messages from interpolation expressions point at the whole f-string rather than at the offending sub-expression. A finer-grained span attribution would track per-interpolation source offsets.
- f-strings depend on registered `to_string` and `concat` natives. Missing registrations surface only at VM construction. Compile-time detection would require a registration manifest known to the compiler.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding. The named B1, B2, B3, B5b, and B6 work is now closed.

## Session Context

This session implemented the string-discipline extensions and string interpolation that the user explicitly opted into. The lex-time desugaring approach for f-strings keeps the parser, type checker, monomorphizer, and compiler unchanged. The natives follow the established context-aware/non-context-aware pattern. The WCET implications are recorded in `BACKLOG.md` so hosts know to declare heap bounds for the string natives if they rely on `verify_resource_bounds`.
