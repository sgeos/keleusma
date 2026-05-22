# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-22
**Status**: V0.2.x research pass and pre-implementation cleanup complete. The branch carries strategy-doc updates, the empirical M1 validation of the corrected R4.1 design, a cross-document consistency audit, the enrolled-keys execution spec, and process-doc updates. None of this work is committed yet.

## Summary of work since the V0.2.0 publication

The session work falls into three layers.

### Layer 1: Autonomous research pass (2026-05-21)

Twenty firings over a single AFK session resolved every open design question recorded in the V0.3.0, V0.4.0, and V0.5.0 strategy documents, plus three cross-cutting threads (STM32N6570-DK testbed, vintage-CPU homebrew, perpetual operational scenarios). All public-side output landed in `tmp/research/`. The perpetual operational scenarios landed in internal materials.

A post-hoc web-research pass surfaced two material corrections and two revisions:

- **R4.1 corrected**: LLVM coroutine intrinsic family is returned-continuation (`@llvm.coro.id.retcon`), not switched-resume. The corrected design is empirically validated by R4.1 milestone M1 (see below).
- **R4.3 revised**: LLVM version pin moved from LLVM 17 to LLVM 19. LLVM 22.1 is the current stable as of May 2026; the original LLVM 17 recommendation reflected a 2025 cutoff.
- **RC.1 revised**: probe-rs has STM32N6 target support but without flash algorithms. The existing `examples/rtos/` infrastructure may flash via STM32CubeProgrammer rather than probe-rs.
- **RC.2 corrected**: SNES is not supported by llvm-mos (65c816 is outside its 6502 scope). The Year-1 NES demonstration deliverable stands; SNES needs a separate effort.

Full record in `tmp/research/WEB_RESEARCH_APPENDIX.md`. Implementation-order synthesis in `tmp/research/IMPLEMENTATION_ORDER.md`. Status log in `tmp/research/STATUS.md`.

### Layer 2: Strategy-doc integration (2026-05-22)

The R-doc resolutions were inlined into the canonical strategy documents under `docs/`. Per operator selection of the "inline" integration option:

- `docs/roadmap/V0_3_0_SELF_HOSTING.md` gained a "Resolved design questions" section covering R3.1-R3.5. The "Required surface-language features" section was reduced to reflect the resolved state. Open questions reduced to three remaining items.
- `docs/roadmap/V0_4_0_NATIVE_CODEGEN.md` gained the same section for R4.1-R4.5 with the corrections applied. The "LLVM coroutine intrinsics" and "Sub-coroutine lowering" sections were rewritten to use retcon instead of switched-resume. Open questions reduced to five items including R4.1 milestone M1 as the load-bearing risk and native-side WCET cost models as the new gap.
- `docs/roadmap/V0_5_0_KELEUSMA_HOST.md` gained the same section for R5.2-R5.5. R5.1 cross-references SUB_COROUTINES.md. Open questions reduced to four items including cross-module monomorphisation and sub-coroutine hot-swap interaction.
- `docs/architecture/SUB_COROUTINES.md` gained R5.1's surface syntax inline (spawn/resume/release with signature clauses) and corrected the LLVM lowering to retcon. Six of seven preliminary open questions marked resolved.
- `docs/decisions/RESOLVED.md` gained R44-R48 as pointer entries to the new sections.
- `docs/roadmap/IMPLEMENTATION_ORDER.md` was copied from `tmp/research/`.
- `docs/process/AUTONOMOUS_RESEARCH_LOOP.md` was newly drafted to document the autonomous-loop process.

### Layer 3: Pre-implementation cleanup (2026-05-22)

Three explicit cleanup tasks completed:

1. **R4.1 milestone M1: LLVM retcon spike.** `tmp/research/llvm_retcon_spike/` carries two LLVM IR fragments, two C harnesses, and the lowered objects. Both lower cleanly through LLVM 22.1.6's coro-early plus coro-split plus coro-cleanup passes. Native object files build via `llc -filetype=obj`. The harness runs validate: 32-byte buffer scenario produces the expected 10/20/30 yield sequence with the allocator never called; 1-byte buffer scenario causes the allocator to fire exactly once with the actual frame size (8 bytes). The corrected retcon design is now empirically grounded. Full results in `tmp/research/llvm_retcon_spike/RESULTS.md`.

2. **Cross-document consistency audit.** `tmp/research/CONSISTENCY_AUDIT.md` records the audit. Five inconsistencies found, all of the same shape: R-docs in `tmp/research/` were not back-edited when their recommendations were corrected. The strategy docs and the appendix are canonical; the R-docs are stale provenance. Five correction banners added to the affected R-docs (R4.1, R4.3, R4.4, R4.5, RC.2) pointing to the appendix and the canonical source.

3. **TASKLOG and REVERSE_PROMPT update.** `docs/process/TASKLOG.md` Current Phase, Active Milestone, Outstanding TODO, and Task Breakdown sections reflect the post-publication state. This document is the AI-to-Human update.

### Additional artefact

A spec for a V0.2.x strict-mode enrolled-keys execution feature was drafted at the operator's request (`tmp/enrolled_keys_execution.md`). Implementation deferred pending operator decision. The cryptographic infrastructure (R42 Ed25519 signing) is in place; the feature is a CLI policy layer estimated at one to two days of work.

## What the operator owns

### Decisions pending

1. **Commit the research-pass integration and cleanup.** Six modified docs (V0.3/V0.4/V0.5 strategies, SUB_COROUTINES.md, RESOLVED.md, roadmap/README.md), several new docs (AUTONOMOUS_RESEARCH_LOOP.md, IMPLEMENTATION_ORDER.md, M1 spike results, consistency audit, enrolled-keys spec), and five R-doc correction banners are uncommitted. Conventional-commit shape suggested: `docs: integrate V0.3/V0.4/V0.5 research findings and M1 spike validation`.

2. **Whether to begin V0.2.x strict-mode enrolled-keys implementation.** Spec at `tmp/enrolled_keys_execution.md`. Estimated effort one to two days. Cryptographic infrastructure exists.

3. **Whether to begin V0.3.0 implementation.** Effort estimate four to eight months for a single developer per `docs/roadmap/IMPLEMENTATION_ORDER.md`. Recommended V0.2.x prep work in step 0 of that document.

4. **Whether to disposition `tmp/research/`.** Two options: retain as historical provenance (correction banners point readers to canonical state) or sunset entirely (the strategy docs are authoritative; the R-docs are scratch).

### Items to verify before next implementation work

- The five R-doc correction banners are accurate; the operator may want to read them in passing.
- The empirical M1 spike under `tmp/research/llvm_retcon_spike/` is reproducible per the commands in `RESULTS.md`. The operator may want to re-run to confirm.
- The strategy doc edits are consistent with operator preference on tone and structure.

## Open concerns

None blocking. Three sub-points:

- **The strict-mode signing feature is not in the V0.2.x BACKLOG.md or PRIORITY.md.** When the operator commits, consider whether to add an entry there for traceability.
- **`docs/decisions/BACKLOG.md` and `PRIORITY.md` were not audited for items now resolved by the R-doc sequence.** Some entries may reference V0.3/V0.4/V0.5 questions that are now settled. A future cleanup pass could mark them resolved.
- **The Phase α N6 testbed scaffolding at `tmp/research/rtos_n6_testbed/`** remains a draft and is not integrated into `examples/rtos/`. The probe-rs flash-algorithm gap surfaced by the web research means the integration path should clarify the actual flashing mechanism.

## Verification

```bash
# The empirical M1 spike rebuild and run.
cd tmp/research/llvm_retcon_spike
/opt/local/libexec/llvm-22/bin/opt -S \
    -passes='module(coro-early),cgscc(coro-split),module(coro-cleanup)' \
    retcon_spike.ll -o retcon_lowered.ll
/opt/local/libexec/llvm-22/bin/llc -filetype=obj retcon_lowered.ll -o retcon.o
clang harness.c retcon.o -o harness
./harness
# Expected output: 10 / 20 / 30 yield sequence; allocator hooks 0; deallocator hooks 0.

# Build remains clean.
cargo build && cargo test --quiet
```

## Intended Next Step

Operator decision on the three pending items in "Decisions pending" above. The implementation-side next-step, contingent on operator authorisation, is the V0.2.x strict-mode enrolled-keys execution feature drafted at `tmp/enrolled_keys_execution.md`. The other paths (V0.3.0 implementation start, longer-horizon work) are operator-scheduled.
