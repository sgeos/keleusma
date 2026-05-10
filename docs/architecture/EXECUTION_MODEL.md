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

### Implementation Mapping

The conceptual regions in the table above are realized in source by the types and members listed here. The mapping is informational. The wire format does not bind to specific implementation choices, but the present runtime build uses these locations.

| Region | Source location | Construction | Runtime access |
|---|---|---|---|
| `.text` | `Module::chunks[i].ops: Vec<Op>` in `src/bytecode.rs`, mirrored as `Vm::decoded_ops[chunk_idx]: Vec<Op>` populated at VM construction in `src/vm.rs` | `compile_with_target` lowers function bodies into per-chunk op vectors. `Vm::construct` and `Vm::view_bytes_zero_copy` decode the archived form into `decoded_ops` once. `Vm::replace_module` re-decodes for hot swap. | `Vm::chunk_op` returns `Op` by direct slice index on the hot dispatch loop. `Op::cost`, `Op::stack_growth`, and `Op::stack_shrink` provide WCET and WCMU costs per opcode. |
| `.rodata` | `Module::chunks[i].constants: Vec<ConstValue>` and `Module::chunks[i].struct_templates: Vec<StructTemplate>` in `src/bytecode.rs`. Enum metadata is encoded inline in operand bytes of `Op::NewEnum` referencing constant pool entries by index. | `compile_with_target` writes the constant pool while lowering each function body. Per-chunk struct templates accumulate during lowering as the compiler encounters struct definitions. The native function name table at `Module::native_names` is also `.rodata` content because it is read-only after compile. | `Vm::chunk_const` lifts a `ConstValue` to a runtime `Value` through `Value::from_const_archived`. `Op::Const`, `Op::NewStruct`, `Op::NewEnum`, and `Op::CallNative` index these tables at dispatch time. |
| `.data` | `Vm::data: Vec<Value>` in `src/vm.rs`, with the layout schema described by `Module::data_layout: Option<DataLayout>` | `Vm::construct` initializes `Vm::data` to one `Value::Unit` per declared slot. The host populates real values through `Vm::set_data` before the first `Vm::call`. `Vm::replace_module` accepts a fresh `initial_data` from the host and replaces the segment atomically; the schema may change across the swap. | `Op::GetData(slot)` and `Op::SetData(slot)` index by slot at dispatch time. The host accesses the segment outside script execution through `Vm::get_data` and `Vm::set_data`. |
| `.bss` | `Vm::stack: StackVec<'arena, Value>` and `Vm::frames: StackVec<'arena, CallFrame>` in `src/vm.rs`, both backed by the bottom region of the host-owned `keleusma_arena::Arena`. The arena's top region (allocated through `keleusma_arena::TopHandle`) holds `KString` allocations for dynamic strings produced by arena-aware natives. | `ArenaVec::new_in(arena.bottom_handle())` at VM construction. The arena's two bump pointers grow toward each other from opposite ends and are bump-allocated on demand. | `Op::GetLocal(slot)` and `Op::SetLocal(slot)` index the operand stack by absolute slot for parameters and locals. `KString::alloc(arena, content)` allocates from the arena top. The execution loop pushes and pops `Vm::stack` directly. |

Lifetime invariants. The `.text` and `.rodata` regions survive across `Op::Reset` because the bytecode buffer and the decoded-op cache do not change at reset. Both are replaced atomically only through `Vm::replace_module` for hot swap. The `.data` region survives across both yield and reset because it is the persistent context. `Vm::reset_after_error` preserves it explicitly when recovering from a runtime error. The `.bss` region is cleared at reset. `Op::Reset` advances the arena's epoch counter and clears the top region, and `Vm::full_reset_arena_internal` recreates the `StackVec` collections so their stale storage references are dropped before clearing both ends.

Memory bookkeeping. The arena's bottom-region budget covers the `.bss` operand-stack and call-frame peak. The arena's top-region budget covers `.bss` `KString` allocations from arena-aware natives. The two are checked together against the arena capacity by `verify::module_wcmu` at `Vm::new` and `Vm::replace_module`. The `.data` region is a separate `Vec<Value>` allocated through the global allocator; its size is determined by the `DataLayout` and is not part of the arena's WCMU budget. The `.text` region's storage cost is the program's total op count multiplied by `size_of::<Op>()` plus the rkyv-archived bytecode buffer; both live in the global heap.

Module ownership. `Vm` holds the bytecode through a `BytecodeStore` enum that is either `Owned(AlignedVec)` (the path used by `Vm::new`, `Vm::load_bytes`, and `Vm::replace_module`) or `Borrowed(&[u8])` (the zero-copy path used by `Vm::view_bytes_zero_copy`). Either backing is valid for the archived `Module` view that the hot path consults indirectly through `Vm::decoded_ops` and the per-chunk constant access methods. The choice between owned and borrowed bytecode is orthogonal to the section mapping; both paths populate the same `.text` and `.rodata` views.

### Arena, Operand Stack, and Heap

The arena is a single contiguous allocation with two pointers growing toward each other from opposite ends. The operand stack grows from one end. The dynamic-string heap and any other arena allocations grow from the other end. There is no fixed boundary between the regions. Either pointer may consume any portion of the arena that the other has not consumed. Allocation fails when the two pointers would meet, producing a runtime error.

The dual-end design ensures O(1) allocation from either end and O(1) reset of both. RESET clears both pointers atomically. The arena persists across yields within a single stream phase. It is cleared only at RESET. No arena memory survives across phases.

Memory bounds are statically analyzable per stream phase. The verifier computes a worst-case stack consumption (`stack_wcmu`) and a worst-case heap consumption (`heap_wcmu`) separately through `wcmu_stream_iteration` in `src/verify.rs`. The function `verify_resource_bounds` checks the inequality `stack_wcmu + heap_wcmu <= arena_size` at module load time. Programs that exceed the bound are rejected at `Vm::new` and `Vm::replace_module`. The analysis is sound for programs without calls and without variable-iteration loops. Programs with transitive calls or variable-iteration loops produce underestimates rather than rejection at present. A sharper analysis with call-graph integration is recorded as P8 follow-on. The arena auto-size, when computed, is exactly the sum of the two bounds plus an optional safety margin. Auto-sizing is not yet implemented; the host configures arena capacity at construction.

#### Arena Implementation

The arena is implemented as the `Arena` type in `src/arena.rs`. It owns a fixed-size `Box<[u8]>` backing buffer and tracks two bump pointers using `Cell<usize>`. Two handle types `StackHandle` and `HeapHandle` borrow the arena and implement the `allocator_api2::Allocator` trait, allowing arena-backed collections through `allocator_api2::vec::Vec::new_in(handle)` and similar constructors.

The `Vm` holds an `Arena` instance configured with a default capacity of 65536 bytes. The capacity is configurable through `Vm::new_with_arena_capacity`. The arena is exposed through `Vm::arena()` and `Vm::arena_mut()` accessors for host-supplied native functions that wish to allocate arena-resident scratch buffers. The arena is reset at every `Op::Reset` boundary and at every `replace_module` call.

The deeper integration of the operand stack and dynamic-string storage with the arena is iterative work tracked as P7 follow-on. Stable Rust does not currently expose a `String` type with a custom allocator, so a custom `DynStr` storage type backed by `allocator_api2::vec::Vec<u8, HeapHandle>` is required for full integration. See R34 for the implementation status.

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

## Bytecode Loading

Compiled modules can be loaded from any addressable byte slice. The runtime crate is `no_std` plus `alloc` and accepts `&[u8]` from any source. Section placement is the host's responsibility. The same input shape covers in-memory `Vec<u8>` data, file-loaded buffers, and `&'static [u8]` data placed in the `.rodata`, `.text`, `.data`, or `.bss` section of the host binary. File loading is left to the host so that the runtime crate retains the `no_std` posture.

### Wire Format

The serialized form begins with a sixteen-byte header followed by the rkyv-encoded module body followed by a four-byte little-endian CRC-32 trailer. The header carries the four-byte magic `KELE`, a little-endian sixteen-bit version, a little-endian thirty-two-bit total framing length, an eight-bit word size encoded as the base-2 exponent, an eight-bit address size encoded as the base-2 exponent, and four reserved bytes that pad the header so the rkyv body begins at an eight-byte-aligned offset within the buffer. The minimum framing size is twenty bytes. The header allows the runtime to reject foreign or incompatible bytecode at load time before any deserialization is attempted. The trailer detects bit-level corruption anywhere in the framed range. The `BYTECODE_MAGIC`, `BYTECODE_VERSION`, `RUNTIME_WORD_BITS_LOG2`, and `RUNTIME_ADDRESS_BITS_LOG2` constants in the bytecode module record the current values. The runtime accepts bytecode whose word and address exponents are less than or equal to the runtime's supported maximum. The VM applies sign-extending integer truncation in `Add`, `Sub`, `Mul`, `Div`, `Mod`, and `Neg` when the declared word size is narrower than the runtime's, so arithmetic overflow points match the declared width.

The body format is rkyv. Rkyv produces a self-relative addressable layout that supports zero-copy access through `rkyv::access`. The `from_bytes` path copies the body to an `AlignedVec` and deserializes to an owned `Module` for compatibility with arbitrary unaligned host slices. The `view_bytes` path skips the body copy when the host supplies an aligned slice and validates the archived form in place via `access_bytes`. The execution loop currently operates on the deserialized owned `Module` regardless of which load path was used. A future zero-copy execution path (P10 Phase 2 step 2) will read directly from the buffer without deserialization, bounded by a lifetime parameter on the `Vm`.

The recorded length is authoritative. The deserializer truncates the input slice to the recorded length before any further processing. Trailing bytes after the recorded length are ignored, supporting bytecode embedded inside a larger buffer such as a flash region with padding. Slices shorter than the recorded length, or recorded lengths below the minimum framing size, are rejected as `Truncated`.

The CRC-32 uses the standard IEEE 802.3 reflected polynomial with init `0xFFFFFFFF`, refin and refout true, and xor-out `0xFFFFFFFF`. The verification path exploits the algebraic self-inclusion residue of this parameterization. Computing the CRC over the entire byte slice including the trailer yields the residue constant `0x2144DF1C` for valid bytecode. The verifier runs the CRC once over the full slice and checks for the residue in a single linear pass. The trailer is part of the checksummed range without requiring zero-fill or position-aware special casing during verification.

The choice of rkyv reflects the constraints of `no_std` plus `alloc` operation and the goal of zero-copy execution from `.rodata` (P10). Rkyv produces a self-relative addressable layout that supports in-place access through `rkyv::access`, and its `bytecheck` integration validates the archived form before access. The serialization uses `#[derive(Archive, Serialize, Deserialize)]` on every type that participates in the Module structure, including `Module`, `Chunk`, `Op`, `ConstValue`, `BlockType`, `StructTemplate`, `DataSlot`, and `DataLayout`. The runtime `Value` enum is not directly archived; `ConstValue` is the archived constant-pool type, and `Value::from_const_archived` lifts it at push time. See R39 in [RESOLVED.md](../decisions/RESOLVED.md) for the full rationale.

The deserialized Module owns heap-allocated data and does not borrow from the input slice. The bytecode buffer can persist in `.rodata` even though the parsed form is heap-allocated. Future work under P10 introduces a zero-copy variant where the parsed Module borrows directly from the input buffer.

### Loading API

| Method | Use |
|---|---|
| `Module::to_bytes()` | Serialize a module to a `Vec<u8>` carrying the magic-and-version header followed by the rkyv-encoded body and the CRC trailer. |
| `Module::from_bytes(bytes)` | Validate the header and deserialize. Copies the body into an `AlignedVec` for alignment regardless of the host slice's alignment. Returns `bytecode::LoadError` on header mismatch or codec failure. Does not run structural or resource verification. |
| `Module::access_bytes(bytes)` | Validate the framing and return a borrowed `&'a ArchivedModule` through `rkyv::access`. Requires the body to be 8-byte aligned within the slice. |
| `Module::view_bytes(bytes)` | Validate through `access_bytes` and deserialize to owned `Module`. Skips the body copy that `from_bytes` performs. Requires alignment. |
| `Vm::new(module)` | Construct the VM. Runs structural verification and resource bounds verification. |
| `Vm::load_bytes(bytes)` | Convenience for `Vm::new(Module::from_bytes(bytes)?)`. Runs full verification. |
| `Vm::view_bytes(bytes)` | Convenience for `Vm::new(Module::view_bytes(bytes)?)`. Skips the body copy. Requires alignment. |
| `unsafe Vm::new_unchecked(module)` | Skip the resource bounds check. Structural verification still runs because the VM execution loop relies on its invariants for memory safety. |
| `unsafe Vm::load_bytes_unchecked(bytes)` | Convenience for `unsafe Vm::new_unchecked(Module::from_bytes(bytes)?)`. |
| `unsafe Vm::view_bytes_unchecked(bytes)` | Convenience for `unsafe Vm::new_unchecked(Module::view_bytes(bytes)?)`. Skips the body copy. Requires alignment. |

The unchecked path is for hosts that load precompiled bytecode whose resource bounds were validated during the build pipeline. The unsafe marker captures the trust contract. The host attests that bytecode was previously verified or originates from a trusted compiler. The bounded-memory and bounded-step guarantees are weakened to host attestation under this path. Exceeding the bound at runtime produces an arena allocation failure rather than memory unsafety. See R39 for the full design rationale.

## Error Recovery

When the VM encounters a runtime error during `Vm::call` or `Vm::resume`, the call returns `Err(VmError)` and the VM is left in an undefined intermediate state. The host inspects the error and decides how to proceed.

`Vm::reset_after_error()` is the explicit recovery API. It clears the operand stack, the call frames, and the arena, returning the VM to a clean callable state. The data segment and the bytecode store are preserved. After recovery, the host can call `Vm::call` to start a fresh iteration of the entry point with accumulated data intact.

This design extends the existing per-iteration RESET model to errors. Streams already use `Op::Reset` as the natural recovery boundary at the script level. Error recovery puts the same boundary mechanism under host control. The host decides whether to retry the same script, replace the module via `Vm::replace_module`, or escalate the error.

The recovery model is consistent with the hot-swap design (R26, R27). Both clear volatile state while letting the host control data continuity. Hosts that want to also reset the data segment can follow `reset_after_error` with calls to `Vm::set_data` for each slot, or use `Vm::replace_module` to swap to a new code image with new initial data.

Bidirectional error handling between the script and the host through the yield boundary is tracked separately as B7.

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

### Indirect Dispatch and Recursion

Definitive WCET and WCMU is the language's load-bearing guarantee. The verifier rejects any program whose execution time or memory use cannot be statically bounded. The two op kinds that violate this contract are rejected outright by `verify::module_wcmu`.

1. `Op::MakeRecursiveClosure` constructs a self-referential closure whose body dispatches to itself through indirect call. The resulting program admits unbounded recursion within a single Stream-to-Reset iteration. Rejected at module verification.

2. `Op::CallIndirect` resolves its target chunk at runtime from a `Value::Func` on the operand stack. The static analysis cannot follow this edge through the call graph, so the cost of the indirect call cannot be bounded without a flow analysis that the present verifier does not implement. Rejected at module verification regardless of which construction op produced the value being dispatched.

The construction ops `Op::PushFunc` and `Op::MakeClosure` remain admissible because they produce values that can be yielded, stored in the data segment, or otherwise consumed without invocation. Only the dispatch through `Op::CallIndirect` introduces the unbounded behavior. A program that pushes a function value but never calls it is bounded.

The valid form of unbounded execution is the top-level `loop` block in the productive divergent function category. The structural verifier enforces the productivity rule at this level: every control flow path from `Op::Stream` to `Op::Reset` must pass through at least one `Op::Yield`. The bounded-step contract holds yield-to-yield, so unbounded execution across the RESET cycle is the expected steady-state of a stream processor.

`Vm::new_unchecked` and `Vm::load_bytes_unchecked` exist for hosts that load precompiled bytecode whose resource bounds were validated during the build pipeline. They skip the resource-bounds check while preserving structural verification. Using these constructors to admit programs that would fail `verify_resource_bounds` is intentional misuse outside the language's WCET contract; programs that fail verification by construction should be rejected at the build pipeline level rather than admitted at load time. The unsafe constructors are not an escape hatch for unbounded programs.

See [TARGET_ISA.md](../reference/TARGET_ISA.md) for the full structural ISA specification.

## Cross-References

- [LANGUAGE_DESIGN.md](./LANGUAGE_DESIGN.md) describes the language-level design goals and guarantees.
- [TARGET_ISA.md](../reference/TARGET_ISA.md) specifies the structural ISA block types and verification rules.
- [COMPILATION_PIPELINE.md](./COMPILATION_PIPELINE.md) describes the current compilation pipeline.
- [RELATED_WORK.md](../reference/RELATED_WORK.md) positions Keleusma within the academic and industrial landscape.

## Citation Key

Citations in this document use bracket notation (e.g., [L1], [AI1]) referring to entries in the bibliography in [RELATED_WORK.md](../reference/RELATED_WORK.md).
