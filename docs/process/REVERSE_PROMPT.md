# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The goal remains active. The complete requirements are in
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
The previous goal turn and this increment both made concrete implementation progress.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `778bfb35`.
The legacy handoff is invalid-and-stale, twelve commits behind that starting tree.
Its ancestry check passes, but its unchanged-stage-source claim is false and its full
verification receipt has not been refreshed. Appendix B was read before tracked writes.

The parser emits metadata tag 91 immediately before each struct construction. Its payload is
the resolved declaration's field count. A private word holds the delayed construction record.
The production driver consumes the metadata into `ParsedFn::struct_counts`, pairs it with the
supplied count in record 27, and removes it before reconstruction. No new expression node,
bytecode opcode or bytecode version is involved. Parser test drivers divert it consistently.

Expression extraction now emits kind 7 from these counts. Nested and reordered literals agree
with reference syntax. Equal-size literals with different declared field counts remain identical
as construction records but differ in metadata. Kind 3 also now covers unary `not`, whose boolean
requirement was previously omitted. The emitted-kind inventory is six. Match-arm pairs and the
field/index rejection paths remain incomplete, along with other documented input-channel gaps.
The selected 640-row capacities and explicit else markers from preceding commits remain in place.

## Verification

Cargo invocations used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Self-hosted stage checks also used `KEL_SELFHOST_CACHE=0`.

- `cargo test --features self-host --test selfhost_typecheck` passed 74 after the struct-count
  change and 75 after the unary-condition change.
- `cargo test --test selfhost_parse` passed 67 with default features.
- `cargo test --features self-host --lib the_struct_field_tag_matches_the_stage` passed one.
- `cargo test --features self-host --test selfhost_codegen self_host_compiles_` passed 89
  selected tests, including all twelve stage sources. That run preceded the final unary-condition
  addition in the type-input extraction helper. Parser and compiler stage sources stayed unchanged.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` passed after the
  final code change with an actual `Checking keleusma` line.
- The final combined documentation, citation, count and monomorphization command is
  `cargo test --test documentation_links --test comment_citations --test claimed_counts --test selfhost_monomorphize_identity`.
  It passed 24 tests. Its receipt is `tmp/order1-struct-final-checks.log`. Formatting and
  diff checks passed.

The struct-count test first failed because kind 7 was absent. The expanded condition comparison
then failed on `not true` because its requirement row was absent. Both controls remain in the tests.
Full workspace checks, fresh advisory scanning, remote integration and release gates remain
unverified. No publication or V0.3.X integration occurred. The existing untracked `nvim.log`
was left untouched.

## Next work

Close the remaining source-fact extraction gaps and integrate independent production rejection.
Match-arm comparisons are a concrete next gap. Reconstruction kind 22 uses `match_parts` with
`[temporary, wildcard, literal, result, ...]`. Kind 39 uses a 20-word entry per enum arm and an
explicit wildcard-presence flag after the entries. Compare the first source arm against later
arms as the reference extractor does. Probe empty blocks, nested matches, arm binders and guards
before claiming every form is reached. Do not infer source presence from synthesized nodes.

Other gaps include actual call arguments, binder and occurrence coverage, and field/index inputs
refused during reconstruction. Keep the full scope in the completion plan. The command-line
unsigned artifact path still invokes `module.to_bytes()` after selecting the self-hosted backend.
The existing Keleusma wire stage has framing and sealing commands, but complete production
artifact emission is still required. Six expression kinds are not six completed input channels.

## Shadow pilot and mailbox

Mailbox checks were empty. The tracked interline checkpoint was exported and reviewed.
It contains four registered routes and no messages.

Pilot task `v023-order1-struct-counts` holds the current increment. Case prefix is
`v023-20261004-struct-`. `counts-triage` contains the initial struct-count assertion failure.
`type-claim`, `byte-claim` and `final-type-claim` contain the 74, 89 and 75 passing-test summaries.
All are local development records. Self-review labels are explicitly non-independent.
No classification accuracy or development improvement is claimed.

Live evaluation remains pending the operator answer to the previous specific approval question.
Automatic approval review rejected sending reviewed excerpts to `api.typesafe.ai` because it did
not recognize authorization for the destination and payload. Do not retry through another route.
The previous task `v023-order1-else-presence` retains its transport failure and pending retry.
The reviewed local export is `tmp/jev-shadow-order1-struct-checkpoint.json`. Its payloads and
source references were reviewed without revealing unfinished peer judgments. The tracked pilot checkpoint belongs to the pilot
implementation line and must not be overwritten concurrently.
