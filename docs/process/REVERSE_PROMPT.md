# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full goal remains active. Requirements remain in
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment unifies scoped scalar expression facts with call requirements.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `303547bd`.
The legacy handoff is invalid-and-stale, fifteen commits behind this starting tree.
Its ancestry check passes but stage sources changed. Its full verification receipt remains
historical. Appendix B was read before tracked writes. The existing `nvim.log` is untouched.
No push, merge or publication occurred. Bytecode version remains 2 and the opcode count 66.

`CallTypeBatch` now includes expression rows. Synthetic identities include source head,
guard or body range and node index. Form-2 bindings address expression result rows within
that same batch. Calls, local initializers and return requirements share these identities.
The forest constraint walk is factored into `expression_rows_from_body`, also used by the
legacy expression projection. That projection retains its original row shapes and raw names.
Do not mix its bindings with the scoped batch bindings.

The Keleusma stage derives scalar results for arithmetic, comparisons, asymmetric shifts,
boolean operators, numeric unary operators, Word and Byte casts, conditional branches and
match results. Host extraction reports shapes and source literal values. Operand form 3 is
an integer literal, distinct from binding form 3, which denotes a field read. The stage applies
Byte coercion from 0 through 255 for numeric binary operators and comparisons only. Calls
and shifts do not use that coercion. Literal local initializers remain ordinary Word bindings.

The parser previously erased xor into the inequality operator tag. It now retains operator
tag 33 until code generation maps it to the existing comparison instruction. This allows the
type stage to require boolean operands without changing emitted bytecode. Type-input row
kinds are separate from bytecode opcodes and do not add an opcode.

The differential covers 33 scalar sources and two 16-link computed chains, including valid
controls and batch-local row-index checks. All twelve stages fit and pass the combined
expression and call channel. Maximum combined expression and argument rows are 299, bindings
288, sites 67 and declarations 492. All remain within the selected 640-row caps.

## Verification

Cargo used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage tests used `KEL_SELFHOST_CACHE=0`. Large corpora used the release profile.
No test subject was edited while its run was active.

- Final `cargo test --release --features self-host --test selfhost_typecheck --test selfhost_artifact_identity --test selfhost_monomorphize_identity`
  passed 90 type tests, two artifact tests and three monomorphization tests. Artifact identity
  covers complete bytes from the production compiler for all twelve stages and checks the
  directory inventory. Both sides still serialize through Rust. The type suite includes both
  all-stage extraction tests. Log `tmp/order1-expr-call-final-suites.log`.
- The earlier combined command also selected `--test selfhost_parse`. It passed 90 parser tests
  with two ignored, alongside the same 90, two and three tests. Log
  `tmp/order1-expr-call-suites.log`. Later changes were a collapsed Rust conditional and comments.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests, including every stage. Log `tmp/order1-expr-call-codegen.log`.
  Parser and code-generation behavior did not change afterward.
- Final `cargo test --test selfhost_typecheck` passed 30 without warnings.
  Log `tmp/order1-expr-call-default-final.log`.
- Broad Clippy with `--tests --features signatures,shell,self-host -- -D warnings` passed.
  Default Clippy with `--test selfhost_typecheck -- -D warnings` passed. Both printed actual
  crate checking. Logs `tmp/order1-expr-call-clippy-final.log` and
  `tmp/order1-expr-call-default-clippy.log`.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-expr-call-doc-build.log`.
- Final documentation, citation and count guards, formatting and diff checks are recorded in
  `tmp/order1-expr-call-final-checks.log`.

The first test failed because `take(1 < 2)` was accepted for a Word parameter. The generic
walk initially failed Rust compilation because a closure was moved instead of borrowed.
Clippy then flagged a collapsible conditional in singleton-match result extraction. Both were
corrected before the final runs. Logs remain under `tmp/order1-expr-call-*`.
Full workspace, advisory, inference-independence refresh and release gates remain outstanding.
No verification process remains running at this handoff.

## Remaining work

The unified batches are a stronger input path, not independent production type rejection.
Source annotations, composite reads and results, declared data-field types, pattern payloads,
array element types, binder and occurrence coverage still need work. Run the complete rejection
and valid-control corpus through this path to identify its next missing facts. Avoid inferring
completion from the scalar case count. Empty tails and unsupported syntax also deserve explicit
coverage while extending rejection beyond the existing extraction tests.

The parser discards match guards. Function guard roots have different reference behavior, so
no extra boolean requirement was added there. Unknown-name fallback to slot zero remains a
separate parser concern. The legacy expression and binding helpers still expose raw names.
The type solver retains bounded rounds and a separate one-hop struct-alias path. Scalar alias
chains use the binding-table bound. Composite derivation must not be inferred from scalar tests.

Production rejection must run without reference-first masking. The shipping driver still
calls the Rust reference first. Complete production emission through Keleusma instead of
`module.to_bytes()`, refresh inference independence, exercise the shipping command and run
final workspace and integration gates. Complete artifact identity has direct all-stage evidence
but must be repeated after later pipeline changes. No full Order 1 requirement is retired merely
because this increment passes.

## Shadow pilot and mailbox

Pilot task `v023-order1-expression-call-facts` has four local cases with prefix
`v023-20261004-expr-call-`. They are `triage`, `type-claim`, `artifact-claim` and `clippy-claim`.
The diagnostic is the original wrong call verdict. Claims describe the final executed checks.
All labels are non-independent self-reviews. No accuracy or development improvement is claimed.
Later diagnostics were omitted under the first-diagnostic convention. The Clippy claim uses a
reviewed excerpt with the workspace path removed and retains the original log as provenance.

External evaluation remains pending the operator answer to the existing specific approval
question. Automatic approval review previously rejected sending excerpts to `api.typesafe.ai`
because it did not recognize destination-and-payload authorization. Do not retry through another
route. Local capture and implementation remain available. The task outcome resolves this
increment only and explicitly leaves full Order 1 active. The reviewed local export is
`tmp/jev-shadow-order1-expr-call-checkpoint.json`. The pilot implementation line owns the tracked
pilot checkpoint, which was not overwritten here.

The interline inbox is empty. The tracked checkpoint was exported and reviewed. It contains
four routes and no messages. No interline messages were sent.
