# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: Data segment formalization pass
**Status**: Complete

## Verification

**Command**: `cargo test && cargo clippy --tests -- -D warnings && cargo fmt --check`
**Result**: 232 tests pass. Zero clippy warnings. Format clean. Sixteen new tests added covering data segment source conformance.

## Summary

Conducted a research and documentation formalization pass for the data segment design that resolves a previously stalled feasibility analysis. The data segment is the fourth memory region in the Keleusma runtime, corresponding to the conventional `.data` section of an executable. The pass added a new section to the related work document covering hot code update with persistent state, updated the architecture and design documents with the four-section memory model analogy, added five new resolved decisions, added a fixed-size field type constraint section to the type system document, added new glossary entries, and refreshed the process documents. The source code in the working tree was then brought into conformance with the specification, namely the singular data block constraint, the fixed-size field type validation, and the verifier bounds check.

## Changes Made

### Knowledge Graph

- **docs/reference/RELATED_WORK.md**: Added Section 8 covering hot code update with persistent state. Subsections cover Erlang and Open Telecom Platform multi-version coexistence, mode change in SCADE and Lustre, dynamic software update in Ksplice and Kitsune, and the conventional executable section layout. The Relationship to Keleusma subsection enumerates the four-region memory model analogy and the differences from each prior art lineage. Added five bibliography entries H1 through H5 in a new Hot Code Update subsection.
- **docs/architecture/EXECUTION_MODEL.md**: Replaced the memory regions section with the four-section table mapping bytecode chunks to `.text`, the constant pool to `.rodata`, the data segment to `.data`, and the arena and operand stack to `.bss`. Replaced the data segment layout invariance claim with Replace semantics. Restructured the hot code swapping section to cover atomicity, cross-swap value handling, rollback, stale slot behavior, and stack quiescence as separate subsections with citations to the new bibliography entries.
- **docs/architecture/LANGUAGE_DESIGN.md**: Replaced the memory model section with the four-region table and the description of host ownership of the data segment. Updated the hot code swapping section to reflect Replace semantics and the relaxed schema invariance.
- **docs/reference/GLOSSARY.md**: Added entries for Data segment, Hot code update, Logical atomicity, Mode change, Replace semantics, Schema, and Stack quiescence. Updated the Dialogue type, Host, RESET, and Module entries to reflect the new specification.
- **docs/design/TYPE_SYSTEM.md**: Added a Data Segment Field Types section enumerating the admissible type forms with rationale.
- **docs/decisions/RESOLVED.md**: Added R24 through R28 covering the data segment as the fourth memory region, schema fixity within image and mutability across hot updates, Replace semantics, logical atomicity, and the singular data block per program constraint.
- **docs/decisions/PRIORITY.md**: Added P6 enumerating the open implementation items required to complete the data segment.
- **docs/decisions/BACKLOG.md**: Updated B4 to reflect the new design.
- **docs/process/TASKLOG.md**: Refreshed to reflect the V0.0-M1 and V0.0-M2 completions and the V0.0-M3 milestone now in progress.

### Source Code

The previously in-progress data segment scaffolding from the prior session was preserved as-is, then extended with the conformance changes required by the specification.

- **src/compiler.rs**: Added the singular data block constraint enforcing R28 with a clear error message. Added the `validate_data_field_type` recursive helper that walks tuple, array, option, struct, and enum type expressions and rejects non-admissible types per the table in TYPE_SYSTEM.md. Added thirteen unit tests covering admissible primitives, unit, tuples, arrays, options, structs, enums, rejection cases for each composition path, and the singular block constraint.
- **src/verify.rs**: Already contained slot bounds checking for GetData and SetData with three unit tests. Verified to be correct.
- **No other source files were modified during the conformance pass.**

## Unaddressed Concerns

1. **Schema mismatch detection.** The specification places the burden of supplying a conforming data segment instance on the host. The mechanism by which the VM detects a schema mismatch at install time is not specified. Three options exist. The host attests conformance and the VM trusts it. The VM checks a schema hash recorded in the module against a hash supplied by the host. The VM performs structural type checking against a schema descriptor in the new code image. The choice has implications for the trust boundary and for the certification gap analysis.

2. **Native function access to the data segment.** The current design does not specify whether native functions may receive the data segment as a parameter or whether they must read fields through `GetData` invoked from the script before the call. The natural choice is the latter for primitive fields. Composite fields require a marshalling discussion that is not in the present documentation.

3. **Default initialization on first load.** The host supplies the initial data segment instance at first load. The protocol for what happens if the host fails to supply an instance, namely whether the VM refuses to construct or whether it constructs with a default-initialized segment, is not specified.

4. **Multiple data blocks.** R28 forbids more than one data block per program. The implementation in the working tree admits multiple blocks at the parser level. The compile-time error must be added.

5. **Soundness of the hot update mechanism.** The hot update mechanism has no formal correctness proof. Tool qualification under any safety standard would require a formal specification of the swap semantics and a machine-checked proof that the swap preserves the four guarantees. This is consistent with the certification gap analysis in RELATED_WORK Section 7.

## Intended Next Step

The remaining V0.0-M3 work is the host interoperability layer (T7) and end-to-end integration tests (T8). T7 requires deciding between the `repr(C)` discipline and an offset-based accessor scheme. This decision interacts with item 1 in Unaddressed Concerns above. T8 requires host-side support and is therefore better deferred until the host interoperability layer is decided.

Recommend resolving item 1 in Unaddressed Concerns next, since it gates T7. The three options are host attestation, schema hash comparison, and structural type checking against a schema descriptor.

Await human prompt before proceeding.

## Session Context

The data segment feasibility analysis stalled in a prior session. The current session resumed it through clarification of four originally ambiguous specification points and one philosophical commitment to a singular data block per program. The formalization pass closed the specification at the level required for a written design document. The source code was then brought into conformance with the specification within the same session.
