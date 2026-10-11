# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next. This increment started at 4e6c2a5f. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md is invalid and stale, measured
44 commits behind the starting head. Its previously checked ancestry passes and its unchanged-stage
claim is false. Historical full validity and advisory checks were not all repeated.

## Run derivation now runs in Keleusma

wire_run_encode.rs replaces the public emitter's host run algorithms. The helper gives logical
DataSlot and SharedSlotLayout values to schema-stage requests. It performs bounds checks,
allocation and transport, stores returned plans and checks completion counts. It does not compare
slot names, choose strides, group entries or split runs. The name coordinator receives the checked
count from the Keleusma data-slot grouper's output. The legacy host algorithms retain their older
callers, but wire_stream_auxiliary no longer uses them.

Commands 89 through 93 implement data-slot grouping. The stage requests a group head, batches up
to eight candidate lengths/visibilities, then compares names in windows of up to 64 bytes. One
reference window and up to eight candidate windows fit the existing byte buffer. The earliest
difference is retained across windows. Empty names skip byte comparison. Group descriptors carry
source indices, normalized visibility and counts. The stage splits each group at 65,535 slots.
The host retrieves the chosen source name and stores each returned piece for later record emission.

Commands 94 through 97 implement shared-layout grouping. Up to five offset/kind/length triples
arrive per reply. The stage chooses a stride from wrapping u32 differences when the first pair
matches kind and length and the stride fits u16. Later entries extend only with the same difference.
Singletons use zero stride. Split tails retain the original stride, including one-slot tails.
Every output record's start offset, logical first slot, count and stride are stage-derived. Start
offsets use u32 modular arithmetic, matching the release reference encoder. Runtime/verifier
constraints remain separate from codec grouping.

Malformed replies are refused before consuming a batch. Active transactions exclude unrelated
commands. Both drivers have input-derived request budgets and validate returned source/storage
coordinates. No extra shared buffer was added. Private dg and sr blocks add fourteen slots each.
The schema stage has 170 chunks, an 85,508-byte artifact, 1,216 shared bytes and 6,880 private bytes.
Its legacy occurrence channel uses 221 of 640 rows. Wire remains unchanged. Inference independence
covers 53,213 recorded types across fourteen stage sources.

## Verification receipts

Runs use empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- Five existing private-data tests and two accounting checks pass in tmp/order1-runs-first.log.
  Command cargo test --release --features self-host --test selfhost_wire_private --test
  wire_private_state_accounting --no-fail-fast -- --nocapture. Session 23499 exits zero.
- Four initial differential controls pass in tmp/order1-runs-differential.log. Session 92787 exits
  zero. The final five controls, including runtime execution across the split boundary, pass in
  tmp/order1-runs-execution.log. Command cargo test --release --features self-host --test
  selfhost_wire_run_encode -- --nocapture. Session 30245 exits zero.
- The legacy capacity check accepts all fourteen sources in tmp/order1-runs-capacity.log.
  Command cargo test --release --features self-host --test selfhost_typecheck
  the_stage_is_measured_against_the_real_stage_sources -- --nocapture. Session 62752 exits zero.
- Forty-six wire unit checks pass in tmp/order1-runs-protocol.log. Command cargo test --release
  --features self-host --lib selfhost::wire_ -- --nocapture. Session 95677 exits zero.
- Six artifact checks and three monomorphization controls pass in tmp/order1-runs-artifacts.log.
  Command cargo test --release --features self-host --test selfhost_artifact_identity --test
  selfhost_monomorphize_identity --no-fail-fast -- --nocapture. Session 34314 exits zero.
- Four inference checks pass in tmp/order1-runs-inference.log. Command cargo test --release
  --features self-host --lib compiler::inference_independence -- --nocapture. Session 66207 exits zero.
- Seven no-default integration/accounting checks pass in tmp/order1-runs-no-default.log.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_wire_run_encode --test wire_private_state_accounting --no-fail-fast.
  Session 34220 exits zero.
- Forty-six no-default wire unit checks pass in tmp/order1-runs-no-default-unit.log. Command cargo
  test --release --no-default-features --features self-host --lib selfhost::wire_ -- --nocapture.
  Session 33183 exits zero.
- The full workspace passes 3,175 tests with zero failures and five ignored across 183 suites.
  Command cargo test --release --workspace --no-fail-fast. Log tmp/order1-runs-workspace.log.
  Session 95655 exits zero. This includes all fourteen stage artifacts and the shipping controls.
- Fresh final Clippy passes with actual Checking. Command cargo clippy --workspace --tests
  -- -D warnings. Log tmp/order1-runs-final-clippy.log. It exits zero directly without a live session.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings. Command cargo
  doc --workspace --no-deps. Log tmp/order1-runs-doc.log. Session 99363 exits zero.
- The no_std check passes with actual Checking. Command cargo check --no-default-features
  --features self-host. Log tmp/order1-runs-nostd.log. Session 31454 exits zero.

The run-encoding controls cover empty tables/names, Unicode names longer than the byte window,
first mismatches at several byte and batch boundaries, visibility changes, zero/maximum strides,
nonmonotonic offsets, wrapped differences and 65,535/65,536/131,071-slot splits. The execution control
compiles a 65,536-byte shared array, emits it through Keleusma, decodes the whole module through
Keleusma and reads 17 and 23 from indices 65,534 and 65,535. The original module is dropped first.

The four documentation guards pass 24 tests after the handoff and plan updates. Command cargo
test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-runs-doc-guards.log. Session 84113 exits zero.
Formatting and diff checks pass. No verification process remains live at handoff.

No production or test content changed after the full workspace run. Documentation updates followed.
The measured root-library count is 1,386. The integration source inventory is 1,589 test functions
across 155 files. Full feature matrices, minimum supported toolchains, Miri, cross-target builds,
advisory scanning and final integration gates remain required for Order 1.

## Next work and the failing type probe

Fix tuple-field type identity, prioritizing valid-source admission. An isolated eight-case probe
compares reference compilation with type_check_source_via_kel, without reference-first masking.
Five cases agree and three do not. No type-input implementation changed in this increment.

The valid source below is falsely rejected by the independent pipeline.

```keleusma
fn f(xs: [Word; 2]) -> Word { 7 }
fn main(t: ([Word; 2], Word)) -> Word { f(t.0) }
```

These invalid sources are accepted by the independent pipeline.

```keleusma
fn f(t: (Word, bool)) -> Word { 7 }
fn main(t: ((Word, Word), Word)) -> Word { f(t.0) }
```

```keleusma
struct P { x: Word }
struct Q { x: Word }
fn f(q: Q) -> Word { 7 }
fn main(t: (P, Word)) -> Word { f(t.0) }
```

Valid controls pass for matching nested tuples and matching nominal types. A bool tuple field
returned as Word is refused, and returned as bool is accepted. The tuple-array dimension mismatch
is refused, but that does not prove dimension checking because the matching dimension also fails.
Promote this probe and its controls into standing tests before repairing it.

The failing probe receipt is tmp/order1-runs-type-probe.log. Session 18854 exits one. Its source is
tmp/order1-type-probe/src/main.rs. It runs with cargo run --offline --release --manifest-path
tmp/order1-type-probe/Cargo.toml, with CARGO_TARGET_DIR set to tmp/order1-admission-probe/target
and the usual wrapper/cache settings. The later detail binary requires --bin detail, and future
runs of the original probe should specify --bin order1-type-probe because both binaries now exist.
The construction recipe and cases are also in ORDER1_COMPLETION_PLAN.md.

The detail receipt tmp/order1-runs-type-detail.log localizes the false rejection further. The
callee requires scalar-array tag 49, while the projected argument is primitive Word tag 1. The
main parameter's structural construction rows also contain two Word members. Repair nested header
capture as well as projection result identity. This additional command exits zero directly.

Read findings for the next increment follow. typed_result_node omits TupleField node 37. The
operand path in try_call_type_inputs_from_pipeline instead classifies it through the baked layout
kind. expression_rows_from_body has an exact array-element query for node 43 and no corresponding
tuple-element query. step_tupleaccess in parse.kel currently preserves a packed byte offset and
layout kind, without a source ordinal. declared_type_operand can consume array-dimension record 8,
but the tuple-of-array fact dump has lost that member's array identity before solving. These are
localization findings, not proof that every cause has been identified. Preserve source identity
rather than inferring nominal or structural types from byte size.

Retain the other type obligations in the completion plan, including nested headers, binders,
occurrences, nominal layouts, aliases, empty arrays, mixed tuples, casts and enum capacity.
Refresh stage identity, inference independence, monomorphization and shipping evidence after the
remaining changes, then run the final integration gates. Full Order 1 remains active.

## Shadow pilot and interline

Task v023-order1-runs records the first diagnostic, the separate tuple-projection comparison failure,
and three bounded claims for run splitting, execution and protocol refusal/continuation. All labels
are non-independent. Exact payloads, request hashes and source hashes were reviewed in
tmp/order1-runs-pilot-export.json. The run-encoding implementation is complete, but the pilot task's
resolved field is false because its captured typing diagnostic remains unresolved. This avoids
reporting that the observed type failure was repaired.

External evaluation was not attempted under the prior automatic approval rejection of its destination
and payload. No accuracy claim is made. Preserve peer ownership of the tracked pilot checkpoint.
No peer messages were sent. The inbox is empty. The interline checkpoint was exported and reviewed
with no diff.
