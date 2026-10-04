# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment closes measured composite member and payload-binding gaps. Order 1 is not complete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `524a312e`.
The previous goal turn made progress by committing structural tuple and nested-array facts.
This turn extends their declaration and binding input paths. The legacy handoff remains
invalid-and-stale, twenty-four commits behind the starting tree. Its unchanged-stage claim
fails. Historical full gates were not rerun. Appendix B was read before tracked writes.
The pre-existing `nvim.log` remains untouched. No push, merge or publication occurred.
Bytecode version remains 2 with 66 public opcodes. Type caps remain 640.
The concurrent V0.3.X line remains untouched.

## Implementation

The initial twenty-source comparison found twelve missed constructor rejections. Enum tuple
payloads and struct tuple fields now emit their ordered source type records. Struct array fields
retain their element type and length. The host translates these declarations into existing
construction and agreement rows, and the Keleusma stage derives exact structural identities.
Constructor member positions remain intact when tuples occur between scalar fields or when
struct initializer fields are reordered.

Adding match-bound payload variables to the comparison exposed five more missed rejections.
The old path converted their flat layout kind to a scalar type, leaving composite values unknown.
New internal parser metadata 97 records variant index times 1024 plus payload position before
each EnumBind. One pending record preserves immediate adjacency without consuming another token.
The host pairs these coordinates with the following EnumBind frame slot, keeps guard and body
ranges separate, checks pairing counts and imports the declared member type into the scoped
binding table. Missing, reused or invalid coordinates fail extraction. They are not source
rejection verdicts. The scalar layout-kind fallback is removed.

The expanded comparison contains twenty-eight sources, with eighteen negatives and ten valid
controls. Every negative requires PipelineRejected from the shipping entry so the reference
cannot mask missing stage rejection. Eight complete artifact and execution controls cover tuple
payload construction, payload access, multiple fields, aliases, reordered struct fields, array
fields and calls receiving complete tuple or array payload values. Both compiler outputs return
seven. A declaration trace checks tuple member order. A second trace checks metadata adjacency
and coordinates through nested matches over two enum declarations, and pins the host metadata
constant against its stage function. Parser and code-generation test decoders skip metadata 97.

This introduces no new public opcode, type-stage table or capacity change. The parser adds one
private pending-record word. Existing mixed-scalar tuple runtime layout and arbitrary nested
tuple or array declaration parsing remain outside this increment's established controls.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. Test subjects remained unchanged during live runs.

- `cargo test --release --features self-host --test selfhost_composite_members --test selfhost_structural_types --test selfhost_enum_types --test selfhost_composite_types --test selfhost_type_transport --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test selfhost_parse --test selfhost_typecheck --test self_hosted_backend`
  passed 232 tests with two existing parser tests ignored. The counts are four member,
  two structural, three enum, three composite, nine transport, six artifact, three
  monomorphization, 91 parser, 103 type and eight backend tests. All twelve complete stage
  artifacts match. Terminal exit zero. Log `tmp/order1-composite-members-final-suites.log`.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 tests with sixty filtered. Terminal exit zero.
  Log `tmp/order1-composite-members-final-codegen.log`.
- `cargo test --test selfhost_typecheck --test selfhost_composite_members` passed thirty
  default type tests. The member test binary is feature-gated and executes zero tests here.
  Terminal exit zero. Log `tmp/order1-composite-members-final-default.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed
  actual crate checking and passed. Log `tmp/order1-composite-members-final-clippy.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-composite-members-final-doc.log`.
- `cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible`
  passed four refusal/artifact tests and two reproducibility tests. Terminal exit zero.
  Log `tmp/order1-composite-members-final-cli.log`.
- `cargo check --no-default-features --features compile,verify` passed.
  Log `tmp/order1-composite-members-final-nostd.log`.
- Documentation, citation and count guards passed 21 tests. Formatting and diff checks passed.
  Log `tmp/order1-composite-members-final-checks.log`.

The initial failure remains in `tmp/order1-composite-members-before.log`. The constructor
repair passed in after1, and the six initial artifact controls plus metadata trace passed in
after2 with the same prefix. The expanded binding failure is
`tmp/order1-composite-members-binders-before.log`, followed by the passing binders-after log.
The final run adds two artifact controls and the nested binding metadata trace.

The complete workspace, advisory scan, refreshed inference-independence and final integration
gates remain unverified. Production artifact serialization still calls Rust Module::to_bytes.

## Next work

Continue the full completion plan. Whole-composite projection identities, arbitrary nested
declaration syntax, array let annotations, remaining occurrence and binder coverage and general
syntax admission remain incomplete. Nominal struct signatures still need source layout facts.
The legacy struct-alias path remains one hop. General cast admission and large enum-dispatch
capacities have not been exhaustively established. Newly retained source member types provide
a basis for projection constraints, but this increment does not claim that integration.

Production type checking is independent before the reference. The reference remains the
acceptance gate while coverage is incomplete. Extraction assertions can still report malformed
or unsupported records as unavailable evidence. Continue complete Keleusma wire emission,
audit framing, opcode, pool, auxiliary and integrity provenance, refresh inference independence
for all twelve stages and run the final gates. Preserve the full Order 1 objective.

## Shadow pilot and mailbox

Task `v023-order1-composite-members` captures the first diagnostic and three bounded completion
claims under case prefix `v023-20261004-composite-members-`. Blind exports preceded self-labels.
All labels are non-independent, so no independent accuracy or development-improvement claim is
justified. Usage and elapsed time remain null where unavailable. The bounded increment is resolved
while the full Order 1 goal remains active. The reviewed export is
`tmp/order1-composite-members-pilot-export.json`, kept ignored. The later binding diagnostic
remains an engineering log under the first-diagnostic sampling convention.
External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to api.typesafe.ai because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture and development continue. The pilot implementation line owns the tracked pilot
checkpoint, which remains untouched.

The assigned inbox is empty. No messages were received or sent. The tracked checkpoint was
exported and reviewed with no communication changes. No verification process remains running.
