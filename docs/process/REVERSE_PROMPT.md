# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment extends production composite facts. Order 1 is not complete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `0501fe2f`.
The previous goal turn made progress through production type integration and its commit.
This turn makes implementation and verification progress. The legacy handoff remains
invalid-and-stale, twenty-one commits behind the starting tree. Its unchanged-stage claim
fails. The fingerprint remains 0x4327_63E1. Historical full gates were not rerun.
Appendix B was read before tracked writes. The pre-existing `nvim.log` remains untouched.
No push, merge or publication occurred. Bytecode version remains 2 with 66 public opcodes.
Selected type caps remain 640. The concurrent V0.3.X line remains untouched.

## Implementation

The initial sixteen-source comparison found twelve missed rejections and one false rejection.
Struct constructor metadata now packs its source name id above the declared field count, using
1024 as the radix. This metadata is consumed before reconstruction. Scoped constructor identities
follow postorder and retain their guard/body range. Field requirements use source declaration
order, matching reconstruction's placement of reordered initializer fields.

Nominal struct and enum tags use separate low four-bit classes, eight and nine. Scalar-array
identities use the element tag in the low bits and length plus one above them. This prevents
same-size structs and arrays from becoming the same type. Array declarations retain their outer
shape, and the stage derives literal-array tags through expression kind 16 after resolving
scalar elements. Lengths through 134217726 fit a 32-bit Word. Non-scalar elements and larger
lengths remain unknown in this channel. Enum-to-Word casts have a stage rule so retaining an
enum's nominal identity does not falsely reject a valid cast.

Function headers reserve a type slot for every parameter before reading its type records.
Tuple headers that emit no type record no longer shift subsequent scalar parameter tags.
Array lengths belong to their parameter or return declaration. Scalar-array chunk parameter
tags and typed-verifier signatures now describe the whole array rather than its scalar element.

Struct-returning calls and struct literals now arm the existing field postfix. This repairs the
initial false unknown-name rejection after a direct call. The broader scalar-access corpus then
found an entire array field being emitted as a scalar read. Whole composite fields now emit the
existing nested-field operation with their container kind. This changes no public opcode.

The sixteen-source corpus now agrees with the reference, with every negative also requiring
PipelineRejected from the shipping entry. Fourteen further valid controls cover aliases, nested
and reordered fields, enum casts, Byte and Boolean arrays, computed elements and conditional
arrays. Three complete artifact/execution controls exercise array signatures, entire array fields
and direct struct-literal access. Both compiler outputs return seven. Transport tests distinguish
element type and length through the 32-bit encoding boundary.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. No test subject was edited during a live run.

- `cargo test --release --features self-host --test selfhost_typecheck --test selfhost_composite_types --test selfhost_type_transport --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test selfhost_parse --test self_hosted_backend`
  passed 103 type tests, three composite tests, five transport tests, six artifact tests,
  three monomorphization tests, 91 parser tests with two ignored and eight backend tests.
  Total executed 219. Terminal exit zero. All twelve complete stage artifacts match.
  Log `tmp/order1-composite-types-final-suites.log`.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests. Log `tmp/order1-composite-types-codegen.log`.
- `cargo test --test selfhost_typecheck --test selfhost_type_transport --test selfhost_composite_types`
  passed thirty default type tests. The feature-gated binaries execute zero tests in this
  configuration. Log `tmp/order1-composite-types-default.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed
  actual crate checking and passed. Log `tmp/order1-composite-types-clippy.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-composite-types-doc.log`.

- `cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible`
  passed four command refusal/artifact tests and two reproducibility tests.
  Log `tmp/order1-composite-types-cli.log`.
- `cargo check --no-default-features --features compile,verify` passed.
  Log `tmp/order1-composite-types-nostd.log`.
- Documentation, citation and count guards, formatting and diff checks are recorded in
  `tmp/order1-composite-types-final-checks.log`.

The initial diagnostic remains in `tmp/order1-composite-types-before.log`. A second diagnostic
confirmed parser code -913 for the valid struct call. The first implementation closed eight of
the thirteen disagreements and the next closed the five scalar-array cases. Logs are
`tmp/order1-composite-types-after1.log` and `tmp/order1-composite-types-after2.log`.
The first expanded controls included an empty-array program the reference rejected, so it was
removed from the positive population. Its log is `tmp/order1-composite-types-controls1.log`.
The first expanded suite exposed the enum-to-Word false rejection. The second exposed the entire
array-field representation problem. Logs are `tmp/order1-composite-types-suites1.log` and
`tmp/order1-composite-types-suites2.log`. The isolated field diagnostic and its passing rerun are
`tmp/order1-composite-types-access-diagnostic.log` and `tmp/order1-composite-types-access-after.log`.
The broad run omitted the failure detail, so the failing test was rerun alone to retain its
diagnostic. A concurrent test's temporary process-wide panic-hook replacement is a plausible cause,
not directly verified. The existing global panic-hook mutation remains a concurrency concern. The new type transport does not alter hooks.

The complete workspace, advisory scan, refreshed inference-independence and final integration
gates remain unverified. Production artifact serialization still calls Rust Module::to_bytes.

## Next work

The continuation reviewed the staged verification logs and pilot records before finalizing
this increment. The preceding status-only turn made no implementation progress.
Source inspection identifies an enum transport gap for the next implementation. EnumInit
retains byte size and children but no nominal identity. The enum declaration payload scanner
updates parser layout tables without emitting field type records. Constructor metadata and
payload declaration facts therefore need transport before independent payload checking.
This is source inspection evidence. No new enum differential corpus was executed in the
continuation, and no enum constructor repair is claimed here.

Continue the full completion plan. Complete tuple and nested-array facts, nominal enum constructor
and payload facts, array let annotations, composite projection aliases, complete occurrence/binder
coverage and general syntax admission remain incomplete. Whole composite field records now retain
correct runtime representation but do not yet carry full source type identity into type operands.
Nominal struct and tuple signatures still need source layout facts. Scalar-array signatures are
now covered, including three artifact and runtime controls. The scalar solver retains bounded
rounds and a separate one-hop struct-alias path.

Production type checking is independent before the reference, but the reference remains the
acceptance gate while coverage is incomplete. Existing extraction helpers can still panic and
are reported as unavailable evidence rather than source rejection. Continue complete Keleusma
wire emission, audit framing/opcode/pool/auxiliary/integrity provenance, refresh inference
independence for all twelve stages and run the final gates. Preserve the full Order 1 objective.

## Shadow pilot and mailbox

Task `v023-order1-composite-types` captures the first diagnostic and bounded completion claims
with case prefix `v023-20261004-composite-types-`. All labels are non-independent self-reviews.
No independent accuracy or development-improvement claim is justified. Later diagnostics remain
in engineering logs under the first-diagnostic sampling convention. Usage and elapsed time remain
null when unavailable. The bounded increment is resolved while full Order 1 remains active.
The reviewed export is `tmp/order1-composite-types-pilot-export.json`, kept ignored. The pilot
implementation line owns the tracked pilot checkpoint, which remains untouched.

External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to api.typesafe.ai because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture, review and development remain available.

The assigned interline inbox is empty. No messages were received or sent. The tracked checkpoint
was exported and reviewed with no communication changes. Check again before dependent work.
No verification process remains running at handoff.
