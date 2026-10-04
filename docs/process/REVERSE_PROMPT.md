# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full goal remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment closes the six failures exposed by the existing scoped rejection corpus.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `1a1108dc`.
The legacy handoff is invalid-and-stale, sixteen commits behind the starting tree.
Its ancestry check passes, but the unchanged-stage claim fails. Historical full gates were
not rerun. Appendix B was read before tracked writes. The pre-existing `nvim.log` remains
untouched. No push, merge or publication occurred. Bytecode version remains 2 with 66 opcodes.
The prior status-only turn was classified as no progress. This increment makes implementation
and verification progress. It does not close the full goal.

The complete existing corpus now rejects twenty invalid programs and accepts seven valid
controls through scoped pipeline inputs. The Rust reference establishes the controls only.
It does not reject on behalf of the pipeline. The initial run accepted undefined identifiers,
unknown fields and a local-call program. Undefined calls and two scalar projection cases
failed during reconstruction. Those six cases now receive explicit source diagnostics.

`ParseRecordError` retains stage tags and payloads and separates execution, protocol and budget
failures. `try_drive_parse_records_with` is the shared fallible transport. Its old wrapper
retains panic behavior. `try_call_type_inputs_from_pipeline` propagates parser errors through
scoped extraction. Stage compilation, lexing, reconstruction and callback failures still have
panic paths. The corpus treats any unexpected panic as unavailable evidence, never rejection.

Parser diagnostic codes 13 and 14 report unknown names and fields. Codes 15, 16 and 17 report
function values and declared-scalar field/index access. Code 22 reports a call through a value.
Code 12 remains retired. Codes 18 through 21 are unsupported syntax, not source errors.
The source-error classifier also retains unmatched closing brackets as code 2. Unknown codes,
capacity failures and transport failures do not establish source rejection.

Scalar annotations are retained in parameter, pending-statement and local-binding storage.
The pending statement stack preserves an outer annotation through nested initializer bindings.
Array and tuple element names never classify the outer composite as scalar. General annotation
compatibility is still not checked. The host now seeds lowercase `bool` instead of uppercase
`Bool`. Calls through parameters and unannotated locals have additional negative controls.

A scalar dot is delayed through the next identifier to distinguish a field from a method call.
Unknown struct fields use the same distinction. Const parameters, unsupported patterns,
assertions and unresolved imported or qualified names have separate unsupported outcomes.
The first lexer pass supplies whole-source import presence. `toks.has_imports` and
`toks.assert_id` are appended shared fields, with corresponding slot constants and layout checks.
Ordinary type-generic signatures still reach the production divergence check. This is a partial
syntax admission analysis, not an exhaustive classification of valid unsupported programs.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. No test subject was edited while a run was live.

- `cargo test --release --features self-host --test selfhost_typecheck --test selfhost_parse --test selfhost_artifact_identity --test selfhost_monomorphize_identity`
  passed 94 type tests, 90 parser tests with two ignored, two complete artifact tests covering
  all twelve stages, and three monomorphization tests. The type suite includes both all-stage
  extraction and capacity checks. Log `tmp/order1-corpus-verified-suites.log`.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests. Log `tmp/order1-corpus-verified-codegen.log`.
- `cargo test --test selfhost_typecheck` passed 30 default-feature tests.
  Log `tmp/order1-corpus-default.log`.
- `cargo test --lib selfhost_host` passed four layout/capacity tests.
  Log `tmp/order1-corpus-verified-host.log`. An earlier command also selected the integration
  binary with that filter and ran zero integration tests. It is not the default type receipt.
- Broad Clippy with `--tests --features signatures,shell,self-host -- -D warnings` passed.
  Default Clippy with `--test selfhost_typecheck -- -D warnings` passed. Both printed actual
  crate checking. Logs `tmp/order1-corpus-verified-clippy.log` and
  `tmp/order1-corpus-default-clippy.log`.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-corpus-verified-doc.log`.
- Final documentation, citation and count guards, formatting and diff checks are recorded
  in `tmp/order1-corpus-final-checks.log`.

The initial diagnostic is in `tmp/order1-corpus-before.log`. Intermediate suites exposed method
classification, the Bool seed and stale probes. Forward data and forward native-import candidates
failed the reference positive-control assertion and were removed from that valid list. Logs remain
under `tmp/order1-corpus-*`. The legacy scalar-projection probe now checks structured diagnostics
instead of counting any panic as success and suppressing the process-wide panic hook.
No verification process remains running at this handoff. Full workspace, advisory,
inference-independence refresh and release/integration gates remain outstanding.

## Next work

Move the rule-versus-form census through the scoped pipeline and retain valid controls for each
form. The twenty-case corpus alone does not establish complete type coverage. General annotation
compatibility, unannotated scalar projections and aliases, composite reads/results, data-field
types, array element types, pattern payloads and binder/occurrence coverage remain incomplete.
Match guards are discarded. Function guards have distinct reference behavior. The type solver
retains bounded rounds and a separate one-hop struct-alias path.

Integrate the type verdict into production without reference-first masking after closing the
input and admission gaps. The shipping driver still calls the Rust reference first. Complete
production emission through Keleusma instead of `module.to_bytes()`, refresh inference independence,
exercise the shipping command and run final workspace/integration gates. Complete artifact identity
has direct current all-stage evidence, but both sides still serialize through Rust.

## Shadow pilot and mailbox

Pilot task `v023-order1-rejection-corpus` has four local cases with prefix
`v023-20261004-corpus-`. They are `triage`, `type-claim`, `artifact-claim` and `clippy-claim`.
All labels are non-independent self-reviews. The type and Clippy claim self-labels are
`unsupported` because their isolated excerpts omit command context required by their wording.
This does not negate the full development receipts above. It records a packet-construction gap.
The Clippy packet also retains a workspace path. Keep it local and do not evaluate or copy it
into a tracked checkpoint as-is. Future replacement samples need reviewed redaction and explicit
command provenance. The artifact excerpt supports its bounded two-test claim.

External evaluation remains pending the answer to the existing specific approval question.
Automatic approval review previously rejected sending excerpts to `api.typesafe.ai` because it
did not recognize destination-and-payload authorization. Do not retry through another route.
No outbound evaluation occurred in this increment. Later engineering diagnostics were omitted
under the first-diagnostic convention. No accuracy or development improvement is claimed.
The outcome resolves this corpus increment only. The local export is
`tmp/jev-shadow-order1-corpus-checkpoint.json`. It was reviewed locally and remains ignored.
The pilot implementation line owns the tracked pilot checkpoint, which was not overwritten.

The interline inbox is empty. The tracked checkpoint was exported and reviewed. It contains
four routes and no messages. No interline messages were sent.
