# Order 2 completion plan

## Objective

Complete the full validator in Keleusma on the V0.2.X line. Preserve the roadmap's
structural, typed and resource verdict requirements, the explicit text-allocation
term, module orchestration and shipping-tool integration. Order 1 remains complete.
Order 2 is complete for the full first-pass scope. See
[the completion audit](./ORDER2_COMPLETION_AUDIT.md) for final evidence and retained limits.

## Requirements and current evidence

| Requirement | Evidence needed | Current state |
|---|---|---|
| Structural verdict | Stage/reference comparisons for the complete corpus and every structural rejection family, with forward-target checks before dependent analysis | Fourteen stages and the extended 46-test detached corpus pass. Structure is checked before region analysis. |
| Typed verdict | Complete parameter, native, enum and data-layout facts, compatible control-flow behavior and explicit capacity handling | Existing typed controls and eight new rejection-family tests pass. Branch, break, floor and post-loop fallback repairs preserve the selected widening profile. |
| Call graph | Stage-owned order and cycle decisions with matching reference traversal and a changing control | Five controls cover every stage graph, graph shapes, cycles, capacities and a stage-decision intervention. |
| Module orchestration and memoization | Keleusma schedules dependent work, retains summaries and determines completion and final admission | Keleusma owns graph order, resource summaries, admission combination and yield fixpoint scheduling. Resource, membership and productivity interventions pass. |
| Text allocation | Keleusma reproduces analyze_chunk_text heap bounds and callee text propagation and includes the term in resource admission | Text lattice transitions, widening, saturation and boundary flags run in analyze.kel and feed transitive resource summaries. Differential controls pass. |
| Resource arithmetic and host costs | Reference agreement for loop bounds, saturating arithmetic, transitive calls, shared copy-out and relevant native attestations | Twelve resource tests pass, including all-stage arena boundaries, heap saturation, native policies, target widths and literal-array bounds. The extreme subtraction test also passes in debug mode. |
| Fallible complete verdict | Rejection remains distinct from capacity, protocol or stage execution failure | Both public admission entries combine structural, typed and resource checks. Incomplete work, missing summaries and malformed responses are errors. Failure-injection controls pass. |
| Shipping integration | An explicit tool path invokes the complete Keleusma verdict and preserves the reference safety gate | Four command tests pass, including both backends on fourteen stage boundaries and a concrete widening disagreement. The self-hosted result is evaluated first. |
| Verification and corpus independence | Complete current-stage comparisons, mutation controls, source inventory, feature checks and compatible stage artifacts | All five configurations and their doctests pass on unchanged validator sources. Artifact and intervention controls pass. Documentation closure is recorded in the completion audit. |

## Initial audit findings and repairs

The roadmap's historical claim that every verifier check was reproduced was too strong.
Tests first demonstrated acceptance of a scalar ninth argument where an array was required
and acceptance of a flat-field access on a known float constant. A valid 65-enum program
was then rejected because the host silently supplied only 64 layouts. Missing caller
signatures also discarded available callee and enum metadata.

Call parameter shapes now occupy packed slices in the existing 12,288-entry pool instead
of eight fixed entries per opcode. Repeated-call controls cover arities 1, 8, 9, 32 and 255.
Float constants carry known scalar sizes. Enum seeding covers 256 layouts, matching the
first-pass parser bound, and refuses overflow explicitly. Module metadata availability is
separate from an individual chunk's optional signature. Isolated chunk checking retains
its existing contract. Native-return metadata is covered by an additional control.

The graph coordinator uses explicit bounded storage in analyze.kel. It requests counts
and opcode operands and chooses traversal, cycle rejection and completion order. Rust
transports those facts and validates response indices and counts. A cycle is returned as
None. Capacity and protocol failures are errors. The first-pass bounds are 1,024 chunks
and 65,535 opcodes per chunk. The legacy resource Boolean helper still panics on such
unavailability; the complete interface must expose that distinction without a panic.

## Earlier fallible verification increment

verify_module_via_kel provides the bounded structural and typed counterpart of verify.
It reports VerificationVerdict only after completing the configured passes. Capacity,
unfinished work, execution faults and malformed transport responses are VerificationError
values. Yielded and stored verdicts must agree. Unknown completion causes are protocol errors.

Structure and forward targets are checked before dependent region walkers. Depth, yield
and typed stages report unfinished work. Operand-stack overflow cannot silently discard
values. The operator's 32-level depth bound remains unchanged. Data-layout initialization
and final count reconciliation run in Keleusma, with coroutine resumes between batches.
The typed stage now rejects consumption below a loop's entry operand floor, including a
witness that restores the height after an invalid pop. Host, WebAssembly and embedded
widths, including float-disabled modules, have valid controls.

That increment provided structural and typed verification. The resource and shipping
increment below adds complete admission and moves yield fixpoint scheduling into Keleusma.

## Completion sequence

Implementation, differential verification and documentation are complete for the
first-pass scope. The completion audit records the exact commands and the selected
typed-widening boundary.
No named Order 2 requirement is deferred into full-language expansion.

## Resource and shipping increment

The resource coordinator stores stack, heap and text summaries by chunk index and
combines arena admission in Keleusma. The host services indexed requests and transports
per-opcode facts and native attestations. External native contributions are deduplicated
and charged per chunk by the stage. Arithmetic saturates heap totals at the reference
unsigned 32-bit ceiling. The analyzer requires a 64-bit host Word to represent this
ceiling positively. Subject word widths of 8, 16, 32 and 64 bits remain supported.

Typed controls exposed and repaired false rejection after a branch replaced an existing
operand, accidental analysis of dead code after terminated branches and incompatible
break-edge heights. The selected widening profile still loses cross-join shapes. An
explicit malformed-module witness is accepted by the stage and rejected by the reference.
The shipping command reports that disagreement with exit code two. Runtime checks and
the shipping reference gate remain required.

The all-stage artifact comparison passed after these stage changes. This establishes
byte identity for fourteen stage sources and does not by itself prove validator coverage.
All five feature configurations, the detached compiler and the final-stage comparisons
pass. The completion audit records the final evidence and documentation closure.

No item may be omitted because the current positive corpus already passes. Full-language
expansion remains Order 6, but the named Order 2 requirements above are not deferred by that.
