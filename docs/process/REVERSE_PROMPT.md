# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment adds structural tuple and nested-array facts. Order 1 is not complete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `6dffc924`.
The preceding status turn found a resource-accounting failure in the completed verification
log, changing the next action. This turn repairs that failure and a newly measured projection
classification failure. The legacy handoff is invalid-and-stale, twenty-three commits behind
the starting tree. Its unchanged-stage claim fails. Its historical full gates were not rerun.
Appendix B was read before tracked writes. The pre-existing `nvim.log` remains untouched.
No push, merge or publication occurred. Bytecode version remains 2 with 66 public opcodes.
Selected type caps remain 640. The concurrent V0.3.X line remains untouched.

## Implementation and evidence

The initial fifteen-source structural comparison found eleven invalid programs accepted by the
independent pipeline. The expanded comparison contains nineteen sources, with thirteen negatives
and six valid controls. Every negative also requires independent PipelineRejected from the
shipping entry. Six complete artifact and execution controls cover tuple, nested-array and
array-of-tuple parameters and returns, with both compiler outputs returning seven.

Parser metadata records 94, 95 and 96 retain tuple opening, named members and closing.
Opening distinguishes parameter and return headers. Array sizes remain postfix records,
including both dimensions of the existing nested-array header form. The driver preserves
these records for type constraints, signatures and chunk parameter metadata. These are
internal metadata kinds, not public opcodes.

The driver translates source types and tuple literals into construction rows. Expression
kind 17 forms tuple cons cells and kind 16 now also forms composite-element arrays. The
Keleusma stage interns exact structural keys, preserving member order, array dimensions and
nominal element identities independently of byte size. A private pool holds at most 640
entries. Exhaustion returns unavailable evidence rather than a source rejection. The bounded
solver permits 640 rounds, with productive rounds resolving previously unknown bindings.
Direct transport controls cover exact identity, reverse dependencies and 639, 640 and 641 rows.

Array-of-tuple literals now retain their tuple element byte width. Return-header scanning
uses its own array-tuple layout index and does not write a parameter table for a return type.
This avoids indexing a nonexistent parameter on zero-parameter functions. Signature assembly
uses checked byte-size arithmetic for structural types. The six artifact controls use Word
members. General mixed-scalar and nested tuple layout remains unestablished.

The first broad run failed because the accounting table priced 42 arrays while the stage
held 45. The new pool adds 1,920 words, fifteen KiB on an eight-byte word, beyond the earlier
56 KiB selected-cap increase. The guards now share one price table and compare capacity
multisets as well as counts. The reported input-corpus lower bound does not measure private
pool occupancy. Projection rules also still treated every known tag as scalar. A focused
negative control exposed that failure, and the rules now recognize the nominal and structural
composite classes while retaining scalar rejection.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. Final verification subjects remained unchanged while
runs were live. The early after1 diagnostic may overlap a formatter and is not gate evidence.

- `cargo test --release --features self-host --test selfhost_structural_types --test selfhost_enum_types --test selfhost_composite_types --test selfhost_type_transport --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test selfhost_parse --test selfhost_typecheck --test self_hosted_backend`
  passed 228 tests with two existing parser tests ignored. The component counts are two
  structural, three enum, three composite, nine transport, six artifact, three monomorphization,
  91 parser, 103 type and eight backend tests. All twelve complete stage artifacts match.
  Terminal exit zero. Log `tmp/order1-structural-types-final-suites.log`.
- The focused structural, transport and type suites passed 2, 9 and 103 tests, respectively.
  Log `tmp/order1-structural-types-final-focused.log`. A later edit only clarifies the
  resource test's comment and output label, so the final broad run supersedes this evidence.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests with sixty filtered. Terminal exit zero.
  Log `tmp/order1-structural-types-final-codegen.log`.
- `cargo test --test selfhost_typecheck --test selfhost_structural_types` passed thirty
  default type tests. The structural binary is feature-gated and executes zero tests here.
  Terminal exit zero. Log `tmp/order1-structural-types-final-default.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed
  actual crate checking and passed. Log `tmp/order1-structural-types-final-clippy.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-structural-types-final-doc.log`.
- `cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible`
  passed four refusal/artifact tests and two reproducibility tests. Terminal exit zero.
  Log `tmp/order1-structural-types-final-cli.log`.
- `cargo check --no-default-features --features compile,verify` passed.
  Log `tmp/order1-structural-types-final-nostd.log`.
- Documentation, citation and count guards passed 21 tests. Final formatting and diff checks
  passed. Log `tmp/order1-structural-types-final-checks.log`.

Retained diagnostics include `tmp/order1-structural-types-before.log`, after1, after2, after3
and suites1 with the same prefix. The initial artifact controls found an array-of-tuple byte
width mismatch in after2. The first broad run in suites1 failed only its array-accounting test
within the type binary. The projection failure is `tmp/order1-structural-projection-before.log`.
These failures remain evidence rather than being overwritten by passing logs.

The complete workspace, advisory scan, refreshed inference-independence and final integration
gates remain unverified. Production artifact serialization still calls Rust Module::to_bytes.

## Next work

Continue the full completion plan. Tuple and other composite payload requirements, arbitrary
nested declaration syntax, array let annotations, composite projection aliases, complete
occurrence and binder coverage and general syntax admission remain incomplete. Whole composite
field records do not yet carry full source type identity into type operands. Nominal struct
signatures still need source layout facts. The legacy struct-alias path remains one hop.
General cast admission and large enum-dispatch capacities are not exhaustively established.

Production type checking is independent before the reference. The reference remains the
acceptance gate while coverage is incomplete. Extraction assertions can still report malformed
or unsupported records as unavailable evidence. Continue complete Keleusma wire emission,
audit framing, opcode, pool, auxiliary and integrity provenance, refresh inference independence
for all twelve stages and run the final gates. Preserve the full Order 1 objective.

## Shadow pilot and mailbox

Task `v023-order1-structural-types` captures the first diagnostic and three completion claims
under case prefix `v023-20261004-structural-types-`. Blind exports preceded non-independent
self-labels. No independent accuracy or development-improvement claim is justified. Later
artifact, accounting and projection diagnostics remain engineering logs under the first-diagnostic
sampling convention. Usage and elapsed time are null where unavailable. The bounded increment
is resolved while the full Order 1 goal remains active. The reviewed pilot export is
`tmp/order1-structural-types-pilot-export.json`, kept ignored. External evaluation
remains pending the existing specific operator authorization request. Automatic approval review
rejected sending excerpts to api.typesafe.ai because it did not recognize destination-and-payload
authorization. Do not retry or bypass that rejection. Local capture and development continue.
The pilot implementation line owns the tracked pilot checkpoint, which remains untouched.

The assigned inbox is empty. No messages were received or sent. The tracked checkpoint was
exported and reviewed with no communication changes. No verification process remains running.
