# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at 5b81bae4. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

The legacy HANDOFF.md is invalid and stale, measured 32 commits behind the starting head.
Ancestry passes and its unchanged-stage-source claim fails. Historical full validity and advisory
checks were not all repeated. Read Appendix B before tracked writes. Public bytecode version
remains 2, opcode count remains 66 and selected type-stage caps remain 640.

## Debug serialization

The dedicated wire_debug.kel stage owns stable bottom-up merge sorting and debug output order.
The host retains two index buffers. The stage supplies read positions, compares semantic record
headers and operand windows, chooses each copy and determines merge boundaries and generations.
The host does not sort records or call DebugPool::encode. The sort driver has a checked request
budget of one plus record count times merge rounds. Equal keys choose the left record.

The encoding state machine chooses strings, spans, types and records in format order. It emits
all count and length prefixes, integer fields and record headers. The host supplies data at
requested indices and offsets. Raw strings and opaque type blobs use 4,096-byte windows.
Operand output uses 1,024-word windows and comparisons use 512 pairs. Pools and operand lists
can cross windows. Operand counts beyond the sixteen-bit wire field are refused. Data sub-pools
retain their original indices. Debug metadata generation itself is unchanged.

Wire commands 207 and 208 derive chunk debug offsets from actual encoded lengths. They receive
presence and length, not host-computed positions. Absent pools retain the sentinel; present
empty DebugPool values encode to sixteen bytes. Debug chunks concatenate without padding between
them. The DEBUG_POOL region occupies its reference position after CHUNKS and before STRING_POOL.

The host is trusted to replay the requested semantic data and copy index entries faithfully.
The stage does not authenticate host memory. Per-call loops are bounded; host storage for sort
indices is linear in record count. No throughput benchmark is claimed.

## Inventory and capacity

There are now thirteen embedded stage sources. The complete artifact, monomorphization,
inference-independence, framing, region and type-input inventories include wire_debug. The
claimed-count guard derives its final embedding phrase from the actual count rather than
hardcoding twelve. The debug stage's two validation helpers return bool, widening the previous
Word-only result surface. Parameters remain Word. The type-surface census checks those exact sets.

Wire has 545 chunks, highest command 208 and a 149,516-byte artifact. Its input is 23,166 bytes
of 49,152, with 668 name events. Its 1,300 constant nodes leave 65 below 1,365, retaining the
64-node reserve. The reset function reuses the existing literal helpers to preserve that margin.
Parse remains the name maximum at 776. No existing buffer cap increased.

The debug stage has 20 chunks, 49 input name events, a 1,380-byte module-input blob and an
11,236-byte artifact. Its fifteen sort fields occupy 480 private bytes, and its nine encoding
fields occupy 288. The two new wire chunk fields add 64 bytes, giving that block seven slots
or 224 bytes. The accounting regression compiles the actual declarations from both stages.

## Verification

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage checks use
KEL_SELFHOST_CACHE zero. No verification subject is edited during a live run.

- Five debug integration tests pass in tmp/order1-debug-integration2.log. They cover all twelve
  record kinds, canonical ordering, duplicate keys, merge boundaries, multiwindow data, the
  operand-count limit, compiled debug artifacts and absent versus empty chunk spans. The same
  command passes the node-budget and widened type-surface guards. Command cargo test --release
  --features self-host --test selfhost_wire_debug --test module_input_node_budget --test
  corpus_type_surface --no-fail-fast -- --nocapture. Session 96806 exited zero.
- Three debug protocol tests pass in tmp/order1-debug-protocol.log. The direct sort comparison
  checks stable indices for 1,030 records, including duplicates. Command cargo test --release
  --features self-host --lib selfhost::wire_debug -- --nocapture. Session 92838 exited zero.
- Seventeen wire protocol tests pass in tmp/order1-debug-all-protocol.log, including debug-span
  overflow, sentinel exclusion and stream completion. Command cargo test --release --features
  self-host --lib selfhost::wire_ -- --nocapture. Session 80327 exited zero.
- The earlier corpus run passed all thirteen complete pipeline artifacts, all three
  monomorphization checks and all seven framing tests. It failed only a stale byte-size pin and
  the embedding phrase hardcoded to twelve. Both guards were corrected before the final run.
  Log tmp/order1-debug-corpus.log. Session 46049 exited 101.
- Four inference tests pass in tmp/order1-debug-inference.log. Clearing 43,831 expression types
  preserves all thirteen artifacts. Command cargo test --release --features self-host --lib
  compiler::inference_independence -- --nocapture. The command exited zero.
- Seven focused no-default checks pass in tmp/order1-debug-no-default.log. Command cargo test
  --release --no-default-features --features self-host --test selfhost_wire_debug --test
  wire_private_state_accounting --test corpus_type_surface --no-fail-fast. Session 77278 exited zero.

- The release workspace diagnostic completed with 3,073 passes, four failures and five ignored
  across 168 suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-debug-workspace.log. Session 29502 exited 101. All failures were stale inventories,
  two expecting twelve stages, one expecting eleven, and one omitting wire_debug.
- The repaired type, wire and round-trip suites pass 290 tests. Command cargo test --release
  --workspace --test selfhost_typecheck --test selfhost_wire --test wire_corpus --no-fail-fast.
  Log tmp/order1-debug-repaired.log. Session 10952 exited zero. The round-trip corpus now also
  includes wire and verify_types, whose old exclusions incorrectly claimed they were not embedded.
- Workspace Clippy performed actual Checking and passed after the repairs. Command cargo clippy
  --workspace --tests -- -D warnings. Log tmp/order1-debug-final-clippy2.log. Session 14293
  exited zero. The earlier production lint also passed in tmp/order1-debug-final-clippy.log.
- Documentation passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc --workspace
  --no-deps. Log tmp/order1-debug-doc.log. Session 16791 exited zero.
- The no_std self-host check performed actual Checking and passed. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-debug-nostd.log. Session 97230
  exited zero.

No production code changed after the full workspace run. Only inventory tests and documentation
were repaired afterward. The full workspace was not repeated after those repairs. The four
documentation guard suites pass 24 tests in tmp/order1-debug-doc-guards.log. Command cargo test
--release --test documentation_links --test claimed_counts --test comment_citations --test
push_order_claims. Session 69068 exited zero. Formatting and diff checks pass. The guards are
repeated after this receipt update. No verification process remains running at final handoff.

The first stage parse failed because error() is reserved checked syntax. Renaming that helper
fixed it. Later diagnostics identified a one-node reserve shortfall, Bool versus bool in the
census expectation and a tuple-return mismatch in a test fixture. Logs remain under
tmp/order1-debug-. No standing bound or coverage guard was weakened. Separate feature matrices,
minimum supported toolchains, Miri, cross-target builds, advisory scanning and final integration
gates remain required after the remaining implementation.

## Next work

Complete composite and named constants, private composite slots and nonempty non-elided
initializer pools. The auxiliary helper still refuses those forms. Constant-name interning
must preserve breadth-first ordering across all roots, including initializer contributors.
The older flattener demonstrates the ordering, but its window and provenance limits remain.

Complete semantic decoding and connect artifact output to the shipping tool while preserving
all supported behavior. Shipping unsigned output still calls Module::to_bytes. Legacy
wire_windowed_via_kel borrows reference positions and can skip unknown regions. Envelope checks
are not semantic decoding. Retain the completion plan's remaining type obligations, including
projection identity, nested headers, binders, occurrences, nominal layout, aliases, empty-array
inference, mixed tuples, casts and enum-dispatch capacity. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-debug retains the first parser diagnostic locally. Later diagnostics belong to
this same increment. Completion claims, blind labels and export review are recorded below.
External evaluation remains unavailable under the prior automatic approval rejection for the
external destination and payload. Do not retry or bypass it. Self-labels are non-independent.
Usage and elapsed time remain null when unavailable. The pilot implementation line owns its
tracked checkpoint. Source references stay local, so claim excerpts include decisive assertions.

Four local samples are complete for this increment, the parser diagnostic and three bounded
claims for stable sorting, chunk spans and window boundaries. Blind exports preceded self-labels.
The exact request payloads and all four request and source hashes were reviewed in
tmp/order1-debug-pilot-export.json. The bounded increment outcome is recorded as resolved.
Full Order 1 remains active. Missing usage and elapsed time remain null. No independent accuracy
claim is supported.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
The mailbox checkpoint was exported, reviewed and has no tracked difference.
Publication requires explicit in-session confirmation.
