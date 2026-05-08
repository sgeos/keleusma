# Structural ISA

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

## Overview

This document describes the Instruction Set Architecture for Keleusma. The structural ISA uses block-structured control flow and block-based nesting to make invalid or unproductive programs impossible to define or load. See [INSTRUCTION_SET.md](./INSTRUCTION_SET.md) for the bytecode instruction reference.

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

## Type Testing

| Instruction | Operands | Description |
|:---|:---|:---|
| IsEnum | u16 type, u16 variant | Pop value, push true if it matches the enum type and variant. |
| IsStruct | u16 name | Pop value, push true if it matches the struct type. |

Type testing instructions push a boolean result onto the stack. They do not contain jump offsets. Conditional dispatch based on type tests uses the block-structured If/Else/EndIf control flow.

## Control Flow

| Instruction | Operands | Description |
|:---|:---|:---|
| If | u32 offset | Pop boolean. If false, skip forward by offset instructions to the matching Else or EndIf. |
| Else | u32 offset | Unconditional skip forward by offset instructions to the matching EndIf. Marks the start of the else branch. |
| EndIf | none | Marks the end of an if or if-else block. No operation. |
| Loop | u32 offset | Marks the start of a loop block. Offset encodes the distance to the matching EndLoop for verification. |
| EndLoop | u32 offset | Unconditional jump backward by offset instructions to the matching Loop. |
| Break | u32 depth | Exit the enclosing loop at nesting depth. Jumps past the matching EndLoop. |
| BreakIf | u32 depth | Pop boolean. If true, exit the enclosing loop at nesting depth. |
| CALL | none | Call a FUNC. |
| RETURN | none | Return from FUNC. |

All control flow is block-structured. There are no flat jump instructions (JMP, BRANCH). Every forward or backward transfer of control is mediated by a matching block delimiter. This constraint ensures that the control flow graph can be statically verified through block nesting alone.

Restrictions:

- No indirect calls or jumps. All call targets are statically resolved.
- No flat jumps. All branches use block-structured If/Else/EndIf or Loop/EndLoop/Break/BreakIf.
- RETURN is only allowed inside FUNC blocks.
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

| Instruction | Operands | Description |
|:---|:---|:---|
| Stream | none | Entry of the streaming region. Only Reset may target it. |
| Yield | none | Exchanges data with the host. Suspends and resumes. Falls through to the next instruction. |
| Reset | none | Clears arena. Activates double-buffered text/rodata swap if scheduled. Jumps to Stream entry. |

```
Stream
  Yield
Reset
```

Semantics:

- **Stream**: Entry of the streaming region. Only Reset may target it.
- **Yield**: Exchanges data with the host (Output B for Input A). Suspends and resumes. Falls through to the next instruction.
- **Reset**: Clears arena. Activates the secondary buffer if a hot swap is scheduled (see double buffering in [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md)). Jumps to Stream entry. Only instruction allowed to target Stream.

## Dialogue Type Interoperability

The dialogue signature `Dialogue A B` specifies the types exchanged between the host and the VM on each Yield. The host defines the Rust types for A and B using the `#[keleusma_type]` attribute macro. This attribute enforces an interoperable memory layout on the annotated type, ensuring that the host and VM agree on the binary representation of the dialogue types. The attribute handles alignment, field ordering, and representation so that values can be passed across the host-VM boundary without serialization.

```rust
#[keleusma_type]
struct SensorReading {
    temperature: f64,
    pressure: f64,
    timestamp: u64,
}

#[keleusma_type]
enum Command {
    Idle,
    Thrust(f64),
    Rotate(f64, f64),
}
```

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

## Implementation Status

The structural ISA is fully implemented. The compiler emits block-structured bytecode, the VM executes it natively, and the structural verifier validates all modules at load time. The surface language (Keleusma source syntax) remains unchanged. The compiler backend targets the structural ISA. Surface-level constructs such as pattern dispatch, pipelines, and dynamic types are syntactic sugar that the compiler lowers to the austere certifiable bytecode.

The structural verifier (`verify()` in `src/verify.rs`) enforces all rules described in this document through five passes.

1. **Block nesting.** Every If is matched by EndIf (with optional Else). Every Loop is matched by EndLoop. No orphaned delimiters.
2. **Offset validation.** All jump targets are within bounds and point to the correct matching delimiter.
3. **Block type constraints.** Func chunks contain no Yield, Stream, or Reset. Reentrant chunks contain at least one Yield and no Stream or Reset. Stream chunks contain exactly one Stream, exactly one Reset, and at least one Yield.
4. **Break containment.** Every Break and BreakIf is inside a Loop/EndLoop.
5. **Productivity rule.** All control flow paths from Stream to Reset pass through at least one Yield.

A WCET analysis function (`wcet_stream_iteration()`) computes the worst-case cost of one Stream-to-Reset iteration using the same block-structured recursive traversal, taking the maximum cost branch at each control flow join.

## Cross-References

- [INSTRUCTION_SET.md](./INSTRUCTION_SET.md) provides the bytecode instruction reference.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) describes the execution model and temporal domains.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the language-level design goals.
