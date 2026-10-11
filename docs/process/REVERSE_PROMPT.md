# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next. This increment started at 9a97397c. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md is invalid and stale, measured
41 commits behind the starting head. Its ancestry check against origin/v0.2.3 passes, while its
unchanged-stage-source claim is false. Historical full validity and advisory checks were not
all repeated. The latest roadmap, completion plan and this file describe current work.

## Parameter tags and debug pools

wire_decode_parameter_tags_via_kel validates selected ranges and all eight type tags through
Keleusma commands 41 through 43. An absent pool ignores its declared range. Float is a recognized
parameter tag independently of floating-point execution support. Sixteen tags fit each reply.

wire_decode_debug_pool_via_kel uses commands 44 through 47. Keleusma requests raw fields, lengths,
string and type payloads and operand batches. String bytes pass through the existing name encoding
validator. Type blobs remain opaque. The reader preserves record order, unresolved pool indices,
record arity and trailing bytes, matching DebugPool::decode. It does not apply later consumer
validation. Count checks precede reservations, and field/payload bounds precede transport.

Controls exercise all 256 type and record-tag bytes, truncation at every boundary of a mixed
fixture, malformed text, noncanonical record order, opaque invalid-text blobs, long strings,
65,535 operands and unaligned input. Protocol tests reject wrong lengths, foreign commands,
overfeeding and premature completion while preserving valid continuation where applicable.

## Complete auxiliary assembly

wire_decode_aux_body_via_kel composes existing directory, record and semantic readers.
Commands 48 through 56 check required metadata dimensions, chunk/native names and block tags,
select header record zero, normalize entry presence and supply template, parameter and debug
ranges. The host constructs owned values from these decisions. The routing function is separate
from transaction guards to keep nesting shallow. Pending chunk assembly permits parameter and
name subtransactions on the same schema machine.

Invalid debug spans become absent. Valid empty spans remain present. Debug payloads stay opaque
at this layer. A zero-chunk module skips constant decoding. Templates, enums and signatures retain
their existing optional-family policies. Native names and return shapes retain independent
lengths. Header widths and entry bounds are copied here, with later loader checks still separate.
Allocation, transport and execution errors propagate through component calls. Exact diagnostic
variants and error ordering for inputs with multiple defects are not established by the tests.

All fourteen complete stage artifacts and auxiliary values match the reference. The runtime
control keeps the original instruction bodies, replaces auxiliary fields and observes shared
input plus persistent initialization, returning 12 then 17. This establishes auxiliary use in
execution. Complete outer framing and module loading remain open.

## Resources

The schema stage has 96 chunks and a 46,964-byte artifact. Shared storage remains 1,216 bytes.
Private state uses 3,264 bytes, including three parameter slots, nine debug slots and nine module
slots added in this increment. Its largest legacy input channel uses 124 of 640 occurrence rows.
Wire remains unchanged at 561 chunks, 1,290 of 1,365 constant nodes and 638 of 640 occurrence rows.
All fourteen sources fit the approved caps. Inference independence covers 49,723 recorded types.

## Verification receipts

Runs use empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- Five initial debug and accounting checks pass in tmp/order1-debug-aux-first.log.
  Session 79684 exited zero.
- The first auxiliary test build fails because a test omitted the named Scalar field.
  Log tmp/order1-debug-aux-tests.log records exit 101. The next run exposes fixture mistakes
  involving padded pool lengths and literal syntax. Session 27059 exits 101 in
  tmp/order1-debug-aux-tests-fixed.log. Its four debug tests pass. These failures were corrected.
- All six artifact checks and four initial auxiliary checks pass in
  tmp/order1-debug-aux-artifacts.log. Command cargo test --release --features self-host --test
  selfhost_wire_aux_decode --test selfhost_artifact_identity --no-fail-fast. Session 62008 exits zero.
- All fourteen sources pass the legacy capacity check in tmp/order1-debug-aux-capacity.log.
  Command cargo test --release --features self-host --test selfhost_typecheck
  the_stage_is_measured_against_the_real_stage_sources -- --nocapture. Session 7305 exits zero.
- Four inference-independence checks pass in tmp/order1-debug-aux-inference.log.
  Command cargo test --release --features self-host --lib compiler::inference_independence
  -- --nocapture. Session 19170 exits zero.
- Three monomorphization controls pass in tmp/order1-debug-aux-monomorphize.log.
  Command cargo test --release --features self-host --test selfhost_monomorphize_identity.
  Session 27167 exits zero.
- Thirty-eight wire unit checks pass in tmp/order1-debug-aux-protocol.log.
  Command cargo test --release --features self-host --lib selfhost::wire_ -- --nocapture.
  Session 31338 exits zero.
- Five auxiliary checks and two accounting checks pass in tmp/order1-debug-aux-runtime.log.
  Command cargo test --release --features self-host --test selfhost_wire_aux_decode --test
  wire_private_state_accounting -- --nocapture. Session 87225 exits zero.
- The full release workspace passes 3,150 tests with zero failures and five ignored across
  180 suites. Command cargo test --release --workspace --no-fail-fast.
  Log tmp/order1-debug-aux-workspace.log. Session 1046 exits zero.
- Eleven no-default integration and accounting checks pass in tmp/order1-debug-aux-no-default.log.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_wire_aux_decode --test selfhost_wire_debug_decode --test wire_private_state_accounting
  --no-fail-fast. Session 45343 exits zero.
- Thirty-eight no-default wire unit checks pass in tmp/order1-debug-aux-no-default-unit.log.
  Command cargo test --release --no-default-features --features self-host --lib selfhost::wire_
  -- --nocapture. Session 91250 exits zero.
- Fresh workspace Clippy passes with actual Checking in tmp/order1-debug-aux-final-clippy.log.
  Command cargo clippy --workspace --tests -- -D warnings. Session 29774 exits zero.
- Fresh documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-debug-aux-final-doc.log.
  Session 3026 exits zero.
- The no_std check passes with actual Checking in tmp/order1-debug-aux-nostd.log.
  Command cargo check --no-default-features --features self-host. Session 47250 exits zero.

The four documentation guards pass 24 tests after the handoff updates. Command cargo test
--release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-debug-aux-doc-guards.log. Session 68344 exits zero.
Formatting and diff checks pass. No verification process remains live at handoff.

No production or test content changed after the workspace run. Documentation updates followed.
The measured root-library count is 1,378. The source inventory contains 1,574 integration test
functions across 152 files. Full feature matrices, minimum supported toolchains, Miri, cross-target
builds, advisory scanning and final integration gates remain required for Order 1.

## Next work

Implement complete outer framing and module reconstruction using the auxiliary reader.
The existing wire framing reader requires canonical unsigned offsets and an exact input length,
which is narrower than module_from_wire_bytes. Preserve valid noncanonical section placement,
trailing frame bytes, shebang stripping and signature-extension metadata consistency. The low-level
reference loader validates extension structure without performing signature authentication here.
Read its implementation before composing helpers. Public security policy is unchanged.

Check every operand-pool entry's parity, including unused entries. Decode only opcode spans
selected by chunks, because unused invalid opcode records are ignored by the reference. Chunk
start offsets have no explicit multiple-of-four requirement. Compare mirrored outer/auxiliary
header fields and runtime width maxima. Carry aux_arena_bytes and persistent_composite_bytes
from the outer header, then decode selected debug pools and construct complete Module values.

Prefer schema-stage metadata work because wire still has only two legacy occurrence rows free.
An independent checksum-start command could reuse wire's existing incremental CRC state without
requiring the canonical framing reader. This is a proposal, not an implemented capability.
Measure all stages against node and type-input limits after any change.

Continue source-admission capacity coverage beyond the stage corpus. Shipping unsigned output
still calls Module::to_bytes. Retain every type obligation in the completion plan, including
projections, nested headers, binders, occurrences, nominal layouts, aliases, empty arrays,
mixed tuples, casts and enum capacity. Refresh all-stage identity, inference independence,
monomorphization, shipping behavior and final gates as those changes land. Full Order 1 is active.

## Shadow pilot and interline

Task v023-order1-debug-aux contains the first compiler diagnostic and three bounded claims covering
stage auxiliary equality, runtime use and noncanonical debug order. Baselines precede review.
All four labels are non-independent. Exact payloads, request hashes and source hashes were reviewed
in tmp/order1-debug-aux-pilot-export.json. The bounded increment is resolved. No external evaluation
was attempted under the earlier automatic approval rejection of the destination and payload.
No accuracy claim is made. The peer-owned tracked pilot checkpoint was not overwritten.

The v0.2.3 inbox is empty. No peer messages were sent. The interline checkpoint was exported
and reviewed with no diff. Preserve the separate pilot checkpoint ownership.
