# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at 5d8d9a47. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

The legacy HANDOFF.md is invalid and stale, measured 36 commits behind the starting head.
Ancestry and its unchanged-stage-source claim were already checked in the preceding increment.
The unchanged-source claim is false. Historical full validity and advisory checks were not all
repeated. Read Appendix B before tracked writes. Public bytecode version remains 2, opcode
count remains 66 and selected type-stage caps remain 640.

## Independent auxiliary directory

The public wire_auxiliary_directory_via_kel helper returns a WireAuxiliaryDirectory containing
borrowed WireAuxiliaryRegion payloads and a header-disagreement flag. It supplies raw prologue
and directory bytes to Keleusma. The stage votes and validates the prologue, derives the header
length, and emits one region per resume. It derives byte offsets and lengths and checks bounds
and duplicate kinds. Rust narrows semantic fields and borrows checked slices. There is no
production reference directory parser or reference-position input in this helper.

The complete header at the format ceiling of 1,024 regions occupies 49,200 bytes, inside the
existing 65,536-byte window. Payload size is independent of that window. The host allocates at
most 1,024 region descriptors, in addition to the stage runtime, and copies no payload bytes.
Controls cover the maximum directory, zero regions, large and unaligned payloads and parity-plane
metadata. Pointer equality establishes that returned slices alias the input buffer.

Reference inspection and execution establish that the container parser accepts overlapping
spans, unknown kinds and flags, and reserved fields. This path preserves those cases. It does
not interpret schema records or verify payload integrity. The needs_scrub flag covers disagreement
among prologue and directory copies, not corruption or correction of payloads.

Commands 216 and 217 begin and advance decoding. Active decoding excludes other commands.
A new begin resets the cursor, a failed begin invalidates the old transaction and a rejected
entry preserves the cursor for retry. Starting a directory invalidates an old opcode pool request.
Framing and signature transactions refuse the new commands. Four private fields add 128 bytes.
The existing region-kind array holds already accepted kinds for the bounded duplicate scan.

The complete artifact corpus now also checks every decoded directory against the reference,
including metadata, offsets, payload bytes and pointer identity. It still checks opcode semantics
and complete artifact bytes for all thirteen current stage sources.

## Measurements and implementation findings

Wire has 566 chunks, highest command 217, 715 name events, 23,793 input bytes and a 160,524-byte
artifact. Its constant forest uses 1,294 of 1,365 nodes, retaining 71 free against the minimum
reserve of 64. The input margin is 25,359 bytes. No cap changed. Inference independence covers
44,978 expression types across thirteen stages. Earlier encoder forest and name bounds still
need source-admission coverage before shipping integration.

The first test fixture named a nonexistent builder method. The first stage version assigned a
local binding, which the language does not allow. Moving the flag into private state repaired it.
The first node measurement left only 50 free. Literal sharing initially changed a canonical loop
bound, so the verifier refused the stage. All three modified loop functions were restored.
Only five functions without loops retain literal sharing. A mutation test then correctly reported
that its old header-size anchor no longer existed. Its replacement still mutates the starting
cursor and observes the resulting wrong bytes. Clippy found a redundant usize conversion in the
host helper. The final implementation uses the already checked command result directly.

## Verification receipts

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited while its run remained live.

- The final targets pass 196 tests. Command cargo test --release --features self-host --test
  selfhost_artifact_identity --test selfhost_wire --test selfhost_wire_directory --test
  module_input_node_budget --test wire_private_state_accounting --no-fail-fast. Log
  tmp/order1-directory-final-targets.log. Session 13054 exited zero. This includes all thirteen
  artifact, directory and opcode comparisons and the repaired mutation detector.
- The final wire unit run passes 24 tests. Command cargo test --release --features self-host
  --lib selfhost::wire_ -- --nocapture. Log tmp/order1-directory-final-protocol.log.
  Session 66426 exited zero. Directory mutations accept 1,182 cases and refuse 181 in agreement
  with the reference. Every one-bit header mutation in the fixture is repaired by the vote.
  The opcode differential still accepts 69,957 and refuses 83,643 raw records.
- No-default directory unit checks pass both tests. Command cargo test --release
  --no-default-features --features self-host --lib selfhost::wire_directory -- --nocapture.
  Log tmp/order1-directory-final-no-default-unit.log. Session 85857 exited zero.
- No-default integration and accounting pass five tests. Command cargo test --release
  --no-default-features --features self-host --test selfhost_wire_directory --test
  wire_private_state_accounting --no-fail-fast. Log
  tmp/order1-directory-final-no-default-integration.log. Session 38325 exited zero.
- Parser, chunk-name, monomorphization and self-compile targets passed 100 tests with two ignored
  in tmp/order1-directory-wire-gates.log. Command cargo test --release --features self-host
  --test selfhost_wire --test selfhost_chunk_names --test selfhost_parse --test
  wire_self_compile_status --test selfhost_monomorphize_identity --test selfhost_wire_directory
  --no-fail-fast. Session 89834 exited 101 because the wire suite found the obsolete mutation
  anchor. The final targets above repeat that complete wire suite after its repair. No stage
  source changed after the parser and identity results.
- Four inference checks pass. Command cargo test --release --features self-host --lib
  compiler::inference_independence -- --nocapture. Log tmp/order1-directory-inference.log.
  Session 60616 exited zero. The final stage source is unchanged from that measurement.
- Workspace Clippy performed actual Checking and passed. Command cargo clippy --workspace
  --tests -- -D warnings. Log tmp/order1-directory-final-clippy.log. Session 15584 exited zero.
- Documentation performed actual Documenting and passed with RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-directory-doc.log.
  Session 13187 exited zero.
- The no_std self-host check performed actual Checking and passed. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-directory-nostd.log.
  Session 45162 exited zero.

Original diagnostics remain in tmp/order1-directory-first.log, -second.log, -budget-second.log,
-targets.log, -protocol.log, -capacity.log, -clippy.log and -wire-gates.log. The first capacity
pin failure measured the new input size before the assertion was refreshed.

The full workspace was not repeated this increment. The preceding opcode-decoder tree passed
3,095 workspace tests with five ignored. Current verification is targeted at the changed wire
paths, all stage sources and shared protocols. Separate feature matrices, minimum supported
toolchains, Miri, cross-target builds, advisory scanning and final integration gates remain open.
The four documentation guards pass 24 tests after the handoff and plan updates. Command
cargo test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-directory-doc-guards.log. Session 64515 exited zero.
Formatting and diff checks pass. The guards are repeated after this receipt update in
tmp/order1-directory-final-doc-guards.log. No verification process remains running at handoff.

## Next work

Finish semantic schema-record decoding and constant reconstruction, then compose the directory,
opcode and framing readers into a complete module decoder. Use the new directory to obtain
borrowed region bytes without reference positions. Existing schema commands 13 through 83 still
assume absolute resident-buffer addresses. Audit that assumption before reusing them for payloads
larger than the window. A schema field reader must interpret tags and ranges in Keleusma rather
than move the old reference decoding into another host helper.

The reference wire_schema::decode_aux_body has different handling for optional-table parse
failures and malformed required tables. Compare those cases explicitly. Constant traversal must
interpret child ranges only for composites and enforce forward child ordering. The wire stage
has seven nodes beyond its standing reserve, so measure any added stage functions before integration.

Then establish admission-preserving capacity coverage and connect Keleusma artifact emission to
shipping compilation. Shipping unsigned output still calls Module::to_bytes. Do not narrow
accepted source behavior to the current stage corpus or silently fall back to Rust as a claim of
independence. Legacy wire_windowed_via_kel still receives reference positions and may skip regions.

Retain the completion plan's type obligations, including projection identity, nested headers,
binders, occurrences, nominal layout, aliases, empty-array inference, mixed tuples, casts and
enum-dispatch capacity. Refresh all-stage, monomorphization, inference-independence, shipping and
full integration evidence after the remaining changes. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-directory retains the first missing-method diagnostic and three bounded claims
for the mutation corpus, borrowed payloads and all-stage directories. Blind exports preceded
non-independent self-labels. The all-stage claim was labelled unsupported because its excerpt
omits the STAGES declaration and executed count of thirteen sources. The full repository evidence
establishes thirteen, but the excerpt alone does not establish that numerical scope. The other
two claims were labelled supported. No independent accuracy claim is available.

All four request hashes, exact payloads and source hashes were reviewed in
tmp/order1-directory-pilot-export.json. The bounded directory increment outcome is resolved.
Later diagnostics were not separately sampled because they belong to the same increment. Usage
and elapsed time remain null. External evaluation remains unavailable under the prior automatic
approval rejection for the external destination and payload. Do not retry or bypass it. The pilot
implementation line owns the tracked pilot checkpoint.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
Its checkpoint was exported and reviewed with no tracked difference. Publication requires explicit
in-session confirmation.
