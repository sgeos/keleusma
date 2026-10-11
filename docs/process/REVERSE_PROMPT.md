# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at b6cfc389. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md remains invalid and stale,
measured 38 commits behind the starting head. Its ancestry was previously checked and its
unchanged-stage-source claim is false. Historical full validity and advisory checks were not
all repeated.

## Semantic signature and native-return decoding

The new wire_schema.kel stage owns signature bounds, shape-tag interpretation, shape request
ordering and the optional-table decision. wire_decode_signatures_via_kel implements the strict
standalone signature reader. wire_decode_signature_metadata_via_kel implements the signature
family's policy inside decode_aux_body, returning signatures, native return shapes and parse_valid.
It does not validate the other auxiliary families or construct a complete module.

All signature ranges and return/resume indices are checked before any shape tag is interpreted.
A bad shared shape/signature view produces empty metadata only when no native return needs it
and the caller selected optional mode. Referenced invalid shape tags remain errors through an
otherwise valid view. Unused invalid shapes remain unobserved. Top discards kind and size; Scalar
discards size. Arbitrary kind bytes are preserved as ShapeRecord::to_shape preserves them.
The codec boundary is separate from later verification. The standalone helper ignores native
return tables, matching decode_signatures.

The stage requests each parameter, return and resume shape by index and role. Host code retrieves
the requested record and constructs the normalized WireShape. Native requests have independent
counts. Failed shape replies preserve the pending request and do not advance. A failed begin
invalidates old state. Completion requires exact signature and native counts. Commands 0 through
7 belong to this new stage, not to wire.kel's command namespace.

The host reuses one wire machine for directory and record decoding and a separate schema machine
for semantic work. No production reference signature view or ShapeRecord::to_shape call occurs.
Transport and allocation errors propagate and are not treated as optional-table absence.
ChunkSignature now derives PartialEq and Eq, enabling structural comparisons without altering
its fields or wire representation.

## Corpus and resources

There are now fourteen embedded stage sources. All applicable corpus lists include wire_schema,
which has its own self-compilation test. The current schema stage has 15 chunks, a 7,052-byte
artifact, 192 shared bytes and fourteen private value slots occupying 448 bytes. Its largest
legacy input channel uses 24 of 640 occurrence rows. The wire stage remains at 561 chunks,
1,290 of 1,365 constant nodes and 638 of 640 occurrence rows. It was not enlarged this increment.

Every current stage matches complete reference artifacts, directory interpretation, fixed-record
reconstruction, opcode semantics and decoded signature values. The signature comparison now
requires one decoded signature per compiled chunk. Inference independence covers 45,894 recorded
expression types across fourteen stages. Parameters remain Word and results remain Word or Bool.

## Verification receipts

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- The initial check failed on missing imports. Log tmp/order1-semantic-shapes-first.log.
  Session 78582 exited 101. The next attempt exposed missing equality traits on ChunkSignature.
  Session 88339 exited 101 in tmp/order1-semantic-shapes-second.log. The fixture was also repaired
  to obtain auxiliary bytes from Module::to_bytes and parse_wire_sections.
- The initial semantic and accounting controls pass five tests in
  tmp/order1-semantic-shapes-targets.log. Session 65795 exited zero. Later controls supersede them.
- The initial artifact, inventory, monomorphization and public semantic controls pass in
  tmp/order1-semantic-shapes-artifacts.log. Session 81064 exited zero. All fourteen sources are
  reached. The schema stage is 15 chunks and 7,052 bytes.
- Four inference checks pass in tmp/order1-semantic-shapes-inference.log. Command cargo test
  --release --features self-host --lib compiler::inference_independence -- --nocapture.
  Session 21438 exited zero. The final production source is unchanged from that measurement.
- The legacy real-stage type-input measurement accepts all fourteen stages in
  tmp/order1-semantic-shapes-capacity.log. Command cargo test --release --features self-host
  --test selfhost_typecheck the_stage_is_measured_against_the_real_stage_sources -- --nocapture.
  Session 79574 exited zero.
- Three final no-default schema unit tests pass in
  tmp/order1-semantic-shapes-final-no-default-unit.log. Command cargo test --release
  --no-default-features --features self-host --lib selfhost::wire_semantics -- --nocapture.
  Session 12420 exited zero. The tag census accepts three and refuses 65,533 values. The 2,304
  kind/size combinations agree with the reference. Retry, completion and native-only state paths
  are exercised separately.
- The release workspace completed with 3,113 passes, one failure and five ignored across 174
  suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-semantic-shapes-workspace.log. Session 48107 exited 101. The sole failure was an
  old assertion expecting thirteen stages before the production type loop could run.
- The assertion now compares the actual directory inventory with the named corpus, including
  duplicate and nonempty checks. The repaired production type check accepts all fourteen sources
  in tmp/order1-semantic-shapes-type-repair.log. Command cargo test --release --features self-host
  --test selfhost_typecheck production_types_accept_all_stage_inputs -- --nocapture.
  Session 52547 exited zero.
- Four final public semantic tests and two accounting tests pass in
  tmp/order1-semantic-shapes-final-targets.log. Session 10742 exited zero. Command cargo test
  --release --features self-host --test selfhost_wire_semantics --test
  wire_private_state_accounting --no-fail-fast -- --nocapture. Controls include eleven optional
  parsing cases, eighteen strict boundary cases with five accepts and thirteen refusals,
  a 3,000-parameter signature, invalid native-only tags, preserved kind bytes and unused bad shapes.
- The same six integration and accounting tests pass without default features in
  tmp/order1-semantic-shapes-final-no-default-integration.log. Command cargo test --release
  --no-default-features --features self-host --test selfhost_wire_semantics --test
  wire_private_state_accounting --no-fail-fast. Session 43243 exited zero.
- All six artifact tests pass again after adding the signature-per-chunk count assertion.
  Command cargo test --release --features self-host --test selfhost_artifact_identity -- --nocapture.
  Log tmp/order1-semantic-shapes-final-artifacts.log. Session 96443 exited zero.
- Workspace Clippy performed actual Checking and passed after the test repair and final assertions.
  Command cargo clippy --workspace --tests -- -D warnings. Log
  tmp/order1-semantic-shapes-final-lint.log. The command exited zero. The earlier nested-if lint
  diagnostic remains in tmp/order1-semantic-shapes-clippy.log.
- Documentation performed actual Documenting and passed with RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-semantic-shapes-doc.log.
  Session 31163 exited zero.
- The no_std check performed actual Checking and passed. Command cargo check --no-default-features
  --features self-host. Log tmp/order1-semantic-shapes-nostd.log. Session 68905 exited zero.

No production code changed after the full workspace run. Only the stale inventory assertion,
stronger test controls and documentation changed afterward. The full workspace was not repeated
following those test-only changes. Full feature matrices, minimum supported toolchains, Miri,
cross-target builds, advisory scanning and final integration gates remain required for Order 1.
The four documentation guards pass 24 tests after the handoff and plan updates. Command
cargo test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-semantic-shapes-doc-guards.log. Session 6419 exited zero.
Formatting and diff checks pass. The guards are repeated after this receipt update in
tmp/order1-semantic-shapes-final-doc-guards.log. No process remains running at final handoff.

## Next work

Extend the dedicated schema stage to name and constant semantics, then the remaining auxiliary
families and complete module reconstruction. The signature component provides the request/reply
pattern and preserves parse-stage failure separately from semantic lookup failure. Reuse the
wire directory and record machines; do not move reference semantic decoding into host adapters.

The reference constant decoder uses a reverse sweep and takes children from Option slots.
Duplicate child ownership is rejected even when all edges point forward. Chunk root ranges are
extracted together. Private initializers decode through a separate pool call, so combining those
ownership scopes can change acceptance. RecordTable::range_is_forward requires first greater
than the parent index even for an empty range, with end no greater than the table length.
Do not interpret scalar payloads as child ranges. Float constant admission remains feature-aware.

Other families remain open, including name lookup, constant side tables and trees, templates,
enum layouts, data-layout expansion and initializer policy, parameter tags, debug decoding and
complete module assembly with framing checks. Preserve each family's optional-table behavior.
The wire stage still has only two occurrence rows free. Add semantic work to the new schema stage
and measure every current source against both constant-node and type-input limits.

Then establish admission-preserving capacity coverage and connect artifact emission to shipping
compilation. Shipping unsigned output still calls Module::to_bytes. Existing encoder forest,
name and input bounds require coverage beyond the stage corpus. Preserve supported source behavior.
Retain all type obligations in the completion plan, including projections, nested headers,
binders, occurrences, nominal layouts, aliases, empty arrays, mixed tuples, casts and enum capacity.
Refresh all-stage, monomorphization, inference-independence, shipping and final integration evidence
as those changes land. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-shapes retains the first missing-import diagnostic and three bounded claims for
shape tags and kinds, optional-table outcomes and all-stage signature metadata. Blind exports
preceded non-independent self-labels. All four request hashes, exact payloads and source hashes
were reviewed in tmp/order1-semantic-shapes-pilot-export.json. The bounded signature increment
outcome is resolved. Later diagnostics belonged to the same increment and were not sampled again.
Usage and elapsed time remain null. No independent accuracy claim is available.

External evaluation remains unavailable under the prior automatic approval rejection for the
external destination and payload. Do not retry or bypass it. Local capture continues. The pilot
implementation line owns the tracked pilot checkpoint.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
The mailbox checkpoint was exported and reviewed with no tracked difference. Publication requires
explicit in-session confirmation.
