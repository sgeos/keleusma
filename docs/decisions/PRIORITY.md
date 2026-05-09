# Priority Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Open decisions that may block near-term development.

## ~~P1. Type checker implementation~~ (Resolved)

A static type checker is in place at `src/typecheck.rs` and is invoked from `compile`. Type errors are surfaced as `CompileError` before bytecode emission. The parser now represents the unit literal `()` as `Literal::Unit` rather than `Literal::Int(0)`. The compiler emits `Op::PushUnit` for the new variant. The type checker recognizes `Literal::Unit` as `Type::Unit`. Five existing tests that relied on lax behavior were updated to declare the types they reference.

Subsequent passes closed four type checker gaps.

- Multiheaded function parameter types are now recorded on the bound locals through `compile_pattern_bind_typed`. The compiler's `TypeInfo` gained an `enums` map for variant payload type lookup. Tuple, struct, and enum patterns all decompose the type expression structurally.
- Native function call type checking distinguishes between user-defined functions (full signature check), names imported via `use` (accepted with any args), names qualified with `::` (treated as natives), and truly undefined names (rejected with a clear error). The earlier silent-pass behavior for unknown names is replaced with explicit categorization.
- Pattern type checking against the scrutinee. Match arms are structurally validated against the scrutinee's static type. Tuple arity must match. Enum variants must exist and have the right payload arity. Struct field names must be declared. Literal pattern types must be compatible with the scrutinee.
- Match arm exhaustiveness. Enum scrutinees must cover every variant or have a wildcard arm. Bool scrutinees must cover both true and false or have a wildcard. Unit scrutinees must cover `()` or have a wildcard. Other types require a wildcard arm.

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

## ~~P2. For-in over expressions~~ (Resolved)

For-in over array expressions is supported when the source expression's static array length is determinable at compile time. The compiler emits a `Const(N)` end bound that the strict-mode WCMU verifier accepts. The cases in scope.

- Array literal source. `for x in [1, 2, 3]`. Length from element count.
- Function return source. `for x in make()` where `make()` returns `[T; N]`. Length from declared return type.
- Data segment field source. `for x in ctx.items` where `items` is declared `[T; N]`. Length from data block declaration.
- Let-bound array literal. `let arr = [1, 2, 3]; for x in arr`. Length traced through the local alias chain to the originating `NewArray`.
- Struct field access from a local. `let b = Box { items: [..] }; for x in b.items`. The compiler tracks local variable types via let annotations and inference (struct construction, function call, field access, identifier, array literal, literal). The local's type is consulted to resolve the field access to a typed array.
- Local of typed array. `let arr: [i64; 4] = make(); for x in arr`. The local's annotated type carries through.
- Function parameter typed array. `fn sum(arr: [i64; N]) -> i64 { for x in arr ... }`. Parameter types are recorded on the locals at function entry.

Nine tests cover the resolved paths. `for_in_over_function_return_passes_strict_verify`, `for_in_over_data_segment_field_passes_strict_verify`, `for_in_over_array_literal_runs`, `for_in_over_struct_field_from_local_passes_strict_verify`, `for_in_over_param_array_passes_strict_verify`, `for_in_over_nested_array_index_passes_strict_verify`, and `for_in_over_match_array_result_passes_strict_verify`.

Additional cases now resolved.

- Nested array indexing. `for x in matrix[0]` where `matrix` is `[[T; N]; M]`. The compiler infers the index expression's type as the element type of the matrix and uses it for the iteration bound.
- Match expression results. `for x in match cond { ... => arr1, _ => arr2 }`. The compiler infers the match result type from the first arm's expression. The type checker (P1) ensures all arms agree.
- Multi-level nested for-in. `for z in matrix3d { for y in z { for x in y { ... } } }` where `matrix3d` is `[[[T; A]; B]; C]`. The for-in iteration variable now records the element type derived from the source's array type, so each nested level resolves its own bound through the iteration variable's type.

Implementation. The compiler tracks local variable types. The `Local` struct gained a `ty: Option<TypeExpr>` field. Let bindings record their declared annotation or inferred type. Parameters record their declared type. For-in iteration variables record the element type derived from the source's array type. The `infer_expr_type` helper covers struct construction, function calls, identifiers, field access, array literals, array indexing, match expressions, and literal values for type inference. The `element_type_of` helper extracts the element type from `TypeExpr::Array`. The `static_for_in_length` helper consults the type of identifier expressions through the local table, of array index expressions through their object's element type, and of match expressions through their first arm. The result is that for-in over any expression whose static type is `[T; N]` produces a `Const(N)` end bound rather than `Op::Len`. Multi-level nested for-in works because each level's iteration variable carries the right element type for the next level to consult.

## ~~P3. Error recovery model~~ (Resolved)

The runtime error recovery model is the explicit-recovery design with host-driven retry. When the VM encounters a runtime error during `Vm::call` or `Vm::resume`, it returns `Err(VmError)` and leaves itself in an undefined intermediate state. The host inspects the error and, if recovery is desired, calls `Vm::reset_after_error()` to return the VM to a clean callable state. The data segment is preserved across recovery so accumulated state survives error events. The operand stack, call frames, and arena are cleared.

The contract.

- A failed `call` or `resume` returns `Err(VmError)`. The VM's volatile state is undefined until the host explicitly recovers.
- `Vm::reset_after_error()` clears the operand stack, call frames, and arena. The data segment and bytecode store are preserved.
- After recovery, the host can call `Vm::call` to start a fresh iteration.
- For unrecoverable conditions or to reset the data segment, the host uses `Vm::replace_module` to swap to a new code image with new initial data.

Rationale. Streams already have RESET as the natural per-iteration recovery point. Error recovery extends the same model to errors. The host decides whether to retry, log, replace the module, or escalate. The model is consistent with the existing hot-swap design (R26, R27) which also clears volatile state while letting the host control data continuity. Three tests cover the recovery path. `reset_after_error_preserves_data` confirms accumulated data survives. `reset_after_trap_clears_volatile_state` confirms a trap can be caught and the VM returned to a callable state. `reset_after_error_idempotent` confirms repeated calls are harmless.

Out of scope and tracked elsewhere.

- Bidirectional error handling between script and host through the yield boundary is recorded as B7. The current model only flows errors host-ward.
- Distinction between halt errors (bytecode invariant violations) and soft errors (division by zero, type errors). The current model treats all errors uniformly. A future iteration may add a category field to the error if hosts need to make policy decisions per kind.

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
