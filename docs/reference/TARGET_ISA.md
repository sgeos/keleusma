# Target ISA

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

## Overview

This document describes the target Instruction Set Architecture for Keleusma. The target ISA uses structured control flow and block-based nesting to make invalid or unproductive programs impossible to define or load. This is a design specification for the long-term execution model. The current implementation uses a different 48-instruction bytecode documented in [INSTRUCTION_SET.md](./INSTRUCTION_SET.md).

## Block Hierarchy

| Block/Opcode | Category | Rules |
|:---|:---|:---|
| STREAM | Global | Zero or one per program. Terminates with RESET. Entry point for the mission loop. |
| REENTRANT | Productivity | Must contain at least one YIELD. Used for logic that interacts with the host. |
| FUNC | Atomic | Cannot contain YIELD or REENTRANT. Pure, total, leaf-node functions. |
| YIELD | I/O | Suspends VM. Exchanges Output B for Input A. Persists arena and instruction pointer. |
| RESET | Boundary | Explicit terminator for STREAM. Resets arena, checks for hot swaps, jumps to STREAM. |
| LOOP_N | Iteration | Bounded by an immediate u32 value. No unbounded recursion allowed. |

## Computation Core

Pure, total arithmetic and logic. No partial operations.

| Instruction | Description |
|:---|:---|
| CONST | Push constant |
| LOAD | Load from local slot |
| STORE | Store to local slot |
| ADD, SUB, MUL, DIV | Arithmetic (DIV is totalized: division by zero yields zero or traps) |
| EQ, LT | Comparison |
| AND, OR, NOT | Logic |

## Control Flow

| Instruction | Description |
|:---|:---|
| JMP | Unconditional jump |
| BRANCH | Conditional branch |
| CALL | Call a FUNC |
| RETURN | Return from FUNC |

Restrictions:

- No indirect calls or jumps. All call targets are statically resolved.
- RETURN is only allowed inside FUNC blocks.
- No jumping into the middle of a LOOP_N body.
- STREAM cannot be called. It is entered only via RESET.

## Structured Bounded Loops

```
LOOP_N n, id
  ...
END_LOOP id
```

Rules:

- n is a compile-time constant (u32).
- The LOOP_N back-edge is the only backward edge allowed besides RESET -> STREAM.
- No jumping into the loop body from outside.
- The loop counter cannot be mutated by the loop body.

## Streaming Machinery

```
STREAM entry
  YIELD
RESET
```

Semantics:

- **STREAM**: Entry of the streaming region. Only RESET may target it.
- **YIELD**: Exchanges data with the host. Suspends and resumes. Falls through to the next instruction.
- **RESET**: Clears arena. Swaps text/rodata if scheduled. Jumps to STREAM entry. Only instruction allowed to target STREAM.

## Structural Verification Rules

### 1. Single Global Cycle

The only allowed unbounded cycle: RESET -> STREAM -> ... -> RESET. All other cycles must be bounded LOOP_N cycles.

### 2. Productivity Rule

Every path from the STREAM entry (s) to RESET (r) must contain at least one YIELD. Equivalent graph test: remove all YIELD nodes and verify that r is not reachable from s. This ensures every top-level branch yields before reset.

### 3. Yield Slice Safety (WCET Rule)

For any YIELD node y, every path from y to RESET must contain another YIELD or reach RESET directly. Equivalent: remove all YIELD nodes except y and verify that r is not reachable from y. This guarantees WCET is bounded between YIELD instructions.

### 4. Function Restrictions

Inside FUNC:

- No YIELD.
- No RESET.
- Must RETURN on all paths.

Inside STREAM:

- No RETURN allowed.

## Relationship to Current Implementation

The current Keleusma VM uses a 48-instruction stack-based bytecode documented in [INSTRUCTION_SET.md](./INSTRUCTION_SET.md). The target ISA described here represents the long-term execution model designed for safety-critical certification. The transition from the current bytecode to the structural ISA requires:

- Adding STREAM, RESET, and REENTRANT block primitives to the bytecode format
- Implementing the arena memory model with RESET-triggered clearing
- Implementing the structural verification pass (block-graph coloring)
- Replacing unbounded stack allocation with arena allocation

The surface language (Keleusma source syntax) remains largely unchanged. The compiler backend will target the structural ISA instead of the current flat bytecode.

## Cross-References

- [INSTRUCTION_SET.md](./INSTRUCTION_SET.md) documents the current implementation bytecode.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) describes the execution model and temporal domains.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the language-level design goals.
