# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at c0dd474f. Work began on
2026-10-08 and resumed on 2026-10-11. Preserve unrelated untracked .codex and nvim.log.
V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md remains invalid and stale.
Its ancestry was previously checked and its unchanged-stage-source claim is false. Historical
full validity and advisory checks were not all repeated. The freshness measurement is 37 commits behind the starting head.

## Fixed schema record decoding

The new wire_schema_records_via_kel helper takes a region kind and raw table bytes and returns
WireSchemaRecord variants. Keleusma selects the stride, checks table divisibility and reads every
field from a bounded record window. Rust constructs the corresponding typed records with checked
narrowing. It supplies no field offsets or widths and calls no production reference record codec.
The constant payload preserves all 64 bits of its signed transport word when represented as u64.

All seventeen fixed-record kinds are handled. Byte pools and unknown kinds receive a named
refusal. Reserved fields and unknown tags retain reference record-reader behavior. These are raw
schema records, not validated module values. Tag interpretation, cross-table validation, constant
child ranges, optional-table policy and complete module reconstruction remain open.

Commands 218 and 219 begin and advance a record stream. The stage owns its count and cursor.
A bad record length preserves the cursor. Completion clears active state. A failed new begin
invalidates the previous stream. Active reading excludes other commands. Existing framing,
signature and directory transactions refuse both new commands. Five private fields add 160 bytes.
No shared array or cap increased. The typed result vector uses fallible reservation.

The classifier enumerates all 65,536 possible kind identifiers against the explicit seventeen-kind
contract. It is a current contract test, not an automatically discovered inventory of future schema
kinds. Field comparisons cover 4,386 records, full-width signed and unsigned patterns, reserved
fields and malformed table lengths. A 9,000-record table exceeds the byte window. Both aligned
and unaligned inputs compare all NameRef fields. The initial unaligned control checked only count
and was strengthened before the final focused run.

The complete artifact corpus now reconstructs every fixed-record region from Keleusma-decoded
fields using reference writers and compares every byte. It explicitly excludes STRING_POOL,
PARAM_TYPES and DEBUG_POOL. Directory metadata, payload pointer identity, opcode semantics and
complete artifact identity remain checked across all thirteen current stages.

## Measurements and first diagnostic

Wire has 561 chunks, highest command 219, 715 name events, 23,713 input bytes and a 162,836-byte
artifact. Its constant forest uses 1,290 of 1,365 nodes, retaining 75 free against the minimum
reserve of 64. Input margin is 25,439 bytes. Inference independence covers 45,329 expression types
across thirteen stages.

The first node-budget test failed with 1,311 nodes and only 54 free. Sharing literals in the
existing get_u16, get_u32 and get_i64 functions restored the reserve. These functions contain
no loops. No loop bound was changed and no cap was raised. The later driver-capacity assertion
measured 23,931 bytes before its old 23,793-byte pin was refreshed. Three chunk pins initially carried
575 after nine new reader functions. They now carry 561 after the fourteen-helper repair.

## Verification receipts

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- Three initial record tests evolved from two passing field tests into the final protocol set.
  Twenty-seven wire unit tests pass in tmp/order1-records-protocol.log. Command cargo test
  --release --features self-host --lib selfhost::wire_ -- --nocapture. Session 88636 exited zero.
  The kind test was subsequently renamed to state its explicit seventeen-kind contract.
- The node budget and private accounting pass three tests in tmp/order1-records-budget-second.log.
  Command cargo test --release --features self-host --test module_input_node_budget --test
  wire_private_state_accounting -- --nocapture. Session 48244 exited zero.
- All thirteen complete artifacts and fixed-record regions pass, with three public reader controls,
  in tmp/order1-records-artifacts.log. Command cargo test --release --features self-host --test
  selfhost_wire_records --test selfhost_artifact_identity --no-fail-fast -- --nocapture.
  Session 56187 exited zero. Nine tests passed. The unaligned assertion was strengthened afterward.
- Four inference tests pass in tmp/order1-records-inference.log. Command cargo test --release
  --features self-host --lib compiler::inference_independence -- --nocapture.
  Session 40533 exited zero. The final stage source is unchanged from that measurement.
- Three no-default record unit tests pass in tmp/order1-records-no-default-unit.log. Command cargo
  test --release --no-default-features --features self-host --lib selfhost::wire_records
  -- --nocapture. Session 77559 exited zero.
- The focused wire, record, parser, chunk, self-compile and monomorphization log reports 287 passes
  and two ignored in tmp/order1-records-final-targets.log. Its handle 84895 was unavailable on
  resumption, so no terminal exit status was recovered. The fresh workspace run supersedes it.
- A fresh no-default integration repeat passes five tests in
  tmp/order1-records-resumed-no-default.log. Command cargo test --release --no-default-features
  --features self-host --test selfhost_wire_records --test wire_private_state_accounting
  --no-fail-fast. Session 35656 exited zero.

Handles 84895, 87013, 12274, 49854 and 92905 were missing on 2026-10-11. Their logs were retained.
Fresh resumption checks for Clippy and documentation returned zero but were cached, so they are
not counted as fresh analysis. The first fresh workspace result and the subsequent repair receipts follow.

The first resumed workspace completed with 3,104 passes, two failures and five ignored across
173 suites. Log tmp/order1-records-workspace-final.log. Session 66307 exited 101. Both failures
were in legacy type-input capacity. Occurrences required 652 rows and bindings 649, above 640.
The production scoped path and artifact comparisons passed, so they did not cover that harness.

Fourteen lightly reused numeric helper functions were inlined. Sixteen existing count and
presence functions now share zero and one literals. No type fact was omitted, no cap changed and
no loop bound changed. The type-capacity price check passes in
tmp/order1-records-type-capacity-repair.log, session 14129 exited zero. The real-stage corpus
check accepts all thirteen stages with 638 of 640 occurrence rows in
tmp/order1-records-type-corpus-repair.log, session 17730 exited zero. The constant-node budget
still passes at 1,290 nodes in tmp/order1-records-node-capacity-repair.log, session 82568 exited zero.
Four repaired-stage inference checks pass at 45,329 types in
tmp/order1-records-inference-repair.log, session 40781 exited zero.

Clippy performed actual Checking and passed in tmp/order1-records-repaired-clippy.log,
session 46929 exited zero. Command cargo clippy --workspace --tests -- -D warnings.
Documentation performed actual Documenting and passed in tmp/order1-records-repaired-doc.log,
session 41387 exited zero. Command cargo doc --workspace --no-deps with RUSTDOCFLAGS set to
-D warnings. The no_std check performed actual Checking and passed in
tmp/order1-records-repaired-nostd.log, session 4581 exited zero. Command cargo check
--no-default-features --features self-host.

The repaired release workspace passes 3,106 tests with zero failures and five ignored across
173 suites. Command cargo test --release --workspace --no-fail-fast. Log
tmp/order1-records-workspace-repaired.log. Session 7525 exited zero. No production or test
contents changed afterward. Documentation updates follow the run and receive separate guards.

The repaired no-default library tests pass three checks in
tmp/order1-records-repaired-no-default-unit.log, session 17369 exited zero. Command cargo test
--release --no-default-features --features self-host --lib selfhost::wire_records -- --nocapture.
Five no-default integration and accounting tests pass in
tmp/order1-records-repaired-no-default-integration.log, session 57351 exited zero. Command
cargo test --release --no-default-features --features self-host --test selfhost_wire_records
--test wire_private_state_accounting --no-fail-fast. The four documentation guard suites pass 24 tests after the handoff and plan updates. Command
cargo test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-records-doc-guards.log. Session 58673 exited zero.
Formatting and diff checks pass. The guards are repeated after this receipt update in
tmp/order1-records-final-doc-guards.log. No verification process remains running at handoff.

Original diagnostics remain in tmp/order1-records-budget-first.log and -capacity.log. The first
pilot diagnostic is the actual reserve failure. The earlier source measurements and stage logs
are retained under tmp/order1-records-. Full feature matrices, minimum supported toolchains,
Miri, cross-target builds, advisory scanning and final integration gates remain required before
completion of Order 1.

## Next work

Implement semantic tag and cross-table checks in Keleusma, then reconstruct constant trees and
complete modules. Reuse the directory and fixed-record readers. Do not call the reference schema
decoder behind the new interface, or interpret packed constant ranges in Rust while claiming
that interpretation is self-hosted. The record interface intentionally preserves unvalidated tags.

Read findings that should shape the next controls follow. The owned reference constant decoder
uses a reverse sweep and takes each child from an Option slot. Duplicate child ownership is
rejected even when all edges point forward. Chunk root ranges are extracted together and reject
duplicate ownership across those ranges. Private initializers decode through a separate pool
call. Combining both extraction scopes could reject an artifact that the reference accepts.
Sources are decode_constant_pools and decode_data_layout in src/wire_schema.rs.

RecordTable::range_is_forward in keleusma-wire/src/view.rs requires first greater than the parent
index even for a zero count, and requires the range end no greater than table length. Its empty
range comment does not override the predicate. SignatureTable::parse checks parameter ranges
and return/resume indices. ShapeRecord::to_shape interprets the tag but carries kind bytes
unchanged. Preserve that codec behavior while distinguishing later verifier checks.

The existing schema commands 13 through 83 still assume resident absolute addresses. The new
record commands read one record at byte zero and therefore avoid that assumption. Decode semantic
references through stage-produced requests and host transport, with bounded stage work per step.
The stage has eleven nodes beyond the standing constant-node reserve, but only two occurrence
rows beyond the current legacy type-input usage. Measure both constraints before further growth.
A separate reader stage or properly scoped input staging deserves consideration before adding
many more functions to wire.kel. Do not weaken either capacity guard to make an increment pass.

Then establish admission-preserving capacity coverage and connect artifact emission to shipping
compilation. Shipping unsigned output still calls Module::to_bytes. Existing forest, name and
input bounds require coverage beyond the current stage corpus. Preserve supported source behavior.
Retain all type obligations in the completion plan, including projections, nested headers,
binders, occurrences, nominal layouts, aliases, empty arrays, mixed tuples, casts and enum capacity.
Refresh all-stage, monomorphization, inference-independence, shipping and final integration evidence
when those changes land. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-records retains the first node-reserve diagnostic and three bounded claims for
field comparisons, the aligned and unaligned long table, and all-stage record reconstruction.
Blind exports preceded non-independent self-labels. The all-stage excerpt includes the complete
stage inventory, the decisive reconstruction assertions and executed stage counts. Its first packet
exceeded the pilot text-field limit and was shortened before successful recording. No fifth sample
was created. Later capacity-pin diagnostics belong to the same increment and were not sampled again.

External evaluation remains unavailable under the prior automatic approval rejection for the
external destination and payload. Do not retry or bypass it. Local capture continues. Usage and
elapsed time remain null. No independent accuracy claim is available. The pilot implementation
line owns the tracked pilot checkpoint. The bounded record increment is resolved. All four request hashes, exact payloads and source
hashes were reviewed in tmp/order1-records-pilot-export.json. The captured claims describe the
pre-repair snapshot. Final workspace verification repeats their properties after helper inlining.

The assigned inbox was empty at startup and after resumption. No messages were sent or received.
The mailbox checkpoint was exported and reviewed with no tracked difference. Publication requires explicit in-session
confirmation.
