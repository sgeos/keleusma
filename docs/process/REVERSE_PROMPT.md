# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T33 Documentation deduplication and streamlining pass.
**Status**: Complete. Architecture docs deduplicated, Implementation Mapping subsection added to EXECUTION_MODEL, stale postcard reference corrected, LANGUAGE_DESIGN streamlined to defer to EXECUTION_MODEL for canonical specs, COMPILATION_PIPELINE accuracy updated.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 506 tests pass workspace-wide. No code changes; the test pass confirms documentation edits did not affect the build.
- Format clean.
- Clippy clean.

## Summary

This pass surveys and streamlines the architecture documentation, removing duplication between LANGUAGE_DESIGN.md and EXECUTION_MODEL.md, fixing a stale wire-format reference, and adding the source-level Implementation Mapping subsection that the prior conversation flagged as missing.

### Implementation Mapping subsection

EXECUTION_MODEL.md gained a new subsection between the existing ABI table and the Arena/Operand/Heap subsection. The mapping table names, for each conceptual region, the source location, the construction path, and the runtime access mechanism. Lifetime invariants document how each region behaves across `Op::Reset` and `Vm::replace_module`. Memory bookkeeping clarifies which costs go through the arena's WCMU budget and which use the global allocator. The `BytecodeStore` Owned vs Borrowed mapping documents the ownership orthogonality.

### Stale postcard reference

Line 129 of EXECUTION_MODEL.md described the wire format as postcard-based. This is stale; the format moved to rkyv earlier in V0.1-M2 (Phase 1 of P10). The paragraph is rewritten to describe the rkyv format with the correct rationale (zero-copy execution from `.rodata` per P10) and the correct list of archived types. Cross-reference to R39 in RESOLVED.md added for the design decision.

### LANGUAGE_DESIGN.md streamlining

Several sections of LANGUAGE_DESIGN.md duplicated content that EXECUTION_MODEL.md specifies canonically. Each was reduced to a language-level summary that references EXECUTION_MODEL.md for the full specification:

- Memory Model. The four-region table and the long arena/data-segment paragraphs collapsed into two short paragraphs about surface-language semantics and a single reference to the canonical specification.
- Hot Code Swapping. The detailed mechanics paragraph reduced to a language-level summary; rollback, atomicity, stale-slot behavior, and update-point details are deferred to EXECUTION_MODEL.
- Turing Completeness. The standalone subsection collapsed into a single paragraph in the new "Turing Completeness and Temporal Domains" section, which absorbed the previously-duplicated Two Temporal Domains list as well.
- Coroutine Model. Retained but extended with the resume-value error pattern (B7) reference.

### Scope section update

The "Scope Exclusions" section was renamed to "Scope Inclusions and Exclusions" and updated to reflect features now implemented: Hindley-Milner inference foundation (B1), generics with traits and bounds (B2.2/B2.3), monomorphization (B2.4), closures with capture and recursion (B3), f-string interpolation (B6), and string concatenation/slicing as utility natives (B5b). The previous list of these features as exclusions was misleading because they all landed during V0.1-M3.

### COMPILATION_PIPELINE.md accuracy

The pipeline diagram was a single-line summary that omitted the typecheck/monomorphize/hoist passes. Expanded to a multi-line layout showing each stage. The `compile()` signature documentation was extended to include `compile_with_target()` (B10). The recursion-detection note was wrong — it claimed compilation rejects cycles, but recursion detection now lives in `verify::module_wcmu`. Corrected. The `Vm::new()` signature documentation was missing the arena parameter and the lifetime annotations; updated. New `Vm::resume_err()` (B7) added to the API surface listing.

## Trade-offs and Properties

The deduplication strategy chose to keep LANGUAGE_DESIGN.md focused on language-level concerns (philosophy, guarantees, surface syntax categories, type system) and EXECUTION_MODEL.md focused on runtime concerns (memory layout, temporal domains, hot swap mechanics, implementation mapping). LANGUAGE_DESIGN now references EXECUTION_MODEL for canonical runtime specifications rather than reproducing them.

The Implementation Mapping subsection adds concrete source-level orientation that previously required reading source comments. The cost is that the subsection now ties EXECUTION_MODEL.md to specific implementation choices; if the implementation changes (such as a future runtime build with a different `Value` representation), the table must be updated. This is acceptable because the subsection explicitly notes "the wire format does not bind to specific implementation choices" and the table describes the present runtime build.

The net documentation size is slightly larger (LANGUAGE_DESIGN -16 lines, EXECUTION_MODEL +17 lines, COMPILATION_PIPELINE +17 lines). The increase is concentrated in concrete new information (the Implementation Mapping table) and accuracy updates. Duplication is reduced even as overall information content goes up.

## Files Touched

- `docs/architecture/EXECUTION_MODEL.md`. Added Implementation Mapping subsection. Fixed stale postcard reference.
- `docs/architecture/LANGUAGE_DESIGN.md`. Streamlined Memory Model, Hot Code Swapping, Turing Completeness, Two Temporal Domains. Renamed Scope Exclusions to Scope Inclusions and Exclusions, updated to reflect implemented features.
- `docs/architecture/COMPILATION_PIPELINE.md`. Expanded pipeline diagram. Updated `compile`, `compile_with_target`, and `Vm::new` signatures. Corrected recursion-detection placement.
- `docs/process/TASKLOG.md`. New row for V0.1-M3-T33.
- `docs/process/REVERSE_PROMPT.md`. This file.

## Remaining Open Priorities

The architecture docs are now consistent with the implementation as of V0.1-M3-T32. The standard document-strategy items remain:

- DOCUMENTATION_STRATEGY review pass for adherence to the maintenance discipline.
- TYPE_SYSTEM.md may reference outdated information about exclusions; not surveyed in this pass.
- GRAMMAR.md is large (1099 lines) and may have stale entries.
- INSTRUCTION_SET.md and TARGET_ISA.md likely need updates for the new opcodes (`Op::MakeRecursiveClosure`, `Op::PushFunc`, `Op::MakeClosure`, `Op::CallIndirect`).

The `keleusma-arena` registry version is still v0.1.0.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session focused on documentation hygiene rather than feature work. The output is a documentation set that is internally consistent, points to canonical specifications rather than duplicating them, and includes the Implementation Mapping subsection that the prior question flagged as missing. The accuracy updates to COMPILATION_PIPELINE close several visible drift issues.
