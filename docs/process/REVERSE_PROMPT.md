# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: Roadmap and documentation pass for V0.3.0 through V0.5.0 complete on the `feat-signed-modules` branch. Three new strategy documents land: V0.3.0 self-hosting expanded for implementation handoff, V0.4.0 native code generation drafted, V0.5.0 Keleusma-hosted host drafted. A preliminary sub-coroutine specification lands under `docs/architecture/`. The `docs/` tree is reorganized so authoritative specifications live in a new `docs/spec/` section; `docs/design/` is retired; `docs/architecture/` and `docs/reference/` are reframed. All 380 markdown cross-references in `docs/` resolve. Project-root README, CHANGELOG, and CLAUDE.md cross-references all updated. Ready for merge to `main`.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Continue the V0.3.0 self-hosting strategy with bootstrap, inter-stage data shapes, success criteria, risks | `docs/roadmap/V0_3_0_SELF_HOSTING.md` expanded to a strategy-ready document. Bootstrap procedure (Phase A cross-compile, Phase B self-compile, Phase C fixed point) documented; inter-stage data shapes sketched (Token, Declaration, CompiledChunk); required surface-language features inventoried in three tiers (sufficient at V0.2.0, exists with caveats, missing or limited); success criteria and risks tabulated. |
| Add incremental migration ordering to V0.3.0 | New "Incremental migration ordering" section recommends Lexer → Parser → Compiler with per-step regression-corpus equivalence checks against the all-Rust baseline. Bootstrap procedure scoped to the final step. The two alternative orderings (compiler-first, parser-first) explicitly rejected. |
| Sub-coroutine preliminary spec lives somewhere | New `docs/architecture/SUB_COROUTINES.md` covers the asymmetric coroutine model (call-down / yield-up); sub-coroutine state (program counter, call-frame stack, operand stack, arena slot, all co-located in the slot); arena-slot reservation discipline (the slot cannot be reassigned during execution; ephemeral and persistent differ in what happens at completion); spawn-time slot availability policies; coroutine handle as a typed value with lifetime bounded by the parent scope; new opcodes `SpawnCoroutine`, `ResumeCoroutine`, `ReleaseCoroutine` (preliminary names) with explicit lowering to LLVM coroutine intrinsics in V0.4.0; surface syntax candidates (open); verifier extensions; hot-replacement quiescence; relationship to existing constructs; out-of-scope items; open questions. |
| Pure-by-default with three-mode purity discipline (pure / impure / transitive) | Folded into V0.5.0 strategy doc. Pure-by-default settled; impurity must be declared explicitly; transitive functions have pure bodies but accept impure callbacks with effective purity inherited from the call site (purity polymorphism). Strict transitive-impurity prohibition affirmed: a pure function cannot reach an impure function through any chain of calls. Vocabulary collision between the two senses of "transitive" addressed in prose. |
| Draft V0.4.0 native code generation strategy | New `docs/roadmap/V0_4_0_NATIVE_CODEGEN.md` covers LLVM as the backend; bytecode-as-verification-IR plus native-as-deployment-shape; sub-coroutine lowering to LLVM coroutine intrinsics with custom arena allocator; static-library `staticlib` deliverable; hot-replacement-friendly versus performance-friendly build modes (cross-module inlining suppression cost); best-effort WCET on native; bootstrap procedure (Phase A IR generator, Phase B cross-compile self-hosted compiler, Phase C validation); vintage-processor targets framed as aspirational (6502 via llvm-mos, 68000 upstream, Z80 via SDCC); three V0.5.0 refinements the V0.4.0 research surfaces. |
| Draft V0.5.0 Keleusma-hosted host strategy | New `docs/roadmap/V0_5_0_KELEUSMA_HOST.md` covers two driver shapes (`impure fn main`, `impure loop main`); three-mode purity discipline; file-based modules in the Modula-2 / Ada tradition with explicit interface declarations carrying declared bounds; declared sub-DAG arena partitions with master-WCMU-based allocation; structured live code update with interface-fingerprint enforcement following the Erlang/OTP model; four-phase bootstrap (α cross-host bytecode, β self-host compiler, γ fixed point, δ migrate to native shape); native shape as primary deployment, bytecode as fallback. Risks, out-of-scope, open questions, prior art with citations. |
| Apply V0.4.0 research findings to V0.5.0 and the sub-coroutine spec | V0.5.0 doc gains "Hot-replacement granularity is a build-mode choice" subsection (table of hot-replacement-friendly versus performance-friendly builds) and "Native WCET is best-effort, not hard" subsection. Sub-coroutine spec gains the lowering-to-LLVM-coroutine-intrinsics table under "New opcodes." |
| Remove `docs/roadmap/PHASE_0_BOOTSTRAP.md` | File deleted. Status was internally contradictory ("In Progress" header with all milestones marked "Complete"); milestone definitions conflicted with TASKLOG.md which is the designated source of truth. Reference removed from `docs/roadmap/README.md` Contents table and from `docs/DOCUMENTATION_STRATEGY.md` directory tree. |
| Remove the Phase Overview table from `docs/roadmap/README.md` | Done. Stale relative to current strategy docs. The Contents table below it is the authoritative roadmap listing. |
| Audit `docs/` tree for staleness and organization; fix what is found | Two-phase response. Option A (staleness): `docs/DOCUMENTATION_STRATEGY.md` tree previously missed `guide/` (9 files), `extras/` (8 files), and three architecture files; refreshed to match the current filesystem. Finding Information table expanded with the previously missing entries. `docs/README.md` Quick Reference table similarly expanded. Option C (reorganization): new `docs/spec/` section consolidates the six authoritative specifications previously scattered across architecture/, design/, and reference/. Six files moved via `git mv` with history preserved. `reference/TARGET_ISA.md` renamed to `spec/STRUCTURAL_ISA.md` (old filename was misleading). `docs/design/` retired entirely. `docs/architecture/` reframed as narrative descriptions of the implemented system. `docs/reference/` pruned to GLOSSARY plus RELATED_WORK. All 380 markdown cross-references in `docs/` validated to resolve. Project-root README, CHANGELOG, and CLAUDE.md cross-references all updated. |

## Verification matrix

```bash
# Cross-reference validation: 380 references in docs/, zero broken
find docs -name "*.md" -type f | xargs -I {} \
    sh -c '...path-resolution loop...'                                   # 380 checked, 0 broken

# Section README claimed contents match directory contents
ls docs/<section>/ vs grep refs in docs/<section>/README.md               # 8 of 8 sections clean

# DOCUMENTATION_STRATEGY.md tree matches filesystem
find docs -maxdepth 2 -type f -name "*.md" vs tree diagram                # 41 files plus 8 directories, exact

# No stragglers in tracked files (CLAUDE.md, README.md, CHANGELOG.md, docs/)
grep for old paths (design/, reference/INSTRUCTION_SET, etc.)             # zero results

# Breadcrumb navigation present on every non-README file in docs/
grep '^> \*\*Navigation' docs/**/*.md                                     # all present
```

No source code changed in this session round; the workspace test, clippy, format, and example results from 2026-05-20 (956 tests across 16 suites green, 963 with `--features signatures`, clippy strict clean, fmt idempotent, STM32N6570-DK boot path verified) remain valid.

## Open concerns

None material. Three items recorded for the record but do not block merge.

1. **`docs/process/TASKLOG.md` historical entries retain old paths verbatim.** Earlier entries describing work done before the docs reorganization continue to mention `docs/design/`, `docs/reference/INSTRUCTION_SET.md`, etc. These are intentionally preserved as historical records; rewriting them would obscure project history. New entries from this date forward use the new paths.

2. **V0.4.0 LLVM coroutine custom-allocator integration carries a research-uncertainty flag.** The custom-allocator API for arena-resident coroutine frames has been stable in LLVM since version 14, but the precise ergonomics need verification during V0.4.0 implementation. The bytecode-shape implementation is unaffected; the V0.4.0 strategy doc records the uncertainty explicitly.

3. **V0.5.0 strategy is preliminary by design.** Several edges remain open (mutual-exclusivity refinement scope, sub-coroutine surface syntax, arena partitioning unit-of-declaration, interface-fingerprint hash function, module file naming, transitive-purity callback-storage edge cases). Each is recorded in the strategy doc's Open Questions section. None blocks the strategy itself.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b, 7c, 8 complete) |
| B21 | Value-side IFC negative labels via product lattice | Deferred (forward-looking; admitted when forcing case appears) |
| B22 | Sub-coroutines as callable ephemeral loops | Now load-bearing for V0.5.0; specification under `docs/architecture/SUB_COROUTINES.md` |

## Intended Next Step

The `feat-signed-modules` branch is ready for merge to `main`. The branch carries the V0.2.0 signed compiled modules feature (R42), the cross-architecture rkyv decode regression fix, the V0.2.0 ISA documentation sync pass, the negative information-flow labels feature (R43), and now the V0.3.0 / V0.4.0 / V0.5.0 strategy documents and the docs/ tree reorganization.

The natural next step is one of:

- Merge `feat-signed-modules` to `main` and prune the branch. Recommended.
- Manual `cargo publish` of the V0.2.0 crate. Operator-owned.
- A B15 follow-on: remove `Type::Unknown` entirely.
- V0.3.0 self-hosting implementation begins. Substantial scope; first concrete step is the Lexer migration per the incremental migration ordering.
- Operator selection of a different directive.
