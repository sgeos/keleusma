# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next, starting this increment at 7e7b0e37. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md remains invalid and stale,
measured 39 commits behind the starting head. Its ancestry was previously checked and its
unchanged-stage-source claim is false. Historical full validity and advisory checks were not
all repeated.

## Names and constant forests

wire_name_via_kel resolves one name and returns a borrowed str. The schema stage requests its
NameRef record and pool span, checks index and range bounds, and validates UTF-8 through a
1,024-byte window. Only the requested name is validated. Unused bad name records and pool bytes
remain unobserved. A checked Rust string conversion follows the stage verdict as a defensive
construction boundary. The stage-only UTF test does not call that conversion.

wire_decode_constant_pools_via_kel constructs owned pools. Keleusma validates forward ranges and
all requested root ranges before building values, then schedules a reverse sweep. It interprets
tags, normalizes Boolean and Byte payloads, gates Float, and requests auxiliary records, names,
children and roots. The host transports fields, holds Option slots for built values and reports
whether a requested move found a value. Keleusma refuses unavailable children or roots. The host
constructs typed values and uses count-derived request budgets. No production reference constant
decoder or recursive host traversal is used.

Root ranges in one call share ownership. Separate calls preserve the reference's separate chunk
and private-initializer scopes. The decoder builds the entire constant table even for zero root
ranges, so an unused unknown or unavailable Float tag still fails. Empty composites require a
strictly forward first index. Empty structs ignore their unused field-name start, as the reference
owned decoder does. Enum None and Some(0) discriminants remain distinct. Scalar payloads never
become child ranges. Comparisons cover acceptance and decoded values, not equality of diagnostic
variants or failure ordering when multiple defects coexist.

Name commands 8 through 11 and constant commands 12 through 21 extend wire_schema.kel.
Transactions exclude incompatible commands. Constant name requests allow nested name validation.
Invalid text poisons the name transaction. Transport-length refusals preserve state. Owned-slot
availability is acknowledged before continuing, and unavailable slots invalidate the constant
transaction. The existing signature component remains unchanged in behavior.

## Corpus and resources

The schema stage has 38 chunks and a 19,756-byte artifact. Shared storage is 1,216 bytes. Private
storage is 1,408 bytes, consisting of 14 signature slots, seven name slots and 23 constant slots,
each a 32-byte Value slot. Its largest legacy input channel uses 56 of 640 occurrence rows.
Wire remains unchanged at 561 chunks, 1,290 of 1,365 constant nodes and 638 of 640 occurrence rows.
All fourteen sources fit the existing caps. Inference independence covers 47,116 recorded types.

The complete artifact test obtains each chunk's constant range from Keleusma-decoded chunk
records, decodes all pools and compares their values by canonical re-encoding. This preserves
floating-point payload comparisons, including NaNs. Every current stage is inventoried.
Controls cover all constant forms, named and nested trees, duplicate ownership, range boundaries,
required tables, disabled Float, 128-level raw trees, 5,000-root pools and names longer than the
byte window. The decoder does not inherit the encoder's 1,365-node or 256-byte-name limits.

Execution controls clear the original pools, install the new decoder results and execute arrays,
nested tuple arrays and a separately decoded non-default private initializer. Composite controls
assert that an instruction references a composite constant. The initializer control asserts that
its range is stored rather than elided. The first private fixture was rejected because its data
was never mutated. A meaningful increment makes its expected result eight and exercises the
restored initializer value.

## Verification receipts

Runs use an empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- The first check failed on an unclosed host-function delimiter. Log tmp/order1-names-first.log.
  The command exited 101. The first constant adapter check then required an explicit closure
  input type. It was replaced with a typed helper. Log tmp/order1-constant-decode-first.log.
  Session 46573 exited 101. These were compile failures, not data refusals.
- Three initial name tests pass in tmp/order1-names-targets.log, session 58415 exited zero.
  Four initial constant tests pass in tmp/order1-constant-decode-targets.log, session 97701 exited
  zero. Later controls supersede these counts.
- Five schema unit tests pass in tmp/order1-names-protocol.log, session 79324 exited zero.
  The stage-only UTF corpus accepts 25,626 and refuses 172,047 byte sequences, matching Rust.
  Command cargo test --release --features self-host --lib selfhost::wire_semantics -- --nocapture.
- The node-state and shared-memory accounting passes both tests in
  tmp/order1-constant-decode-memory.log, session 94200 exited zero.
- All six artifact tests and six then-current constant controls pass in
  tmp/order1-constant-decode-artifacts.log. Command cargo test --release --features self-host
  --test selfhost_wire_constant_decode --test selfhost_artifact_identity --no-fail-fast
  -- --nocapture. Session 34463 exited zero. All fourteen stage pools are compared.
- Four inference checks pass in tmp/order1-constant-decode-inference.log. Command cargo test
  --release --features self-host --lib compiler::inference_independence -- --nocapture.
  Session 46966 exited zero. The final stage source is unchanged from that measurement.
- The legacy real-stage type check accepts all fourteen sources in
  tmp/order1-constant-decode-capacity.log. Command cargo test --release --features self-host
  --test selfhost_typecheck the_stage_is_measured_against_the_real_stage_sources -- --nocapture.
  Session 88829 exited zero.
- The release workspace passed 3,126 tests and failed one documentation-count guard, with five
  ignored across 176 suites. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-names-constants-workspace.log. Session 51856 exited 101. CLAUDE.md still said 137
  integration files while the tree had 148. It now records the measured source inventory of
  1,557 integration test functions across 148 files and 1,373 executed root-library tests.
- The refreshed documentation-count guard and all eight final constant controls pass 21 tests
  in tmp/order1-names-constants-repaired-controls.log. Command cargo test --release --features
  self-host --test selfhost_wire_constant_decode --test claimed_counts --no-fail-fast.
  Session 54062 exited zero. The initial execution-fixture failure remains in
  tmp/order1-names-constants-final-controls.log, session 18695 exited 101.
- All thirteen final no-default integration and accounting checks pass in
  tmp/order1-names-constants-final-no-default.log. Command cargo test --release
  --no-default-features --features self-host --test selfhost_wire_constant_decode --test
  selfhost_wire_names --test wire_private_state_accounting --no-fail-fast.
  Session 35480 exited zero.
- The no-default wire unit suite passes in tmp/order1-names-constants-no-default-unit.log.
  Command cargo test --release --no-default-features --features self-host --lib selfhost::wire_
  -- --nocapture. Session 27951 exited zero. It includes constant protocol ordering and ownership
  refusals, UTF checks and the prior read/write protocols.
- Workspace Clippy performed actual Checking and passed after the final execution fixture repair.
  Command cargo clippy --workspace --tests -- -D warnings. Log
  tmp/order1-names-constants-final-clippy.log. The command exited zero.
- Documentation performed actual Documenting and passed with RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-names-constants-doc.log.
  Session 46964 exited zero.
- The no_std check performed actual Checking and passed. Command cargo check --no-default-features
  --features self-host. Log tmp/order1-names-constants-nostd.log. Session 50827 exited zero.

No production code changed after the full workspace run. Only a new execution test, its fixture
repair and documentation changed afterward. The workspace was not repeated after those changes.
Full feature matrices, minimum supported toolchains, Miri, cross-target builds, advisory scanning
and final integration gates remain required for Order 1. The four documentation guards pass 24 tests after the handoff and plan updates. Command
cargo test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-names-constants-doc-guards.log. Session 94454 exited zero.
Formatting and diff checks pass. The guards are repeated after this receipt update in
tmp/order1-names-constants-final-doc-guards.log. No process remains running at final handoff.

## Next work

Port the remaining auxiliary semantics into the schema stage, then assemble complete modules
with framing and opcode checks. The name and constant request paths are available for reuse.
Templates, enum layouts, data-layout expansion and initializer elision, parameter tags and debug
metadata remain open. Preserve the reference's per-family validation and optional-table policy.
A successful component result does not establish a complete module decoder.

Names are checked only when requested. Constant pools build every record. Duplicate child and
root moves fail within one decode operation. Private initializers use a separate constant-pool
call. The DATA_INIT absence sentinel is rejected by the constant-pool API; its all-zero initializer
meaning belongs to the future data-layout decoder. Avoid combining ownership scopes or validating
unused names merely to simplify assembly.

Continue using the new schema stage rather than growing wire.kel's nearly full type-input
channels. Measure every source against both node and type-input limits. The encoder still has
forest, name and input bounds that need admission-preserving coverage. Shipping unsigned output
still calls Module::to_bytes. Do not narrow accepted source behavior to the stage corpus.

Retain all type obligations in the completion plan, including projections, nested headers,
binders, occurrences, nominal layouts, aliases, empty arrays, mixed tuples, casts and enum capacity.
Refresh all-stage, monomorphization, inference-independence, shipping and final integration evidence
as those changes land. Full Order 1 remains active.

## Shadow pilot and mailbox

Task v023-order1-names-constants retains the first syntax diagnostic and three bounded claims
for UTF-8 decisions, execution and all-stage constant pools. Blind exports preceded non-independent
self-labels. The UTF-8 claim was labelled unsupported because its excerpt omitted the
validate_name_bytes helper that produces the verdict. The repository helper body and complete
unit test establish stage-only validation, but the excerpt alone does not establish that part
of the claim. The other two claims were labelled supported. No independent accuracy claim is made.

All four request hashes, exact payloads and source hashes were reviewed in
tmp/order1-names-constants-pilot-export.json. The bounded name and constant increment outcome is
resolved. Later compile, fixture and documentation diagnostics belong to the same increment and
were not sampled separately. Usage and elapsed time remain null.

External evaluation remains unavailable under the prior automatic approval rejection for the
external destination and payload. Do not retry or bypass it. Local capture continues. The pilot
implementation line owns the tracked pilot checkpoint.

The assigned inbox was empty at startup and before handoff. No messages were sent or received.
The mailbox checkpoint was exported and reviewed with no tracked difference. Publication requires
explicit in-session confirmation.
