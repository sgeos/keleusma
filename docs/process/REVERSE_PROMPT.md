# Reverse prompt

## Work-in-progress snapshot on 2026-10-15

Line v0.2.3, branch feat/v02-order3-trap-analysis. This increment starts at
09e74c94d9a2c30a58daa9a6fa57439d41f8b9ef. The operator requested a note for
first-class compiler-managed ring buffers and a commit and push of all current
work. Backlog B42 records that requirement, with design and scheduling open.

The Order 3 goal is paused and incomplete. This snapshot does not resume it.
ORDER3_COMPLETION_PLAN.md remains the completion scope, with the implementation
contracts in ORDER3_BOUNDS_CONTRACTS.md, ORDER3_ARITHMETIC_CONTRACTS.md,
ORDER3_TRAP_CLASSIFICATION.md, ORDER3_HOST_POLICY.md and ORDER3_PROVEN_EXECUTION.md.

## Preserved implementation and audit repairs

The committed baseline implements total first-pass arithmetic and scalar bounds
outcomes, both compiler lowerings, Rust and Keleusma trap verdicts, persistent host
trap policy and proof-gated operand-stack check elision. The shipping runner and
validator expose strict trap policy and the runner exposes proven execution.
Unproved type, storage, lifetime and host-contract guards remain active.

This snapshot contains the subsequent audit repairs.

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

## Remaining work when the operator resumes

1. Resolve source-level indexed-write handler scope. An earlier clarification
   received no answer. Bytecode writes expose handled outcomes, but source
   assignments have no checked-write handler syntax. The earlier working
   assumption included source handlers. Do not silently narrow that scope.
2. Rerun the complete feature matrix and relevant final gates on a fixed tree,
   preserving earlier logs. Include required detached and cross-target coverage.
3. Audit every acceptance row in ORDER3_COMPLETION_PLAN.md against code, artifacts
   and terminal results. Complete the explicit completion audit before declaring
   Order 3 complete.

## Constraints and handoff

BYTECODE_VERSION remains 2, FORMAT_FINGERPRINT remains 0x4327_63E1 throughout
V0.2.3-WIP, and the public instruction count remains 66. Selected type capacities
remain 640, verifier and trap opcode capacities 2048, verifier depth 32 and Rust
parser depth 24. V0.2.2 remains the last published release. No merge or publication
is part of this snapshot. Appendix B was read before tracked writes.

Legacy HANDOFF.md is invalid-and-stale, measured at 72 commits behind the starting
head. Ancestry passes but its stage-count and unchanged-stage claims no longer
hold. Its historical verification is not current evidence.

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
