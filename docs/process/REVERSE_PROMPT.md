# Reverse prompt

## Proven execution on 2026-10-15

The active goal remains Complete Order 3, trap analysis. Work is on line v0.2.3
and branch feat/v02-order3-trap-analysis. This increment starts from pushed commit
765dcbb894b395fd928fd4be2675a3945eeefc43. The previous goal turn made progress
by enforcing host trap policy and repairing signed replacement.

The implementation frontier is now the full completion audit. Do not redesign the
implemented contracts or substitute a smaller goal. ORDER3_COMPLETION_PLAN.md is
authoritative, with the execution proof in ORDER3_PROVEN_EXECUTION.md.

## Implemented execution mode

ExecutionChecks::Checked is the default. Proven mode requires RequireTrapFree and
a private certificate for the actual immutable image and instruction cache.
It repeats required validation even on borrowed or trust-skip instances. It cannot
be enabled without verification support or retained while allowing traps.

src/verify_execution.rs implements a bounded minimum-depth worklist over actual
instruction edges. It requires a real Return operand, merges minima, follows loop
back edges and models Reset reentry after Stream with one resume input. This is
independent of the ordinary verifier's upper-bound walk, which remains unchanged.
The queue budget follows from finite nonnegative decreasing depths. Fall-through,
invalid targets, missing operands and incomplete analysis cannot grant a proof.

The VM removes pop and peek emptiness checks and count-underflow checks for PopN,
NewComposite and internal call arguments. Typed pointer access and proved
subtraction do actual work in the proven path. All unproved type, representation,
lifetime, storage, allocation, callee and other contract guards remain active.
An internal counter control requires unchecked pops and zero checked pops in proven
mode, with the opposite result in checked mode.

A runtime proof state distinguishes ready, prepared, yielded, reset, breakpoint,
finished and poisoned states. Preparation and dispatch poison state before fallible
work or user-value destruction. Success restores the exact suspension kind.
Wrong entry methods are rejected without consuming a valid suspension. Returned
errors and caught panics cannot resume a partial frame. Explicit reset establishes
freshness after successful reset. Enabling proven execution requires empty stacks
and no started frame.

Replacement validates and certifies its encoded candidate before mutation and
revokes the old proof at commit. Old private slots become Unit before invoking
host destructors, preventing a second drop after unwinding. A successful image,
cache, reset and constant-pool installation precedes assignment of the new proof.
Direct public function entry, mode configuration and replacement clear borrowed
shared-buffer bindings. Internal entry with a current buffer uses a private helper.
These boundaries prevent a pointer retained during unwinding from being reused.

## Shipping interface

The runner accepts --require-trap-free and --proven-execution. The latter also
selects strict trap policy. Source and serialized artifacts use the options.
Offline validate accepts --require-trap-free with rust or self-hosted backends,
including the reference consistency gate. Unavailable analysis remains distinct
from a completed rejection. A trap verdict alone cannot enable proven execution.
Proven execution and --print-memory are explicitly incompatible. Strict memory
reporting checks trap classification before reporting a bound.

Five command-line controls cover source acceptance and refusal, both compiler
artifacts, both validation backends, unavailable analysis, argument separators,
memory-reporting behavior and an empty Return that passes trap classification
but fails the stronger execution proof.

## Verification

All 1541 selected release tests pass with self-host, signatures and encryption,
with zero skips. This includes the new proof and policy cases, current stage
artifacts and resource admission, hostile modules, narrow runtimes and persistent
storage. The runtime implementation was fixed during this run. The eight-bit
integration control was added afterwards and passes separately.

The focused proof/policy suite passes 31 tests. The final proven-execution
integration binary passes twelve tests with signatures and encryption. The shipping
command-line binary passes five controls. Verification-disabled builds refuse
proven execution. Workspace all-target Clippy and the detached compiler build pass.
Formatting and diff checks pass.

Tree Borrows evidence is explicit and has three scopes.

- Ten integration tests passed in tmp/order3-elision-miri-integration2.log. This
  run preceded the final borrowed-buffer cleanup changes. It took about 33 minutes
  because the tests also compile their source fixtures under Miri.
- Nine library controls passed on the final runtime in
  tmp/order3-elision-miri-library.log. These include actual unchecked pops,
  cache/image mismatch, lower-depth counterexamples, dispatch unwinding,
  private-value destructor unwinding and borrowed-buffer recovery routes.
- The final eight-bit runtime control passed in tmp/order3-elision-miri-narrow.log.

The initial sandboxed Miri attempt could not create its local compiler sysroot.
The authorized elevated invocation built the cache and executed the tests.
Nightly, Miri and rust-src are installed. No memory-check process remains live.

Other receipts are tmp/order3-elision-acceptance.log,
 tmp/order3-elision-final-focused.log, tmp/order3-elision-final-integration.log,
 tmp/order3-elision-cli-final.log, tmp/order3-elision-no-verify.log,
 tmp/order3-elision-last-clippy.log and tmp/order3-elision-detached.log.
Initial compile failures are retained in first-check, focused and focused2 logs.
They concerned an integer-width comparison and test-only imports/construction.
The recursive proof prototype was replaced by the bounded graph proof before the
successful focused3 run. src/verify.rs was restored to its prior implementation.

Reproduction commands for the principal memory controls follow.

```sh
CARGO_HUSKY_DONT_INSTALL_HOOKS=true RUSTC_WRAPPER= MIRIFLAGS=-Zmiri-tree-borrows \
cargo +nightly miri test --no-default-features --features compile,verify \
  --lib proof_elision
CARGO_HUSKY_DONT_INSTALL_HOOKS=true RUSTC_WRAPPER= MIRIFLAGS=-Zmiri-tree-borrows \
cargo +nightly miri test --no-default-features --features compile,verify \
  --test proven_execution
```

## Next work and completion audit

Run the full five feature configurations and the full workspace and isolated
suites. Include the detached compiler's relevant tests, actual shipping commands,
compatibility checks, minimum Rust and cross-target checks required by the project's
gates. Complete final memory-safety evidence on the final implementation, including
rerunning the integration memory suite if claiming all memory evidence is from the
same final tree. The earlier ten-case receipt must not be silently relabelled.

Create the explicit requirement-by-requirement completion audit required by
ORDER3_COMPLETION_PLAN.md. Inspect actual code, artifacts and terminal results for
each row. Source assertions, selected counts and successful partial gates cannot
stand in for the full audit. Fix regressions found by broader coverage while
preserving the original total-operation, classification, policy and real-elision
requirements. Do not mark the goal complete until every required item is proved.

No full five-configuration matrix, complete release gate, complete detached tests
or final completion audit has run for the assembled Order 3 changes. That is now
the outstanding work, rather than another design increment.

## Constraints and handoff

BYTECODE_VERSION remains 2 and FORMAT_FINGERPRINT remains 0x4327_63E1 during
V0.2.3-WIP. Public instruction count remains 66. Selected type capacities remain
640, verifier and trap opcode capacities 2048, verifier depth 32 and reference
parser depth 24. V0.2.2 remains the last published release. Development artifacts
require rebuilding after the earlier stack-contract changes. No merge or
publication occurred. Appendix B was read before tracked writes. B42 retains
first-class compiler-managed ring buffers pending design and scheduling.

Legacy HANDOFF.md remains invalid-and-stale, measured at 71 commits behind this
increment's starting head. Its historical checks were not treated as current.

Pilot task v023-elision-765d records the first diagnostic and three completion
claims. All labels are non-independent self-review. The task outcome resolves this
implementation increment only. External evaluation was not retried after the earlier
automatic approval rejection. The reviewed export is
 tmp/order3-elision-pilot-checkpoint.json. This branch does not own the shared tracked
pilot checkpoint.

The v0.2.3 inbox is empty. Its tracked checkpoint is exported and reviewed before
commit. All verification processes are terminal. Local .codex/ settings and nvim.log
remain unrelated and untracked. No peer messages were sent. The concurrent V0.3.X
worktree was not modified. Feature-branch push authorization persists, using the
hook's documented work-in-progress bypass where appropriate.
