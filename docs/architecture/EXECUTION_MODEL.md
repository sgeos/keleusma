# Execution Model

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

This document describes the execution model for Keleusma. The model separates temporal control, memory phase control, and host interaction into distinct primitives. It defines two temporal domains and a structural program layout that enables static verification of safety properties.

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

The arena is a single contiguous allocation using bump allocation. The stack grows from one end of the arena. There is no heap initially. Allocations advance a pointer linearly through the contiguous buffer. Deallocation occurs only at RESET, when the entire arena is cleared by resetting the bump pointer to the start. This design eliminates fragmentation and ensures O(1) allocation and deallocation.

The arena persists across yields within a single stream phase. It is cleared only at RESET. No dynamic allocation survives across phases. Memory bounds are statically analyzable per stream phase.

Three memory regions exist.

- **Arena.** Ephemeral per stream phase. Single contiguous bump-allocated buffer with stack growing from one end. Cleared at RESET.
- **Read-only sections.** Immutable text (code) and rodata (constants). Double-buffered and swappable at RESET boundaries.
- **Host state.** External to the VM. Managed by the host application.

## Hot Code Swapping

Hot code swapping occurs only at RESET boundaries and uses double buffering. The following requirements apply.

- The YIELD signature (dialogue type A exchanged for B) remains invariant across the entire STREAM and across swaps.
- Only text and rodata segments may change.
- The arena is cleared before new code executes.
- WCET and reset-to-reset bounds are certified per routine independently.

Different routines (f vs g) may have different WCETs, which are declared in a static header for the host scheduler to validate before accepting the swap.

### Double-Buffered Swap Mechanism

The host loads new text and rodata into a secondary buffer while the current code continues executing in the primary buffer. When the VM reaches RESET, it activates the secondary buffer, making it the new primary. The old primary buffer is retained as the secondary, available for rollback if the host determines that the new code should be reverted. This mechanism ensures that the swap is atomic from the VM's perspective and that no partially loaded code is ever executed.

## Turing Completeness

Individual time slices are not Turing complete. Each yield-to-yield slice executes a bounded number of instructions and then suspends. The VM in isolation cannot perform unbounded computation within a single slice.

The VM-Host pair is Turing complete. Turing completeness arises from the unbounded RESET cycle with the host providing the "tape" through YIELD exchanges. The host supplies new input on each resumption, and host-controlled state that persists across resets serves as the unbounded external memory. Computation can span arbitrarily many RESET cycles.

This separation is deliberate. The VM executes finite, certifiable slices. The host drives the unbounded computation loop. Industrial certification applies to individual slices, not to the overall infinite execution.

## Structural Verification

Programs are verified at load time through structural analysis. The verifier (`verify()` in `src/verify.rs`) performs five passes per chunk.

1. **Block nesting.** Every If is matched by EndIf (with optional Else). Every Loop is matched by EndLoop.
2. **Offset validation.** All jump targets are within bounds and point to correct matching delimiters.
3. **Block type constraints.** Func chunks contain no Yield, Stream, or Reset. Reentrant chunks contain at least one Yield. Stream chunks contain exactly one Stream, one Reset, and at least one Yield.
4. **Break containment.** Every Break and BreakIf is inside a Loop/EndLoop.
5. **Productivity rule.** Abstract interpretation over a two-element lattice verifies that all control flow paths from Stream to Reset pass through at least one Yield.

A program is valid only if all paths satisfy these constraints. Invalid programs are rejected before execution begins.

Additionally, `wcet_stream_iteration()` computes the worst-case execution cost of one Stream-to-Reset iteration. Each instruction carries a relative cost via `Op::cost()`. The analysis recursively traverses block-structured control flow, taking the maximum cost branch at each join point, and returns the worst-case total as a unitless integer.

See [TARGET_ISA.md](../reference/TARGET_ISA.md) for the full structural ISA specification.

## Cross-References

- [LANGUAGE_DESIGN.md](./LANGUAGE_DESIGN.md) describes the language-level design goals and guarantees.
- [TARGET_ISA.md](../reference/TARGET_ISA.md) specifies the structural ISA block types and verification rules.
- [COMPILATION_PIPELINE.md](./COMPILATION_PIPELINE.md) describes the current compilation pipeline.
