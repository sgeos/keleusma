# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next. This increment started at 7b161e05. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md is invalid and stale, measured
43 commits behind the starting head. Its previously checked ancestry passes and its unchanged-stage
claim is false. Historical full validity and advisory checks were not all repeated.

## Streaming auxiliary emission

The public wire_auxiliary_from_module_via_kel export now comes from wire_stream_auxiliary.rs.
It does not construct the old module-input blob. The previous bounded emitter is compiled only
for library tests, with a direct comparison control. Other standalone legacy helpers retain
their documented bounds and are not the shipping serialization path.

Keleusma owns name hashing, byte equality, last-equal selection, fresh-name policy, pool offsets
and name indices. Rust retains borrowed strings and BTreeMap buckets keyed only by stage-produced
hashes. Bucket matches never decide equality. Keleusma compares name windows and chooses the latest
matching entry, preserving the reference's overwriting behavior after fresh insertions. The host
appends opaque bytes only on the stage's append decision and checks returned storage coordinates.

The first streaming draft still scheduled name families in Rust. The audit caught that regression
in provenance, and a stage coordinator now requests family, parent and child ordinals. The Rust
transport verifies those requests against its source iteration. Keleusma orders chunk names,
enum types and fresh variants, natives, data-slot groups, constant names and template names.
It rejects wrong-family names, premature template processing and incomplete contributor counts.

Constants use a stage-driven breadth-first queue backed by host references to typed values.
Keleusma selects every node, child range, auxiliary-table index and required name role. The host
marshals intrinsic value facts and appends children at the requested queue position. Generation
checks bind name replies to their pending node. Template field runs and empty checkpoints are
stage-derived. Header sentinels, parameter tags, chunk ranges, opcode offsets and debug offsets
also come from stage decisions. The existing wire record emitters encode the resulting fields.
Struct and enum auxiliary records use their dedicated commands 128 and 129 because command 164
still refuses those two kinds.

Schema commands 64 through 77 implement encoder reset, interning and forest traversal. Command 78
is unused. Commands 79 through 84 handle derived metadata. Commands 85 through 88 coordinate name
contributors. New private blocks are en, bf, ec and np. All previous reader protocols remain.

The former fixed root, name-length, name-event, pool and aggregate module-input caps are removed
from this public path. Wire field widths and storage failures remain explicit limits. Input/output
windows remain bounded. Tests retain byte comparisons above the former thresholds, including
65,536 scalar roots, 65,536-byte names, more than 1,024 names/chunks, repeated large names and large
fresh template runs. Root/source tests cover exact 1,365/1,366 boundaries and 256/257-byte text,
plus a 60,000-byte text literal. The compiler already accepted these source fixtures.

## Shipping integration

Unsigned command-line compilation now calls serialize_unsigned_module with the selected backend.
Backend::SelfHosted routes through wire_module_artifact_via_kel. The reference backend retains
Module::to_bytes. Signing and encryption continue through their dedicated paths.

The dispatch test has an artificial-module discriminator that fails if both branches use the
reference serializer. Command-line tests compile the large forest and long-text fixtures and
compare their artifacts against both the Keleusma emitter and reference bytes. Existing command
controls still execute seven, preserve reproducibility and reject bad types without output files.

## Resources and current evidence

The schema stage has 150 chunks and a 75,876-byte artifact. Shared storage remains 1,216 bytes.
Private state is 5,984 bytes, adding 57 slots across the four encoder blocks. Its largest legacy
input channel uses 197 of 640 occurrence rows. Wire remains unchanged at 561 chunks, 1,294 of
1,365 constant nodes and 638 of 640 occurrence rows. Legacy residency measurements do not describe
the new streaming emitter's input limits. All fourteen source stages still fit those legacy controls.

All fourteen current stage artifacts compare byte-identically through the final workspace test.
Inference independence covers 52,389 recorded types. A collision control uses the equal-length
strings 6a2ef2cd and f41fb47a, both in bucket 0xec8732b2, and checks distinct indices plus latest-fresh
reuse. A compiled comparison executes seven after serialization and loading, so hash equality alone
cannot make the two strings equal. Additional protocol controls check input completeness, wrong
roles, contributor order, constant-before-template sequencing and final completion.

## Verification receipts

Runs use empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited while its run remained live.

- The two new admission controls initially fail at the old caps. Session 17418 exits 101 in
  tmp/order1-encoder-first.log. That diagnostic is the pilot triage case.
- The first streaming build references the wrong enum-discriminant field. Session 33833 exits 101
  in tmp/order1-encoder-stream-first.log. After correction, both admission controls pass in
  tmp/order1-encoder-stream-tests.log, session 4947 exits zero.
- A rich constant test catches command 164 refusing struct/enum auxiliary records. Session 51049
  exits 101 in tmp/order1-encoder-families.log, with all other family checks passing. The driver now
  uses commands 128 and 129. The subsequent constants, framing and admission run passes in
  tmp/order1-encoder-boundaries.log, session 96796 exits zero.
- Clippy initially requests as_chunks for a constant-width loop. Session 39730 exits 101 in
  tmp/order1-encoder-clippy.log. That loop is corrected without changing its grouping algorithm.
- Nineteen ordered-emitter family, admission and accounting tests pass in
  tmp/order1-encoder-order-tests.log. Command cargo test --release --features self-host --test
  selfhost_wire_encoder_capacity --test selfhost_wire_constants --test selfhost_wire_natives
  --test selfhost_wire_templates --test wire_private_state_accounting --no-fail-fast -- --nocapture.
  Session 85652 exits zero.
- The final three admission and collision-execution tests pass in tmp/order1-encoder-final-admission.log.
  Command cargo test --release --features self-host --test selfhost_wire_encoder_capacity
  -- --nocapture. Session 36312 exits zero.
- Forty-four wire unit checks pass in tmp/order1-encoder-final-protocol.log. Command cargo test
  --release --features self-host --lib selfhost::wire_ -- --nocapture. Session 75278 exits zero.
- Final legacy capacity and inference-independence checks pass in tmp/order1-encoder-order-capacity.log
  and tmp/order1-encoder-order-inference.log. Commands cargo test --release --features self-host
  --test selfhost_typecheck the_stage_is_measured_against_the_real_stage_sources -- --nocapture,
  and cargo test --release --features self-host --lib compiler::inference_independence -- --nocapture.
  Sessions 63531 and 37930 exit zero.
- The command-line route discriminator passes in tmp/order1-encoder-cli-route.log. Command cargo
  test --release -p keleusma-cli --bin keleusma unsigned_serializer_dispatch_reaches_the_selected_backend
  -- --nocapture. Session 94387 exits zero.
- Seven command-line refusal, emission and reproducibility tests pass in tmp/order1-encoder-cli.log.
  Command cargo test --release -p keleusma-cli --test self_hosted_refusal --test compile_reproducible
  --no-fail-fast -- --nocapture. Session 94000 exits zero.
- The full workspace passes 3,168 tests with zero failures and five ignored across 182 suites.
  Command cargo test --release --workspace --no-fail-fast. Log tmp/order1-encoder-workspace.log.
  Session 95708 exits zero. This includes the final fourteen-stage comparison and monomorphization
  controls after the contributor coordinator was added.
- Twenty-five no-default integration/accounting tests pass in tmp/order1-encoder-final-no-default.log.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_wire_encoder_capacity --test selfhost_wire_constants --test selfhost_wire_natives --test
  selfhost_wire_templates --test selfhost_wire_private --test wire_private_state_accounting
  --no-fail-fast. Session 47900 exits zero.
- Forty-four no-default wire unit tests pass in tmp/order1-encoder-final-no-default-unit.log.
  Command cargo test --release --no-default-features --features self-host --lib selfhost::wire_
  -- --nocapture. Session 92870 exits zero.
- Fresh final Clippy passes with actual Checking. Command cargo clippy --workspace --tests
  -- -D warnings. Log tmp/order1-encoder-shipping-clippy.log. Session 10611 exits zero.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings. Command cargo
  doc --workspace --no-deps. Log tmp/order1-encoder-shipping-doc.log. Session 75229 exits zero.
- The no_std check passes with actual Checking. Command cargo check --no-default-features
  --features self-host. Log tmp/order1-encoder-shipping-nostd.log. Session 57418 exits zero.

The four documentation guards pass 24 tests after the handoff and plan updates. Command cargo
test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-encoder-doc-guards.log. Session 38077 exits zero.
Formatting and diff checks pass. No verification process remains live at handoff.

No production or test content changed after the full workspace run. Documentation updates followed.
The measured root-library count is 1,384. The integration source inventory is 1,584 test functions
across 154 files. Full feature matrices, minimum supported toolchains, Miri, cross-target builds,
advisory scanning and final integration gates remain required for Order 1.

## Next work

Move the remaining host run processing into Keleusma. wire_stream_auxiliary::slot_runs still groups
logical data slots using name and visibility equality, and its caller splits those groups at
u16::MAX. shared_slot_fields in selfhost/mod.rs still detects constant-stride runs, calculates
record starts and splits those runs. These are serialization decisions, and shipping dispatch does
not establish that they are self-hosted.

Preserve reference details while moving them. Shared grouping compares kind and len, uses wrapping
u32 offset differences to choose a stride, starts a two-slot run only when that stride fits u16,
and extends while the same difference persists. Singletons use stride zero. Both tables split at
65,535 slots. Shared first_slot counts logical slots and each split's offset advances from the
run's starting offset. Empty tables and runs spanning adjacent source fields need explicit controls.
Large stages contain hundreds of thousands of logical slots, so use bounded batches and avoid
re-interning each repeated slot name through a separate sequence of host requests when possible.
The current name coordinator receives grouped slots as input, so change that interface alongside
the grouping implementation instead of relabelling the host grouping as a stage decision.

Retain all type obligations in ORDER1_COMPLETION_PLAN.md, including projection identity, nested
headers, binders, occurrences, nominal layouts, aliases, empty arrays, mixed tuples, casts and
enum capacity. After remaining changes, refresh current-stage identity, inference independence,
monomorphization, shipping behavior and final gates. Full Order 1 remains active.

## Shadow pilot and interline

Task v023-order1-encoder records the first admission diagnostic and three bounded claims for
admission preservation, collision behavior and unsigned shipping dispatch. Baselines precede
review. All four labels are non-independent. Exact payloads, request hashes and source hashes
were reviewed in tmp/order1-encoder-pilot-export.json. The bounded increment is resolved.

External evaluation was not attempted under the earlier automatic approval rejection of its
destination and payload. No accuracy claim is made. The peer-owned tracked pilot checkpoint was
not overwritten. The inbox is empty and no peer messages were sent. The interline checkpoint was
exported and reviewed with no diff.
