# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment closes measured enum construction and cast gaps. Order 1 is not complete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `1c6247fc`.
The previous goal turn made progress by committing the nominal and scalar-array increment.
This turn makes implementation and verification progress. The legacy handoff remains
invalid-and-stale, twenty-two commits behind the starting tree. Its unchanged-stage claim
fails. The fingerprint remains 0x4327_63E1. Historical full gates were not rerun.
Appendix B was read before tracked writes. The pre-existing `nvim.log` remains untouched.
No push, merge or publication occurred. Bytecode version remains 2 with 66 public opcodes.
Selected type caps remain 640. The concurrent V0.3.X line remains untouched.

## Implementation

The initial nineteen-source enum comparison found fourteen invalid programs accepted by the
independent pipeline. The negatives include nominal identity, payload types and arity, unknown
variants, constructor returns and aliases. All nineteen now agree with the reference, and every
negative must produce PipelineRejected from the shipping entry.

Parser metadata tag 93 carries the declaration-order variant index immediately before EnumInit.
A parallel construction stack preserves that identity through nested payload parsing. The driver
consumes metadata before reconstruction and pairs its guard/body postorder with enum nodes.
Both metadata and node counts are checked. Enum declaration records now carry scalar and named
payload types through PTYPE and scalar-array lengths through ASIZE. Tuple payloads reserve one
unknown field, preserving subsequent field positions. Extraction emits existing arity and
agreement constraints. Nominal identity is never inferred from byte size.

Unknown constructors use the existing enum-variant source diagnostic. Empty constructors requiring
payloads use diagnostic 26, including the direct enum-to-Word folding path. This is classified as
source rejection rather than unavailable evidence. Other payload arity is checked by the type
stage. The host parser decoders in the parser and code-generation tests now skip metadata tag 93.

The artifact controls exposed an enum value cast being emitted as ByteToWord. The reference
instead evaluates the value once and emits a variant-dispatch loop. Parser record 74 now builds
this dispatch through the existing enum-arm protocol. Reconstruction retains it in the existing
Cast node, with the dispatch table base plus two in its argument and the variant count in rhs.
Code generation shares the enum-match emitter with the cast-specific EnumVariantUnmapped trap.
Neither record 74 nor metadata tag 93 is a public opcode.

Expanded controls exposed missing enum facts on parser aliases. Parameter and local enum facts
now propagate into subsequent bindings. Scalar literals and Byte casts clear stale enum facts,
and calls retain their declared enum return fact. Eleven complete artifact and execution controls
cover enum parameters, scalar/struct/array payload access, explicit discriminants, aliases,
returned values, payload constructor casts and a scalar cast following an enum value. Both
compiler outputs return seven in every control. A metadata control checks variant order across
two declarations and adjacency before each constructor.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. No test subject was edited during a live run.

- `cargo test --release --features self-host --test selfhost_enum_types --test selfhost_composite_types --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test selfhost_parse --test selfhost_typecheck --test selfhost_type_transport --test self_hosted_backend`
  passed three enum tests, three composite tests, six artifact tests, three monomorphization
  tests, 91 parser tests with two existing ignored, 103 type tests, five transport tests and
  eight backend tests. Total executed 222. Terminal exit zero. All twelve complete stage
  artifacts match. Log `tmp/order1-enum-types-final-suites.log`.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests with sixty filtered. Log `tmp/order1-enum-types-final-codegen.log`.
- `cargo test --test selfhost_typecheck --test selfhost_enum_types`
  passed thirty default type tests. The enum binary is feature-gated and executes zero tests
  in this configuration. Log `tmp/order1-enum-types-final-default.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed
  actual crate checking and passed. Log `tmp/order1-enum-types-final-clippy.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-enum-types-final-doc.log`.
- `cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible`
  passed four refusal/artifact tests and two reproducibility tests.
  Log `tmp/order1-enum-types-cli.log`.
- `cargo check --no-default-features --features compile,verify` passed.
  Log `tmp/order1-enum-types-nostd.log`.
- Documentation, citation and count guards, formatting and diff checks are recorded in
  `tmp/order1-enum-types-final-checks.log`.

The first diagnostic remains in `tmp/order1-enum-types-before.log`. The first type repair passed
in `tmp/order1-enum-types-after1.log`. The expanded suite stopped at the enum-cast artifact
failure in `tmp/order1-enum-types-suites1.log`. The isolated source diagnostic is
`tmp/order1-enum-types-artifact-diagnostic.log`. The dispatch repair passed the first five
artifact controls in `tmp/order1-enum-types-after2.log`. Expanded alias controls failed in
`tmp/order1-enum-types-controls.log`, with an isolated reproduction in
`tmp/order1-enum-types-controls-diagnostic.log`. The alias repair and all eleven artifact controls
passed in `tmp/order1-enum-types-after3.log`, before the final broad suite above.

The complete workspace, advisory scan, refreshed inference-independence and final integration
gates remain unverified. Production artifact serialization still calls Rust Module::to_bytes.

## Next work

Continue the full completion plan. Complete tuple and nested-array facts, tuple and other composite
payload requirements, array let annotations, composite projection aliases, complete occurrence/binder
coverage and general syntax admission remain incomplete. Whole composite field records retain
correct runtime representation but do not yet carry full source type identity into type operands.
Nominal struct and tuple signatures still need source layout facts. Scalar-array signatures are
covered. The scalar solver retains bounded rounds and a separate one-hop struct-alias path.
General cast admission and large enum-dispatch capacities have not been exhaustively established.

Production type checking is independent before the reference, but the reference remains the
acceptance gate while coverage is incomplete. Existing extraction helpers can still panic and
are reported as unavailable evidence rather than source rejection. Continue complete Keleusma
wire emission, audit framing/opcode/pool/auxiliary/integrity provenance, refresh inference
independence for all twelve stages and run the final gates. Preserve the full Order 1 objective.

## Shadow pilot and mailbox

Task `v023-order1-enum-types` captures the first diagnostic and three bounded completion claims
with case prefix `v023-20261004-enum-types-`. All labels are non-independent self-reviews.
No independent accuracy or development-improvement claim is justified. Later cast and alias
diagnostics remain in engineering logs under the first-diagnostic sampling convention. Usage
and elapsed time remain null when unavailable. The bounded increment is resolved while full
Order 1 remains active. The reviewed export is `tmp/order1-enum-types-pilot-export.json`, kept
ignored. The pilot implementation line owns the tracked pilot checkpoint, which remains untouched.

External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to api.typesafe.ai because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture, review and development remain available.

The assigned interline inbox is empty. No messages were received or sent. The tracked checkpoint
was exported and reviewed with no communication changes. Check again before dependent work.
No verification process remains running at handoff.
