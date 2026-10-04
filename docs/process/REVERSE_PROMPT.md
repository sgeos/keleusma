# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The goal remains active. The complete requirements remain in
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment implements call inputs and strengthens complete-artifact evidence.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `e65069b4`.
The legacy handoff is invalid-and-stale, fourteen commits behind this starting tree.
Ancestry passes, but stage sources changed. Its full verification receipt is historical.
Appendix B was read before tracked writes. The existing `nvim.log` remains untouched.
No push, merge or publication occurred. Bytecode version remains 2 and the opcode count 66.

`call_type_inputs_from_pipeline` returns declarations and independent `CallTypeBatch` ranges.
Each source head's guard and body retains its own sites, argument requirements and bindings.
Arguments come from reconstructed call indices. Literal tags, declared scalar parameters,
local aliases and declared function returns are available without reference syntax or inferred
expression types. Locals use source-head, range and frame-slot identities. Function returns
use a separate namespace. Each batch imports only referenced function return declarations.
The legacy declaration-and-site helper flattens the sites without changing its signature.
Expression extraction reuses the same source operand classifier and slot-name helper.

The aggregate wire input initially contained 831 bindings and exceeded the selected cap.
Across all twelve stage sources, batches now have maxima of 67 sites, 88 arguments and
62 bindings. The largest declaration table has 492 entries. All call batches pass the type
stage. No capacity increased. This is call-channel evidence, not complete type rejection.

Three further regression failures are corrected. The parser now resolves locals before global
function and data names. Scalar alias resolution follows at most one edge per binding row,
reaching all acyclic chains in the table while cycles and absent targets remain unknown.
The two module assembly helpers omit the data layout when there are no slots, avoiding extra
artifact regions that the reference does not emit. Valid shadowing controls compare complete
artifacts, and a new inventory-checked corpus covers all twelve production stage artifacts.

## Verification

Cargo used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. Release-profile tests were used for the larger corpus.
No test subject was edited during its run.

- Final `cargo test --release --features self-host --test selfhost_typecheck --test selfhost_artifact_identity --test selfhost_monomorphize_identity`
  passed 88 type tests, two artifact tests and three monomorphization tests. The artifact test
  compares complete bytes from `self_host_compile_scratch` for all twelve stages and checks
  the directory inventory. Both sides still serialize through Rust. The type run includes
  both all-stage extraction tests. Log `tmp/order1-call-final-suites.log`.
- `cargo test --release --features self-host --test selfhost_typecheck --test selfhost_parse --test selfhost_monomorphize_identity`
  passed 88, 90 with two ignored, and three respectively. Log `tmp/order1-call-suites.log`.
  This preceded comment-only edits in the driver and type test. Parser stage code was unchanged.
- `cargo test --release --features self-host --test selfhost_artifact_identity --test selfhost_codegen self_host_compiles_`
  passed 89 selected compilation tests, including all twelve stages. The name filter selected
  zero artifact tests. The separate artifact run and final combined command above executed both.
  Log `tmp/order1-call-stage-identity.log`.
- `cargo test --test selfhost_typecheck --test selfhost_artifact_identity` passed 30 type tests.
  Artifact tests correctly compile out without `self-host`. Log `tmp/order1-call-default.log`.
- Broad-feature Clippy with `--tests --features signatures,shell,self-host -- -D warnings`
  and default-feature Clippy with `--test selfhost_typecheck --test selfhost_artifact_identity -- -D warnings`
  passed with actual crate checking. Logs `tmp/order1-call-clippy.log` and
  `tmp/order1-call-default-clippy.log`.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-call-doc-build.log`.
- Final documentation, citation and count guards, formatting and diff checks are recorded in
  `tmp/order1-call-final-checks.log`.

The first diagnostic was missing documentation on three new fields, before tests executed.
Later failures exposed empty argument rows, cross-function name collision, local-versus-global
precedence, aggregate capacity, alias chaining and empty data-layout regions. Two parser edits
also failed because they used unsupported local mutation syntax before being corrected to the
existing parser state field. Logs are retained under `tmp/order1-call-*`.
Full workspace, advisory and release gates have not been rerun. Inference-independence evidence
still needs refreshing. No verification process remains running at this handoff.

## Remaining work

Complete the pipeline type-input path. Computed call arguments and composite types remain
unknown. The call channel now uses scoped identities, but the legacy expression and binding
helpers still expose raw names. Do not combine those tables as though their identities match.
Unifying expression-derived facts and call requirements within the same source-head batches is
one concrete next step. Preserve arity checks and valid controls while extending operand forms.

Source annotations, field and index rejection before reconstruction, binder and occurrence
coverage, array and payload types remain incomplete. The parser still discards match guards.
The reference accepts a Word-valued function guard, so no extra root boolean requirement was
introduced. Unknown-name fallback to local slot zero remains a separate parser concern.
The call extraction repeats lexical work for chunk-name lookup; reuse the existing ordering
algorithm rather than deriving another one.

Integrate rejection without reference-first masking. The shipping driver still calls the Rust
reference first. Complete production emission through the Keleusma wire implementation instead
of `module.to_bytes()`, refresh inference independence, exercise the shipping command and run
final workspace and integration gates. Complete artifact identity now has direct all-stage
production evidence, but it must be repeated after subsequent pipeline changes.

## Shadow pilot and mailbox

Pilot task `v023-order1-call-inputs` has four local development cases with prefix
`v023-20261004-call-`. They are `triage`, `clippy-claim`, `type-claim` and `artifact-claim`.
The claims describe the completed Clippy command and final passing counts. All labels are
non-independent self-reviews. No accuracy or development improvement is claimed.
Later diagnostics were omitted under the first-diagnostic sampling convention.

Live evaluation remains pending the operator answer to the existing specific approval question.
Automatic approval review previously rejected sending excerpts to `api.typesafe.ai` because it
did not recognize authorization for the destination and payload. Do not retry through another
route. Local capture and compiler work continue independently. The pilot outcome resolves this
increment only and explicitly leaves full Order 1 active. The reviewed local export is
`tmp/jev-shadow-order1-call-checkpoint.json`. The tracked pilot checkpoint belongs to the pilot
implementation line and was not overwritten here.

The interline inbox is empty. The tracked checkpoint was exported and reviewed. It contains
four routes and no messages. No interline messages were sent.
