# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at ac3bf702. This handoff belongs
to the scoped commit carrying opcode production and signature numbering. No push, merge or
publication occurred. Preserve the unrelated untracked nvim.log. V0.3.X remains separate.

The legacy HANDOFF.md is invalid and stale. It was 29 commits behind the starting head.
Ancestry passes and its unchanged-stage-source claim fails. Its historical advisory and full
validity checks were not all repeated. Current plans and this handoff carry the live state.
Read Appendix B before tracked writes. Public bytecode version remains 2, public opcode count
remains 66 and selected type-stage caps remain 640.

## Implemented work

Commands 193 and 194 encode semantic opcode fields. Keleusma selects inline or pooled forms,
assigns pool ordinals, packs endian fields, writes sentinels and computes parity. The Rust
transport does not call the reference opcode encoder. All 66 identifiers and composite forms
have differential controls. Oversized fields are refused before output or ordinal changes.
The complete unsigned helper rejects signing and encryption flags.

Commands 195 through 200 derive signature numbering from semantic shape fields. Parameter shapes
append unshared. Return and resume shapes reuse the first identical earlier record. The host
retains emitted shape bytes and replays them in windows of at most 49,152 bytes. Keleusma searches
and numbers the records. It requires complete scan coverage before accepting a single shape.
Invalid fields, incomplete or excessive windows and out-of-order commands preserve pending state.
Explicit reset discards an unfinished signature. Legacy commands are refused during a pending
signature, and framing refuses these signature commands while a framing stream is active.

The auxiliary artifact producer now calls wire_signature_sections_via_kel. It no longer calls
Rust signature_tables to number this path. The older wire_shapes_via_kel and
wire_signatures_via_kel comparison helpers still use the reference numbering and say so.
The replay transport trusts the host to preserve its own emitted records. It does not authenticate
host memory. This is an encoder protocol, not a semantic artifact decoder.

All twelve pipeline-produced modules emit complete artifacts through Keleusma and match the
reference. The actual comparison calls wire_module_artifact_via_kel on self_host_compile_scratch
output. No reference section bytes, region positions or artifact lengths enter that path.

This does not complete Order 1. Native records and native return shapes, templates, debug pools,
composite and named constants, private composite slots and nonempty non-elided initializer pools
remain refused by the new auxiliary helper. Parameter tags, record presence and scalar metadata
retain their documented host provenance. Semantic decoding is not integrated. The shipping
unsigned path still calls Module::to_bytes in keleusma-cli/src/main.rs near line 705.

## Capacity and memory accounting

The final wire stage has 516 chunks, highest command 200 and a 141,436-byte artifact. Its module
input uses 48,233 of 49,152 bytes, leaving 919. Wire contributes 625 name events. Parse remains
the name maximum at 776 of 1,024. Constant nodes use 1,291 of 1,365, leaving 74 and retaining
the standing 64-node reserve. No buffer cap increased.

Opcode growth initially used 1,344 nodes. Sharing 65 literal accessors through four helpers
reduced that to 1,283. Signature growth then used 1,325 nodes. Sharing 39 more accessors through
five helpers reduced it to 1,291. These aliases add one fixed call level. The source offset
comparison follows exactly one alias hop. Throughput has not been benchmarked.

Private Word fields occupy 32-byte Value slots. The earlier framing handoff's 32-byte total was
wrong. Framing costs 128 bytes, opcode state 256 bytes and signature state 352 bytes. The runtime
already used the correct allocation rule. wire_private_state_accounting compiles the actual
three declarations and checks both header and persistent arena capacity.

## Verification

Cargo used an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs used
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- The release workspace diagnostic run completed with 3,048 passes, one failure and five
  ignored tests across 165 reported suites. Terminal session 67903 exited 101. Command cargo
  test --release --workspace --no-fail-fast. Log tmp/order1-signatures-workspace.log. Its only
  failure was the outdated integration-file count in CLAUDE.md. Production tests passed.
  The inventory and the two Clippy iterator findings were repaired after that terminal result.
  No production code changed afterward. Final affected checks are recorded below.
- Integrated comparisons passed six artifact tests, three signature tests and the node-reserve
  test. All twelve complete artifacts agree. The wire suite passed 182 and failed only its
  old byte-size pin, repaired before the workspace run. Command cargo test --release --features
  self-host --test module_input_node_budget --test selfhost_wire_signatures --test selfhost_wire
  --test selfhost_artifact_identity --no-fail-fast -- --nocapture. Log
  tmp/order1-signatures-integrated.log.
- The signature protocol regression passed. Command cargo test --release --features self-host
  --lib selfhost::wire_signatures -- --nocapture. Log tmp/order1-signatures-protocol.log.
- Four inference tests passed. Clearing 42,195 expression types preserved all twelve artifacts.
  Command cargo test --release --features self-host --lib compiler::inference_independence
  -- --nocapture. Log tmp/order1-signatures-inference.log.
- Documentation passed with RUSTDOCFLAGS set to -D warnings. Command cargo doc --workspace
  --no-deps. Log tmp/order1-signatures-doc.log.
- The warm no_std check passed. Command cargo check --no-default-features --features
  compile,verify. Log tmp/order1-signatures-nostd.log.

The repaired signature and private-accounting tests pass four tests. The first repaired
documentation run fixed the inventory but failed because the edit changed a phrase used by
the feature-set guard. The original guarded phrase was restored without changing that guard.
Final Clippy performed actual Checking and passed in tmp/order1-signatures-clippy2.log.
The final affected run passes 28 tests, covering the four documentation guards, signature
comparisons and private accounting. Command cargo test --release --workspace --test claimed_counts
--test documentation_links --test comment_citations --test push_order_claims --test
selfhost_wire_signatures --test wire_private_state_accounting --no-fail-fast. Log
tmp/order1-signatures-repaired2.log. Terminal session 87919 exited zero. The later Clippy run
also performed actual Checking and passed in tmp/order1-signatures-clippy3.log. Formatting and
git diff --check pass. The full workspace was not repeated after the documentation and test-only
repairs. Final documentation guards are repeated after this receipt update. No process is left
running at handoff.

The first signature attempt failed parsing because local mut syntax is not supported. Private
working fields repaired it. The next run passed three signature comparisons and failed only the
constant reserve. Restoring literal sharing repaired that guard. Integrated comparison then
passed every artifact and failed only the stale input-byte pin. The first Clippy run reported
two chunks_exact_mut style violations in the new test. The workspace found a stale CLAUDE.md
integration inventory. These diagnostics remain in tmp/order1-signatures- logs. No guard was
weakened. The inventory now records 1,505 test functions in 137 integration source files.

Separate feature configurations, minimum supported Rust versions, Miri, cross-target builds,
advisory scanning and final integration gates remain required. A green workspace result does
not establish those gates or complete the missing implementation.

## Next work

Complete the remaining auxiliary forms and integrate semantic decoding, then wire complete
artifact output into the shipping tool while preserving every supported behavior. Native
return shapes must share the same interner as chunk signatures. Encoding a separate native
shape table would change indices. Preserve native-name and native-return vectors independently,
since the reference permits different lengths. Existing low-level record writers and readers
cover the wire shapes, but their host-derived values do not prove independent production.

The old wire_windowed_via_kel accepts reference-supplied positions and may skip unknown regions.
Do not use it unchanged for production serialization. A complete decoder must validate parity,
tags, indices and field semantics before reconstructing typed instructions. Envelope integrity
alone does not establish that. Reuse bounded window transport and measure stage growth first.

Remaining type obligations include general tuple projection identity, arbitrary nested header
syntax, complete binder and occurrence coverage, nominal struct signature layout, one-hop
struct aliases, empty-array inference, mixed-scalar tuple layout, general cast admission and
large enum-dispatch capacities. Preserve reference admission. Refresh shipping comparisons and
complete final gates after the remaining work. The full Order 1 goal remains active.

## Shadow pilot and mailbox

The local opcode task v023-order1-wire-ops retained one reserve diagnostic and three bounded
claims. Its four source and request hashes were reviewed before signature edits began. Later
signature changes supersede its stage snapshot, which remains historical evidence. Its completed
export is tmp/order1-wire-ops-pilot-export.json. Usage and elapsed time remain null.

Task v023-order1-signatures contains the first parser diagnostic and three bounded completion
claims. Blind exports preceded labels. Four request hashes, payloads and source hashes were
reviewed against tmp/order1-signatures-pilot-export.json. The bounded task outcome is recorded
as resolved, with usage and elapsed time null. Later capacity, byte-pin, Clippy and documentation
diagnostics are retained in their full logs and belong to the same increment.

Self-review changed two initial supports judgments to unsupported. The artifact excerpt omits
the actual byte-equality assertion and opcode call path. The reserve excerpt omits the threshold
and its unchanged status. The fuller local code and logs support the implementation statements,
but those references are not sent to the classifier. Future payloads must carry the decisive
assertions. The interning excerpt supports its narrower test claim. These are self-reviews,
not independent accuracy measurements.

External evaluation remains unavailable under the previous automatic approval rejection for
external destination and payload authorization. No retry or bypass occurred. Labels are
non-independent self-reviews. The pilot implementation line owns the tracked pilot checkpoint,
which is untouched. No independent accuracy claim is supported.

The assigned inbox was empty at startup and before handoff. No messages were received or sent.
The exported mailbox checkpoint was reviewed and has no tracked difference.

Publication requires explicit in-session confirmation.
