# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at b227ce3c. Preserve the unrelated
untracked .codex directory and nvim.log. The concurrent V0.3.X line remains separate.

The legacy HANDOFF.md is invalid and stale, measured 31 commits behind the starting head.
Ancestry passes and its unchanged-stage-source claim fails. Its historical full verification and
advisory checks were not all repeated. Read Appendix B before tracked writes. Public bytecode
version remains 2, opcode count remains 66 and selected type-stage caps remain 640.

## Implemented work

The auxiliary helper now emits struct templates from semantic type and field names. Commands
205 and 206 configure the optional input section and emit records. Template type names deduplicate,
field names append fresh, and a private checkpoint records the next name index after each type
without emitting a name. This preserves the first-field index for empty runs. Checkpoint behavior
is enabled only for configured template input. Native and template sections have independent
input ranges. Chunk template ranges remain derived by the existing stage counters.

Five integration controls compare auxiliary and complete artifact bytes. They cover a compiled
boxed struct whose large field comes from a native, cross-chunk template ranges, repeated names,
empty and Unicode names, zero-field runs and exact name-event capacity. A template checkpoint
consumes one input event, so the validator counts it. Invalid sections and calls after completion
are refused. Seven private template fields cost 224 bytes, verified from the actual declaration.

The constant-node reader previously returned counts above 1,024 without emitting any records,
because it reused the name-table clamp. The new regression failed on node zero for 1,025 nodes.
The repaired loop uses the node table's 1,365 limit. Both legacy and compact transports are checked
at zero, 1,024, 1,025 and 1,365 nodes, with every emitted field inspected. The next count is refused
before looping. Compact input also refuses named, composite and unknown tags.

Scalar-only module_input now marks bit 15 of the private count and carries a two-byte tag plus
an eight-byte payload. Four zero fields are reconstructed in Keleusma. Other forests keep the
legacy grammar. This does not change published artifacts. None and floating-point payloads now
reach the producer, including signed zero and a NaN payload. The module-input budget test masks
the flag and checks the compact byte extent against the actual module constant count.

The first all-stage comparison failed on unresolved name 510, identified by the lexical trace
as templates. Its private declaration followed a helper using it. The declaration now precedes
use, restoring parsing and artifact identity. No general forward-data support was added.
A no-default-features self-host build also found an existing unguarded Float enum variant in
the host tag decoder. It now uses ScalarKind::from_tag with checked integer conversion. Invalid
and feature-disabled tags reach the existing bad-tag panic guard. This internal decoder is not
a new fallible source interface.

## Capacity

Wire has 543 chunks, highest command 206 and a 148,596-byte artifact. Its compact module input
uses 23,080 of 49,152 bytes, leaving 26,072. It contributes 664 name events. Parse remains the
name maximum at 776 of 1,024. Constant nodes use 1,297 of 1,365, leaving 68 and preserving the
standing 64-node reserve. No buffer cap increased. Further stage growth must still respect the
node bound even though compact input has recovered substantial byte capacity.

The first template implementation used 1,315 nodes. Ten additional literal helpers, calls to the
existing tag constants and a shared template-error helper reduce use to 1,297. Five template
functions and ten literal helpers account for fifteen added chunks. Literal aliases add fixed
call levels. The scalar capacity guard now uses the same bounded alias reader as offset checks.
Throughput has not been benchmarked. The input-event limit remains 1,024, including checkpoints.

## Verification

Cargo uses an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject is edited during a live run.

- The first node-reader regression fails in tmp/order1-templates-node-before.log and passes in
  tmp/order1-templates-node-after.log. Command cargo test --release --features self-host --lib
  constant_node_reader_emits_the_tail_above_the_name_capacity -- --nocapture. Sessions 18311 and
  15693 exited 101 and zero. The later expanded regression covers both formats and the limit.
- Twelve wire protocol tests pass in tmp/order1-templates-protocol.log. Command cargo test
  --release --features self-host --lib selfhost::wire_ -- --nocapture. Session 17103 exited zero.
  This preceded declaration reordering and the later canonical scalar-tag decoder repair.
- The repaired integrated run passes 202 tests across node budget, artifact identity, wire,
  native metadata, templates and private accounting. All twelve pipeline artifacts match.
  Command cargo test --release --features self-host --test selfhost_artifact_identity --test
  selfhost_wire --test module_input_node_budget --test selfhost_wire_templates --test
  selfhost_wire_natives --test wire_private_state_accounting --no-fail-fast -- --nocapture.
  Log tmp/order1-templates-integrated2.log. Session 50157 exited zero.
- Four inference tests pass in tmp/order1-templates-inference.log. Clearing 42,813 expression
  types leaves all twelve artifacts unchanged. Command cargo test --release --features self-host
  --lib compiler::inference_independence -- --nocapture. Session 63556 exited zero.
- Six integration checks pass without default features in tmp/order1-templates-no-default2.log.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_wire_templates --test module_input_node_budget. Session 48132 exited zero.
- The no-default tag-decoder test passes in tmp/order1-templates-no-default-tags.log. Command
  cargo test --release --no-default-features --features self-host --lib
  scalar_tag_decoding_obeys_feature_gates_without_narrowing_tags. Session 8548 exited zero.

The earlier integrated run failed parsing wire and two stale capacity readers. Those diagnostics
remain in tmp/order1-templates-integrated.log. The lexical probe is in
tmp/order1-templates-parser-probe.log. Its process handle was unavailable on continuation, so
no terminal exit status is claimed for that probe. The subsequent parser test passed in
tmp/order1-templates-parser-after.log. The first no-default compilation failure is retained in
tmp/order1-templates-no-default.log. These failures were repaired without weakening guards.

- The final release workspace run passes 3,067 tests with zero failures and five ignored across
  167 reported suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-templates-workspace.log. Terminal session 23291 exited zero. This includes the
  final canonical tag decoder and all thirteen wire protocol tests.
- Workspace Clippy performed actual Checking and passed. Command cargo clippy --workspace
  --tests -- -D warnings. Log tmp/order1-templates-final-clippy.log. Session 64196 exited zero.
- Documentation passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc --workspace
  --no-deps. Log tmp/order1-templates-doc.log. Session 84575 exited zero.
- The no_std self-host check performed actual Checking and passed. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-templates-nostd.log. Session
  94498 exited zero. This includes compile and verify through the self-host feature.

No production or test code changed after the full workspace run. The four documentation guard
suites pass 24 tests in tmp/order1-templates-doc-guards.log. Command cargo test --release --test
documentation_links --test claimed_counts --test comment_citations --test push_order_claims.
Terminal session 44715 exited zero. Formatting and diff checks pass. The documentation guards
are repeated after this receipt update. Separate feature matrices,
minimum supported toolchains, Miri, cross-target builds, advisory scanning and final integration
gates remain required after the missing implementation. No verification process remains running
at final handoff.

## Next work

Complete debug pools, composite and named constants, private composite slots and nonempty
non-elided initializer pools. The auxiliary helper still refuses those forms. DebugPool is
structured input whose Rust encoder sorts records by operation index, kind and operands.
Passing its encoded bytes through Keleusma would leave serialization in Rust. Constant-name
interning must preserve the breadth-first ordering already demonstrated by the older flattener.

Complete semantic decoding and then connect complete artifact output to the shipping tool.
Shipping unsigned output still uses Module::to_bytes. Preserve every supported behavior.
Legacy wire_windowed_via_kel borrows reference positions and can skip unknown regions. Envelope
integrity alone is not semantic decoding. Retain the full completion plan and its type obligations,
including projection identity, nested headers, binder and occurrence coverage, nominal layout,
aliases, empty-array inference, mixed tuple layout, casts and enum-dispatch capacity.

Refresh all-stage identity, monomorphization, inference independence, shipping comparisons and
final gates after the remaining work. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-templates contains the first node-reader diagnostic and three bounded claims
for template ranges, both constant transport forms and the measured input capacity. Blind exports
preceded non-independent self-labels. All four request hashes, exact payloads and source hashes
were reviewed in tmp/order1-templates-pilot-export.json. The bounded increment outcome is recorded
as resolved. Full Order 1 remains active. Later diagnostics belong to this
same increment and remain in their logs. External evaluation remains unavailable under the prior
automatic approval rejection for the external destination and payload. Do not retry or bypass
it. Local self-labels are non-independent. Usage and elapsed time remain null when unavailable.
Source references stay local, so excerpts must include the decisive assertions themselves.
The pilot implementation line owns its tracked checkpoint.

The assigned inbox was empty at startup, on continuation and before handoff. No messages were
sent or received. The exported mailbox checkpoint was reviewed and has no tracked difference. Publication requires in-session confirmation.
