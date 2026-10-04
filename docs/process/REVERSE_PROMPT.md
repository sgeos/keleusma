# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment closes measured match admission and loop scope gaps. It does not complete Order 1.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `3e97c81d`.
The previous goal turn made progress through implementation, verification and its commit.
This turn also makes implementation and verification progress. The legacy handoff remains
invalid-and-stale, nineteen commits behind the starting tree. Its unchanged-stage claim fails.
Appendix B was read before tracked writes. The pre-existing `nvim.log` remains untouched.
No push, merge or publication occurred. Bytecode version remains 2 with 66 public opcodes.
Selected type caps remain 640. The concurrent V0.3.X line remains untouched.

## Implementation

The parser checks supported match coverage using unguarded catch-alls or every declared enum
variant. Guarded arms do not count, including literal true guards. Variant rows carry one bit
per active match depth, so duplicates and nested matches cannot manufacture coverage. The
current pattern row is saved per depth. Diagnostic 23 reports missing coverage, 24 reports an
unknown or mismatched variant, and 25 reports the eight-level match depth limit. Only 23 and 24
are source errors. Boolean and variable patterns remain unsupported syntax with diagnostic 20.
The depth check occurs before any match-stack access.

Direct enum constructors and calls retain their declared enum scrutinee type at the drained
expression root. Unit enum construction must consume its following token before the match
boundary acts. Valid controls exposed both lost type information and this premature boundary.
Complete artifact comparison then exposed missing enum signature shapes. The driver now derives
flat enum parameter and return shapes from the declared payload size plus discriminant size.
Other composite signature layouts remain incomplete.

The parser saves lexical binding counts at loop entry and restores them after both bare and
bounded loop forms. Frame slots and the per-function named-binding counter remain monotonic.
Scoped type inputs derive Word induction-variable facts and Word bound requirements from the
reconstructed loop side tables.

Artifact comparison exposed a Rust reference defect. Its induction name remained visible after
the loop. The reference now scopes every loop form, including array iteration, and hides the
allocated induction name while compiling the upper bound. Both bounds therefore use the outer
scope, matching type checking, while slot and opcode allocation order remain stable.

## Verification

Cargo commands used `RUSTC_WRAPPER=` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`.
Stage runs used `KEL_SELFHOST_CACHE=0`. No test subject was edited during a live run.

- `cargo test --release --features self-host --test selfhost_artifact_identity --test selfhost_typecheck --test selfhost_parse --test selfhost_monomorphize_identity --test for_limit`
  passed 101 type tests, 91 parser tests with two ignored, six artifact tests, three
  monomorphization tests and 27 bounded-loop tests. Total executed 228. Log
  `tmp/order1-admission-final-suites.log`. All twelve complete stage artifacts match.
  Inventory, extraction and selected-cap checks passed.
- The match admission comparison covers 25 valid and invalid sources. The loop type and scope
  comparison covers twelve. The boundary test distinguishes unsupported Boolean/variable
  patterns from source errors and admits eight match levels while diagnosing nine as capacity.
- `loop_scope_and_match_admission_preserve_artifacts` compares complete artifacts and executes
  both compilers for twelve programs, with expected result seven. Eight cover loop scope and
  bounds, four cover direct or returned enum values and signatures. A separate default-feature
  regression covers six reference loop programs, including array iteration and outcome arms.
- `cargo test --release --features self-host --test selfhost_codegen self_host_compiles_`
  passed 89 selected tests. Log `tmp/order1-admission-final-codegen.log`.
- `cargo test --lib compiler::tests` passed 89 compiler tests.
  Log `tmp/order1-admission-reference-compiler.log`.
- `cargo test --test for_limit --test selfhost_typecheck` passed 27 loop and thirty default
  type tests. Log `tmp/order1-admission-default.log`.
- Broad Clippy with `--tests --features signatures,shell,self-host -- -D warnings` checked
  the crate and passed. Log `tmp/order1-admission-final-clippy.log`.
- Warnings-denied workspace documentation passed. Log `tmp/order1-admission-final-doc.log`.
- Documentation, citation and count guards, formatting and diff checks are recorded in
  `tmp/order1-admission-final-checks.log`.

Initial diagnostics remain in `tmp/order1-admission-before.log` and
`tmp/order1-loop-scope-before.log`. The reference scope disagreement is in
`tmp/order1-loop-artifacts-diagnostic.log`. The direct-enum false-rejection probes are in
`tmp/order1-admission-scrutinee-before.log` and the first after log. The enum signature
artifact disagreement is in `tmp/order1-admission-scrutinee-artifacts.log`, followed by a
passing second artifact log. One proposed data-array control initially violated the existing
immutable-private-data rule and was corrected to mutate the field. Failed broader runs are
retained as `tmp/order1-admission-suites1.log` and `tmp/order1-admission-suites2.log`.

These checks do not establish full workspace, advisory, inference-independence, shipping-command
or final integration gates. Both complete-artifact paths still serialize through Rust.
No verification process is left running at handoff.

## Next work

Continue the full completion plan. Composite inputs and results, unannotated composite aliases,
complete binder/occurrence coverage and general syntax admission remain incomplete. The legacy
occurrence channel is separate from scoped slot identities. The scalar solver retains bounded
rounds and a separate one-hop struct-alias path. The finite match corpus does not establish
all pattern or scrutinee forms. Composite signature shapes beyond declared enums remain open.

Integrate the independent type verdict into production, preserving the Rust differential oracle
without reference-first masking. The current `self_hosted_compile` still calls the reference
first. Keep source diagnostics, unsupported syntax, resource limits and execution failures
separate. Complete Keleusma artifact emission in place of `module.to_bytes()`, refresh the
all-stage inference-independence evidence, exercise the shipping command and run the final gates.

## Shadow pilot and mailbox

Task `v023-order1-admission-scope` records the first diagnostic and three completion claims,
with case prefix `v023-20261004-admission-scope-`. All labels are non-independent self-reviews.
No independent accuracy or development-improvement claim is justified. Later diagnostics remain
in the engineering logs under the first-diagnostic sampling convention. Usage and elapsed time
remain null when unavailable. The bounded admission increment is resolved, while full Order 1
remains active. The reviewed export is `tmp/order1-admission-pilot-export.json`, kept ignored.
The pilot implementation line owns the tracked pilot checkpoint, which remains untouched.

External evaluation remains pending the existing specific operator authorization request.
Automatic approval review rejected sending excerpts to `api.typesafe.ai` because it did not
recognize destination-and-payload authorization. Do not retry or bypass that rejection.
Local capture, review and development remain available.

The assigned interline inbox is empty. No messages were received or sent. The tracked checkpoint
was exported and reviewed with no communication changes. Check again before dependent work and
handoff, as required by the installed protocol.
