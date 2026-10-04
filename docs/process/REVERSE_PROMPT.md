# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This commit closes the measured match-guard gap and adds scalar access facts. It does not complete Order 1.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `1102cf9d`.
The preceding status turn collected a terminal verification receipt and confirmed the next work.
This turn adds scalar access coverage, verifies the combined changes and prepares one scoped commit.
The legacy handoff is invalid-and-stale, eighteen commits behind the starting tree. Its ancestry
check passes, the fingerprint remains `0x4327_63E1`, and the unchanged-stage claim fails.
Historical full-suite, advisory, shell and detached-package gates were not rerun. Appendix B
was read before tracked writes. The pre-existing `nvim.log` remains untouched. No push, merge
or publication occurred. Bytecode version remains 2 with 66 public opcodes. Selected type
caps remain 640. The concurrent V0.3.X line remains untouched.

## Implementation

The lexer emits token 64 for `when`, distinct from the return arrow's token 4. Both reference
adapters were updated, and a lexer control distinguishes the keyword, arrow and `whenx` name.
Function guards now use the distinct token. Match parser phase 5 reads the guard as an ordinary
expression and drains it at the fat arrow. Phase 4 reads the result. Record 71 then wraps the
guard and result into one binary forest node. These are internal records, not bytecode opcodes.

Wildcards remain in source order. Record 72 pushes a negative integer-pattern sentinel during
reconstruction. Record 73 supplies an enum wildcard descriptor. A final wildcard compacts into
the existing trailer, preserving the previous table capacity. Earlier wildcards stay in the
ordered arm table. The MatchBuild count includes every source arm. Reconstructed node counts
exclude a compacted final wildcard. Existing parser differential projections now match that
record protocol, including guard wrappers.

Code generation's `push_match_result` emits the guard after pattern binding, with the arm Break
inside the guard If. False guards therefore continue to later arms. `MatchMark` separates the
outer operator context. `match_boundary` checks grouping markers and nested block context so
commas and braces inside calls, arrays, tuples, structs or conditionals cannot close the arm.
Match-arm exit restores binding names to the match baseline. Frame slots and the total named
binding count remain monotonic. Loop scope restoration remains unestablished.

The type walk emits a bool requirement for node 71 and compares its result child across arms.
Scoped slots include reconstructed local references. EnumBind records supply declared scalar
payload tags and frame slots. Word, Byte and bool payloads are recognized. Composite payloads
remain unknown. The legacy occurrence/name channel still omits some binders and is separate
from this scoped slot path.

Complete artifact comparison found two additional omissions. Enum declaration END records now
carry the declared maximum payload byte size, already computed by `enum_bytesize`. The driver
preserves it as `EnumLayout.min_payload`. Boolean parameters now retain both `TypeTag::Bool`
and scalar verifier shape kind 1. Source-based assembly remains independent of the reference.


Scalar access facts now reach the scoped operand adapter. Struct and tuple fields carry
an offset plus scalar kind, and scalar array-element nodes carry the kind directly. The
adapter maps Word, Byte and bool without reference inference. Nested composite access kinds
remain separate. A twenty-six-program comparison initially found nine invalid programs
accepted by the pipeline. All now agree with the reference, including valid container
arguments, aliases, tuple offsets and contextual Byte arithmetic. No stage capacity changed.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. No test subject was edited during a live run.

- `cargo test --release --features self-host --test selfhost_artifact_identity --test selfhost_typecheck --test selfhost_parse --test selfhost_monomorphize_identity`
  passed 98 type tests, 91 parser tests with two ignored, five artifact tests and three
  monomorphization tests. All twelve complete stage artifacts match the reference. Inventory,
  all-stage extraction and selected-cap checks passed. Log `tmp/order1-scalar-access-final-suites.log`.
- `match_guards_preserve_complete_artifacts_and_execution` covers fourteen valid sources,
  compares complete artifacts and executes both compilers' output. Controls include false
  fallthrough, repeated patterns, guarded wildcards, payload binding, nested match/if guards,
  calls, outer operators, skipped trapping results and guard side effects. A separate twelve-
  variant case checks final-wildcard capacity. Both are in the artifact suite above.
- `pipeline_match_guard_types_agree_with_reference` covers twelve valid and invalid sources,
  including literal, parameter, computed, called-function and scalar-payload guards. The scoped
  census now rejects all sixteen negatives and accepts its eight controls. Both passed in the
  type suite above. A separate initial census receipt is `tmp/order1-match-guards-census.log`.
- On the preceding match-guard tree, `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests. Log `tmp/order1-match-guards-final-codegen.log`.
- On the preceding match-guard tree, `cargo test --release --features self-host --test selfhost_lexer --test selfhost_pipeline`
  passed twelve lexer tests and nine pipeline tests. Log `tmp/order1-match-guards-final-lexpipe.log`.
- `cargo test --test selfhost_typecheck` passed thirty default-feature tests.
  Log `tmp/order1-scalar-access-final-default.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` passed with
  actual crate checking. Log `tmp/order1-scalar-access-final-clippy.log`.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-scalar-access-final-doc.log`.
- Final documentation, citation and count guards, formatting and diff checks are recorded
  in `tmp/order1-scalar-access-final-checks.log`.

The initial test authoring errors are in `tmp/order1-match-guards-before.log`. The corrected
baseline demonstrates discarded guard instructions in `tmp/order1-match-guards-baseline.log`.
Intermediate logs retain payload-size and Boolean-metadata discrepancies. A proposed guarded-
only Word wildcard control was rejected by the reference as non-exhaustive and was corrected
with an unconditional fallback. This does not establish pipeline exhaustiveness checking.
No verification process remains running at handoff. Full workspace, advisory, inference-
independence refresh and release/integration gates remain outstanding.

## Next work

Continue the remaining input coverage in the completion plan. The scoped census is finite and
not exhaustive. Composite reads/results, composite payloads, unannotated
projection aliases, complete binder/occurrence coverage, loop scope and syntax admission still
need work. Audit match exhaustiveness before production integration, including guarded-only
fallbacks and enum coverage. The scalar solver retains bounded rounds and a separate one-hop
struct-alias path.

The next concrete work is the admission and scope audit, starting with guarded-only match
fallbacks, enum coverage and loop-local names. Scalar access tags now reach scoped constraints,
but composite results and unannotated composite aliases still need coverage. Inspect the
parser's shape propagation and distinguish unsupported syntax from source errors. The legacy
occurrence channel remains separate and still omits some binders.

Then integrate the type verdict into production without reference-first masking. The shipping
driver still calls the Rust reference first. Complete Keleusma artifact emission in place of
`module.to_bytes()`, refresh inference independence, exercise the shipping command and run the
final gates. Complete artifact comparisons still serialize both sides through Rust.

## Shadow pilot and mailbox

The additional task `v023-order1-scalar-access` records four cases prefixed
`v023-20261004-scalar-access-`, with suffixes `triage`, `type-claim`, `access-claim`
and `artifact-claim`. The first diagnostic reports nine missed rejections in the initial
comparison. The three claims have terminal command evidence and suite identity. Their
combined command executed 197 tests. All four labels are non-independent self-reviews.
The bounded scalar access increment is resolved, with unavailable usage and elapsed time
left null. Its reviewed export is `tmp/order1-scalar-access-pilot-export.json`, kept ignored.
The exported packets and outbound inputs match the reviewed local packets. No external
evaluation was retried, and no independent accuracy or improvement claim is justified.

Pilot task `v023-order1-match-guards` has the initial `triage` case and three completion claims
with suffixes `type-claim`, `artifact-claim` and `codegen-claim`, all prefixed
`v023-20261004-match-guards-`. Claims retain exact command, terminal exit and suite identity.
The combined command metadata reports 196 executed tests, while its excerpts identify the
individual suite being claimed. All labels are non-independent self-reviews. No independent
accuracy or improvement claim is justified. Later diagnostics remain in engineering logs under
the first-diagnostic sampling convention. The bounded guard increment outcome is resolved.
The full Order 1 objective remains active.

External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to `api.typesafe.ai` because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture, review and development remain available. The four current packets in
`tmp/order1-match-guards-pilot-export.json` were reviewed without revealing hidden results.
The export also includes older samples with recorded limitations. Keep it ignored. The pilot
implementation line owns the tracked pilot checkpoint, which remains untouched.

The assigned interline inbox is empty. No messages were received or sent. The tracked checkpoint
was exported and reviewed with no communication changes. Check again before dependent work
and handoff, as required by the installed protocol.
