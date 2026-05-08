# Execution Model

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

This document describes the execution model for Keleusma. The model separates temporal control, memory phase control, and host interaction into distinct primitives. It defines two temporal domains and a structural program layout that enables static verification of safety properties.

## Two Temporal Domains

### Yield Domain (Control Clock)

WCET is measured from YIELD to YIELD. The YIELD primitive performs a bidirectional exchange with the host (A exchanged for B), suspends execution, and resumes at the next instruction. YIELD does not clear the arena. Multiple YIELDs may occur before RESET. Each yield-to-yield slice must be statically bounded. The yield domain corresponds to the synchronous hypothesis from the synchronous reactive language tradition [L1, SY1], which assumes that computation completes within one logical tick.

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

## Memory Model

The runtime memory layout corresponds directly to the four conventional executable sections found in the Unix linker tradition and the System V Application Binary Interface, namely `.text`, `.rodata`, `.data`, and `.bss`. This analogy is the organizing frame for runtime memory and is used throughout the documentation. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 8 for the full discussion.

| Region | Conventional analogue | Contents | Mutability | Lifetime |
|---|---|---|---|---|
| Bytecode chunks | `.text` | Compiled instruction sequences | Immutable | Until hot swap at RESET |
| Constant pool and templates | `.rodata` | Constants, struct templates, enum definitions | Immutable | Until hot swap at RESET |
| Data segment | `.data` | Host-supplied preinitialized context | Mutable | Persists across yield and reset |
| Arena and operand stack | `.bss` | Working storage for one stream phase | Mutable | Cleared at RESET |

### Arena and Operand Stack

The arena is a single contiguous allocation using bump allocation. The operand stack grows from one end of the arena. There is no heap initially. Allocations advance a pointer linearly through the contiguous buffer. Deallocation occurs only at RESET, when the entire arena is cleared by resetting the bump pointer to the start. This design eliminates fragmentation and ensures O(1) allocation and deallocation.

The arena persists across yields within a single stream phase. It is cleared only at RESET. No arena memory survives across phases. Memory bounds are statically analyzable per stream phase.

### Data Segment

The data segment is a fixed-size, fixed-layout region of mutable storage owned by the host and presented to the script as a preinitialized `.data` section. It is the sole region of mutable state observable to the script that persists beyond a single function activation. Scripts read and write the segment through `GetData` and `SetData` instructions, which address slots by index. The host is responsible for supplying a memory instance that conforms to the schema declared by the script.

Cross-yield value preservation is not guaranteed. The host may write to the segment between yields and is expected to do so in many designs. Within a single code image, the schema is fixed at compile time and does not change. Across hot updates, the schema may change arbitrarily because hot updates occur only at RESET, where no script invariant spans the boundary on the script side. Cross-swap value handling follows Replace semantics, in which the host atomically supplies the data instance appropriate for the new code version. The host may keep, modify, migrate, or substitute the underlying storage transparently.

Concurrency is single-ownership. The script holds exclusive access to the segment while executing. Ownership returns to the host at YIELD and at RESET. Concurrent access from another host thread during script execution is unspecified. In the Rust host environment, the borrow checker enforces single ownership at compile time, namely the host cannot call `set_data`, `get_data`, or `replace_module` while a `call` or `resume` invocation holds the mutable reference to the VM.

The data segment design draws on the persistent state model of the Erlang and Open Telecom Platform multi-version code coexistence pattern [H1, H2] and on the state vector model of mode automata in the synchronous reactive language tradition [H3, SC1].

### Host Interoperability Layer

The host interacts with the data segment through a slot-based `Vec<Value>` interface rather than through a `repr(C)` Rust struct mapping. The choice avoids unsafe pointer manipulation and keeps the runtime consistent with the rest of the VM, where every value is represented as a `Value` enum. The host is free to back its application-level state in any Rust struct it prefers and to marshal between that struct and the slot vector at the YIELD and RESET boundaries.

The Vm public API for the data segment consists of the following methods.

| Method | Use |
|---|---|
| `Vm::new(module)` | Construct the VM. The data segment is allocated to match the declared layout slot count and zero-initialized to `Value::Unit`. |
| `set_data(slot, value)` | Initialize or update a single slot. Valid between calls to `call` and `resume`. |
| `get_data(slot)` | Read a single slot. |
| `data_len()` | Return the number of slots in the current data segment. |
| `replace_module(new_module, initial_data)` | Hot swap the code and data segment atomically. Verifies the new module. Requires the host-supplied data vector length to match the new module's declared slot count. Clears frames and stack so the next `call` starts the new module's entry point. Suitable for both forward update and rollback. |

Schema mismatch detection is by size check plus host attestation. The size check compares the supplied data vector length against the declared slot count of the new module. Dialogue type compatibility between modules across a swap is the host's responsibility because dialogue types are erased at the bytecode level. Schema hash comparison and structural type checking against a schema descriptor are deferred to a later phase.

### Host State

External to the VM and managed by the host application. Not directly observable to the script except through the data segment and through native function calls.

## Hot Code Swapping

Hot code swapping occurs only at RESET boundaries. The following requirements apply.

- The YIELD signature, namely the dialogue type A exchanged for B, remains invariant across the entire STREAM and across swaps.
- Text, rodata, and the data segment schema may all change across a swap. Only the dialogue type is invariant.
- The arena is cleared before new code executes.
- WCET and reset-to-reset bounds are certified per routine independently.

Different routines may have different WCETs, which are declared in a static header for the host scheduler to validate before accepting the swap.

### Atomicity

Atomicity is logical only. The new code text and rodata must be resident in memory and the host-supplied data segment instance must conform to the new schema before the candidate is eligible for installation. The host writes the candidate slot. The VM reads the slot at the next RESET and applies the swap as a single atomic transition from the script's point of view. Crash atomicity, namely recovery from a fault that interrupts the swap, is the responsibility of the host platform and is out of scope for the VM specification. The Ksplice and Kitsune literature treats this question in detail [H4, H5].

### Cross-Swap Value Handling

Value handling across the swap follows Replace semantics. The host owns the data segment storage and supplies a memory instance appropriate for the new code version. From the script's point of view, the data segment seen after RESET is whatever the host presents. The host may transparently keep, modify, migrate, or substitute the underlying storage, including supplying an instance of an entirely different schema if the new code requires it. This is consistent with the multi-version code coexistence pattern of Erlang and the Open Telecom Platform [H1, H2], with the simplification that the migration callback resides in the host rather than in the script.

### Rollback

Rollback occurs at RESET and is mechanically identical to a forward update with an older code version selected. The host bears responsibility for tracking which code versions are eligible and for supplying a data segment instance compatible with the selected version. From the script's point of view, rollback is indistinguishable from any other update.

### Stale Slot Behavior

The most recent valid code version runs at each RESET. If no new candidate has been slotted, the existing image continues. After a rollback, the host must mark the rejected version as ineligible or must operate in a rollback mode so that the VM does not automatically reinstall the rejected candidate at the next opportunity.

### Update Points and Stack Quiescence

RESET is the only update point. Stack quiescence is trivial because the operand stack is empty at RESET by construction. This contrasts with the dynamic software update literature for general-purpose C programs, where update points must be inferred and stack quiescence must be reasoned about explicitly [H4, H5]. The structural ISA of Keleusma makes both properties hold by construction.

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
5. **Productivity rule.** Abstract interpretation over a two-element lattice verifies that all control flow paths from Stream to Reset pass through at least one Yield. The productivity analysis and the WCET analysis are both instances of abstract interpretation in the sense of Cousot and Cousot [AI1], operating over finite lattices (a two-element boolean lattice for productivity, the natural numbers for WCET cost).

A program is valid only if all paths satisfy these constraints. Invalid programs are rejected before execution begins.

Additionally, `wcet_stream_iteration()` computes the worst-case execution cost of one Stream-to-Reset iteration. Each instruction carries a relative cost via `Op::cost()`. The analysis recursively traverses block-structured control flow, taking the maximum cost branch at each join point, and returns the worst-case total as a unitless integer.

See [TARGET_ISA.md](../reference/TARGET_ISA.md) for the full structural ISA specification.

## Cross-References

- [LANGUAGE_DESIGN.md](./LANGUAGE_DESIGN.md) describes the language-level design goals and guarantees.
- [TARGET_ISA.md](../reference/TARGET_ISA.md) specifies the structural ISA block types and verification rules.
- [COMPILATION_PIPELINE.md](./COMPILATION_PIPELINE.md) describes the current compilation pipeline.
- [RELATED_WORK.md](../reference/RELATED_WORK.md) positions Keleusma within the academic and industrial landscape.

## Citation Key

Citations in this document use bracket notation (e.g., [L1], [AI1]) referring to entries in the bibliography in [RELATED_WORK.md](../reference/RELATED_WORK.md).
