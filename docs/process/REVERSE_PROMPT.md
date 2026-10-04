# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment closes measured composite field and array-projection gaps. Order 1 is not complete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `82f5b642`.
The previous goal turn made progress by committing composite member and payload-binding facts.
This turn preserves type identity through struct field and composite array reads. The legacy
handoff remains invalid-and-stale, twenty-five commits behind the starting tree. Its unchanged-stage
claim fails. Historical full gates were not rerun. Appendix B was read before tracked writes.
The pre-existing `nvim.log` remains untouched. No push, merge or publication occurred.
Bytecode version remains 2 with 66 public opcodes. Type caps remain 640.
The concurrent V0.3.X line remains untouched.

## Implementation

The initial eighteen-source comparison found eleven missed rejections. Complete struct fields
and composite array elements previously lost their source type before reaching calls, aliases
and returns. The expanded field path now retains declaration coordinates independently of the
runtime byte layout. Metadata 98 carries source struct name id times 1024 plus field position.
A saved read follows that metadata without consuming another token. Field selection records
coordinates before nested postfix handling can change the active receiver type.

The driver pairs metadata with reconstructed scalar and composite field-read nodes separately
for each source range and checks their counts. It imports the declared member type into the
result binding and emits a requirement that the receiver match the stated nominal struct.
Missing or invalid metadata fails extraction rather than producing a source rejection verdict.
Parser and code-generation test decoders skip metadata 98 before reconstruction.

Type-input expression kind 18 projects an array element identity in the Keleusma stage.
Compact scalar arrays supply their scalar class. Composite arrays use a guarded lookup into
the exact structural pool. Known non-arrays reject, and unknown facts remain undecided.
Production extraction emits this row for nested array reads, retaining the separate Word index
requirement. Scalar projection layout facts remain available through their existing path.

The comparison contains eleven negatives and seven valid controls. Every negative requires
PipelineRejected from the shipping entry so reference rejection cannot mask missing stage work.
Six complete artifact and execution controls cover nested-array aliases, array-of-tuple
parameters, complete tuple and array fields passed as arguments, and nested nominal fields.
Both compiler outputs return seven. A direct metadata trace checks field-coordinate order and
adjacency through nested field and array access, and pins the host constant against the stage.
A transport test checks exact scalar, nominal and nested-array element identities, mismatching
requirements and invalid bases.

The parser adds two private state words. No type-stage array or capacity changed. Metadata 98
and type-input expression kind 18 are not public bytecode opcodes. General tuple projections,
arbitrary nested type declarations and mixed-scalar tuple runtime layout remain incomplete.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. Final verification subjects remained unchanged during
live runs. The early after2 diagnostic may overlap a formatter and is not gate evidence.

- `cargo test --release --features self-host --test selfhost_composite_projections --test selfhost_composite_members --test selfhost_structural_types --test selfhost_enum_types --test selfhost_composite_types --test selfhost_type_transport --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test selfhost_parse --test selfhost_typecheck --test self_hosted_backend`
  passed 236 tests with two existing parser tests ignored. The counts are three projection,
  four member, two structural, three enum, three composite, ten transport, six artifact,
  three monomorphization, 91 parser, 103 type and eight backend tests. All twelve complete
  stage artifacts match. Terminal exit zero. Log `tmp/order1-composite-projections-final-suites.log`.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 tests with sixty filtered. Terminal exit zero.
  Log `tmp/order1-composite-projections-final-codegen.log`.
- `cargo test --test selfhost_typecheck --test selfhost_composite_projections` passed thirty
  default type tests. The projection binary is feature-gated and executes zero tests here.
  Terminal exit zero. Log `tmp/order1-composite-projections-final-default.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed
  actual crate checking and passed. Log `tmp/order1-composite-projections-final-clippy.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-composite-projections-final-doc.log`.
- `cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible`
  passed four refusal/artifact tests and two reproducibility tests. Terminal exit zero.
  Log `tmp/order1-composite-projections-final-cli.log`.
- `cargo check --no-default-features --features compile,verify` passed.
  Log `tmp/order1-composite-projections-final-nostd.log`.
- Documentation, citation and count guards passed 21 tests. Formatting and diff checks passed.
  Log `tmp/order1-composite-projections-final-checks.log`.

The initial failure remains in `tmp/order1-composite-projections-before.log`. The first repair
passed in after1 with the same prefix. The expanded artifact and transport diagnostic is after2.
The final broad run supersedes that diagnostic on an unchanged source tree.

The complete workspace, advisory scan, refreshed inference-independence and final integration
gates remain unverified. Production artifact serialization still calls Rust Module::to_bytes.

## Next work

Continue the full completion plan. General tuple projection identity, arbitrary nested
declaration syntax, array let annotations, remaining occurrence and binder coverage and general
syntax admission remain incomplete. Nominal struct signatures still need source layout facts.
The legacy struct-alias path remains one hop. General cast admission and large enum-dispatch
capacities have not been exhaustively established. Do not infer complete projection coverage
from the tested struct-field and array-element forms.

Production type checking is independent before the reference. The reference remains the
acceptance gate while coverage is incomplete. Extraction assertions can still report malformed
or unsupported records as unavailable evidence. Continue complete Keleusma wire emission,
audit framing, opcode, pool, auxiliary and integrity provenance, refresh inference independence
for all twelve stages and run the final gates. Preserve the full Order 1 objective.

## Shadow pilot and mailbox

Task `v023-order1-composite-projections` captures the first diagnostic and three bounded claims
under case prefix `v023-20261004-composite-projections-`. Blind exports preceded self-labels.
All labels are non-independent, so no independent accuracy or development-improvement claim is
justified. Usage and elapsed time remain null where unavailable. The bounded increment is resolved
while the full Order 1 goal remains active. The reviewed export is
`tmp/order1-composite-projections-pilot-export.json`, kept ignored.
External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to api.typesafe.ai because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture and development continue. The pilot implementation line owns the tracked pilot
checkpoint, which remains untouched.

The assigned inbox is empty. No messages were received or sent. The tracked checkpoint was
exported and reviewed with no communication changes. No verification process remains running.
