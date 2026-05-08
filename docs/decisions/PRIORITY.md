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

The arena is currently simulated. R32 specifies the dual-end design and the verification rule. Implementation requires the following.

1. Add `allocator-api2` as a dependency.
2. Implement Keleusma's own arena allocator, namely a type that owns a fixed-size buffer and exposes two bump pointers, one growing up from the bottom and one growing down from the top.
3. Implement the `allocator_api2::Allocator` trait for a handle to the arena, so that `Vec<T, A>` and similar collections can use it.
4. Migrate the operand stack and dynamic-string storage to use the arena allocator. Note that stable Rust does not provide a `String` type with a custom allocator. A custom `DynStr` type backed by `Vec<u8, A>` with string-specific methods is the practical path.
5. Wire up the arena into `Vm::new` and `Op::Reset`.

This is substantial infrastructure work that warrants its own milestone. The two-string-type discipline (V0.0-M5) is operational without it. Programs that allocate dynamic strings currently use the global allocator, and the arena lifetime is enforced through Rust drop semantics rather than through bump-pointer reset.

## P8. WCMU instrumentation and auto-arena sizing

R31 specifies WCMU as the fifth guarantee. Implementation requires the following.

1. Add `Op::memory()` paralleling `Op::cost()`. Each instruction declares its byte footprint, with alignment accounted for.
2. Add `wcmu_stream_iteration()` paralleling `wcet_stream_iteration()`. The same recursive block-structured traversal applies, taking the maximum at branches and summing along sequential paths.
3. Compute `stack_wcmu` and `heap_wcmu` separately. Record both in `Module`.
4. Verify `stack_wcmu + heap_wcmu <= arena_size` at load time.
5. Auto-arena sizing produces `arena_size = stack_wcmu + heap_wcmu`.
6. Widen the host-attestation API. Native function registration accepts a WCMU declaration in addition to the WCET declaration.
7. Reject programs whose WCMU cannot be statically computed.

This pairs naturally with P7 because both require the arena to exist.

All P6 items are complete.

1. ~~Enforce the singular data block constraint (R28) at compile time with a clear diagnostic.~~ Complete.
2. ~~Enforce the fixed-size field type constraint at the data block declaration boundary, per the table in [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md).~~ Complete.
3. ~~Extend the structural verifier to reject `GetData` and `SetData` operands that exceed the segment slot count.~~ Complete.
4. ~~Define the host interoperability layer.~~ Complete. Slot-based `Vec<Value>` interface chosen over `repr(C)` struct mapping. Schema mismatch detection by size check plus host attestation. Hash and structural checking deferred. See R29.
5. ~~Specify the concurrency contract.~~ Complete. Single-ownership enforced by Rust borrow checker. Documented in EXECUTION_MODEL.md.
6. ~~Add end-to-end integration tests.~~ Complete. Six new hot swap tests cover same-schema preserved, new-schema replaced, size mismatch rejected, no-data module, swap at reset starts new module, and rollback to prior version.
