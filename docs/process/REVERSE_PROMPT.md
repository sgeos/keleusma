# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment closes measured composite let annotation gaps. Order 1 is not complete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `35fa61fc`.
The previous status-only turn made no implementation progress. This turn resumed the existing
annotation diagnostic and implements its repair. The legacy handoff is invalid-and-stale,
twenty-six commits behind the starting tree. Its ancestry check passes but its unchanged-stage
claim fails. Historical full gates were not rerun. Appendix B was read before tracked writes.
The pre-existing `nvim.log` remains untouched. No push, merge or publication occurred.
Bytecode version remains 2 with 66 public opcodes. Type caps remain 640.
The concurrent V0.3.X line remains untouched.

## Implementation

Source annotation records now preserve named types, tuples, arrays with literal dimensions,
grouping and unit. Metadata 99 opens an annotation keyed by its colon token position, 100 carries
a full-width postfix array dimension, 101 closes it and 102 references it before the folded let.
Existing records 94 through 96 carry tuple structure and names. Record 103 closes grouping or
unit. Named annotation record 92 retains its original adjacency to the binding name and let.

The host checks metadata pairing and source range, stores records under deterministic ordered
keys and supplies structural construction and agreement rows to the Keleusma type stage. It
does not infer annotation types from byte layout or invoke the reference parser for these facts.
The iterative scanner checks its 32-level bound before array writes. Unsupported syntax and
capacity diagnostics remain unavailable evidence, not source rejection. Constant-expression
array dimensions remain explicitly unsupported.

The initial seventeen-source comparison found eleven missed rejections. The expanded comparison
has twenty-three sources, fourteen negatives and nine valid controls. Each negative must also
produce PipelineRejected from the shipping compiler. Seven complete artifact and execution
controls cover unit, arrays, nested arrays, tuples, arrays of tuples and nominal values, and
branch-local shadowing. Both compiler outputs return seven.

Valid controls exposed missing unit literal emission and let-bound tuple-array element layout.
Existing executable nodes now carry both. Written unit has internal node payload 3, distinct
from synthesized payload 0, so a written unit tail retains its return requirement. Code generation
maps both forms to the existing unit immediate. Two deliberate historical refusal pins were
replaced with positive artifact coverage and a negative Word-returning unit-tail control.
The construct-support table now includes unit.

Literal tuple layout allocation now checks layout count, member count and total field storage
before writing. Its existing all-Word representation remains a limitation. Annotation storage and
tuple-array binding facts add 164 private words, or 1,312 bytes at eight bytes per word. The
selected type-stage caps do not change. Metadata and node payload changes are internal protocol
changes, not public bytecode opcodes.

Running the full code-generation file exposed seven failures outside the earlier 89-test filter.
The Rust test reconstruction mirror now handles ordered wildcard patterns and guarded results.
Its self-compilation pin now requires all 78 current functions. The live boundary-count guard
reads the roadmap instead of the historical handoff. The executable table reports 102 cases.
An initial source-text count missed two commented entries and was discarded.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. Verification subjects retained unchanged content during
live runs. The final broad suite ran on the current production sources. Subsequent edits repaired
the separate code-generation test mirror and documentation, which have their own final checks.

- `cargo test --release --features self-host --test selfhost_composite_annotations --test selfhost_composite_projections --test selfhost_composite_members --test selfhost_structural_types --test selfhost_enum_types --test selfhost_composite_types --test selfhost_type_transport --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test selfhost_parse --test selfhost_typecheck --test self_hosted_backend`
  passed 242 tests with two existing parser measurement tests ignored. All twelve complete stage
  artifacts match. Terminal exit zero. Log `tmp/order1-composite-annotations-final-suites2.log`.
- `cargo test --release --features self-host --test selfhost_codegen` passed all 149 tests.
  This includes the construct-support table, all current atomic code-generation functions and
  the repaired reconstruction mirror. Terminal exit zero.
  Log `tmp/order1-composite-annotations-final-codegen3.log`.
- `cargo test --test selfhost_typecheck --test selfhost_composite_annotations` passed thirty
  default type tests. The annotation binary is feature-gated and executes zero tests here.
  Terminal exit zero. Log `tmp/order1-composite-annotations-final-default2.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed actual
  crate checking and passed. Log `tmp/order1-composite-annotations-final-clippy3.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-composite-annotations-final-doc2.log`.
- `cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible`
  passed four refusal/artifact tests and two reproducibility tests. Terminal exit zero.
  Log `tmp/order1-composite-annotations-final-cli2.log`.
- `cargo check --no-default-features --features compile,verify` passed using cached build state.
  Log `tmp/order1-composite-annotations-final-nostd2.log`.
- Final documentation, citation and count guards passed 21 tests. Formatting and diff checks
  passed. Test log `tmp/order1-composite-annotations-final-checks2.log`.

Diagnostic history uses the same prefix. The before log preserves eleven missed rejections.
After1 failed on a missing Rust type qualification. After2 passed the initial comparison.
After3 exposed the reference depth boundary and missing tuple-array access. After4 passed those
repairs. After5 exposed missing unit emission. The first broad final-suites log exposed the unit
provenance pins. The final-codegen2 log records the seven full-file failures repaired afterward.
These diagnostic runs are superseded by the passing final evidence above.

The complete workspace, advisory scan, refreshed inference-independence and final integration
gates remain unverified. Production artifact serialization still calls Rust Module::to_bytes.

## Next work

Continue the full completion plan. General tuple projection identity, arbitrary nested header
syntax, remaining occurrence and binder coverage and general syntax admission remain incomplete.
Nominal struct signatures still need source layout facts. The legacy struct-alias path remains
one hop. Empty-array inference, mixed-scalar tuple runtime layout, general cast admission and
large enum-dispatch capacities have not been exhaustively established. Do not infer complete
annotation or projection coverage from these finite comparisons.

Production type checking is independent before the reference. The reference remains the
acceptance gate while coverage is incomplete. Extraction assertions can still report malformed
or unsupported records as unavailable evidence. Complete Keleusma wire emission, audit framing,
opcode, pool, auxiliary and integrity provenance, refresh inference independence for all twelve
stages and run the final gates. Preserve the full Order 1 objective.

## Shadow pilot and mailbox

Task `v023-order1-composite-annotations` captures the initial diagnostic and three bounded claims
under case prefix `v023-20261004-composite-annotations-`. Blind exports preceded self-labels.
All labels are non-independent, so no independent accuracy or development-improvement claim is
justified. Usage and elapsed time remain null where unavailable. The bounded annotation increment
is resolved while the full Order 1 goal remains active. The reviewed local export is
`tmp/order1-composite-annotations-pilot-export.json`, kept ignored.
External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to api.typesafe.ai because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture and development continue. The pilot implementation line owns the tracked pilot
checkpoint, which remains untouched.

The assigned inbox is empty. No messages were received or sent. The tracked mailbox checkpoint
was exported and reviewed with no communication changes. No verification process remains running.
