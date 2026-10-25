# Order 4 plan, Workstreams E and F

> **Navigation** [Decisions](./README.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

## Scope

The operator set Order 4 as this line's goal on 2026-10-25. The roadmap gives
it the gate "WCET and WCMU bounds carried onto native; the
total-opcode-plus-`Trap` ISA lowered". Workstream E covers the preservation of
worst-case execution time and worst-case memory usage across native
compilation. Workstream F covers the native lowering of the partial operations.
No earlier record scoped either workstream beyond the roadmap's paragraphs, so
the requirements below are derived from those paragraphs, from the native
codegen strategy in [`V0_4_0_NATIVE_CODEGEN.md`](../roadmap/V0_4_0_NATIVE_CODEGEN.md),
and from the findings already on record in
[`NATIVE_BOUNDS_TRANSFER.md`](./NATIVE_BOUNDS_TRANSFER.md) and
[`NATIVE_WCET_ASYMMETRY.md`](./NATIVE_WCET_ASYMMETRY.md).

`BYTECODE_VERSION` stays 2 and no opcode is added. Root `src/` and `tests/`
are not edited. Publication is held.

## Starting state, measured on 2026-10-25 at `cd5476a2`

- The V0.2.X line made the first-pass partial operations total in its Order 3.
  Each produces an in-band outcome flag, and the compiler emits an explicit
  `Trap` when a program writes no handler. The native backend already lowers
  the flag-producing forms. `partial_operation_census.rs` shows that in each of
  its five subjects the trap block is reached only through an explicit `Trap`.
- Every native trap ends in `llvm.trap`, so it aborts the process. The virtual
  machine instead returns a trap to the host as a recoverable error.
- The native host interface has no soft-failure channel. A reified native call
  pushes a constant zero flag, so neither a `host_error` arm nor the default
  `Trap(HostError)` is reachable in native code.
- `bound_transfer.rs` compares the operand slots and composite region the
  backend provisions with the verifier's footprint. `native_stack_bound.rs`
  computes a machine stack bound for a toy program on an ELF target through
  `llc`, and nothing compares it with measured execution or exports it.
- No measurement relates the verifier's cost figures to native execution.

## Two premises this plan takes, and their provenance

1. **Native WCET is best effort, measured in bytecode cost units.** Roadmap
   open decision 1 asks whether native WCET is hard or best effort, and it
   belongs to the operator. The native strategy recommends best effort for
   V0.4.0 and measurement-based bounds for V0.4.x. This plan follows that
   recommendation as a working assumption, not as a ruling. Nothing below
   claims a time bound for native code.
2. **An unhandled partial operation traps natively.** The roadmap's
   Workstream F paragraph says an unhandled operation lowers to a native trap
   and a handled one to a native branch. The operator ruling of 2026-10-20, as
   recorded by the V0.2.X line in `ORDER3_HANDLER_SURFACE_PLAN.md`, says only
   `Trap` traps. [`RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md) still
   specifies platform default values for native code instead. The bytecode
   now carries the explicit `Trap`, so producing a default would make native
   code disagree with the verified bytecode. This plan follows the roadmap and
   the ruling, and reports the specification's disagreement to the V0.2.X
   line, which owns it.

## Requirements

| Id | Requirement | Planned evidence |
|---|---|---|
| F1 | A native trap in a coroutine library is a recoverable outcome carrying its kind | The stable handles gain status three, whose value is the `TrapKind` code. A trap in the entry, in an ordinary callee and in a delegated callee each reaches the host. The instance is inactive afterwards. Each first-pass kind is compared with the virtual machine's error |
| F2 | A native host function can fail softly | A host contract marks a native fallible. Its symbol then returns a value and a failure flag. A handled `host_error` arm runs natively, and a bare call reaches status three with `HostError`. Both are compared with the virtual machine |
| F3 | Each first-pass total partial operation lowers to its flag and a handled outcome to a branch | Handled subjects for division, remainder, checked arithmetic, indexing and indexed data access execute natively on failing inputs and agree with the virtual machine |
| F4 | The superseded designs are recorded | The `OverflowPolicy::Trap` option and the specification's native defaults are recorded as superseded, the latter reported to V0.2.X |
| E1 | Native code executes the bytecode's cost path | An opt-in meter counts each bytecode instruction natively. Per call and per resume, the native counts equal the counts in the virtual machine's instruction trace |
| E2 | The bytecode WCET claim is carried onto the library | A per-step bound composed through calls between scripts is exported in nominal cost units with an explicit status. The traced cost of every step stays within it |
| E3 | The machine stack is bounded | The library computes its machine stack bound from the code generator's own frame sizes over the emitted call graph and exports it. A host measures its actual use by stack painting and stays within the bound |
| E4 | The arena bounds are carried | The exported frame, slot, shared, private and region figures are checked against the verifier's memory figures and against native use |

## Order of work

F1 changes the slot contract, which E3 and the exports read, so it lands
first. F2 builds on F1's status. F3, E1, E2 and E3 follow, then the completion
record, the roadmap status and the handoff.

## Known limits accepted in advance

- A trap in a function lowered outside a coroutine still aborts through
  `llvm.trap`. Only the coroutine library interface reports traps.
- A trap reports its kind, not its payload. The index and bound of an
  indexing trap and the host's error code are not returned.
- The stack bound covers code this library emits. A host native or a compiler
  runtime routine the object calls is outside it.
- One host target is executed, `aarch64-apple-darwin`.
