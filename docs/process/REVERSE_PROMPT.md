# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The goal remains active. Do not treat the completed branch-pair increment as completion.
The full requirements and evidence are in
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `563cc273`.
The selected 640-row type-input capacities remain in place.
The legacy handoff is invalid-and-stale, eleven commits behind that starting tree.
Its ancestry check passes, but its unchanged-stage-source claim is false and its full
verification receipt has not been refreshed.

Parser If records now distinguish omitted else branches with argument 1 from written branches
with argument 0. Reconstruction stores an omitted else as `-(condition_index + 1)`.
Code generation decodes it. No bytecode opcode or version changed.
Expression extraction emits conditional branch pairs only for written else branches and
compares block tails against the reference. Empty branches contribute unknown operands.
The emitted-kind inventory is five, but kind 4 still lacks match-arm extraction.
The monomorphization identity test now includes all twelve stages and checks the file inventory.

## Verification

Cargo invocations used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
The latter avoids reinstalling Git hooks after the sandbox denied that build-script write.
Self-hosted stage checks also used `KEL_SELFHOST_CACHE=0`.

- `cargo test --features self-host --test selfhost_parse --test selfhost_typecheck` completed
  the parser suite with 90 passed and two ignored. Its initial type run had 72 passed and one
  failure because the emitted-kind inventory still expected four kinds.
- `cargo test --features self-host --test selfhost_typecheck` passed all 73 after updating
  that inventory to five and documenting the remaining match-arm gap.
- `cargo test --features self-host --test selfhost_codegen self_host_compiles_` passed 89
  selected tests, including all twelve stage sources. These are the existing compilation
  differentials, not a new proof of complete production serialization.
- `cargo test --test selfhost_codegen explicit_and_implicit_else_compile_byte_identically -- --exact`
  passed one test with four reconstruction and byte-identity witnesses.
- `cargo test --test selfhost_monomorphize_identity` passed three tests.
- `cargo test --test documentation_links --test comment_citations --test claimed_counts`
  passed 21 tests. The final combined rerun with `--test selfhost_monomorphize_identity`
  passed all 24 after the handoff update.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` passed with an
  actual `Checking keleusma` line after the code changes. Formatting and diff checks passed.

The first parser witness failed before implementation because both record streams were identical.
A missing test-local import and the stale four-kind pin were corrected during development.
Full workspace checks, fresh advisory scanning, remote integration and release gates remain
unverified. No publication or V0.3.X integration occurred. The pre-existing untracked `nvim.log`
was left untouched.

## Next work

Preserve struct-literal identity or its declared field count through the parser record stream.
`step_sc_closing` already resolves `structdefs.sc_name` but emits only flat byte size and supplied
field count. The equal-size, unequal-field-count witness in `tests/selfhost_typecheck.rs` pins
why size cannot replace identity. Then close the other type-input residuals and production
rejection integration. Keep complete production wire emission in the same Order 1 objective.

## Shadow pilot and mailbox

The mailbox was empty. Its reviewed tracked checkpoint contains four registered routes and
no messages. Recheck it before dependent work and handoff using the installed protocol.

Pilot task `v023-order1-else-presence` is recorded as a completed increment, while Order 1 is
still open. Case prefix is `v023-20261004-else-`.
The short `hook-triage` excerpt was superseded before evaluation because it omitted the
permission detail. `hook-triage-full` records the complete sanitized diagnostic and has a
transport-error result with HTTP status 0. Its self-review label is environment.
`hook-triage-network` is the same reviewed payload, but automatic approval review rejected
execution before the network retry. A specific approval question to the operator is pending.
Do not bypass that rejection. The request digest is
`ebae2eeac5dd3fa6c2dae478a60655ad69419838f09185cd5233557cb7787531`.

`monomorphize-claim` and `stage-byte-identity-claim` hold two narrow passing-test-count claims.
They are captured and self-labelled, with no live evaluation yet. No independent reviewer was
used and no classification accuracy is claimed. No pilot result altered a gate.
The reviewed local export is `tmp/jev-shadow-order1-else-checkpoint.json`.
Do not overwrite the shared pilot implementation line's tracked checkpoint without coordinating
ownership. The local records remain available in the installed pilot store.
