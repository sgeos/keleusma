# Order 2 completion audit

> **Navigation** [Completion plan](./ORDER2_COMPLETION_PLAN.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope and disposition

Order 2 is complete for the full first-pass validator scope on 2026-10-13.
The implementation, five feature configurations and documentation are verified. This audit preserves the roadmap's structural, typed, resource,
text-allocation, module orchestration and shipping requirements. Full-language
expansion remains Order 6.

The admission contract combines the checks represented by verify and
verify_resource_bounds. Native attestations and host value-slot width have an
additional public entry. Acceptance retains the previously selected typed
control-flow widening profile and the reference consistency gate.

## Requirement evidence

| Requirement | Current implementation and evidence | Disposition |
|---|---|---|
| Structural verdict | verify_structural.kel, verify_depth.kel and verify_yield.kel check targets, operands, block types, depth and productivity. Forward targets are checked before region walks. The detached 46-test structural corpus now invokes both legacy and fallible whole-module entries. All 46 pass. | Verified |
| Typed verdict | verify_typed.kel consumes complete parameter, native-return and enum facts. Existing typed tests and eight new controls cover projection forms, stride, locals, resume shapes, loop heights, layout reconciliation and transport capacity. The selected widening boundary is tested explicitly. | Verified for the first-pass profile |
| Complete stage corpus | Directory-derived inventories cover all fourteen current stage sources. Every module passes the fallible structural/typed verifier. Full resource admission agrees with the reference below, at and above its arena boundary. All fourteen complete artifacts match the reference and decode. | Verified |
| Call graph | analyze.kel selects traversal, completion order and cycle rejection. Rust services indexed opcode requests. Five controls cover all stage graphs, graph shapes, cycles, capacities and an intervention that changes only the stage's cycle decision. | Verified |
| Module summaries and admission | analyze.kel retains per-chunk stack, heap and text summaries and decides completion and stream arena admission. Independent interventions disable memoized heap use or admission and change the verdict for identical subject input. Missing summaries are errors. | Verified |
| Yield fixpoint and productivity | verify_yield.kel schedules rounds, retains membership, decides convergence and emits the productivity rejection flag. Separate interventions change membership and productivity decisions. | Verified |
| Text allocation | analyze.kel owns NotText, Known and Unbounded transitions, widening, saturating heap totals and boundary flags. Host storage transports abstract values. Resource summaries include text heap and transitive return flags. Differential controls cover calls, locals, branches, loops, natives and early saturation. | Verified |
| Resource arithmetic and host costs | Heap addition and multiplication saturate at the reference unsigned 32-bit ceiling. Loop-bound extraction checks arithmetic overflow and follows literal-array length aliases. Keleusma charges native costs per site or per unique external native according to attestations. Shared copy-out is supplied as a primitive opcode cost fact. Twelve resource tests include all-stage boundaries and subject widths. | Verified |
| Fallible admission | admit_module_via_kel combines structural, typed and resource results. The attested variant accepts native bounds and host value-slot width. Capacity, unfinished work, execution and protocol failures remain errors. Failure injection proves that unavailable analysis cannot become a completed verdict. | Verified |
| Shipping integration | keleusma validate selects rust or self-hosted. The Keleusma result is computed before the reference comparison. Disagreement and unavailable analysis return two, rejection one and acceptance zero. Four actual command tests include both validators on all fourteen stage boundaries and a concrete disagreement witness. | Verified |
| Compatibility and operator limits | Five feature configurations, minimum Rust and embedded compilation pass. Public bytecode version remains 2, instruction count 66, selected type capacities 640 and the depth limit 32. The analyzer uses a 64-bit host Word. Subject widths include 8, 16, 32 and 64 bits. | Verified |
| Recorded completion and documentation | The requirement register, roadmap, process records and public command documentation carry the implemented scope and limits. Citation, roadmap and public-link controls pass. | Verified |

## Host and trust boundary

The host decodes bytecode and transports opcode classes, operands, primitive costs,
widths and declared metadata. It allocates stage machines and buffers, services
indexed requests, selects regions from validated markers, sequences passes and
reports rejection or unavailable analysis. Keleusma performs the control-flow
walks, graph ordering, yield fixpoint updates, type and layout checks, text
transitions, resource summary storage and arena admission.

Reference verification of the subject remains in the shipping consistency gate
and differential tests. Bootstrapping the trusted analyzer stages still uses the
Rust compiler and safe virtual-machine constructor. The resource admission audit
covers the current reference memory model and finite-loop decisions. It does not
extend the separate timing-estimate interfaces or claim a trap-freedom verdict.

The typed stage widens shapes across control-flow boundaries under the selected
first-pass profile. A concrete cross-join scalar witness is accepted by the stage
and rejected by the more precise reference. The shipping command reports this
as disagreement and cannot report successful admission. Runtime guards remain
required. Independent review remains a prerequisite for reference retirement.

The bounded profile admits 1024 chunks, 1536 opcodes per chunk, 256 local and
operand slots, 256 enum layouts and 12288 transported call shapes per chunk.
Exhaustion is unavailable analysis. The existing 32-level depth limit is preserved.
The call-shape control accepts exactly 12288 entries and reports capacity at 12416.
Dead calls isolate that transport boundary from stack and opcode capacities.

The command executes no subject instructions and does not authenticate signatures.
Native attestations are empty in the offline command. Hosts with attested costs use
the library interface. The runner's security and deployment policies remain in force.

## Defects demonstrated and repaired

Earlier Order 2 controls repaired missed ninth call arguments, float constants
misclassified as unknown, enum-table truncation and module metadata lost when a
caller signature was absent. The fallible boundary added explicit unfinished-work
and storage outcomes and enforced the loop operand floor.

The resource increment repaired branch-prefix widening, analysis after terminated
branches and incompatible break heights. A later audit caught an error in the
pending loop change. When a terminated loop body has no break state, the reference
restores the entry state and checks subsequent instructions. The stage skipped
those checks and the complete admission entry accepted a malformed scalar
projection. The identical witness now rejects, and its valid control accepts.
The permanent control is in tests/selfhost_rejection_families.rs.

The extreme loop-count control also exposed unchecked subtraction in the Rust
reference. Checked subtraction prevents a debug overflow panic. The corresponding
debug test passes separately from the release suite.

## Verification receipts

The validator source tree remained fixed during the corrected gate. Each step
recorded an empty changed-subject list against
tmp/order2-corrected-subject-hashes.json. Full commands, exit codes and durations
are in tmp/order2-corrected-gate-results.json. Tests used release mode, an empty
RUSTC_WRAPPER and KEL_SELFHOST_CACHE set to zero. The full nextest ci profile was
used, with separate documentation tests. Unqualified log names in the table use
the prefix tmp/order2-corrected-.

| Check | Result and receipt |
|---|---|
| Workspace | 3281 tests pass, two skipped. Twelve documentation tests pass, three ignored. tmp/order2-corrected-workspace.log and workspace-doctests.log. |
| No default features | 329 tests pass. Documentation tests pass. tmp/order2-corrected-no-default.log. |
| Signatures | 2350 tests pass. Documentation tests pass. tmp/order2-corrected-signatures.log. |
| Signatures and shell | 2367 tests pass. Documentation tests pass. tmp/order2-corrected-broad.log. |
| Isolated self-host | 3031 tests pass, two skipped. Documentation tests pass. tmp/order2-corrected-self-host.log. |
| Wire configurations | 57 tests pass with all features and 20 without defaults. tmp/order2-corrected-wire-all.log and wire-no-default.log. |
| Detached compiler | 86 tests pass, including the extended 46-test structural corpus. Clippy and formatting pass. tmp/order2-corrected-detached.log and detached-clippy.log. |
| Workspace Clippy and formatting | Fresh all-target Clippy reports actual Checking and passes. It is repeated after the link-control addition in tmp/order2-handoff-clippy.log. Formatting passes in tmp/order2-handoff-fmt.log. |
| Documentation builds | Workspace documentation with signatures, encryption, shell and self-host passes with warnings denied. Wire and arena all-feature documentation also pass. tmp/order2-corrected-docs.log, wire-docs.log and arena-docs.log. |
| Minimum Rust and embedded target | Rust 1.88 checks tests with self-host. The thumbv7em-none-eabihf build passes. tmp/order2-corrected-msrv.log and no-std.log. |
| Book | 85 assertions pass with zero failures and floor violations. tmp/order2-corrected-book.log. |
| Debug arithmetic boundary | One extreme iteration-bound test passes in debug mode. tmp/order2-final-debug-iteration.log. |
| Shell scripts | ShellCheck exits zero. tmp/order2-final-shellcheck.log. |
| Link-gate correction | The initial link check reached generated copies under the ignored secret directory. The checker now prunes that local store. A new command control proves public broken links still fail and restored public links pass. tmp/order2-doc-link-controls.log and public-doc-links.log. Six citation checks, the roadmap consistency check and the new public-link control pass. Final link and formatting receipts are in tmp/order2-handoff-doc-links.log and tmp/order2-handoff-fmt.log. |

The first workspace run completed 3284 passes, one stale residency-pin failure and
five ignored tests. It is not a green receipt. Its remaining sequence was stopped
before the loop repair. Measured pins now use analyzer total 3215 and peak 367,
and typed-validator total 1438 and peak 395, preserving the existing threefold
fusion requirement. The corrected full workspace run executes those pins successfully.

The link-gate repair and its one new command test followed the full feature runs.
The validator implementation was unchanged. That new test and the final documentation
checks have separate receipts, rather than being added to the earlier test counts.
The source inventory is 1682 integration test attributes across 169 root files.
Workspace feature unification executes 1407 library tests, and isolated self-host
executes 1357. These counts describe this increment.

Memory-interpreter, dependency-advisory and unrelated platform checks were not repeated.
No unsafe arena code, dependency manifest, lockfile or platform implementation changed.
Remote continuous integration, merging and publication are not claimed.

## Pilot record

Task v023-order2-resource-a12c contains its first diagnostic and three bounded claims.
The diagnostic is labelled compiler_or_test. Protocol failure and post-loop repair
claims are labelled supports. The resource-boundary claim is labelled unsupported
because its short excerpt omits proof that the limit is positive. The actual command
corpus explicitly asserts positive capacity and passes for all fourteen stages.
All labels are non-independent self-reviews. External evaluation remains unavailable
under the earlier automatic approval rejection. No provider judgment, independent
accuracy or avoided-reasoning claim is made. The increment outcome is resolved.
The reviewed export tmp/jev-shadow-order2-resource-a12c-final.json contains 193 samples
and 49 task records. All four current request digests and source hashes match.
The peer-owned tracked pilot checkpoint was not overwritten. Usage remains null.

## Next milestone

Order 3 adds trap analysis and a host mode gated on the agreed trap-free verdict.
