# Execution Model

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

This document describes the target execution model for Keleusma. The model separates temporal control, memory phase control, and host interaction into distinct primitives. It defines two temporal domains and a structural program layout that enables static verification of safety properties.

## Two Temporal Domains

### Yield Domain (Control Clock)

WCET is measured from YIELD to YIELD. The YIELD primitive performs a bidirectional exchange with the host (A exchanged for B), suspends execution, and resumes at the next instruction. YIELD does not clear the arena. Multiple YIELDs may occur before RESET. Each yield-to-yield slice must be statically bounded.

### Reset Domain (Phase Clock)

Swap latency is measured from RESET to RESET. The RESET primitive clears arena memory, performs a hot swap if one is scheduled, and jumps to the STREAM entry point. RESET is the only global back-edge in the program. Arena memory persists across yields but is cleared at RESET.

## Minimal Valid Stream Program

The simplest streaming program consists of three primitives.

```
STREAM
  YIELD
RESET
```

Execution follows the cycle: RESET -> STREAM -> YIELD -> RESET -> ...

This establishes the single infinite control cycle that defines Keleusma execution.

## Global Structural Constraints

A Keleusma program contains the following structural elements.

- Zero or one STREAM region.
- If STREAM exists, exactly one RESET. RESET -> STREAM is the only unbounded cycle.
- Zero or more FUNCTION regions (atomic).
- Structured bounded loops only (LOOP_N with compile-time constant bounds).

## Temporal Hierarchy

```
Instruction
  |
Basic Block
  |
Yield Slice (bounded WCET)
  |
Stream Phase (bounded RESET-to-RESET)
  |
Infinite Execution (via RESET cycle)
```

This defines a two-clock deterministic control VM. Fine-grained scheduling operates via YIELD. Coarse-grained phase control operates via RESET.

## Arena Memory Model

The arena consists of a stack and a scratchpad heap. It persists across yields within a single stream phase. It is cleared only at RESET. No dynamic allocation survives across phases. Memory bounds are statically analyzable per stream phase.

Three memory regions exist.

- **Arena.** Ephemeral per stream phase. Stack and scratchpad heap. Cleared at RESET.
- **Read-only sections.** Immutable text (code) and rodata (constants). Swappable at RESET boundaries.
- **Host state.** External to the VM. Managed by the host application.

## Hot Code Swapping

Hot code swapping occurs only at RESET boundaries. The following requirements apply.

- The YIELD signature (dialogue type A exchanged for B) remains invariant across the entire STREAM and across swaps.
- Only text and rodata segments may change.
- The arena is cleared before new code executes.
- WCET and reset-to-reset bounds are certified per routine independently.

Different routines (f vs g) may have different WCETs, which are declared in a static header for the host scheduler to validate before accepting the swap.

## Turing Completeness

Internal slices are bounded, but the overall system is Turing complete for the following reasons.

- STREAM provides unbounded execution via the RESET cycle.
- Host interaction through YIELD allows unbounded state evolution.
- Computation can span arbitrarily many RESET cycles.

Each slice is finite and certifiable. The overall stream machine is computationally universal.

## Structural Verification

Programs are verified at load time through structural analysis. A linear scan "colors" blocks based on productivity.

- All paths from STREAM to RESET must pass through at least one YIELD (productivity rule).
- All FUNC blocks must be free of yields (atomic function rule).
- All REENTRANT blocks must contain at least one YIELD.
- LOOP_N bounds must be compile-time constants.

A program is valid only if all paths satisfy these constraints. Invalid programs are rejected before execution begins. See [TARGET_ISA.md](../reference/TARGET_ISA.md) for the full structural ISA specification.

## Cross-References

- [LANGUAGE_DESIGN.md](./LANGUAGE_DESIGN.md) describes the language-level design goals and guarantees.
- [TARGET_ISA.md](../reference/TARGET_ISA.md) specifies the structural ISA block types and verification rules.
- [COMPILATION_PIPELINE.md](./COMPILATION_PIPELINE.md) describes the current compilation pipeline.
