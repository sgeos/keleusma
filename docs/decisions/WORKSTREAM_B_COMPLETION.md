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
| Value transfers | Scalar and composite inputs, outputs, replies and completion values with proven extents and lifetimes | Per-site dialogues, native lifetime contracts and bounded mixed-kind copies implemented, final audit outstanding |
| Host lifecycle | Start, resume, normal completion, early release, repeated release and reuse with stable caller-owned storage | Completion status and result lifetime implemented, final lifecycle sweep outstanding |
| Admission soundness | Every remaining refusal distinguished as invalid input, unsafe contract, another workstream dependency, or an implementation gap | Mixed receiver and extent support implemented, guarded nested parameter reads remain a demonstrated gap |
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

The mixed enum receiver gap now has executed positive coverage in
`mixed_enum_receiver_kinds_execute_with_guarded_body_inspection`. Runtime tags
prevent scalar dereferences. Enum-test facts refine payload reads only while the
receiver binding remains unchanged. Private body slots preserve the stored kind.
Owned mixed bodies retain their selected extent in statically bounded storage.
Field reads use a proven minimum while array bounds use the selected actual size.
Subprocess tests require an observed native bounds trap for negative indices and
indices outside the smaller array. Targeted verification is recorded in the
native archive. Remaining refusal classification and complete final gates remain
outstanding. The current evidence does not establish full completion.

A further source-level counterexample is now explicit in
`guarded_nested_parameter_use_remains_a_lowering_gap`. A private Boolean permits
a nested stream to read its Word parameter only on the first iteration. The
virtual machine yields 5 then zeros. Native lowering rejects every non-Unit
parameter read in a nested stream, including this valid case. Guarded handling of
possibly cleared values is the next implementation step. Do not classify the
whole refusal family as necessarily faulting virtual-machine behavior.

## Completion rule

Every row needs current evidence. A smaller passing subset, a plausible refusal,
or a historical green run does not establish completion. This document records
requirements and unresolved work, not permission to narrow the goal.
