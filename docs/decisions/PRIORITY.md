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

## P9. Bounded-iteration loop analysis

The current WCMU analysis treats variable-iteration loops as one iteration. Programs that compile from `for i in 0..n` produce sound bounds at the static iteration count, but the analysis underestimates by the iteration factor at present. Lifting this requires either a side-table associating loop instructions with their iteration counts, or a structural pattern match on the compiled loop shape. The same limitation exists in the WCET analysis. A coordinated fix for both analyses is the V0.1 candidate.

All P6 items are complete.

1. ~~Enforce the singular data block constraint (R28) at compile time with a clear diagnostic.~~ Complete.
2. ~~Enforce the fixed-size field type constraint at the data block declaration boundary, per the table in [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md).~~ Complete.
3. ~~Extend the structural verifier to reject `GetData` and `SetData` operands that exceed the segment slot count.~~ Complete.
4. ~~Define the host interoperability layer.~~ Complete. Slot-based `Vec<Value>` interface chosen over `repr(C)` struct mapping. Schema mismatch detection by size check plus host attestation. Hash and structural checking deferred. See R29.
5. ~~Specify the concurrency contract.~~ Complete. Single-ownership enforced by Rust borrow checker. Documented in EXECUTION_MODEL.md.
6. ~~Add end-to-end integration tests.~~ Complete. Six new hot swap tests cover same-schema preserved, new-schema replaced, size mismatch rejected, no-data module, swap at reset starts new module, and rollback to prior version.
