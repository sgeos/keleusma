# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T32. B10 portability and target abstraction foundation.
**Status**: Foundation complete. The compiler accepts a `Target` descriptor; the wire format records the target's declared widths; the compiler rejects programs that use features unsupported by the target. Cross-target codegen and target-specific runtime representations remain future work, documented in BACKLOG.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 506 tests pass workspace-wide. 438 keleusma unit (9 new), 17 keleusma marshall integration, 17 keleusma `kstring_boundary` integration, 28 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

Keleusma's bytecode wire format already records the producer's declared word, address, and float widths in the framing header. The runtime accepts bytecode whose widths are at most its own, and the integer arithmetic path masks results to the declared width via `truncate_int`. What was missing was a producer-side surface that lets a host explicitly choose the compilation target and have the compiler validate the program against that target's capabilities.

This session adds that surface as a new `crate::target::Target` descriptor and a `compile_with_target(program, target)` entry point. The descriptor carries the three width fields (encoded as base-2 exponents matching the wire-format fields) and two capability flags (`has_floats`, `has_strings`). Const presets cover the practical cases: `host` (64-bit, all features), `wasm32` (32-bit word and address with 64-bit floats), `embedded_32` (32-bit with 32-bit floats), `embedded_16` (16-bit with no floats), and `embedded_8` (8-bit word with 16-bit address per the 6502 class, no floats, no strings).

The compiler runs two validations before lowering to bytecode. `Target::validate_against_runtime` rejects targets whose declared widths exceed the runtime's. `validate_program_for_target` walks the program AST looking for float types, string types, float literals, and string literals; programs that use features absent from the target are rejected with descriptive error messages pointing at the offending source span. After validation, the target's widths are baked into the resulting module's wire-format header, and the rest of the compilation runs unchanged.

The pre-existing `compile(program)` entry point is now a thin wrapper over `compile_with_target(program, &Target::host())`, so existing callers see no behavior change.

## What is in scope

The pre-existing infrastructure the implementation builds on.

- The wire format already records `word_bits_log2`, `addr_bits_log2`, and `float_bits_log2` in the 16-byte framing header. Mirror copies live in the archived module body.
- The runtime already accepts bytecode whose declared widths are at most the runtime's. Oversized bytecode is rejected at load time with `LoadError::WordSizeMismatch`, `AddressSizeMismatch`, or `FloatSizeMismatch`.
- The integer arithmetic path already masks results via `truncate_int` to the declared word width, so 32-bit-declared bytecode running on the 64-bit runtime produces 32-bit overflow semantics.

The new surface lets a host explicitly choose the target, validate program features against the target's capabilities, and emit bytecode whose declared widths are accurate. The same compiled module can then run on the current 64-bit runtime during development and, in principle, on a future narrower-runtime build.

## What remains open

This is a foundation, not a complete cross-target story. Several substantial extensions remain documented in BACKLOG:

- Target-specific runtime builds. The current `Value` enum carries 64-bit `Int` and `Float` variants and uses `Vec<Value>` for the operand stack. Building a 16-bit or 8-bit native runtime requires a different `Value` layout and a corresponding execution-loop variant. The wire format declares the bytecode's intended target, but no runtime build currently consumes that declaration to choose a representation.
- Cross-target codegen. Emitting native assembly for the 6502 or ARM64 from Keleusma bytecode is out of scope and has not been pursued. The synchronous-language tradition's approach of target-independent intermediate representations feeding target-specific backends is referenced in RELATED_WORK as the path of record.
- Target-defined primitive types. The original B10 entry mentioned `byte`, `bit`, `word`, and `address` as candidate primitives. The current type system continues to use `i64` for integers; the target's declared word width controls arithmetic masking but does not change the surface type. Adding the new primitives would require parser, AST, and type-checker work beyond this session's scope.

## Tests

Nine new tests in `src/target.rs::tests`:

- `host_target_admits_full_program` covers basic host-preset compilation.
- `host_target_admits_floats_and_strings` covers full-feature admission.
- `embedded_16_rejects_float_literal` covers float-literal rejection.
- `embedded_16_rejects_float_type_in_param` covers float-type rejection in parameter signatures.
- `embedded_8_rejects_string_literal` covers string-literal rejection.
- `embedded_8_admits_int_only_program` covers int-only programs on the most restricted preset.
- `target_widths_propagate_to_module` covers width propagation through to the wire format.
- `host_widths_match_runtime_constants` covers the host-target-equals-runtime invariant.
- `target_validation_against_runtime_rejects_oversized` covers oversized-target rejection.

One new example: `examples/target_aware_compile.rs` demonstrates compilation against host, embedded_32, embedded_16, and embedded_8 targets with the appropriate float and string rejections, and prints the declared widths from the resulting modules.

## Trade-offs and Properties

The choice to put `Target` in its own module rather than fold its fields into the existing `Module` struct keeps the producer-side concern (which target am I compiling for?) separate from the artifact-side concern (what does the bytecode declare?). The two are linked by `compile_with_target` writing the target's widths into the module's header, but they remain conceptually distinct.

The capability flags `has_floats` and `has_strings` are coarse-grained on purpose. Finer gating (such as "no dynamic strings" while still allowing static string literals) would require more capability axes and is recorded as future work. The current granularity matches the practical embedded-vs-server split.

The AST walker is conservative. It rejects any occurrence of a float or string type or literal, even within unreachable branches. A more permissive variant would only reject reachable uses; the conservative variant is simpler and matches the typical static-analysis discipline.

The `compile_with_target` API is additive. The original `compile(program)` continues to work and is now a thin wrapper that passes `Target::host()`. Callers that do not care about cross-target portability see no change.

## Changes Made

### Source

- **`src/target.rs`** (new). Public `Target` struct, const presets (`host`, `wasm32`, `embedded_32`, `embedded_16`, `embedded_8`), bit-width accessors, runtime-validation method, AST walker for feature validation, and nine unit tests.
- **`src/compiler.rs`**. New `compile_with_target` public entry point. Existing `compile` is now a thin wrapper. The module emission uses the target's widths instead of the runtime constants.
- **`src/lib.rs`**. New `pub mod target` declaration.
- **`examples/target_aware_compile.rs`** (new). End-to-end demonstration.

### Knowledge Graph

- **`docs/decisions/BACKLOG.md`**. B10 marked as foundation-complete with the implemented surface, pre-existing infrastructure, and remaining open work documented separately.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T32.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The named B10 work has its foundation in place. The remaining open BACKLOG items are smaller refinements or items whose cost is not yet justified:

- B10 follow-on. Target-specific runtime builds, cross-target codegen, and target-defined primitive types.
- Recursion-depth attestation API for recursive closures.
- `Op::CallIndirect` flow analysis for tighter WCET bounds.
- `Type::Unknown` sentinel removal (B1 follow-on, requires native function signatures).
- f-string finer-grained span attribution.
- Block expressions as primary parsing form.

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session added a producer-side surface for cross-architecture portability that builds on the wire-format groundwork already in place. The compiler now accepts a `Target` descriptor and validates the program against the target's capabilities before emitting bytecode. The implementation is intentionally scoped to the producer side; cross-target runtime builds and target-defined primitive types are documented as the remaining open work but not pursued here.
