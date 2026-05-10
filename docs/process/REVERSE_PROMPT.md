# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M3-T37 Document conservative-verification stance.
**Status**: Complete. New top-level section in LANGUAGE_DESIGN documents the property that the surface admits descriptions of programs the verifier rejects, with two formalized rejection categories.

## Verification

**Commands**:

```bash
cargo test --workspace
```

**Results**:

- 508 tests pass workspace-wide. No code changes in this commit.

## Summary

The user identified a Keleusma property that should be documented explicitly. The property may seem alien to readers from other paradigms because it inverts the conventional relationship between description and admission.

In most languages, programs that compile typically admit runtime execution. Static analysis is layered on top to flag potential issues. In Keleusma, the verifier is the source of truth and rejects programs whose execution time or memory use cannot be statically bounded. This rejection is intentional and defines the language's contract.

Two rejection categories are formalized in the new section.

**First category, provably unbounded constructs.** Programs that demonstrably admit unbounded execution at runtime fall here. Examples include `apply(apply, x)` on a generic identity-applier and any closure constructed through `Op::MakeRecursiveClosure`. The language describes these constructs so the verifier can definitively reject them. No future analysis would admit such programs because they are unbounded by construction.

**Second category, bounded but not yet proven constructs.** Programs whose execution is bounded in fact but whose proof has not yet been implemented also fall in the rejection set. The motivating example is non-recursive closure invocation, which is bounded for any individual call but currently rejected because indirect dispatch through `Op::CallIndirect` requires a flow analysis that has not been implemented. Future analysis improvements can move such programs out of the rejection set without changing the surface language.

The stance has design implications:

- Hosts develop scripts knowing that the verifier defines the admitted set, not the surface language.
- Tooling can highlight verifier-rejected constructs so developers see the gap before runtime.
- The language can grow its admitted set over time without surface changes. Candidate analysis improvements include flow analysis for indirect dispatch, attestation APIs for declared bounds, and inter-procedural reach extension.

The closure feature is the canonical example of this property in action. Closures are implemented end to end in the parse, type-check, monomorphize, hoist, and emit pipeline. The verifier rejects programs that invoke them through `Op::CallIndirect`. The implementation cost is justified because the verifier needs precise structures to reject precisely, and because future analysis improvements can lift second-category rejections without surface change.

## Trade-offs and Properties

The choice to add this as a top-level section in LANGUAGE_DESIGN rather than as a separate document reflects that it is a design property rather than a technical specification. The verification mechanics live in EXECUTION_MODEL and `verify.rs`. The stance about description versus admission is a language-level design choice and belongs in the language design document.

The two-category framing matches the user's articulation. Naming the categories First and Second helps later sections cite them precisely. An alternative naming such as "definitive rejection" versus "pending rejection" would emphasize the temporal aspect, but the user's emphasis was on what the analysis can and cannot prove, which the chosen names reflect.

The cross-references from EXECUTION_MODEL and BACKLOG ensure the stance is reachable from the practical loci where readers encounter rejection. Readers seeing the verifier rejection in EXECUTION_MODEL can follow the link to understand why the rejection is the language's intended behavior. Readers seeing the closure feature marked "Implemented; not WCET-safe" in BACKLOG can follow the link to understand why the implementation exists at all.

## Files Touched

- **`docs/architecture/LANGUAGE_DESIGN.md`**. New top-level "Conservative Verification" section after Five Guarantees. Cross-references list extended.
- **`docs/architecture/EXECUTION_MODEL.md`**. Indirect Dispatch and Recursion subsection cross-references the new stance.
- **`docs/decisions/BACKLOG.md`**. B3 entry cross-references the stance to explain why the closure feature exists despite verifier rejection.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T37.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The conservative-verification stance is now documented. Subsequent work falls outside this session's scope.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session documents a design property that has been implicit in recent verifier-hardening work but had not been articulated as a coherent stance. The new LANGUAGE_DESIGN section makes the stance explicit and gives readers a frame for understanding why the language describes more than the verifier admits.
