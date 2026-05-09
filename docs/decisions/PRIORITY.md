# Priority Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Open decisions that may block near-term development.

## P1. Type checker implementation

The compiler currently produces bytecode without type checking or name resolution validation. Adding a semantic analysis pass would catch type errors at compile time rather than runtime. This affects the reliability of the compilation pipeline. The scope of such a pass needs to be determined, including whether it should be a simple type-check against declared signatures or a full inference system, and how it interacts with native function type declarations.

## P2. For-in over expressions

The compiler currently only supports range-based for loops of the form `for i in 0..n`. Support for iterating over array expressions, such as `for item in array`, is specified in the grammar but not yet implemented. The implementation requires deciding on iterator semantics, including whether iteration consumes the array or borrows it, and how to handle mutation of the array during iteration.

## P3. Error recovery model

What should happen when a script encounters a runtime error? Options include yielding a default value, suspending the script, or notifying the host via `VmError`. The current implementation halts execution on error. A recovery model would need to define whether the host can resume a script after an error, and if so, what value the host supplies at the recovery point.

## ~~P4. Structural ISA implementation~~ (Resolved as R22)

## ~~P5. WCET analysis tooling~~ (Resolved as R23)

## P7. Arena allocator implementation

Foundation complete. R34 records the implementation. The remaining work is iterative integration.

1. ~~Add `allocator-api2` as a dependency.~~ Complete.
2. ~~Implement Keleusma's own arena allocator.~~ Complete. See `src/arena.rs`.
3. ~~Implement the `allocator_api2::Allocator` trait for arena handles.~~ Complete. See `StackHandle` and `HeapHandle`.
4. ~~Wire up the arena into `Vm::new`, `Op::Reset`, and `replace_module`.~~ Complete.
5. Migrate the operand stack to use `allocator_api2::vec::Vec<Value, StackHandle>`. Open. Requires propagating an arena lifetime parameter through the `Vm` struct, which cascades through every signature that touches `Vm`. Substantial refactor.
6. Replace `Value::DynStr(String)` with a custom `DynStr` storage type backed by `allocator_api2::vec::Vec<u8, HeapHandle>`. Open. Requires propagating the arena lifetime through `Value`. Equally substantial.

Items 5 and 6 are coordinated. They cannot be done independently because both touch the lifetime story of `Value`. They are the next major refactor and should be addressed together. The current arena is operational and reset on schedule, but its principal use today is host-supplied native functions that wish to allocate arena-resident scratch buffers. The operand stack and dynamic-string storage continue to use the global allocator with Rust drop semantics enforcing the arena lifetime.

## ~~P8. WCMU instrumentation and auto-arena sizing~~ (Resolved as R35 and R37)

All P8 items are complete except for the bounded-iteration loop analysis, which is tracked separately as P9.

1. ~~Add `Op::stack_growth`, `Op::stack_shrink`, and `Op::heap_alloc` methods.~~ Complete.
2. ~~Add `wcmu_stream_iteration()`.~~ Complete.
3. ~~Compute `stack_wcmu` and `heap_wcmu` separately.~~ Complete.
4. ~~Verify `stack_wcmu + heap_wcmu <= arena_size` at load time.~~ Complete.
5. ~~Auto-arena sizing.~~ Complete via `Vm::new_auto` and `Vm::auto_arena_capacity`. R37.
6. ~~Widen host-attestation API.~~ Complete via `Vm::set_native_bounds`.
7. ~~Reject programs whose WCMU cannot be statically computed.~~ Complete for the call-graph case. The analysis now walks the call DAG topologically and includes transitive contributions of called chunks and natives. R37.

## ~~P9. Bounded-iteration loop analysis~~ (Resolved as R38)

## P10. Zero-copy bytecode execution from rodata

Phase 1 of P10 is complete. The body format is rkyv as of `BYTECODE_VERSION = 4`. Rkyv produces a self-relative addressable layout that supports in-place access. The wire format reserves alignment padding so the body begins at an eight-byte-aligned offset when the buffer base is eight-byte-aligned. The current `Module::from_bytes` path copies the body to `AlignedVec` to satisfy alignment for arbitrary host slices, then deserializes to an owned `Module`. The runtime continues to execute against the owned form for now.

Phase 2 adds the actual zero-copy execution path. The work cascades:

- `Vm` gains a lifetime parameter. Either `Vm<'a>` everywhere or a separate `BorrowedVm<'a>` alongside the owned `Vm`.
- The execution loop reads from `&ArchivedModule` instead of `&Module`. Match arms over `Op` become match arms over `ArchivedOp`. Vector accesses go through `ArchivedVec`. String accesses go through `ArchivedString`.
- A new constructor `Vm::view_bytes(&'a [u8])` validates framing and calls `rkyv::access` to obtain `&'a ArchivedModule`.
- The lifetime cascades through every public method that touches the VM.
- Tests for execution against borrowed buffers, including from a `&'static [u8]` placed in `.rodata`.

Phase 2 also interacts with two backlog items.

B10 (target portability) interacts because the rkyv encoding is endian-stable but float and integer width assumptions still affect runtime semantics. The recent log2 encoding work covers integer widths through masking. Float widths are still hardcoded to f64.

B9 (hot update of yielded static strings) interacts because yielded `Value::StaticStr` under path B is an `ArchivedString` that points into a specific bytecode buffer. A hot update that swaps the buffer invalidates outstanding archived references the host has retained. The resolution paths in B9 (host-responsibility consumption or eager materialization at yield) must be in place before path B fully replaces path A.

Both the WCMU and WCET analyses now multiply the loop body cost by the iteration count when the loop matches the canonical for-range pattern emitted by the compiler. The pattern detector in `extract_loop_iteration_bound` recognizes `Loop GetLocal(var) GetLocal(end) CmpGe BreakIf` followed by a body and traces backward to find the literal `Const` initializers of the var and end slots. Loops whose bounds are not literal integers fall back to the conservative one-iteration treatment, which remains sound but loose. R38 records the implementation.

All P6 items are complete.

1. ~~Enforce the singular data block constraint (R28) at compile time with a clear diagnostic.~~ Complete.
2. ~~Enforce the fixed-size field type constraint at the data block declaration boundary, per the table in [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md).~~ Complete.
3. ~~Extend the structural verifier to reject `GetData` and `SetData` operands that exceed the segment slot count.~~ Complete.
4. ~~Define the host interoperability layer.~~ Complete. Slot-based `Vec<Value>` interface chosen over `repr(C)` struct mapping. Schema mismatch detection by size check plus host attestation. Hash and structural checking deferred. See R29.
5. ~~Specify the concurrency contract.~~ Complete. Single-ownership enforced by Rust borrow checker. Documented in EXECUTION_MODEL.md.
6. ~~Add end-to-end integration tests.~~ Complete. Six new hot swap tests cover same-schema preserved, new-schema replaced, size mismatch rejected, no-data module, swap at reset starts new module, and rollback to prior version.
