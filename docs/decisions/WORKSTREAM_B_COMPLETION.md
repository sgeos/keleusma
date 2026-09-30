# Workstream B completion requirements

The active objective is full Workstream B in the V0.3.X roadmap. The bounded
coroutine slice merged at `a1aa53b8` is the baseline, not the completion claim.
Work takes place on `feat/workstream-b-completion`.

## Acceptance matrix

| Requirement | Evidence needed | Current disposition |
|---|---|---|
| General suspension and resumption | VM differentials for non-tail yields, branches, bounded loops, live operands and locals | Existing tests need final rerun |
| Delegated suspension | Nested and guarded reentrant calls preserve caller and callee state | Existing tests need final rerun |
| Entry shapes | Source-emittable stream and reentrant entries, including completion, supported argument shapes and independent instances | Scalar and flat completion plus zero and multiple arguments tested, remaining shapes to census |
| Value transfers | Scalar and composite inputs, outputs, replies and completion values with proven extents and lifetimes | Per-site dialogues and native snapshot or instance-borrow results implemented, remaining mixed-kind transfers to audit |
| Host lifecycle | Start, resume, normal completion, early release, repeated release and reuse with stable caller-owned storage | Completion status and result lifetime implemented, final lifecycle sweep outstanding |
| Admission soundness | Every remaining refusal distinguished as invalid input, unsafe contract, another workstream dependency, or an implementation gap | Mixed receiver inspection remains a demonstrated implementation gap, other refusals still need classification |
| Integration | Existing public lowering routes and host entry points have a documented, tested selection contract | Explicit route contract documented and tested, no callback fallback |
| Native deployment | Execute real linked host artifact and optimized/unoptimized differentials, inspect target emission within the roadmap target boundary | Local C snapshot completion runs at both optimization levels, four tier-one object formats and architectures checked, final coverage audit outstanding |
| Resource preservation | No live machine-stack state across suspension, no hidden allocation, bounded frame reservation, guarded extents and independent regions | Existing checks need completion-path coverage |
| Verification | Both complete native configurations on a frozen final tree, relevant root documentation checks, and green integration continuous integration | Outstanding |

## Scope boundaries

The roadmap separates coroutine lowering from arena pool packaging, general host
packaging, and native timing attestation. Implement the ownership and host
mechanisms necessary for correct coroutine behavior here. Record dependencies
rather than declaring the whole language implemented from a corpus count.

Future source syntax for nested arena instances and cross-thread snapshot
communication is not a prerequisite for lowering the current language. Do not
add opcodes or change the bytecode version. Root runtime and verifier sources
remain owned by V0.2.X. A runtime discrepancy must be recorded with a reproducer.

## Current evidence

On resume the native worktree was clean at `a1aa53b8`. All 150 handoff ancestry
anchors passed. The archive stamp was 23 commits behind, the brief still named
completed integration as pending, and the complete-gate reporter returned
UNVERIFIED for three documentation inputs changed after `5a0fec1d`. The handoff
is invalid-and-stale. The version-branch run `36673486533` completed successfully
at `a1aa53b8`. After fetching, the upstream version-branch backlog was zero.

The initial new differential requires a reentrant entry to report two yields
and its final result, to complete without yielding on a branch that does not yield,
and to complete after delegated suspension. Each result is checked against a
hardcoded expectation and the virtual machine. Slot reuse and inert operations
after completion are part of the same lifecycle test.

The current native differentials cover original reentrant parameters, delegated
completion, exactly-once completion, slot reuse, multiple arguments and owned flat
results after input reuse and deliberate overwriting of the ended LLVM frame.
Both float configurations passed all 46 coroutine tests and test-target Clippy.
The implicit-completion negative test confirms that the current verifier rejects
fallthrough before lowering. The native archive records the commands and limits.

Explicit contracts now describe each yield site's output and reply separately
from the entry result. A bounded metadata word identifies the suspended site.
Tests cover heterogeneous direct and delegated dialogues, all scalar completion
kinds, changing flat reply extents, host-buffer reuse and completion-body lifetime.
An undersized reply negative test exposed an accepted field overread before the
new read and call-boundary extent checks. That unsafe contract is now refused.
Extent analysis converges across branch joins and loop backedges. Private-slot
kind inference preserves aliasing while admitting a previously refused call.
Composite-kind mismatches are checked against virtual-machine faults. Both float
configurations pass 53 coroutine tests and test-target Clippy with warnings denied.

Native flat results now accept explicit snapshot or instance-borrow contracts.
Execution tests cover host-buffer reuse, callee returns, early release and
completion storage. Unused chunks no longer impose coroutine lowering contracts,
and original yield-site identifiers remain unchanged. Both float configurations
pass 58 coroutine tests and test-target Clippy with warnings denied.

The admission audit identifies a source-emittable gap in
`mixed_enum_receiver_kinds_remain_an_explicit_boundary`. A branch joins a constructed
enum with a Word reply, then matches the result. The virtual machine handles both
paths correctly, while native lowering refuses the mixed receiver. Runtime kind
tracking and safe conditional body inspection are required before this can count
as supported. Do not classify that case as invalid input or another workstream.
Remaining transfer proofs, the full refusal audit and complete final gates remain
outstanding. The current evidence does not establish full completion.

## Completion rule

Every row needs current evidence. A smaller passing subset, a plausible refusal,
or a historical green run does not establish completion. This document records
requirements and unresolved work, not permission to narrow the goal.
