# Priority Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Open decisions that may block near-term development.

## ~~P1. Type checker implementation~~ (Resolved)

A static type checker is in place at `src/typecheck.rs` and is invoked from `compile`. Type errors are surfaced as `CompileError` before bytecode emission. The parser now represents the unit literal `()` as `Literal::Unit` rather than `Literal::Int(0)`. The compiler emits `Op::PushUnit` for the new variant. The type checker recognizes `Literal::Unit` as `Type::Unit`. Five existing tests that relied on lax behavior (programs referencing struct or enum names without definitions, or returning a tuple from a unit-typed function) were updated to declare the types they reference.

Coverage in place.

- Function call argument count and argument types against parameter declarations.
- Function return expression type against declared return type.
- Let binding type against the value's type when annotation is present.
- Arithmetic and comparison operations have type-compatible operands.
- Field access references defined fields on the operand type.
- Struct construction provides defined fields with the right types.
- Tuple index in range and array index of i64.
- Cast operations are between admissible types (i64 to f64 and back).
- Identifier references resolve to known locals or function names.
- If-else branch type agreement.
- For-range bound types and for-in element type extraction.
- Enum variant existence and payload arity and types.
- Logical operator operand types.

Out of scope and deferred.

- Hindley-Milner inference (B1).
- Detailed pattern type checking against the scrutinee. Match arms accept any pattern; the runtime detects mismatches.
- Match arm exhaustiveness.
- Native function call types. Natives are registered at runtime.
- Yielded value types. The dialogue type is not yet tracked.

Follow-up work to integrate the checker into the compile pipeline.

1. Add `Literal::Unit` to the AST or change the parser to produce `TupleLiteral` with empty elements for `()`.
2. Update the compiler to handle the new representation by emitting `Op::PushUnit`.
3. Update the type checker to recognize the new representation as `Type::Unit`.
4. Invoke `typecheck::check` from `compile` and convert errors to `CompileError`.
5. Update existing test programs that relied on the lax behavior.

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

## ~~P10. Zero-copy bytecode execution from rodata~~ (Resolved)

P10 is complete across all phases.

Phase 1 (`BYTECODE_VERSION = 4`). Body format switched from postcard to rkyv. Rkyv produces a self-relative addressable layout that supports in-place access. Header padded for 8-byte body alignment.

Phase 2 step 1. `Module::access_bytes` returns a borrowed `&'a ArchivedModule` after framing validation. `Module::view_bytes` deserializes from access without the body copy. `Vm::view_bytes` and `unsafe Vm::view_bytes_unchecked` constructors compose this with the existing safe and unchecked paths.

Phase 2 step 2 foundations. `Op` derives `Copy`. `op_from_archived` covers all 48 variants. `value_from_archived` covers all 11 variants recursively. Round-trip tests verify identity preservation.

Phase 2 step 2 execution refactor.

- `Vm` gained lifetime parameter `Vm<'a>` with `BytecodeStore<'a>` enum carrying owned `AlignedVec` or borrowed `&'a [u8]`.
- The execution loop reads from `&ArchivedModule` via the `archived()` helper and the per-access converters (`chunk_op`, `chunk_const`, `chunk_const_str`, `struct_template`, `native_name`, `chunk_op_count`, `chunk_local_count`, `word_bits_log2`).
- Cold-path methods (`verify_resources`, `auto_arena_capacity`) deserialize to owned `Module` on call via `module_owned()`.
- `replace_module` serializes the new module to `AlignedVec` and replaces the bytecode store.
- `unsafe Vm::view_bytes_zero_copy(&'a [u8])` is the true zero-copy constructor. Validates framing only. Stores the borrowed slice. The execution loop reads ops and constants directly from the buffer with no owned `Module` materialized.
- The cascade reached the `register_*_natives` helpers and the marshalling test harness, both updated to thread the lifetime parameter through their signatures.

The runtime now supports four entry points spanning the design space.

| Entry point | Source | Verification | Allocation |
|---|---|---|---|
| `Vm::new(Module)` | Owned module | Full | Serializes module internally for archived access |
| `Vm::load_bytes(&[u8])` | Unaligned bytes | Full | Body copy to `AlignedVec` before deserialize |
| `Vm::view_bytes(&[u8])` | Aligned bytes | Full | Skip body copy. Deserialize for verification then store. |
| `unsafe Vm::view_bytes_unchecked(&[u8])` | Aligned bytes | Skip resource bounds | Same as `view_bytes` minus bounds check |
| `unsafe Vm::view_bytes_zero_copy(&'a [u8])` | Aligned bytes | Skip everything | True zero-copy. Borrow the buffer. |

The zero-copy path borrows the buffer's lifetime through `Vm<'a>`. A program loaded via this path executes entirely against the buffer with no module-side heap allocation.

Future work that interacts with P10 but is outside its scope:

- B10 target portability. The wire format is endian-stable through rkyv. Float widths are still hardcoded to f64.
- B9 hot update of yielded static strings. Under zero-copy execution from a swappable buffer, `Value::StaticStr` materialized from the buffer must be valid for as long as the host retains it.
- Optimization. The per-op `op_from_archived` call costs a discriminant match per fetch. A future iteration may cache the chunk's archived ops slice or use a JIT for hot paths.

Phase 2 step 2 also interacts with two backlog items.

B10 (target portability) interacts because the rkyv encoding is endian-stable but float and integer width assumptions still affect runtime semantics. The recent log2 encoding work covers integer widths through masking. Float widths are still hardcoded to f64.

B9 (hot update of yielded static strings) interacts because yielded `Value::StaticStr` under step 2 would be an `ArchivedString` that points into a specific bytecode buffer. A hot update that swaps the buffer invalidates outstanding archived references the host has retained. The resolution paths in B9 (host-responsibility consumption or eager materialization at yield) must be in place before step 2 fully replaces the owned execution path.

Both the WCMU and WCET analyses now multiply the loop body cost by the iteration count when the loop matches the canonical for-range pattern emitted by the compiler. The pattern detector in `extract_loop_iteration_bound` recognizes `Loop GetLocal(var) GetLocal(end) CmpGe BreakIf` followed by a body and traces backward to find the literal `Const` initializers of the var and end slots. Loops whose bounds are not literal integers fall back to the conservative one-iteration treatment, which remains sound but loose. R38 records the implementation.

All P6 items are complete.

1. ~~Enforce the singular data block constraint (R28) at compile time with a clear diagnostic.~~ Complete.
2. ~~Enforce the fixed-size field type constraint at the data block declaration boundary, per the table in [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md).~~ Complete.
3. ~~Extend the structural verifier to reject `GetData` and `SetData` operands that exceed the segment slot count.~~ Complete.
4. ~~Define the host interoperability layer.~~ Complete. Slot-based `Vec<Value>` interface chosen over `repr(C)` struct mapping. Schema mismatch detection by size check plus host attestation. Hash and structural checking deferred. See R29.
5. ~~Specify the concurrency contract.~~ Complete. Single-ownership enforced by Rust borrow checker. Documented in EXECUTION_MODEL.md.
6. ~~Add end-to-end integration tests.~~ Complete. Six new hot swap tests cover same-schema preserved, new-schema replaced, size mismatch rejected, no-data module, swap at reset starts new module, and rollback to prior version.
