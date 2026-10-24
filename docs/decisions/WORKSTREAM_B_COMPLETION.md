# Workstream B completion requirements

The active objective is Workstream B in the V0.3.X roadmap. Its coroutine
implementation integrated through pull request 482. Absorption 82 subsequently
updated division and indexed-access contracts. Absorption 83 reconciles the
implementation with upstream `f213f421`, including the total `FixedDiv` contract.
This record distinguishes executed evidence from remaining coverage limits.

## Acceptance matrix

Unless another file is named, tests below are in
[`retcon_bytecode.rs`](../../native_codegen/tests/retcon_bytecode.rs).
They compare execution with the reference virtual machine and independent
expectations. The private self-assignment exception is stated below.

| Requirement | Executable evidence | Disposition |
|---|---|---|
| General suspension and resumption | `non_tail_yield_preserves_locals_operands_and_reply`, branch joins, loop backedges, private state and seeded lexer checks | Implemented within the admitted representation |
| Delegated suspension | Nested calls, guarded heads, nested stream Reset, preserved callee arguments and release inside a callee | Implemented within the admitted representation |
| Entry shapes | Zero and multiple arguments, all six scalar kinds, bounded flat structures, tuples, arrays and enums, independent instances | Implemented within the native representation boundary below |
| Value transfers | Per-site dialogues, actual kinds and lengths through packing, calls, private storage, direct and delegated replies, snapshot and instance-borrow contracts | Implemented with runtime kind and extent checks |
| Host lifecycle | `reentrant_entries_report_completion_once_and_reuse_the_slot`, completion without yielding, inert resume after completion, repeated release, early release and independent slot reuse | Implemented |
| Admission soundness | Refusal classification below and negative subprocess tests requiring a valid prefix before a fault | Audited over the named paths and executable cases |
| Total division | `bare_integer_division_and_remainder_are_total_on_zero`, `checked_numeric_dispatch_preserves_low_high_and_flags`, `fixed_division_outcomes_survive_suspension_and_delegation` | Outcome slots are preserved, including zero divisors and successful saturation |
| Indexed access | Mixed array extent checks and `handled_nested_writes_preserve_state_across_suspension` | Valid writes and failures at either nested index agree across delegated suspension |
| Integration | `host_control_routes_are_explicit_and_do_not_fall_back_to_callbacks` and documented explicit coroutine route | Implemented. This absorption remains on its feature branch pending integration |
| Native deployment | Linked C hosts at both optimization levels, native snapshot completion, object emission for four tier-one target configurations | Local execution and target emission checked. Foreign execution remains unverified |
| Resource preservation | Bounded caller reservation, compile-time oversized-frame refusal, completion tail outside the ended frame, allocation and intrinsic postconditions | Implemented. These checks do not attest native execution cost |

The direct lowering also executes
`fixed_division_outcome_slots_agree_without_a_trap` in
[`differential.rs`](../../native_codegen/tests/differential.rs).
It observes all three slots without compiler dispatch, including fraction counts
zero, sixteen and sixty-three. Source handlers receive a zero-divisor numerator.
Uncaught source division still reaches the compiler's explicit `Trap`.

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
| Text-bearing and opaque host representations, legacy boxed field and index forms | Native representation dependencies. The coroutine route supports the shared scalar and bounded flat representation |
| `Len` and surviving `IsStruct` | Shared opcode dependencies in Workstream A. `Len` has no current compiler emission path. The bounded compiler search found no `IsStruct` producer. Hand-built bytecode still reaches explicit refusal |
| Indexed access spanning private scalar and composite placements | Shared placement dependency. The negative test constructs verified bytecode spanning placements. The positive nested-write test uses a homogeneous array |

The former composite stream reply restriction was an implementation gap.
The virtual machine admits differing bounded bodies and scalar values through
its Composite parameter category. Those cases now execute with actual kind,
ownership and length preserved through direct yields, transitive delegation and
Reset. Smaller bodies trap before an out-of-bounds consumer read.

The inspected paths and executed tests have not identified another
suspension-specific implementation gap. This is an engineering conclusion,
not an enumeration of all possible bytecode programs.

## Resource and verification limits

Lowering inlines suspension-capable callees before coroutine splitting and
rejects any survivor. It verifies the generated module, rejects retained
coroutine or dynamic-stack intrinsics and unexpected deallocator uses, and
rejects any frame exceeding its caller reservation. Completion bodies occupy a
disjoint tail that remains readable after deliberate overwriting of the ended
continuation frame. External pointers and declared readable extents remain host
obligations. These checks do not attest native worst-case execution time.

Private composite self-assignment retains its documented reference limitation.
The absorbed runtime still uses `copy_nonoverlapping` on the aliased body.
The native regression uses independent expected values and region sentinels,
without executing that undefined reference operation. No root runtime repair
is made by this line.

The corpus no longer supplies a single-head stage subject. The stage tests use
`11_signed.kel` without its `signed` modifier. `verify_datalayout.kel`,
`wire_debug.kel` and `wire_schema.kel` agree only on unseeded runs. These are
coverage residuals and are not closed by the coroutine acceptance matrix.
Fault subprocess checks observe the local AArch64 trap signal. Object emission
for other targets establishes neither execution nor their signal convention.

The committed [gate record](../../native_codegen/GATE_RECORD.md) contains clean,
frozen full-gate passes at `ece800a5`. It is stale for this tree.
The prior handoff records hosted Native gate run `39988352001` passing both
complete configurations at `4c19e026`. That hosted result does not certify
absorption 83. Whether hosted verification satisfies the full-gate requirement
before publication or a merge into `main` remains an unresolved operator ruling.
Current local commands and results are recorded in the
[absorption receipt](./ABSORPTION_83_BRIEF.md). Targeted or non-corpus checks
must not be described as either complete native gate.

## Scope and completion rule

The roadmap separates coroutine lowering from arena pool packaging, general host
packaging and native timing attestation. Future source syntax for nested arena
instances and cross-thread snapshots is not a prerequisite for current-language
suspension. These boundaries do not make the whole native backend complete.
Every matrix row needs relevant current evidence and every remaining dependency
must stay explicit. No opcode or bytecode version changes are authorized.
