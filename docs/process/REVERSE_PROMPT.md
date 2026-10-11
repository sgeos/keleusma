# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at 684cc0e5. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md remains invalid and stale,
measured 40 commits behind the starting head. Its ancestry was previously checked and its
unchanged-stage-source claim is false. Historical full validity and advisory checks were not
all repeated.

## Template and enum layouts

wire_decode_struct_templates_via_kel and wire_decode_enum_layouts_via_kel implement strict
standalone reads. Both validate the shared template and enum-layout ranges before reading names,
matching LayoutTable::parse. wire_decode_layout_metadata_via_kel returns independent optional
families, matching the auxiliary decoder's two defaulting operations. A malformed name in one
family does not discard a valid other family. A malformed shared view invalidates both.

Keleusma requests each record, name and variant, retains discriminants and minimum payloads,
and controls completion. Host code supplies typed records and constructs values. NameReadError
now distinguishes explicit name data rejection from execution, transport and allocation failure.
Optional layout handling sends data rejection back to the stage for its discard decision.
Other failures propagate. A direct protocol control exercises that distinction.

Commands 22 through 29 belong to the schema stage's layout protocol. Active transactions exclude
incompatible commands. Nested name validation is allowed, and an explicit failed-name acknowledgement
can cancel the child name transaction before discarding the optional family.

## Data layouts

wire_decode_data_layout_via_kel preserves absent versus empty DATA_SLOTS. If absent, unrelated
layout tables are not interpreted. If present, required table presence and strides are checked.
All slot names, visibility tags and nonzero run lengths are validated before decoding names.
The stage supplies name requests, normalized visibility and repeat counts. The host copies those
values into named slots and acknowledges the completed count.

Shared offsets are calculated in Keleusma and transported five at a time. The last offset of a
run is checked for u32 overflow before emission. Kind and length values are retained. The owned
reference reader does not use first_slot or enforce private-placement slot bounds, so these
codec paths preserve that behavior. Later verifier constraints are separate.

The stage requests initializer record zero and selects either repeated Int(0) values or a
separate constant-pool operation. Extra initializer records remain unobserved. An empty stored
range still requires a valid constant table, while elision does not. Completion checks the actual
initializer count. Commands 30 through 40 implement the data protocol.

StructTemplate, EnumVariantDisc, EnumLayout and DataSlot now derive structural equality for
complete value comparisons. Their fields and wire representation are unchanged.

## Corpus and resources

The schema stage has 67 chunks and a 32,964-byte artifact. Shared storage remains 1,216 bytes.
Private storage is 2,592 bytes, adding seventeen layout slots and twenty data slots to the previous
state. Its largest legacy input channel uses 87 of 640 occurrence rows. Wire remains unchanged
at 561 chunks, 1,290 of 1,365 constant nodes and 638 of 640 occurrence rows. All fourteen sources
fit the selected caps. Inference independence covers 48,424 recorded expression types.

All fourteen artifact comparisons now include decoded templates, enum layouts and data layout,
as well as previous decoder families. Initializers are compared through canonical constant
encoding to preserve floating-point payload comparisons. The runtime control removes the original
layout, installs the decoded result, and observes shared input plus persistent initialization
across two calls. A separate control reaches 65,536 logical slots across record and batch boundaries.

## Verification receipts

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- Initial checks pass in tmp/order1-layouts-first.log and tmp/order1-layouts-data-check.log.
  Sessions 97120 and 62786 exited zero. No diagnostic triage case was manufactured.
- The initial three layout tests pass in tmp/order1-layouts-tests.log, session 72601 exited zero.
  The initial six data and layout controls pass in tmp/order1-layouts-data-tests.log,
  session 70901 exited zero. Final workspace and no-default runs include the expanded controls.
- All six artifact tests and two accounting checks pass in tmp/order1-layouts-artifacts.log.
  Command cargo test --release --features self-host --test selfhost_artifact_identity --test
  wire_private_state_accounting --no-fail-fast -- --nocapture. Session 3615 exited zero.
- Thirty-four initial wire unit checks pass in tmp/order1-layouts-protocol.log. Command cargo
  test --release --features self-host --lib selfhost::wire_ -- --nocapture. Session 84551 exited
  zero. The data-protocol control was added afterward and runs in the final suites.
- Four inference checks pass in tmp/order1-layouts-inference.log. Command cargo test --release
  --features self-host --lib compiler::inference_independence -- --nocapture.
  Session 42882 exited zero. The final production stage source is unchanged from this measurement.
- The real-stage type-input check accepts all fourteen sources in tmp/order1-layouts-capacity.log.
  Command cargo test --release --features self-host --test selfhost_typecheck
  the_stage_is_measured_against_the_real_stage_sources -- --nocapture. Session 34589 exited zero.
- The final release workspace passes 3,138 tests with zero failures and five ignored across
  178 suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-layouts-workspace.log. Session 46484 exited zero.
- Ten no-default integration and accounting checks pass in tmp/order1-layouts-no-default.log.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_wire_data_decode --test selfhost_wire_layout_decode --test
  wire_private_state_accounting --no-fail-fast. Session 48003 exited zero.
- Thirty-five no-default wire unit checks pass in tmp/order1-layouts-no-default-unit.log.
  Command cargo test --release --no-default-features --features self-host --lib selfhost::wire_
  -- --nocapture. Session 96137 exited zero.
- Workspace Clippy performed actual Checking and passed. Command cargo clippy --workspace --tests
  -- -D warnings. Log tmp/order1-layouts-final-clippy.log. Session 47749 exited zero.
- Documentation performed actual Documenting and passed with RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-layouts-doc.log.
  Session 36510 exited zero.
- The no_std check performed actual Checking and passed. Command cargo check --no-default-features
  --features self-host. Log tmp/order1-layouts-nostd.log. Session 84673 exited zero.

No production or test contents changed after the full workspace run. Documentation updates
followed it. CLAUDE.md now records the measured 1,375 executed root-library tests and the source
inventory of 1,565 integration test functions across 150 files. Full feature matrices, minimum
supported toolchains, Miri, cross-target builds, advisory scanning and final integration gates
remain required for Order 1. The four documentation guards pass 24 tests after the handoff and
plan updates. Command cargo test --release --test documentation_links --test claimed_counts
--test comment_citations --test push_order_claims. Log tmp/order1-layouts-doc-guards.log.
Session 91225 exited zero. Formatting and diff checks pass. The guards are repeated after this
receipt update in tmp/order1-layouts-final-doc-guards.log. No process remains running at handoff.

## Next work

Implement parameter-tag and debug decoding, then assemble complete modules with framing and
opcode checks. Reuse the current schema component protocols, including explicit data-versus-
execution failure distinctions. Signature, name, constant, template, enum and data-layout value
paths are available. A successful component result does not establish a complete module decoder.

Read findings for the remaining implementation follow. DebugPool::decode reads strings, spans,
opaque type blobs and records in input order. It does not reject trailing bytes, validate pool
indices or operand arity, or sort records. Its prose describes canonical input, but its behavior
must govern compatibility. Strings require UTF-8 and type blobs remain opaque. Parameter type
tags include Float regardless of the floats feature. Sources are src/debug_meta.rs and
src/wire_schema.rs type_tag_from_byte.

module_from_wire_bytes checks every operand-pool record's parity, including unused entries, but
decodes opcode records only through chunk spans. Do not decode the whole opcode section first
and thereby reject unused records the reference ignores. It bounds each chunk span without an
explicit requirement that the starting byte offset be a multiple of four. Trailing frame bytes
are truncated to the declared total length, and an optional shebang prefix is stripped. Recheck
these behaviors before composing the existing helpers. Source src/wire_format.rs.

Continue semantic work in the schema stage. Wire still has only two occurrence rows free.
Measure every source against both node and type-input limits. Encoder forest, name and input
bounds still need admission-preserving coverage. Shipping unsigned output still calls
Module::to_bytes. Preserve supported source behavior beyond the current stage corpus.

Retain all type obligations in the completion plan, including projections, nested headers,
binders, occurrences, nominal layouts, aliases, empty arrays, mixed tuples, casts and enum capacity.
Refresh all-stage, monomorphization, inference-independence, shipping and final integration evidence
as those changes land. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-layouts retains three bounded completion claims for failure classification,
runtime data-layout use and all-stage layout values. No failed or unexpectedly empty run required
diagnostic triage. Blind exports preceded non-independent self-labels. All three request hashes,
exact payloads and source hashes were reviewed in tmp/order1-layouts-pilot-export.json. The bounded
layout increment is resolved. Usage and elapsed time remain null. No independent accuracy claim
is available.

External evaluation remains unavailable under the prior automatic approval rejection for the
external destination and payload. Do not retry or bypass it. Local capture continues. The pilot
implementation line owns the tracked pilot checkpoint.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
The mailbox checkpoint was exported and reviewed with no tracked difference. Publication requires
explicit in-session confirmation.
