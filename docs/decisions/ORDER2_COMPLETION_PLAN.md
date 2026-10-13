# Order 2 completion plan

## Objective

Complete the full validator in Keleusma on the V0.2.X line. Preserve the roadmap's
structural, typed and resource verdict requirements, the explicit text-allocation
term, module orchestration and shipping-tool integration. Order 1 remains complete.
Order 2 is active and incomplete.

## Requirements and current evidence

| Requirement | Evidence needed | Current state |
|---|---|---|
| Structural verdict | Stage/reference comparisons for the complete corpus and every structural rejection family, with forward-target checks before dependent analysis | The fallible verifier accepts all fourteen current stages. Structure and forward targets are checked before region walks. Target, depth, productivity, layout and loop-floor negatives are covered. Broader malformed-module coverage remains under audit. |
| Typed verdict | Complete parameter, native, enum and data-layout facts, compatible control-flow behavior and explicit capacity handling | Initial repairs close ninth-argument and float-constant false acceptance, silent enum-table truncation and missing-signature suppression of module metadata. The loop-entry operand floor is now checked. Join, loop-carried state and broader capacity behavior remain under audit. |
| Call graph | Stage-owned order and cycle decisions with matching reference traversal and a changing control | Implemented in analyze.kel. Five controls cover all fourteen stage graphs, graph shapes, cycles, exact bounds and an intervention that disables only stage cycle rejection. |
| Module orchestration and memoization | Keleusma schedules dependent work, retains summaries and determines completion and final admission | Graph ordering is now stage-owned. Resource summary storage and admission aggregation remain in Rust. The always-yielding fixpoint still has Rust scheduling and set updates. |
| Text allocation | Keleusma reproduces analyze_chunk_text heap bounds and callee text propagation and includes the term in resource admission | Existing text_flow transports Boolean boundary facts only. The resource validator does not include text heap allocation. Required work remains. |
| Resource arithmetic and host costs | Reference agreement for loop bounds, saturating arithmetic, transitive calls, shared copy-out and relevant native attestations | Existing per-chunk and detached tests pass. Full arithmetic and attestation coverage is not yet established. |
| Fallible complete verdict | Rejection remains distinct from capacity, protocol or stage execution failure | verify_module_via_kel now distinguishes completed structural/typed verdicts from capacity, unfinished work, execution and protocol errors. Resource admission is not included yet. Legacy Boolean helpers remain for compatibility. |
| Shipping integration | An explicit tool path invokes the complete Keleusma verdict and preserves the reference safety gate | No validator selection is implemented in the shipping command. Required work remains. |
| Verification and corpus independence | Complete current-stage comparisons, mutation controls, source inventory, feature checks and compatible stage artifacts | Targeted checks and artifact comparisons are recorded in REVERSE_PROMPT.md. Full Order 2 verification remains a final gate. |

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

## Fallible verification increment

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

This is not yet complete resource admission. Do not promote its Accepted result to that
claim. The module driver still sequences passes and schedules the yield fixpoint in Rust.

## Next implementation order

1. Complete fallible resource admission on top of the structural/typed verification
   boundary. Extend the rejection-family census without conflating unavailable analysis
   with a proven invalid module.
2. Move resource summary ownership, admission combination and the always-yielding
   fixpoint orchestration into Keleusma. Retain host allocation and transport only.
3. Port the text-allocation lattice and fold its bounds into transitive resource results.
   Audit arithmetic saturation, data-copy costs and native attestations against the reference.
4. Close remaining typed and data-layout discrepancies. Account explicitly for the
   previously chosen control-flow over-approximation; do not describe it as equality
   on arbitrary malformed modules without evidence.
5. Integrate the complete verdict into the shipping tool and run the full completion audit.

No item may be omitted because the current positive corpus already passes. Full-language
expansion remains Order 6, but the named Order 2 requirements above are not deferred by that.
