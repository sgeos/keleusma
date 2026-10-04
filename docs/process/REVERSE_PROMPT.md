# Reverse Prompt

> **Navigation** [Process](./README.md) | [Documentation Root](../README.md)

## Active goal

Complete roadmap Order 1 on V0.2.X and participate in the System 1 shadow pilot.
The full objective remains active under
[ORDER1_COMPLETION_PLAN.md](../decisions/ORDER1_COMPLETION_PLAN.md).
This increment repairs workspace failures and refreshes inference-independence evidence.
Order 1 is not complete.

## Current state

2026-10-04. Branch `feat/v02-order1-next`, starting from `ace23d6c`.
The previous status turn collected the terminal workspace failure result, which established
additional work. This turn implements and verifies the repairs. The legacy handoff remains
invalid-and-stale, twenty-seven commits behind the starting tree. Its ancestry check passes
and its unchanged-stage claim fails. Historical full handoff gates were not repeated.
Appendix B was read before tracked writes. The pre-existing nvim.log remains untouched.
Bytecode version remains 2, public opcode count 66 and selected type caps 640.
The concurrent V0.3.X worktree remains untouched. No push, merge or publication occurred.

## Implementation and evidence

A private compiler hook runs after authoritative expression recording and before emission.
Public compilation supplies a no-op. Four tests inventory all twelve sources, erase populated
tables, compare complete artifacts, poison a consulted array type and retain invalid-source
admission. The current measurement clears 40,824 records without changing any stage artifact.
This establishes recorded-table independence for this corpus, not removal of admission or
full-language independence. The earlier shadow sample measured 40,782 before the parser repair.

Parser diagnostics now stop unsupported expression blocks and non-range iteration before
interior names become false invalid-source verdicts. Destructuring lets and unsupported
identifier-led declarations also stop explicitly. The shipping driver recognizes the new
unsupported codes. Six named examples are reference-valid but unsupported, including the signed
example whose flag the existing assembler does not preserve. Six minimal unsupported controls,
four supported artifact controls and an independent undefined-name rejection accompany them.
Finite admission coverage remains distinct from a complete syntax proof.

The original workspace run had eight failures across seven targets. Forward data tests now
require structured unsupported diagnostics and a shipping backward-order control. The opcode
inventory covers twelve sources and reports fifteen unexercised internal tags. Adding the named
examples leaves three. A separate consistency check initially retained the old four-tag set.
It now agrees with the census while retaining the boundary corpus checked-negation witness.

The test driver explicitly discards composite signature metadata within its historical scalar
scaffold. Shipping signature metadata is retained. The textual arm guard does not prove semantic
parity. Wildcard stack tests exercise the exact record cap and one past it. Public-document
scanning excludes private state and peer worktrees while retaining translation and specification
fixtures. The wire capacity audit now inventories all twelve stages. Parse uses 776 of 1,024
names. Wire uses 45,297 of 49,152 input bytes. The remaining byte margin is 3,855. No cap increased.

## Verification

Cargo commands used RUSTC_WRAPPER empty and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one.
Stage runs used KEL_SELFHOST_CACHE zero. Verification subjects remained unchanged during live
runs. The final workspace run covers the final production and test sources. Subsequent changes
update the process documents and have a separate documentation guard run.

- `cargo test --release --workspace --no-fail-fast` passed 3,031 tests, zero failures and five
  ignored across 162 reported suites. Terminal session 95187 exited zero. Complete log
  `tmp/order1-workspace-admission-final-workspace2.log`. Workspace features include shell,
  signatures, encryption and self-host through the command-line dependency. All twelve complete
  artifact comparisons, monomorphization controls, parser, type and code-generation suites ran.
- `cargo test --release --lib compiler::inference_independence -- --nocapture` passed four tests.
  Terminal exit zero. Log `tmp/order1-inference-independence-final.log`.
- `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` performed actual
  checking and passed after the final test edit. Log `tmp/order1-workspace-admission-final-clippy2.log`.
- `cargo check --no-default-features --features compile,verify` performed actual checking and
  passed. Log `tmp/order1-workspace-admission-final-nostd.log`.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passed. Log
  `tmp/order1-workspace-admission-final-doc.log`.

- Final documentation links, claimed counts, comment citations and public-document guards
  passed 24 tests. Log `tmp/order1-workspace-admission-final-doc-guards.log`. Formatting and
  diff checks passed. The final handoff receipt receives the same documentation guard check.

Diagnostic history remains under the inference-independence and workspace-admission prefixes.
The original workspace log records 3,019 passes and eight failures. The first admission comparison
has two failures. After1 exposed mistaken test expectations for Unsupported versus unavailable
and the previously silently parsed signed modifier. After2 passed 573 tests and failed one census
expectation. The first final workspace log passed 3,030 tests and failed its dependent consistency
check. The final-workspace2 result supersedes those diagnostic runs.

The advisory scan, all separate feature configurations, minimum supported Rust versions, Miri,
cross-target checks and final integration gates have not been re-established for this increment.
No verification process remains running at handoff.

## Next work

Continue the complete Order 1 plan. Production artifact serialization still uses Rust
Module::to_bytes. The next wire integration must derive layout from module facts rather than
borrow a reference artifact. wire_windowed_via_kel currently takes artifact length and region
positions supplied by its caller. wire.kel already has directory and layout helpers, but its
seal_artifact command checksums one 65,536-byte window. Both parse and wire artifacts exceed that
window. Add bounded incremental checksum and framing integration while preserving the full
encoder and decoder obligations. highest_command currently returns 187. Internal commands do
not authorize public opcode or bytecode-version changes. Measure the remaining input margin.

Remaining type obligations include general tuple projection identity, arbitrary nested header
syntax, complete binder and occurrence coverage, nominal struct signature layout, the one-hop
struct alias path, empty-array inference, mixed-scalar tuple layout, general cast admission and
large enum-dispatch capacities. Preserve existing supported behavior. Do not infer complete
coverage from the current finite comparisons. The reference remains the acceptance gate.

## Shadow pilot and mailbox

Tasks v023-order1-inference-independence and v023-order1-workspace-admission each retain one
initial diagnostic and three bounded claims. Blind exports precede non-independent self-labels.
The first identity sample retains its original measurement and the task outcome records the
later 40,824-entry result. Usage and elapsed time remain null where unavailable. No independent
accuracy or development-improvement claim is justified. Local record schema errors were corrected
before recording the affected packets. The reviewed export is
`tmp/order1-workspace-admission-pilot-export.json`, kept ignored.

External evaluation remains pending the existing specific authorization request. Automatic
approval review rejected sending excerpts to api.typesafe.ai because it did not recognize
explicit destination-and-payload authorization. No retry or bypass occurred. Local capture
continues. The pilot implementation line owns the tracked pilot checkpoint, which is untouched.

The assigned inbox is empty. No messages were received or sent. The tracked mailbox checkpoint
is exported and reviewed before handoff. The full Order 1 goal remains active.
