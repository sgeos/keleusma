# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The goal remains active. The full requirements remain in
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
The previous goal turn committed an increment. This turn made further implementation progress.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `6da83f22`.
The legacy handoff is invalid-and-stale, thirteen commits behind this starting tree.
Its ancestry check passes, but stage sources changed and its full verification receipt remains
historical. Appendix B was read before tracked writes. The existing `nvim.log` is untouched.

Expression kind 4 now reads integer and enum match result indices from `Body.match_parts`.
Each first result is compared with later results. The enum wildcard contributes only when its
presence flag is set. The syntax differential covers fourteen comparisons across ten sources.
Ten verdict cases pass these rows directly to the Keleusma type stage, with valid controls,
nested matches, wildcard-only dispatch and later mismatched arms.

Expression extraction reconstructs each parsed source head separately. This restores individual
return rows and each head's own parameter and local binding names. Guards and bodies are separate
ranges, so guard subexpressions remain available without a synthetic dispatch root. Derived-row
indices still point into the accumulated expression rows. Compilation retains its original
multihead dispatch path. No stage source, bytecode opcode or bytecode version changed.

## Verification

Cargo used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage tests also used `KEL_SELFHOST_CACHE=0`.

- `cargo test --features self-host --test selfhost_typecheck` passed 80 in 655.31 seconds.
  This includes `pipeline_expression_extraction_reaches_every_stage`, which reaches all twelve
  sources, compares their source-tail counts and checks derived binding indices. It does not
  establish a complete type verdict or serialized artifact identity. The log is
  `tmp/order1-match-typechecks.log`.
- That run preceded five test-helper feature gates and comment edits. The final command
  `cargo test --features self-host --test selfhost_typecheck -- --skip pipeline_expression_extraction_reaches_every_stage`
  passed 79 with one filtered out. The extraction implementation and filtered test were unchanged.
  The final log is `tmp/order1-match-typechecks-final.log`.
- `cargo test --test selfhost_typecheck` passed 29 without warnings after those feature gates.
  Its first run passed with five unused-helper warnings, now corrected. Final log is
  `tmp/order1-match-default-final.log`.
- `cargo clippy --test selfhost_typecheck -- -D warnings` and
  `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` passed with actual
  `Checking keleusma` lines. Logs are `tmp/order1-match-default-clippy.log` and
  `tmp/order1-match-clippy-final.log`.
- `cargo test --test documentation_links --test comment_citations --test claimed_counts --test selfhost_monomorphize_identity`
  passed 24 tests. Formatting and diff checks passed. The final receipt is
  `tmp/order1-match-final-checks.log`.

The initial match comparison failed because no row was emitted. The initial source-head test
failed because only the ordinary caller's return row was present. Both now pass. The long
all-stage test remained active and was not restarted. Two stack samples showed lexer execution
and then reconstruction before its successful completion. Full workspace checks, advisory scan,
complete artifact identity, shipping integration and release gates remain unverified.

## Remaining work

Actual call argument facts are a concrete next channel. `decl_call_rows_from_pipeline` carries
callee names and arities but omits actual argument types. Reuse reconstructed call argument
indices and source operand forms. Preserve unknown operands and valid controls. Avoid another
independent chunk-name ordering algorithm. The driver currently repeats the first lexical pass
for chunk-name lookup, a measured cost during the all-stage extraction run.

Other gaps include source annotations, field and index rejection before reconstruction, binder
and occurrence coverage, scoped name identity and payload-type resolution. The enum payload
binder in the match corpus remains unknown on both input paths. Bare block match results are
rejected by the reference parser. Integer matches without a wildcard fail during reconstruction.
The self-hosted parser discards match guards, so their conditions remain unavailable.
The reference accepts `fn f(x: Word) -> Word when 1 { 1 } fn f(y: Word) -> Word { 2 }`.
No additional boolean requirement on function guard roots was introduced. These are separate
concerns and neither is evidence that guard typing is complete.

Production rejection still needs integration without reference-first masking. The shipping
unsigned artifact path still uses `module.to_bytes()`. Complete Keleusma wire emission,
inference independence, all-stage serialized identity, the shipping command and final gates
remain in scope. Six expression kinds do not establish six completed input channels.

## Shadow pilot and mailbox

Pilot task `v023-order1-match-arms` contains four local development cases with prefix
`v023-20261004-match-`. They are `triage`, `clippy-claim`, `type-claim` and `default-claim`.
The first captures the missing-match-row assertion. The claims describe one completed Clippy
run and the 80-test and final 29-test results. All labels are non-independent self-reviews.
No accuracy or development improvement is claimed. Later diagnostics were not sampled under
the first-diagnostic convention. The first record attempt was refused for an incorrect metadata
shape, then corrected successfully.

Live evaluation remains pending the operator answer to the existing specific approval question.
Automatic approval review rejected sending reviewed excerpts to `api.typesafe.ai` because it
did not recognize authorization for the destination and payload. Do not retry through another
route. Local capture and compiler work remain available. The pilot task is finished for this
increment only. Its outcome states that full Order 1 remains active. The local export is
`tmp/jev-shadow-order1-match-checkpoint.json`. All sample inputs and source references were
reviewed without exposing unfinished peer judgments. The tracked pilot checkpoint belongs to
the implementation line and is not overwritten here.

Mailbox checks were empty. The tracked interline checkpoint was exported and reviewed.
It contains four routes and no messages.
