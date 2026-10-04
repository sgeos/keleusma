# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The feature branch is feat/v02-order1-next. The starting commit is dacd1bb3.
This handoff is included in the scoped commit that follows the checks below. No push, merge
or publication is authorized by this handoff. Preserve the unrelated untracked nvim.log.
The V0.3.X workstream remains separate.

The legacy HANDOFF.md is invalid and stale. Its stamp is 28 commits behind the starting tree.
Ancestry passes and the unchanged-stage claim fails. The historical advisory and full gate
checks were not repeated. Read the current plans and this handoff for live evidence.
Read Appendix B before tracked writes. Public bytecode version remains 2, the opcode count
remains 66 and selected type-stage caps remain 640. Publication requires in-session confirmation.

## Implemented increment

The wire stage now supports internal commands 188 through 192. They derive the unsigned
framing header, stream the checksum, produce the trailer and validate the canonical unsigned
envelope across windows. Four private words add 32 bytes at the host word width. Wrong-mode,
excess-length and incomplete-stream operations fail explicitly. Legacy commands cannot replace
an active checksum accumulator. Semantic decoding is still separate.

The new wire_framing module supplies opaque-section assembly, integrity checking and ordinary
auxiliary-container assembly. Keleusma determines section offsets, padded region positions,
triplicated directory bytes and checksum values. Host buffers transport the results.
The container helper checks duplicate kinds and the 1,024-region capacity.

The new wire_auxiliary module takes only Module facts. It produces complete auxiliary containers
matching the reference for all twelve current stage modules, without reference bytes, directory
positions or artifact lengths. Record streams return actual output lengths. Header fields now
contain actual resource values and schema hash, and chunks retain opcode counts and positions.
Mandatory empty tables remain present. Oversized names and counts fail before the old packer.

This does not complete production serialization. Signature numbering uses the existing Rust
signature-table helper. Parameter tags, record presence and scalar metadata remain host supplied.
Native records, templates, debug pools, composite constants, private composite slots and nonempty
non-elided initializer pools are explicitly unsupported by the new auxiliary helper. Existing
shipping behavior is preserved because the shipping path still calls Module::to_bytes.

Five new functions raise wire to 497 chunks. Three count pins now carry that cause. The current
input uses 47,442 of 49,152 bytes, leaving 1,710. Parse remains the name maximum at 776 of 1,024.
The dispatch_window chain has 23 arms and is close to the parser nesting limit. Measure both
source-growth effects before adding more stage commands. No array cap increased.

## Verification receipts

Cargo runs used an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one.
Stage runs used KEL_SELFHOST_CACHE zero. No verification subject changed during its live run.
All production sources were final before the full workspace diagnostic run. Subsequent source
edits updated three stale test pins and their comments. Documentation edits have separate guards.

- The targeted wire, framing, region coverage, artifact identity and monomorphization run passes
  204 tests. Terminal session 57584 exited zero. Log tmp/order1-wire-framing-final-targets.log.
  Command cargo test --release --features self-host --test selfhost_wire_framing --test
  selfhost_wire --test selfhost_artifact_identity --test selfhost_monomorphize_identity --test
  selfhost_region_coverage --no-fail-fast.
- Two private framing protocol tests pass. Log tmp/order1-wire-framing-final-protocol.log.
  Command cargo test --release --features self-host --lib wire_framing.
- Four inference-independence tests pass. All twelve stages remain identical after clearing
  41,227 recorded expression types, with populated-table, intervention and admission controls.
  Log tmp/order1-wire-framing-final-inference.log. Command cargo test --release --features
  self-host --lib compiler::inference_independence -- --nocapture.
- The full workspace diagnostic run reports 3,037 passes, three failures and five ignored across
  163 reported suites. Terminal session 46391 exited 101. Complete log
  tmp/order1-wire-framing-final-workspace.log. Command cargo test --release --workspace
  --no-fail-fast. All three failures were the old 492-chunk pin against the observed 497.
- After updating those pins, their three complete suites pass 97 tests with two ignored under
  workspace feature unification. Terminal session 85438 exited zero. Log
  tmp/order1-wire-framing-final-counts.log. Command cargo test --release --workspace --test
  selfhost_chunk_names --test selfhost_parse --test wire_self_compile_status --no-fail-fast.
  The whole workspace was not rerun after that repair.
- Final Clippy performed actual Checking and passed after the test edits. Command cargo clippy
  --tests --features signatures,shell,self-host -- -D warnings. Log
  tmp/order1-wire-framing-final-clippy3.log.
- The documentation build passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc
  --workspace --no-deps. Log tmp/order1-wire-framing-final-doc.log.
- The compile-and-verify no_std check exited zero using a warm build. Command cargo check
  --no-default-features --features compile,verify. Log tmp/order1-wire-framing-final-nostd.log.

Seven new integration tests cover all twelve module-driven auxiliary containers and framed
artifacts, nonzero metadata, multiple windows, exact and excessive directory capacity, invalid
input widths, malformed layouts with recomputed valid checksums and corrupted/truncated bodies.
The opcode and operand-pool sections in the full-artifact comparison are still reference inputs.

Initial diagnostic logs remain under tmp/order1-wire-framing-. The first broad run passed 195
and failed one old capacity pin. The first container run failed to compile because it used
Builder instead of WireBuilder. These are superseded by the targeted receipts above. They are
retained alongside the full workspace count-pin failures. No initial diagnostic was erased.

Separate feature configurations, minimum supported Rust versions, Miri, cross-target builds,
advisory scanning and final integration gates remain unverified for this increment.
Final documentation-link, claimed-count, comment-citation and public-document guards pass
24 tests. Command cargo test --release --test documentation_links --test claimed_counts
--test comment_citations --test push_order_claims. Log tmp/order1-wire-framing-final-doc-guards.log.
Formatting and diff checks pass. The final handoff receipt is checked again before commit.
No verification process remains running.

## Next work

Continue the original complete Order 1 plan. The next wire work is module-driven opcode and
operand-pool production, the remaining auxiliary forms and semantic decoding. Existing wire
commands 86 and 96 through 99 provide record and pool primitives. They do not yet consume a
complete semantic Op stream. Audit host marshalling versus Keleusma-derived layout rather than
counting copied bytes as independently produced. Preserve the full encoder and decoder scope.

The shipping unsigned path is still Module::to_bytes in keleusma-cli/src/main.rs. Replace it only
with complete artifact production that preserves supported behavior. The all-stage auxiliary
comparison is a corpus result, not evidence for every supported source program. The old
wire_windowed_via_kel still accepts caller-supplied artifact size and positions and can leave
unknown regions zeroed. It must not become the production serializer unchanged.

Remaining type obligations include general tuple projection identity, arbitrary nested header
syntax, complete binder and occurrence coverage, nominal struct signature layout, one-hop
struct aliases, empty-array inference, mixed-scalar tuple layout, general cast admission and
large enum-dispatch capacities. Preserve existing supported behavior and reference admission.
Complete shipping tests and final gates after the remaining integration. Full Order 1 is active.

## Shadow pilot and mailbox

Task v023-order1-wire-framing retains the initial capacity diagnostic and three bounded claims
for auxiliary identity, stream completion and chunk count. Blind exports precede non-independent
self-labels. Missing usage and elapsed time remain null. The unavailable external result cannot
support an accuracy or development-improvement claim. The later builder-name and chunk-count
failures belong to this same increment, whose first diagnostic was already captured.

Automatic approval review previously rejected sending excerpts to api.typesafe.ai because it
did not recognize destination-and-payload authorization. External evaluation remains pending
that specific request. No retry or bypass occurred. Local capture continues. The pilot
implementation line owns the tracked pilot checkpoint, which is untouched. The reviewed local
export is tmp/order1-wire-framing-pilot-export.json.

The final assigned inbox is empty. No messages were received or sent. The mailbox checkpoint
was exported and reviewed with no tracked change. The pilot export was reviewed for all four
samples, matching input payloads, request hashes, source hashes and non-independent labels. The complete Order 1 goal remains active.
