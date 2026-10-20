# Reverse prompt

## Handoff to Opus 5.5 on 2026-10-20

Line v0.2.3, branch feat/v02-order3-trap-analysis. Implementation and audit repairs
are committed and pushed at 53b88f52e8b762dfdf98cce73a8f6c9b2c326359.
The operator requested this handoff and expects that Opus may do little more
than audit work. Begin by auditing the existing implementation and evidence.
Treat that expectation as a likely work profile, not a claim of completion or a
resolution of outstanding requirements.

The Order 3 goal remains paused and incomplete. This preparation adds handoff
documentation only. No new implementation or tests were started.
ORDER3_COMPLETION_PLAN.md remains the completion scope, with contracts in
ORDER3_BOUNDS_CONTRACTS.md, ORDER3_ARITHMETIC_CONTRACTS.md,
ORDER3_TRAP_CLASSIFICATION.md, ORDER3_HOST_POLICY.md and ORDER3_PROVEN_EXECUTION.md.
First-class compiler-managed ring buffers remain requested under backlog B42,
with design and scheduling open.

## Preserved implementation and audit repairs

The committed baseline implements total first-pass arithmetic and scalar bounds
outcomes, both compiler lowerings, Rust and Keleusma trap verdicts, persistent host
trap policy and proof-gated operand-stack check elision. The shipping runner and
validator expose strict trap policy and the runner exposes proven execution.
Unproved type, storage, lifetime and host-contract guards remain active.

Commit 53b88f52 contains the subsequent audit repairs.

- Metadata-only wire commands use a names-only scan with interning. The scan cap
  is 4915, derived from the existing 49152-byte input window. Raw-node storage
  remains capped at 1365 and forest storage at 1536.
- Windowed assembly validates region extents before skipping empty regions.
- The command sweep resets after each tolerated execution error.
- Div and Mod benchmark patterns discard all three returned operands.
- Parser fixtures execute newly supported checked arithmetic and retain the
  current refusal for unsupported tuple destructuring.
- Source counts and the InvalidBytecode census match the inspected population.
- Indexed-write boundaries and malformed index controls cover checked and proven
  execution. Malformed flat-array stride retains its runtime guard in both modes.

## Existing verification receipts

All receipts below live under ignored tmp/order3-audit/. They are local evidence
and do not travel with this commit. No new test run was started for this snapshot.

The full matrix ran on baseline 09e74c94, before these repairs.

| Configuration | Passed | Failed | Skipped |
|---|---|---|---|
| Workspace | 3410 | 17 | 2 |
| No default features | 328 | 1 | 0 |
| Signatures | 2430 | 4 | 0 |
| Signatures and shell | 2447 | 4 | 0 |
| Self-host | 3152 | 16 | 2 |

matrix-status.json records commands, exit codes and baseline identity. All five
doctest runs passed. gates-status.json records the supporting build, minimum Rust,
cross-target, detached compiler, documentation, book, shell, language-server and
WebAssembly host checks. Those supporting receipts precede the current repairs.

The repair receipts are as follows.

| Receipt | Result |
|---|---|
| repairs-final-targeted.log | 54 passed, 179 skipped |
| wire-full-repair.log | 186 passed, zero skipped |
| bench-repair.log | Six benchmark tests passed |
| repair-clippy.log | Workspace all-target Clippy completed successfully with actual Checking output |
| new-boundary-controls.log | 21 selected boundary controls passed |

The Clippy repair invocation used self-host, signatures and encryption. The full
five-configuration matrix has not been rerun after the repairs. Initial failed
repair logs remain available, including the missing-interner failure corrected
before the passing wire run. No final Order 3 completion claim is supported.

Miri receipts on the baseline runtime include eleven proven-execution integration
controls in miri-integration.log, the arena suite in miri-arena.log and the null
text-pointer control in miri-c1.log. They do not establish memory verification of
every subsequent test or wire change. The refreshed dependency audit is recorded
in dependency-audit2.log. The WebAssembly target build is in wasm-target.log.

## Audit priorities for Opus

1. Resolve source-level indexed-write handler scope. An earlier clarification
   received no answer. Bytecode writes expose handled outcomes, but source
   assignments have no checked-write handler syntax. The earlier working
   assumption included source handlers. Do not silently narrow that scope.
2. Rerun the complete feature matrix and relevant final gates on a fixed tree,
   preserving earlier logs. Include required detached and cross-target coverage.
3. Audit every acceptance row in ORDER3_COMPLETION_PLAN.md against code, artifacts
   and terminal results. Complete the explicit completion audit before declaring
   Order 3 complete.

## Audit map and reproduction

| Area | Implementation | Primary controls |
|---|---|---|
| Total operations and explicit traps | src/vm.rs, src/compiler.rs, src/selfhost/kel/parse.kel and codegen.kel | tests/runtime_fault_census.rs, trap_lowering.rs, indexed_read_outcomes.rs, indexed_write_outcomes.rs and data_read_outcomes.rs |
| Trap classification | src/trap_analysis.rs, src/selfhost/traps.rs, src/selfhost/kel/verify_structural.kel | tests/trap_analysis.rs |
| Host policy and proof lifetime | src/vm.rs | tests/trap_policy.rs and proven_execution.rs, plus library proof_elision controls |
| Actual operand-check removal | src/verify_execution.rs and src/vm.rs | tests/proven_execution.rs and library proof_elision controls |
| Shipping commands | keleusma-cli | keleusma-cli/tests/proven_execution.rs |
| Latest wire repairs | src/selfhost/kel/wire.kel and src/selfhost/mod.rs | tests/selfhost_wire.rs, selfhost_region_coverage.rs and module_input_node_budget.rs |

Audit the unsafe operand access against the minimum-depth proof, actual control
flow, reset and yield entry protocols, partial errors, unwinding and replacement.
Check every retained guard before extending the claim beyond the admitted profile.
A completed trap-free verdict is not an execution certificate. Some older roadmap
prose describes a broader final design than the implemented first-pass profile.
Use the explicit acceptance table and contracts, and record discrepancies.

The following reproduces the five feature configurations and separate doctests.
Use a new receipt directory, capture full output and each exit status, and record
the exact commit and any diff. Do not edit a verification subject during its run.
These commands are instructions for the receiving audit and were not run for this
handoff. The old tmp/order3-audit/matrix.py overwrites baseline logs if reused
unchanged.

```sh
export CARGO_HUSKY_DONT_INSTALL_HOOKS=true RUSTC_WRAPPER= KEL_SELFHOST_CACHE=0
cargo nextest run --release --profile ci --no-fail-fast -j 4 --workspace
cargo test --release --doc --workspace
cargo nextest run --release --profile ci --no-fail-fast -j 4 -p keleusma --no-default-features
cargo test --release --doc -p keleusma --no-default-features
cargo nextest run --release --profile ci --no-fail-fast -j 4 -p keleusma --features signatures
cargo test --release --doc -p keleusma --features signatures
cargo nextest run --release --profile ci --no-fail-fast -j 4 -p keleusma --features signatures,shell
cargo test --release --doc -p keleusma --features signatures,shell
cargo nextest run --release --profile ci --no-fail-fast -j 4 -p keleusma --features self-host
cargo test --release --doc -p keleusma --features self-host
```

Consult .github/workflows/ci.yml and scripts/release-gate.sh for remaining gates,
including detached compiler, minimum Rust, embedded targets and memory checks.
Do not invoke release-gate.sh while another line may be testing in this checkout.
Its startup includes a broad pkill against target/debug/deps. Coordinate execution
or run the required commands individually without that process termination.

Runtime memory controls use Tree Borrows.

```sh
MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --no-default-features --features compile,verify --lib proof_elision
MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test --no-default-features --features compile,verify --test proven_execution
```

A cache permission error is an unavailable run, not a test failure or pass. Use
the normal approval path if required. Do not retry a rejected external pilot call.
No test or background job was launched by this handoff preparation. Prior recorded
verification runs are terminal. No current remote integration result is claimed.

## Constraints and handoff

BYTECODE_VERSION remains 2, FORMAT_FINGERPRINT remains 0x4327_63E1 throughout
V0.2.3-WIP, and the public instruction count remains 66. Selected type capacities
remain 640, verifier and trap opcode capacities 2048, verifier depth 32 and Rust
parser depth 24. V0.2.2 remains the last published release. No merge or publication
is part of this snapshot. Appendix B was read before tracked writes.

The former HANDOFF.md startup block was invalid-and-stale, measured at 73 commits
behind the implementation anchor. The new leading block is current and supplies
explicit ancestry, freshness and content checks. The old block remains below a
historical separator. Its historical verification is not current evidence.

The v0.2.3 inbox is checked and its tracked checkpoint exported and reviewed before
commit. No peer messages are sent. Local .codex/ settings and the empty nvim.log
remain unrelated and untracked. Feature-branch push uses the hook's documented
work-in-progress bypass. The final response reports the actual commit and push.

Pilot task v023-audit-09e7 has a recorded diagnostic, v023-audit-triage-09e7.
The task remains unresolved. Completion-claim sampling and further labels are
omitted for this administrative snapshot because the audit is paused and no
completion is claimed. External evaluation is not retried after the prior
automatic approval rejection. The shared tracked pilot checkpoint remains under
its existing ownership.
