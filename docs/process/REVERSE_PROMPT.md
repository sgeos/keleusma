# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
Production type rejection is now integrated. Order 1 remains incomplete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `e6822599`.
The preceding status-only turn made no implementation progress. This turn advances the
implementation and its verification. The legacy handoff remains invalid-and-stale, twenty
commits behind the starting tree. Ancestry and fingerprint 0x4327_63E1 pass, while its
unchanged-stage claim fails. Historical full gates were not rerun.
Appendix B was read before tracked writes. The pre-existing `nvim.log` remains untouched.
No push, merge or publication occurred. Bytecode version remains 2 with 66 public opcodes.
Selected type caps remain 640. The concurrent V0.3.X line remains untouched.

## Implementation

`src/selfhost/typecheck.rs` is the production transport for scoped source facts. It compiles
`verify_types.kel` for bootstrap without compiling user source through the reference first.
The compiled stage layout supplies shared-slot ranges and capacities. Stage functions supply
verdict constants and the step budget. Batches use fresh owned arenas and zeroed shared buffers.
Name interning is deterministic. The host encodes facts and the stage resolves their types.

`type_check_source_via_kel` returns false only for explicit parser source diagnostics or a
stage rejection. Parser limitations, capacity, protocol errors, execution failures and extraction
panics remain errors. The shipping `self_hosted_compile` obtains this verdict before the reference
can reject the source. `PipelineRejected` suppresses the backend retry hint. Unavailable type
evidence remains distinct, while known unsupported syntax retains construct-specific messages.
Successful checking still reaches the reference gate because composite coverage is incomplete.
All twelve stage sources are now embedded through `read_stage`.

The production corpus checks twenty invalid sources and seven valid controls. A separate
sixteen-cell rule-versus-form census has eight valid controls. Every negative also asserts the
shipping entry returns `PipelineRejected`. All twelve real stages pass both type transports.
Capacity checks cover 640 and 641 declarations, sites, bindings and combined expression/argument
rows. Eight malformed transport cases return protocol errors. Repeated calls and source-head
batches have isolation controls. Unsupported parser patterns remain unavailable evidence.

The command-line tests assert three independent source rejections without artifact output or a
retry hint. A valid source emits the production library artifact and execution returns seven.
An initial command probe compared the two compiler flags and found differing artifact sizes.
Inspection showed the Rust command adds native declarations through its preamble and the
self-hosted command does not. The maintained test compares identical effective source inputs.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. No test subject was edited during a live run.

- `cargo test --release --features self-host --test selfhost_typecheck --test self_hosted_backend --test selfhost_type_transport`
  passed 103 type tests, eight backend tests and four transport tests. Terminal exit zero.
  Log `tmp/order1-production-types-suites2.log`.
- `cargo test --release --features self-host --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test selfhost_repeat_compile --test selfhost_typed_opcode_boundary --test selfhost_parse`
  passed six artifact tests, three monomorphization tests, one repeated-compilation test,
  seven opcode-boundary tests and 91 parser tests with two ignored. Terminal exit zero.
  All twelve complete stage artifacts match after production type integration.
  Log `tmp/order1-production-types-integration.log`.
- `cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible`
  passed four command refusal/artifact tests and two reproducibility tests. Terminal exit zero.
  Log `tmp/order1-production-types-cli-tests1.log`.
- `cargo test --test selfhost_typecheck --test selfhost_type_transport` passed thirty default
  type tests. The feature-gated transport binary executes zero tests in this configuration.
  Log `tmp/order1-production-types-default.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed actual
  crate checking and passed, including after the inventory-guard change. Logs
  `tmp/order1-production-types-clippy.log` and `tmp/order1-production-types-clippy-final.log`.
- `cargo clippy -p keleusma-cli --tests -- -D warnings` checked the command-line crate and
  passed. Log `tmp/order1-production-types-cli-clippy.log`.
- `cargo check --no-default-features --features compile,verify` passed the no_std build check.
  Log `tmp/order1-production-types-nostd.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed.
  Log `tmp/order1-production-types-doc.log`.
- `cargo test --release --features self-host --lib construct_naming_guard` passed three tests,
  including the expanded twelve-stage naming inventory. Log `tmp/order1-production-types-naming.log`.
- Documentation, citation and count guards, formatting and diff checks are recorded in
  `tmp/order1-production-types-final-checks2.log`.

The first baseline diagnostic is `tmp/order1-production-types-before.log`, showing reference
rejection masking the missing production stage. The first implementation compilation failed
for five undocumented public fields in `tmp/order1-production-types-after1.log`. The first
backend run required updating an undefined-name expectation to independent rejection, recorded
in `tmp/order1-production-types-backend1.log`. The first expanded suite failed to compile an
artifact comparison of Result values, corrected by explicitly requiring successful serialization.
That log is `tmp/order1-production-types-suites1.log`. The stage-count guard then detected the stale instruction that only eleven stages were embedded.
`CLAUDE.md` now records all twelve, and the guard compares the full directory inventory with the
embedded names. Its first failure is retained in `tmp/order1-production-types-final-checks.log`.
The unequal-effective-source command probe is retained in `tmp/order1-production-types-cli.log`. Later passing command tests supersede it.

The full workspace, advisory scan, refreshed inference-independence and final integration gates
remain unverified. Production artifact serialization still calls Rust `Module::to_bytes`.

## Next work

Continue the full completion plan. Composite inputs and results, unannotated composite aliases,
complete binder/occurrence coverage and general syntax admission remain incomplete. The legacy
occurrence channel is separate from scoped slot identities. The scalar solver retains bounded
rounds and a separate one-hop struct-alias path. Composite signature shapes beyond declared enums
remain open. Existing extraction helpers can still panic. The new wrapper classifies those as
unavailable rather than as source rejection but does not eliminate default panic-hook output.
The existing scratch compiler still temporarily mutates the process panic hook, a concurrency
concern that this increment does not resolve.

Complete the required subset type coverage and production Keleusma wire emission. Audit framing,
opcode records, operand pools, auxiliary regions and integrity-field provenance against the wire
plan. Refresh all-stage inference-independence evidence and run final gates. The command tests
now prove scoped type integration and artifact execution, not Keleusma serialization.

## Shadow pilot and mailbox

Task `v023-order1-production-types` captures the first diagnostic and completion claims with
case prefix `v023-20261004-production-types-`. Labels are non-independent self-reviews and
justify no independent accuracy or development-improvement claim. Later diagnostics remain in
engineering logs under the first-diagnostic sampling convention. Usage and elapsed time remain
null when unavailable. The bounded production integration is resolved while full Order 1 remains
active. The reviewed export is `tmp/order1-production-types-pilot-export.json`, kept ignored.
The pilot implementation line owns the tracked pilot checkpoint, which remains untouched.

External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to `api.typesafe.ai` because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture, review and development remain available.

The assigned interline inbox is empty. No messages were received or sent. The tracked checkpoint
was exported and reviewed with no communication changes. Check again before dependent work.
No verification process remains running at handoff.
