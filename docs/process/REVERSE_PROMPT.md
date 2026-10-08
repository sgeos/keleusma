# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at 7543abf9. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

The legacy HANDOFF.md is invalid and stale, measured 35 commits behind the starting head.
Ancestry passes and its unchanged-stage-source claim fails. Historical full validity and advisory
checks were not all repeated. Read Appendix B before tracked writes. Public bytecode version
remains 2, opcode count remains 66 and selected type-stage caps remain 640.

## Semantic opcode decoding

The public wire_decode_opcode_sections_via_kel helper accepts raw opcode and operand-pool
sections. The wire stage checks parity and interprets representations, sentinels, tags and pool
indices. Rust copies bounded byte windows and constructs typed Op values from semantic fields.
Every pool entry receives a parity check, including unused entries. Malformed lengths, unavailable
scalar kinds and invalid referenced entries are refused. There is no production reference decoder
call in this helper. It does not decode complete modules or auxiliary records.

Commands 214 and 215 begin one opcode and finish a requested pool entry. A new begin invalidates
any previous pending request. A rejected pool response preserves the pending request for retry.
Framing and signature transactions refuse both commands while their state is pending.

Reference acceptance includes noncanonical encodings. Array access ignores unused operand bytes,
boxed tuple and enum access ignore the middle byte, and pooled construction treats only flag one
as boxed. The raw differential grid varies high, middle and low bytes independently. It compares
153,600 records per feature configuration. With default features, 69,957 accept and 83,643 refuse.
Without default features, 67,317 accept and 86,283 refuse. Both agree with the reference.
The comparison is finite, not exhaustive. Separate controls cover all 66 canonical identifiers,
composite forms, single-bit corruption, unused pool corruption and maximum pool-index widths.

The all-stage artifact comparison now also decodes the Keleusma-produced opcode sections and
compares instruction sequences with the reference. All thirteen sources remain inventoried.

## Capacity and implementation findings

Wire now has 564 chunks, highest command 215 and a 159,188-byte artifact. Module input occupies
23,738 bytes and 709 name events. The constant forest uses 1,299 of 1,365 nodes, retaining 66
free against the standing minimum of 64. Private decoder state adds nine value slots or 288 bytes.
No cap changed. Existing forest and name bounds still require source-admission coverage before
shipping integration. The previous forest increment's implementation and receipts remain in the
design journal and wire plan.

The initial integration inventory imported a crate-private cfg(test) item. The canonical inventory
now lives in library tests, while integration enumerates wire identifiers. The first stage growth
left only 47 nodes free. Existing small literal helpers in four loop-free functions restore the
reserve. A test-only unqualified logging macro failed compilation and was corrected. The old
capacity pin then failed at the measured new size and was refreshed along with three chunk pins.
A final test formatting change names the complete operand in mismatch diagnostics.

## Verification

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. Verification subjects remain unchanged while their runs are live.

- Twenty-two wire unit tests pass in tmp/order1-opdecode-expanded2.log. Command cargo test
  --release --features self-host --lib selfhost::wire_ -- --nocapture. Session 72130 exited zero.
- Eleven tests pass in tmp/order1-opdecode-artifacts.log, covering all thirteen complete artifacts
  and decoded instruction sequences, decoder integration, node reserve and private accounting.
  Command cargo test --release --features self-host --test selfhost_artifact_identity --test
  selfhost_wire_decode --test module_input_node_budget --test wire_private_state_accounting
  --no-fail-fast -- --nocapture. Session 45613 exited zero.
- Four inference tests pass in tmp/order1-opdecode-inference.log. Clearing 44,835 expression
  types preserves all thirteen artifacts. Command cargo test --release --features self-host
  --lib compiler::inference_independence -- --nocapture. Session 75391 exited zero.
- Three no-default decoder unit tests pass in tmp/order1-opdecode-no-default-unit.log.
  Session 78012 exited zero. Four integration and accounting tests pass in
  tmp/order1-opdecode-no-default-integration.log. Session 3338 exited zero. Both use
  cargo test --release --no-default-features --features self-host with the named targets.
- The refreshed wire, chunk-name, parser, self-compile and monomorphization suites pass 284 tests,
  with two ignored. Command cargo test --release --features self-host --test selfhost_wire
  --test selfhost_chunk_names --test selfhost_parse --test wire_self_compile_status
  --test selfhost_monomorphize_identity --no-fail-fast. Log tmp/order1-opdecode-wire-gates.log.
  Session 90360 exited zero.
- Workspace Clippy performed actual Checking and passed after the final test formatting change.
  Command cargo clippy --workspace --tests -- -D warnings. Log
  tmp/order1-opdecode-final-clippy.log. Session 16614 exited zero.
- Documentation passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc --workspace
  --no-deps. Log tmp/order1-opdecode-doc.log. Session 72813 exited zero.
- The no_std self-host check performed actual Checking and passed. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-opdecode-nostd.log.
  Session 83511 exited zero.

The final no-default library repeat passes all three decoder tests after the test formatting
change. Log tmp/order1-opdecode-final-no-default.log. Command cargo test --release
--no-default-features --features self-host --lib selfhost::wire_decode -- --nocapture.
Session 81120 exited zero.

The final release workspace passes 3,095 tests with zero failures and five ignored across
171 suites. Command cargo test --release --workspace --no-fail-fast. Log
tmp/order1-opdecode-workspace.log. Session 74316 exited zero. No production or test code changed
after that run. Documentation updates follow it and receive separate documentation guards.
The four documentation guards pass 24 tests after the handoff and plan updates. Command
cargo test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-opdecode-doc-guards.log. Session 23446 exited zero.
Formatting and diff checks pass. The documentation guards are repeated after this receipt update
in tmp/order1-opdecode-final-doc-guards.log. No verification process remains running at handoff.
The original diagnostics remain in tmp/order1-opdecode-first.log, -second.log, -expanded.log and
-capacity.log. The earlier first-stage build check and repairs are recorded in the local logs.
Separate feature matrices, minimum supported toolchains, Miri, cross-target builds, advisory
scanning and final integration gates remain required after the missing work.

## Next work

Finish semantic auxiliary decoding and complete module reconstruction. Existing wire commands
read schema fields, but the legacy windowed host path supplies reference region positions and
may skip unsupported regions. It is not an independent semantic decoder. The reference
wire_schema::decode_aux_body documents optional-table behavior that the differential controls
must distinguish from malformed required tables. Constant reconstruction needs a bounded walk
that interprets child ranges only for composites and enforces forward child ordering.
Only two constant nodes remain above the standing reserve, so measure any further stage growth
before integrating it. Existing schema readers occupy commands 13 through 83 and are candidates
for reuse, but their resident-buffer assumptions require an audit for large artifacts.

Establish admission-preserving capacity coverage, then connect the Keleusma artifact path to
shipping compilation. Shipping unsigned output still calls Module::to_bytes. Do not narrow
accepted behavior to the current stage corpus or silently fall back to Rust as a claim of
independent serialization. Current forest, name and input bounds need explicit source-admission
checks. Envelope integrity alone is not semantic decoding.

Retain the completion plan's type obligations, including projection identity, nested headers,
binders, occurrences, nominal layout, aliases, empty-array inference, mixed tuples, casts and
enum-dispatch capacity. Refresh all-stage, monomorphization, inference-independence, shipping and
full integration evidence after the remaining changes. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-opdecode retains the first inventory-import diagnostic and three bounded claims
for raw-record comparisons, all-stage artifact and opcode identity, and node reserve. Blind exports
preceded non-independent self-labels. Later failures were not separately sampled because they
belong to this increment. Unknown usage and elapsed time remain null.

External evaluation remains unavailable under the prior automatic approval rejection for the
external destination and payload. Do not retry or bypass it. Local capture proceeds. No independent
accuracy claim is supported. The pilot implementation line owns the tracked pilot checkpoint.

The assigned inbox was empty at startup and during work. No messages were sent or received.
Before handoff the mailbox was checked again and its checkpoint exported and reviewed with no
tracked difference. Four pilot request hashes, exact payloads and source hashes were reviewed
in tmp/order1-opdecode-pilot-export.json. The bounded decoder increment outcome is resolved.
Full Order 1 remains active. Initial export inspection assumed decoded fields and a direct state
payload. Reading the stored JSON strings and state.input repaired that inspection, and every
verification assertion then passed. Publication requires explicit in-session confirmation.
