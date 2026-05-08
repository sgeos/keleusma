# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.0-M5 partial completion. Two-string-type discipline and the fifth guarantee.
**Status**: Partial. Type discipline complete. Arena allocator and WCMU instrumentation deferred to V0.0-M6.

## Verification

**Command**: `cargo test && cargo clippy --tests --all-targets -- -D warnings && cargo fmt --check`
**Result**: 272 tests pass, up from 268. Zero clippy warnings. Format clean. Four new tests cover the cross-yield prohibition on dynamic strings, namely yield of static string succeeds, yield of dynamic string fails, yield of tuple containing dynamic string fails, and a unit test for the `Value::contains_dynstr` helper.

## Summary

Recorded R31, R32, R33, and B6, B9, B10. Documented the fifth Keleusma guarantee, namely bounded-memory (WCMU). Documented the dual-end arena. Implemented the two-string-type discipline at runtime with `Value::StaticStr` and `Value::DynStr` as distinct variants. Source-level string literals compile to `StaticStr`. The `to_string` native returns `DynStr`. The string concatenation operator produces `DynStr`. The cross-yield prohibition is enforced at runtime via a structural check on the yielded value. The arena allocator and WCMU instrumentation are recorded as P7 and P8 in the priority list and deferred to V0.0-M6.

## Changes Made

### Source Code

- **src/bytecode.rs**: Replaced `Value::Str(String)` with two distinct variants `Value::StaticStr(String)` and `Value::DynStr(String)`. Updated `Value::type_name()` to report each variant. Updated `PartialEq` to allow cross-variant equality on string contents because the discipline is about lifetime and provenance rather than value identity. Added `Value::as_str()` accessor for sites that read string contents without caring about provenance. Added `Value::contains_dynstr()` for the cross-yield runtime check.
- **src/compiler.rs**: Source-level string literals now compile to `Value::StaticStr` constants. Type names and variant names in the constant pool also use `Value::StaticStr` since they are static labels embedded in the code image.
- **src/vm.rs**: Updated all pattern matching on string values to handle both variants. The string concatenation result of `Op::Add` produces a `Value::DynStr` because the resulting string is computed at runtime. Comparison operators handle both variants. The `Op::Yield` handler now rejects yielded values that transitively contain a `DynStr`. Four new tests cover the yield discipline.
- **src/utility_natives.rs**: `to_string` returns `Value::DynStr` because the produced string is computed. `length` accepts either string variant. The pattern matching on string contents in the recursive `to_string` calls handles both variants.

### Knowledge Graph

- **docs/decisions/RESOLVED.md**: Added R31 (WCMU as the fifth guarantee), R32 (dual-end arena with separate stack and heap WCMU bounds), R33 (modern 64-bit target assumption for V0.0).
- **docs/decisions/BACKLOG.md**: Updated B5 to reflect the V0.0-M5 partial completion. Added B9 (hot update of yielded static strings) and B10 (portability and target abstraction).
- **docs/decisions/PRIORITY.md**: Added P7 (arena allocator implementation) and P8 (WCMU instrumentation and auto-arena sizing) for V0.0-M6.
- **docs/architecture/LANGUAGE_DESIGN.md**: Now lists five guarantees instead of four. Memory model section describes the dual-end arena and the two-string-type discipline.
- **docs/architecture/EXECUTION_MODEL.md**: Memory section rewritten to describe the dual-end arena with separate stack and heap WCMU bounds.
- **docs/design/TYPE_SYSTEM.md**: Primitive type table updated with size and alignment columns. New String Types section documents StaticStr and DynStr disciplines. Data segment admissibility table updated to reflect string constraints. Runtime value table updated.
- **docs/reference/GLOSSARY.md**: Added Arena (revised), Dual-end arena, DynStr, StaticStr, and WCMU entries.
- **docs/process/TASKLOG.md**: V0.0-M5 partial completion recorded. Active milestone none, ready for V0.0-M6 or V0.1 planning.

## Unaddressed Concerns

1. **Arena allocator is still simulated.** The runtime continues to use the global allocator for `String` and `Vec` content. The two-string-type discipline is enforced at the Value level, but the actual storage of `DynStr` is not arena-allocated yet. The arena lifetime is enforced through Rust drop semantics rather than through bump-pointer reset. P7 is the implementation work.
2. **WCMU is not yet computed.** The fifth guarantee is documented but not enforced. The host-attestation surface for native functions does not yet include WCMU declarations. P8 is the implementation work.
3. **Cross-yield prohibition is runtime-only.** The current check fires at the moment of yield, not at compile time. A compile-time check would require type tracking through the surface compiler that is beyond V0.0-M5 scope. The runtime check is sufficient for safety but does not catch the violation as early as ideal.
4. **String surface concatenation operator remains in place.** The `+` operator on strings produces a `DynStr`. This was kept for backward compatibility with the existing `eval_string_concat` test. The user has indicated that surface-language string operations beyond literals are deferred. Future work could remove this operator entirely and force concatenation to host functions.
5. **Static strings in `.data` is permitted at the bytecode level but not exposed in the surface grammar.** This is consistent with the user's clarification. The host can write `Value::StaticStr` values into data slots through `set_data` and is responsible for validity across hot updates.
6. **Stable Rust does not provide String with a custom allocator.** The arena work in P7 will require a custom `DynStr` type backed by `Vec<u8, Allocator>` rather than reusing the standard `String`. This is a known engineering item.

## Intended Next Step

Two paths forward, in order of independence.

A. V0.0-M6 implementing P7 (arena allocator) and P8 (WCMU instrumentation) together. This delivers the full dual-end arena with bump-pointer reset and the fifth guarantee enforcement. Substantial infrastructure work.

B. Other V0.1 candidates from the prior reverse prompt, namely the type checker (P1), for-in over arbitrary expressions (P2), or the error recovery model (P3). These are language-layer improvements that do not require the arena.

Recommend B if the language layer is the priority. Recommend A if the certification posture is the priority. The arena and WCMU work is a clear next step for the certification path because both directly support the bounded-memory guarantee.

Await human prompt before proceeding.

## Session Context

The session resumed a previously stalled data segment design and carried through three milestones of work, V0.0-M3 for the data segment, V0.0-M4 for the static marshalling layer, and V0.0-M5 partial for the two-string-type discipline and the fifth guarantee. The total session arc covered specification clarification, research and documentation formalization, source conformance, host interoperability layer, hot swap API, ergonomic Rust type interop with macro-generated marshalling, and now the type-discipline portion of strings and memory bounds. Six commits accumulated during the session. The arena allocator and WCMU instrumentation are recorded as P7 and P8 in the priority list and constitute the next implementation milestone.
