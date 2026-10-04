# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment closes three of four initial scoped census misses and fixes branch scope.
It does not complete Order 1. The required census remains red on match-arm guards.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `21fdd4ad`.
The legacy handoff is invalid-and-stale, seventeen commits behind the starting tree.
Its ancestry check passes, the fingerprint remains `0x4327_63E1`, and the stage inventory
still has twelve files. Its unchanged-stage claim fails. Historical full-suite, advisory,
shell and detached-package gates were not rerun. Appendix B was read before tracked writes.
The pre-existing `nvim.log` remains untouched. No push, merge or publication occurred.
Bytecode version remains 2 with 66 public opcodes. Selected type table caps remain 640.

The previous status turn collected the terminal successful branch-scope test result and so
provided verification evidence. This turn adds direct scope and capacity controls, collects the
broader results, records the pilot outcome and persists the implementation. Both make progress.

## Implementation

The reference-input and pipeline-input rule census share `RULE_FORM_CELLS` and
`RULE_FORM_CONTROLS` in `tests/selfhost_typecheck.rs`. Sixteen negatives and eight valid controls
reach the scoped pipeline. Initially four negatives were accepted, with none unavailable.
Only the match-arm guard remains accepted. The required test
`scoped_pipeline_reaches_the_rule_form_census` asserts no gaps and has no expected-gap allowance.
Do not ignore or weaken it to make the branch green. Feature-branch red commits are authorized.

Parser metadata tag 92 carries a source let-annotation type name, following tag 90 and preceding
LetIn. `ParsedFn.let_annotations` pairs it with source range and frame slot. Pending statement
frames preserve nested initializers. Existing expression row 8 compares the initializer with
its declared tag and supplies the binding result. The old invisible-annotation probe is replaced
by `let_type_annotations_survive_the_record_stream`. Harness consumers skip tag 92 alongside
other metadata. The public opcode and artifact formats are unchanged.

Source data facts use bounded slot ranges in parser declaration order, excluding const slots.
The assembler's shared-first slot order is unsuitable here. Scalar data reads supply declared
tags, and assignments compare their values against those declarations. Index requirements cover
indexed data reads/stores and scalar or nested value-array reads. Integer literals do not receive
implicit Byte conversion in annotations or stores. Explicit casts and Byte-valued controls pass.
Composite declarations, reads and results remain incomplete.

A positive nested-shadowing control exposed then-branch locals leaking into else. The parser
now restores the live binding stack after each if branch. Frame slots remain monotonic. A
separate `ps.named_local_count` retains the original total-per-function cap, and `clear_let_shape`
clears composite metadata before a lexical-name slot is reused. The counter lives in `ps` to
avoid introducing a forward reference to `stmt` in `let_push`. Loop and match scope restoration
remain unestablished.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. No test subject was edited during its live run.

- `cargo test --release --features self-host --test selfhost_typecheck --test selfhost_parse --test selfhost_artifact_identity --test selfhost_monomorphize_identity`
  exited 101. Type tests passed 95 and failed one, the required scoped census. Parser tests
  passed 90 with two ignored. Artifact tests passed two, including complete byte identity for
  all twelve stages and inventory coverage. Monomorphization tests passed three. All-stage
  extraction and selected-cap checks passed. Log `tmp/order1-rule-form-final-suites.log`.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests. Log `tmp/order1-rule-form-final-codegen.log`.
- After adding further scope controls, three exact-filter release runs each passed one test.
  `branch_scope_restoration_preserves_complete_artifacts` checks three valid sources, including
  sibling branches, composite-name reuse and implicit else. Log
  `tmp/order1-rule-form-scope-artifacts.log`.
  `local_binding_cap_counts_disjoint_branch_scopes` accepts 64 bindings and refuses 65 across
  disjoint branches. Its caught negative-path panic is expected. Log
  `tmp/order1-rule-form-scope-cap.log`.
  `pipeline_declarations_and_indices_agree_with_reference` checks 28 valid and invalid sources,
  including inaccessible branch locals. Log `tmp/order1-rule-form-declarations4.log`.
  The added tests were verified separately after the broader suite. Production stage sources
  and extraction code did not change after that suite.
- Final `cargo test --test selfhost_typecheck` passed 30 default-feature tests.
  Log `tmp/order1-rule-form-final2-default.log`.
- Final `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` passed
  with actual crate checking. Log `tmp/order1-rule-form-final2-clippy.log`.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-rule-form-final-doc.log`.
- Final documentation, citation and count guards, formatting and diff checks are recorded
  in `tmp/order1-rule-form-final-checks.log`.

The initial four misses are in `tmp/order1-rule-form-before.log`. Intermediate logs preserve
annotation, index, assignment and shadowing failures. Full workspace, advisory, inference-
independence refresh and release/integration gates remain outstanding. The full Order 1 gate
is contradicted by the required census failure as well as missing production integrations.
No verification process remains running at this handoff.

## Next work

Preserve match-arm guards through parsing, reconstruction, type facts and execution. The current
parser's pattern phase skips tokens until the fat arrow. Bare enum patterns use `step_mpat` phase
3, while parenthesized enum, integer and wildcard patterns use `step_match` phase 3. The generic
lexer token mapping does not currently distinguish the `when` keyword. Function-guard parsing
resets whole-body state and cannot be reused unchanged for nested match guards.

The integer match table contains literal/result pairs and assumes a final wildcard. The enum
match table has twenty words per arm plus wildcard metadata. Code generation emits pattern
checks and arm results but no guard checks. The Rust reference evaluates a guard after payload
bindings, emits a nested If and closes that condition after the arm Break. Preserve those
semantics, including false guards falling through. Guard syntax retention alone cannot close
this gap. Cover genuine execution and complete byte identity alongside type rejection.

Continue composite facts, unannotated projections and aliases, array elements, pattern payloads,
binder/occurrence coverage and scope restoration. The scalar type solver retains bounded rounds
and a separate one-hop struct-alias path. Production still calls the Rust reference first.
Integrate independent rejection, then complete Keleusma artifact emission in place of
`module.to_bytes()`. Refresh inference independence, exercise the shipping command and run the
final gates. Existing complete-artifact comparisons serialize both sides through Rust.

## Shadow pilot and mailbox

Pilot task `v023-order1-rule-form` has the initial `triage` case and three completion claims with
prefix `v023-20261004-rule-form-`. The suffixes are `scope-artifact-claim`, `scope-cap-claim` and
`declarations-claim`. Each claim includes the actual command, terminal exit and executed test
count in its reviewed evidence. All labels are non-independent self-reviews. No independent
accuracy or improvement claim is justified. Later diagnostics remain in engineering logs under
the first-diagnostic sampling convention. The recorded task outcome is unresolved because the
census still fails. Full Order 1 remains active.

External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to `api.typesafe.ai` because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture, review and development remain available. The four current packets in
`tmp/order1-rule-form-pilot-export.json` were reviewed without revealing hidden results.
The export also contains older samples with recorded limitations. Keep it ignored. The pilot
implementation line owns the tracked pilot checkpoint, which remains untouched.

The assigned interline inbox is empty. No messages were received or sent. The tracked checkpoint
was exported and reviewed with no communication changes. Before the next dependent work and
handoff, check the inbox again and export the checkpoint as the installed protocol requires.
