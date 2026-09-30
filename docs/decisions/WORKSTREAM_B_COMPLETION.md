# Workstream B completion requirements

The active objective is full Workstream B in the V0.3.X roadmap. The bounded
coroutine slice merged at `a1aa53b8` is the baseline. Implementation and acceptance
work continue on `feat/workstream-b-completion`. Final gates and version
integration remain outstanding. This document does not yet claim completion.

## Acceptance matrix

The named tests are in
[`retcon_bytecode.rs`](../../native_codegen/tests/retcon_bytecode.rs).
They execute virtual-machine comparisons and independent expectations, with
exceptions and deployment limits stated below.

| Requirement | Current evidence | Disposition |
|---|---|---|
| General suspension and resumption | `non_tail_yield_preserves_locals_operands_and_reply`, branch joins, loop backedges, private state and seeded lexer checks | Implemented, final gates pending |
| Delegated suspension | Nested calls, guarded heads, nested stream Reset, preserved callee arguments and release inside a callee | Implemented, final gates pending |
| Entry shapes | Zero and multiple arguments, all six scalar kinds, bounded flat structures, tuples, arrays and enums, independent instances | Implemented within the native representation boundary below |
| Value transfers | Per-site dialogues, actual kinds and lengths through packing, calls, private storage, direct and delegated replies, snapshot and instance-borrow contracts | Implemented, final gates pending |
| Host lifecycle | `reentrant_entries_report_completion_once_and_reuse_the_slot`, completion without yielding, inert resume after completion, repeated release, early release and independent slot reuse | Implemented, final gates pending |
| Admission soundness | Refusal classification below, runtime type and extent checks, negative subprocess tests requiring a valid prefix before a fault | Audited within stated boundaries, final gates pending |
| Integration | `host_control_routes_are_explicit_and_do_not_fall_back_to_callbacks` and documented explicit coroutine route | Implemented, version integration pending |
| Native deployment | Linked C hosts at both optimization levels, native snapshot completion, object emission for four tier-one target configurations | Local execution and target emission checked, foreign execution unverified |
| Resource preservation | Bounded caller reservation, compile-time oversized-frame refusal, completion tail outside the ended frame, allocation and intrinsic postconditions | Implemented, final gates pending |
| Verification | 95 coroutine tests and test-target Clippy in each float configuration, frozen default non-corpus run of 776 tests | Both complete native gates and green version integration remain outstanding |

## Refusal classification

The audit covers admission in `coroutine.rs`, `coroutine/dialogue.rs`,
`coroutine/types.rs`, `coroutine/ownership.rs` and the shared emitter it invokes.
A passing corpus count does not establish coverage of every program.

| Refusal family | Classification and reason |
|---|---|
| Failed bytecode verification or resource admission, missing entry or signatures, malformed stream layout, invalid slot ranges | Invalid input to this lowering route |
| Missing per-site dialogue, a contract naming a non-yield site, incompatible scalar stream replies | Invalid host contract. The virtual machine also rejects scalar reply type mismatches |
| Unknown transfer extent, undersized body, construction exceeding its reservation, unproved native body lifetime | Unsafe contract. Bounded flat transfers, snapshots and instance borrows have positive execution coverage |
| Too little frame space, completion or metadata reservation overflow, persistent private metadata overflow | Insufficient explicit memory reservation |
| Suspension-capable call or coroutine intrinsic surviving splitting, unexpected deallocator or dynamic stack intrinsic | Failed lowering postcondition. Refusal prevents exposing an invalid artifact |
| Numeric or body operation with no admissible selected kind | Invalid operand use. Mixed values with an admissible selected kind use runtime checks |
| Text-bearing and opaque host representations, legacy boxed field and index forms | Native representation dependencies. The coroutine route supports the shared scalar and bounded flat representation, not a new string or opaque-handle contract |
| `Len` and surviving `IsStruct` | Shared opcode dependencies in Workstream A. `Len` has no current compiler emission path. The compiler records no `IsStruct` producer found by its bounded search. Hand-built bytecode still reaches the explicit backend refusal |
| Indexed access spanning private scalar and composite placements | Shared placement dependency. The negative test constructs verified bytecode spanning placements rather than a compiler-emitted homogeneous array |

The former composite stream reply restriction was an implementation gap, not a
necessary refusal. The virtual machine admits differing bounded bodies and scalar
values through its Composite parameter category. This now executes with actual
kind, ownership and length preserved through direct yields, transitive delegation
and Reset. Smaller bodies trap before an out-of-bounds consumer read.

The audit has not identified another suspension-specific implementation gap.
This is an engineering conclusion from the inspected paths and executed tests,
not a proof that all possible bytecode programs have been enumerated.

## Resource and verification limits

Lowering inlines suspension-capable callees before coroutine splitting and
rejects any survivor. It verifies the generated module, rejects retained
coroutine or dynamic-stack intrinsics and unexpected deallocator uses, and
rejects any frame exceeding its caller reservation. Completion bodies occupy a
disjoint tail that remains readable after deliberate overwriting of the ended
continuation frame. External pointers and declared readable extents remain host
obligations. These checks do not attest native worst-case execution time.

Private composite self-assignment exposes an upstream virtual-machine defect.
Its overlapping `copy_nonoverlapping` operation aborts in the debug runtime.
The native regression uses explicit expected values and region sentinels rather
than executing undefined reference behavior. Root runtime sources remain unchanged.

Fault subprocess checks currently observe the local AArch64 trap signal.
Object emission for other targets does not establish their runtime behavior or
signal convention. Complete gates must run separately on a clean frozen commit.
Historical gates at `5a0fec1d` do not cover these changes. Receipt updates after a
run must not be described as the identical tested tree.

## Scope and completion rule

The roadmap separates coroutine lowering from arena pool packaging, general host
packaging and native timing attestation. Future source syntax for nested arena
instances and cross-thread snapshots is not a prerequisite for current-language
suspension. These boundaries do not make the whole native backend complete.

Every matrix row needs current evidence and every remaining dependency must stay
explicit. Do not infer completion from a smaller passing subset or a historical
run. No opcode or bytecode version changes are authorized by this workstream.
