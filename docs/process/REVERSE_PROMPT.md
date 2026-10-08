# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at 075a1cff. Preserve unrelated
untracked .codex and nvim.log. The concurrent V0.3.X line remains separate. No push, merge or
publication occurred.

The legacy HANDOFF.md is invalid and stale, measured 33 commits behind the starting head.
Ancestry passes and its unchanged-stage-source claim fails. Its historical full verification
and advisory checks were not all repeated. Read Appendix B before tracked writes. Public bytecode
version remains 2, opcode count remains 66 and selected type-stage caps remain 640.

## Private data integration

The auxiliary helper now emits private composite placements through the existing Keleusma record
formatter. Slot numbers and persistent-memory offsets remain compiler-owned module facts. The
host passes those typed fields and the formatter packs their wire representation.

Commands 209 through 212 derive the private initializer record. The stage receives chunk root
counts, accumulates the first initializer index, then scans initializer tags and payloads in
batches of at most 512. Only a nonempty sequence of Int(0) is elided. Empty pools and all other
scalar zero forms remain distinct. The stage writes either the absent sentinel or the computed
root index and returns its elision decision. The host follows that result when supplying constant
values. This production path no longer calls the reference elision predicate or root selector.

The initializer range begins after chunk ROOTS. Descendants are emitted later in breadth-first
order, so their count does not change the root range. The legacy data_init_fields helper retains
its older comparison-only scope and is not used by the new auxiliary producer.

Two compiled controls preserve complete artifact bytes, decode and execute with the expected
results. Further controls cover exact typed-zero behavior, 512-value batch boundaries, 2,000-value
pools, cross-chunk ranges, no-chunk modules and full-width placement fields. The placement-width
control compares serialization; its maximal artificial offsets are not an executable layout.
Protocol controls reject wrong order, invalid counts and tags, incomplete streams and overflowing
ranges. A refused batch does not partially change the elision result or consume values.

Composite and named constant trees remain refused, including such initializer values. The current
helper still uses the scalar constant-record stream. Do not describe this as general tree encoding.

## Capacity

The wire stage has 551 chunks, command 212 and a 152,140-byte artifact. Its input uses 23,417
of 49,152 bytes, leaving 25,735. It contributes 681 name events. Parse remains the maximum at
776. Constant nodes use 1,301 of 1,365, leaving exactly the required 64-node reserve. Further
stage growth must recover node capacity or change the representation. The guard is unchanged.

Six initializer functions account for the six added chunks. Reset literals reuse existing helpers
to preserve the reserve. The initializer state uses seven private Value slots or 224 bytes.
No existing buffer cap increased. The thirteen-stage inventory includes wire_debug. Clearing
44,063 recorded expression types leaves all thirteen artifacts unchanged. No throughput benchmark
is claimed. The host remains trusted to transport its module facts faithfully.

## Verification

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage checks use
KEL_SELFHOST_CACHE zero. Verification subjects remain frozen until their processes are terminal.

- The first fixture failed compilation because it used the wrong Vm path. Log
  tmp/order1-private-before.log. Session 88721 exited 101 with zero tests executed.
- After that fixture repair, both integration cases reached the existing unsupported guards.
  Log tmp/order1-private-before2.log. Session 82714 exited 101. These establish the original gaps.
- The first stage implementation referred to misnamed initializer-offset helpers. The corrected
  implementation passed both execution controls but failed the standing node reserve. Logs
  tmp/order1-private-first.log and tmp/order1-private-second.log retain those diagnostics.
- Five private-data integration tests plus capacity and accounting pass in
  tmp/order1-private-targets.log. Command cargo test --release --features self-host --test
  selfhost_wire_private --test module_input_node_budget --test wire_private_state_accounting
  --no-fail-fast -- --nocapture. Session 32258 exited zero.
- Eighteen wire protocol tests pass in tmp/order1-private-protocol.log. Command cargo test
  --release --features self-host --lib selfhost::wire_ -- --nocapture. Session 41951 exited zero.
- The integrated run passed all thirteen complete artifact comparisons and every functional
  wire check. Its only failure was the old byte-size pin, corrected before final verification.
  Log tmp/order1-private-integrated.log. Session 21705 exited 101.
- Four inference tests pass in tmp/order1-private-inference.log. Command cargo test --release
  --features self-host --lib compiler::inference_independence -- --nocapture. It exited zero.
- Seven focused no-default checks pass in tmp/order1-private-no-default.log. Command cargo test
  --release --no-default-features --features self-host --test selfhost_wire_private --test
  module_input_node_budget --test wire_private_state_accounting --no-fail-fast. Session 69848
  exited zero.

- The final release workspace passes 3,083 tests, zero failures and five ignored across
  169 reported suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-private-workspace.log. Terminal session 85272 exited zero.
- Workspace Clippy performed actual Checking and passed. Command cargo clippy --workspace
  --tests -- -D warnings. Log tmp/order1-private-clippy.log. Session 38835 exited zero.
- Documentation passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc --workspace
  --no-deps. Log tmp/order1-private-doc.log. Session 5485 exited zero.
- The no_std self-host check performed actual Checking and passed. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-private-nostd.log. Session
  12367 exited zero.

No production or test code changed after the full workspace run. The four documentation guard
suites pass 24 tests in tmp/order1-private-doc-guards.log. Command cargo test --release --test
documentation_links --test claimed_counts --test comment_citations --test push_order_claims.
Session 16318 exited zero. Formatting and diff checks pass. The guards are repeated after this
receipt update. No verification process remains running at final handoff.

Separate feature matrices, minimum supported toolchains, Miri, cross-target builds, advisory
scanning and final integration gates remain required after the missing implementation.

## Next work

Complete composite and named constant trees, including private initializer trees. Preserve
breadth-first indexing and name interning across all root contributors. The existing flattener
has useful ordering controls, but its input-window and provenance constraints still need
integration. The new initializer-range protocol already accumulates roots rather than descendants.

Complete semantic decoding, then connect artifact output to the shipping tool while preserving
all supported behavior. Shipping unsigned output still uses Module::to_bytes. Envelope checks
are not semantic decoding. Retain the completion plan's remaining type obligations, including
projection identity, nested headers, binders, occurrences, nominal layout, aliases, empty-array
inference, mixed tuples, casts and enum-dispatch capacity. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-private retains the first fixture diagnostic. Later failures belong to this
same increment and remain in the logs. External evaluation remains unavailable under the prior
automatic approval rejection for the external destination and payload. Do not retry or bypass it.
Self-labels are non-independent, and unknown usage and elapsed time remain null. The pilot
implementation line owns its tracked checkpoint. Source references stay local, so excerpts
must include their decisive assertions.

Four samples are complete, the fixture diagnostic and three bounded claims for decoded
execution, typed-zero elision and rejected batches. Blind exports preceded self-labels. All four
request hashes, exact payloads and source hashes were reviewed in
tmp/order1-private-pilot-export.json. The bounded increment outcome is recorded as resolved.
Full Order 1 remains active. Missing usage and elapsed time remain null. No independent accuracy
claim is supported.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
The mailbox checkpoint was exported, reviewed and has no tracked difference.
Publication requires explicit in-session confirmation.
