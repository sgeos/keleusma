# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at cd585078. Preserve the unrelated
untracked .codex directory and nvim.log. No push, merge or publication is authorized by this
handoff. The concurrent V0.3.X workstream remains separate.

The legacy HANDOFF.md is invalid and stale, measured 30 commits behind the starting head.
Ancestry passes and its unchanged-stage-source claim fails. The historical advisory and full
validity checks were not all repeated. Read Appendix B before tracked writes. Public bytecode
version remains 2, the opcode count remains 66 and selected type-stage caps remain 640.

## Native metadata integration

The module-driven auxiliary helper now emits native names and return shapes. Commands 201 and
202 configure an optional native-name section appended to the existing input blob and emit name
records. Existing callers retain the original blob grammar unless explicitly configured. The
stage interns native names after enum names and before data-slot names. The host supplies names,
not wire indices. Allocation size is checked before extending the native input.

Commands 203 and 204 continue the signature-shape interner for native returns. The host transports
semantic shapes and replays previously emitted records. It never supplies their derived indices.
Native-name and native-return vectors retain independent lengths. Tests cover empty vectors,
both unequal-length directions, repeated shapes and reuse of existing chunk parameter shapes.
Real compiled scalar and array-returning native calls match complete reference artifacts.

Native collisions exposed bounds that used the deduplicated name count to constrain input
positions. Slot bounds now use the input run count. Enum cursors use their input section end.
The interner checks its 16,384-byte output pool before copying. The constant walker initially
hid that new refusal and returned a positive count. Its regression failed with 64 instead of
minus 255. The wrapper now propagates the interner error, and the regression passes.

Incomplete name sections, invalid ranges and native indices are refused. Native-return protocol
checks require completed interning before record output. Framing excludes all four new commands.
The host is trusted to transport its own module facts and emitted shape bytes faithfully. These
are encoder checks. Semantic artifact decoding is still an open obligation.

## Capacity and memory

Wire has 528 chunks, highest command 204 and a 144,396-byte artifact. Its module input uses
48,492 of 49,152 bytes, leaving 660. It contributes 642 name events. Parse remains the maximum
at 776 of 1,024. The constant table uses 1,291 of 1,365 nodes, leaving 74 and retaining the
standing 64-node reserve. No buffer cap increased.

The first native implementation used 1,317 nodes and left 48. Sharing 32 numeric accessors
through nine literal helpers, six newly added, removes 26 duplicate nodes. Six native functions
and the six helpers account for the twelve added chunks. These aliases add one fixed call level.
The source offset comparison still permits exactly one alias hop. Throughput is unbenchmarked.

Four native-name fields add 128 private bytes. The signature mode field adds 32 bytes. Signature
state now occupies twelve value slots or 384 bytes, and the complete name-interner block occupies
36 slots or 1,152 bytes. The accounting regression compiles the actual declarations. Framing
remains 128 bytes and opcode state remains 256 bytes. Each private Value slot is 32 bytes.

## Verification

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage checks use
KEL_SELFHOST_CACHE zero. Verification subjects remain frozen until each process is terminal.

- Five native integration tests and the node reserve pass in tmp/order1-native-final-targets.log.
  Command cargo test --release --features self-host --test selfhost_wire_natives --test
  module_input_node_budget -- --nocapture. Terminal session 88454 exited zero.
- Nine wire protocol tests passed in tmp/order1-native-protocol.log. Command cargo test --release
  --features self-host --lib selfhost::wire_ -- --nocapture. Terminal session 17532 exited zero.
  This preceded the later constant-walker error propagation and host allocation preflight.
- The constant-walker regression fails before the repair and passes afterward. Logs
  tmp/order1-native-pool-regression-before.log and tmp/order1-native-pool-regression-after.log.
  Command cargo test --release --features self-host --test selfhost_wire
  constant_walk_propagates_an_interner_pool_refusal -- --nocapture. Sessions 98299 and 76730
  exited 101 and zero respectively. No diagnostic was erased.
- Four inference tests pass on the final production tree. Clearing 42,471 expression types
  preserves all twelve complete artifacts. Log tmp/order1-native-inference.log. Command cargo
  test --release --features self-host --lib compiler::inference_independence -- --nocapture.
  Terminal session 50625 exited zero.
- The earlier integrated run passed all twelve pipeline artifact comparisons and failed only
  the old module-input byte pin. Log tmp/order1-native-integrated.log. The pin and all three
  chunk-count pins were updated with measured values and causes before final verification.

- The fresh workspace diagnostic completed with 3,056 passes, one failure and five ignored
  across 166 suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-native-workspace.log. Terminal session 15581 exited 101. All production and
  native integration tests passed. The sole failure was the_fixtures_exist in the command-line
  bad-input suite. Several tests truncated the same good.kel in one shared temporary directory.
- The fixture helpers now own exclusively created directories, distinguish processes and
  instances, and clean up only their owned directory. A deterministic interleaving tests
  isolation and cleanup. No command-line production code changed. The first repair passed
  eight tests in tmp/order1-native-cli-repair.log, before simplifying the new isolation test
  to avoid dependence on thread scheduling. The final repair uses a checked compare_exchange loop within the fixed retry bound, avoiding
the atomic method deprecated by the current compiler. Final command-line and lint receipts follow.
- Clippy performed actual Checking and passed on production changes in
  tmp/order1-native-final-clippy.log. The fixture repair receives a separate workspace lint.
- The documentation build passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc
  --workspace --no-deps. Log tmp/order1-native-doc.log. Terminal session 24981 exited zero.
- The warm no_std compile-and-verify check passed. Command cargo check --no-default-features
  --features compile,verify. Log tmp/order1-native-nostd.log. Terminal session 84685 exited zero.

- All 74 command-line tests pass across eleven suites after the final fixture repair. Command
  cargo test --release -p keleusma-cli --tests --no-fail-fast. Log
  tmp/order1-native-cli-final2.log. Terminal session 44601 exited zero.
- Workspace Clippy performed actual Checking and passed after that repair. Command cargo clippy
  --workspace --tests -- -D warnings. Log tmp/order1-native-workspace-clippy2.log. Terminal
  session 57936 exited zero. The preceding workspace lint rejected a deprecated atomic method
  in the fixture helper, whose replacement uses the established compare_exchange interface.

The full workspace was not repeated after the test-only fixture repair and an artifact-test
comment correction. No production code changed after the full run. Formatting and diff checks
pass. The four documentation guard suites passed 24 tests in tmp/order1-native-doc-guards.log.
Command cargo test --release --test documentation_links --test claimed_counts --test
comment_citations --test push_order_claims. Terminal session 52171 exited zero. The guards are
repeated after this receipt update. No verification process is left running at final handoff.
Separate feature configurations, minimum supported Rust versions, Miri, cross-target builds,
advisory scanning and final integration gates remain required after implementation completion.

## Next work

Complete templates, debug pools, composite and named constants, private composite slots and
nonempty non-elided initializer pools. The new auxiliary helper still refuses those forms.
Inspection for the next increment found that SchemaBuilder::finish interns template names
AFTER the constant forest. Adding a template pool only accumulates it and does not intern names.
Template type names deduplicate and field names append fresh. DebugPool is structured data whose
encoder sorts records by operation index, kind and operands. Passing DebugPool::encode output
through a Keleusma byte copier would leave its serialization in Rust.

Finish semantic decoding, then connect complete artifact output to the shipping tool without
narrowing supported behavior. Shipping unsigned output still calls Module::to_bytes. Legacy
wire_windowed_via_kel borrows region positions and can skip unknown regions. Do not substitute
that path as independent production serialization. Envelope integrity is not semantic decoding.

Remaining type obligations include general tuple projection identity, arbitrary nested header
syntax, complete binder and occurrence coverage, nominal struct signature layout, one-hop
struct aliases, empty-array inference, mixed-scalar tuple layout, general cast admission and
large enum-dispatch capacities. Preserve reference admission and retain the full completion plan.
Refresh all-stage identity, monomorphization, inference independence, shipping comparisons and
final gates after the remaining work. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-native contains the first reserve diagnostic and three bounded claims for
native vector behavior, constant-walk refusal propagation and the twelve artifact comparisons.
The latter excerpts include the decisive assertions and actual-versus-reference call path.
Blind exports preceded non-independent labels. All four request hashes, exact input payloads
and source hashes were reviewed in tmp/order1-native-pilot-export.json. The bounded task outcome
is recorded as resolved. Full Order 1 remains active. Later diagnostics belong to this same
increment and remain in the logs. Usage and elapsed time are null rather than estimated.
External evaluation remains unavailable under the prior automatic approval rejection for the
external destination and payload. Do not retry or bypass it. Local labels are non-independent.
Source references are not included in the outbound evidence. Include decisive assertions and
thresholds in the excerpts themselves. The pilot implementation line owns its tracked checkpoint.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
The mailbox checkpoint was exported, reviewed and has no tracked difference. Publication requires explicit in-session confirmation.
