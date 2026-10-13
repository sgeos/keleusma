# Reverse prompt

## Current state

Continue the full Order 2 goal on line v0.2.3. The goal remains active and incomplete.
The authoritative requirement register is
[ORDER2_COMPLETION_PLAN.md](../decisions/ORDER2_COMPLETION_PLAN.md).
Do not infer completion from positive acceptance of the stage corpus or the older
roadmap statement that the verifier was already complete.

This increment started at 99b7ab01 and created feat/v02-order2-validator. Order 1 remains
complete. The legacy HANDOFF.md is invalid-and-stale, with its stamp 53 commits behind
the starting head, fourteen stage sources instead of twelve and changed stage files.
Appendix B was read before tracked writes. The V0.3.X worktree and pre-existing .codex/
and nvim.log remain untouched. No peer messages, push, merge or publication occurred.

## Implemented in this increment

analyze.kel now owns depth-first call-graph traversal in declaration and opcode order.
Commands 10 and 11 use shared count and opcode facts. Private color and frame arrays
hold the traversal. The stage requests chunk headers and opcode operands and emits
completion indices. It detects cycles. Rust transports facts, validates response
indices and stores the emitted order. It does not compute traversal order or cycle verdicts.

The public call_order_via_kel function returns Some(order), None for a cycle, or an
explicit error for capacity and protocol failure. Its first-pass limits are 1,024 chunks
and 65,535 opcodes per chunk. Both exact limits and the next values are tested. A control
changes only the stage's cycle-rejection assignment and makes the same cyclic input
accepted while preserving an acyclic control. All fourteen stage graphs match the
reference order. The resource validator and compiler text-boundary scheduling use this
path. The legacy resource Boolean helper still panics on stage unavailability.

The initial typed-verdict audit found several concrete discrepancies.

- The ninth call argument was not checked because only eight parameter shapes were seeded.
  Shapes now occupy per-call slices in the existing 12,288-entry pool. Controls cover
  repeated calls at arities 1, 8, 9, 32 and 255, with a wrong final argument rejected.
- Float constants were transported as unknown, hiding invalid flat-field access. They
  now carry scalar sizes when the float feature is enabled.
- A valid program with 65 enum layouts was rejected because only the first 64 were seeded.
  The table now covers the first-pass 256-layout bound, with all declarations transported.
  The next layout count is an explicit capacity panic in the legacy Boolean interface.
- Absence of a caller signature discarded available callee and enum metadata. Private
  TypedSeeding distinguishes isolation from module context with an optional local signature.
  Module checking retains callee, native-return and enum facts without a caller signature.
  The public seed helper preserves its existing None-means-isolation contract.

Seven tests in tests/selfhost_validator.rs cover these defects, valid controls and bounds.
Malformed modules are inspected and never executed. The positive validator census accepts
all fourteen stage modules in agreement with the reference. This is positive coverage only.

Public bytecode version remains 2, the public opcode count remains 66 and selected type
capacities remain 640. No public compiler signature changed. The analyzer now has 22 chunks
and a 20,484-byte artifact. The typed stage still has 22 chunks and a 14,572-byte artifact.
All fourteen current complete stage artifacts match the reference.

## Remaining work and next action

Start with the complete fallible module-admission interface and a rejection-family census.
The existing Boolean wrappers are not a suitable final public verdict because capacity,
protocol and execution failures can panic or default-deny. Keep those distinct from a
proven invalid module. Validate forward control-flow targets before dependent walkers.

Then move the remaining resource summary table, admission aggregation and always-yielding
fixpoint scheduling into Keleusma. The graph order is ported, but validate_module_via_kel
still retains chunk_wcmu and combines flags in Rust. self_hosted_always_yielding still
runs fixpoint rounds and updates its set in Rust. Its documentation now states this.

Port the text-allocation term and fold it into resource admission. Existing text_flow.rs
and commands 1 through 4 in analyze.kel track Boolean boundary facts only. The reference
text_size.rs tracks NotText, Known lengths and Unbounded, control-flow widening, heap
saturation and callee text-return facts. compute_chunk_wcmu adds that heap term. The
Keleusma resource driver does not yet do so. Audit resource arithmetic saturation,
shared copy-out and native attestation coverage as part of the complete verdict.

Continue the typed and structural audit. The reference has a loop-floor check and
precise joins; the stage uses a previously approved over-approximation. Account for that
scope explicitly and do not claim exact equality on arbitrary malformed modules.
Existing op, local, stack, region, shape-pool and enum limits also need explicit fallible
handling. The module typed helper checks chunk shapes; the structural wrapper separately
adds data-layout checks. Preserve that distinction when designing the complete interface.

Finally integrate the complete verdict into the shipping tool. No validator selection
exists there yet. The completion plan keeps every named roadmap requirement open until
its own evidence is present. No operator decision currently blocks independent progress.

## Verification receipts

All handles below are terminal. Subjects remained fixed during their relevant runs.
Cargo used an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one.
Self-host execution used KEL_SELFHOST_CACHE zero.

| Check | Command and receipt |
|---|---|
| Graph controls | cargo test --release --features self-host --lib selfhost::call_graph -- --nocapture. tmp/order2-final-graph.log, session 40808 exit zero. Five tests pass, including all fourteen graphs and the stage-decision intervention. |
| Artifact, boundary and validator regressions | cargo test --release --features self-host --test selfhost_artifact_identity --test selfhost_text_boundary --test selfhost_validator -- --nocapture. tmp/order2-final-regressions.log, session 37510 exit zero. Six artifact tests, three text-boundary tests and seven validator tests pass. All fourteen complete artifacts compare and decode. |
| Detached validator suites | cargo test --release --manifest-path compiler/Cargo.toml --test validator --test verify_typed --test verify_structural. tmp/order2-final-detached.log, session 61956 exit zero. Five resource, 46 structural and 12 typed tests pass. |
| Clippy | cargo clippy --workspace --all-targets -- -D warnings. tmp/order2-handoff-clippy.log, session 57373 exit zero with actual Checking after the final source-comment clarification. |
| Minimum Rust | cargo +1.88.0 check -p keleusma --tests --features self-host. tmp/order2-msrv.log, session 87159 exit zero. |
| Feature gating | cargo test --release --no-default-features --test selfhost_validator. tmp/order2-feature-gating.log, session 93574 exit zero. Zero tests is expected because this file requires self-host. This establishes gating only. |
| Positive validator census | tmp/order1-type-probe/src/bin/validator_census.rs. tmp/order2-validator-census.log, session 35349 exit zero. All fourteen stages are accepted by both validators. |
| Documentation guards | Five documentation guard targets pass 29 tests in tmp/order2-documentation-guards.log, session 74638 exit zero. Final handoff changes are checked again before commit. |

The first graph build failed because the no_std test module did not import ToString.
The assertion now matches the error variant directly. The first negative-verdict tests
failed as expected, proving the two missed rejections. The enum-prefix control then
proved a false rejection, and the two missing-signature controls proved further false
acceptance. Those logs remain under tmp/order2-*-first.log. The expected capacity panic
in the 257-layout control is caught by its test and is not an unexpected test failure.

This increment used targeted verification. The full workspace suite, all five complete
feature suites, a new actual CLI corpus, memory-interpreter runs, books and dependency
advisory checks were not repeated. Order 1's earlier complete receipts remain in its audit.
The full Order 2 completion gate remains outstanding. Source inventory is 1,648 integration
test functions across 166 files. Five library graph tests were added; no new full-library
execution count is claimed.

## Shadow pilot and interline

Task v023-order2-graph-99b7 contains one initial diagnostic and two bounded claims.
Triage is labelled compiler_or_test. The cycle-decision and ninth-argument claims are
labelled supports. All labels are non-independent self-review. The initial increment's
outcome is resolved, while the full Order 2 goal remains active. Usage and elapsed values
remain null. External evaluation remains unavailable under the prior approval rejection;
no provider judgment, independent accuracy or saved reasoning claim is made.

The reviewed export is tmp/jev-shadow-order2-graph-99b7-final.json. It contains 185 samples
across 47 tasks. All three current request digests and immutable source hashes match.
No third completion claim was needed. The peer-owned tracked pilot checkpoint remains
untouched. The assigned inbox is empty. The tracked interline checkpoint is exported and
reviewed before handoff. No verification process from this increment remains running.
