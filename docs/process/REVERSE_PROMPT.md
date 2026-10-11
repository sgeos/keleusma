# V0.2.X reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next. This increment started at 1925af16. Preserve unrelated
untracked .codex and nvim.log. V0.3.X remains separate. No push, merge or publication occurred.

Read Appendix B before tracked writes. Public bytecode version remains 2, opcode count remains
66 and selected type-stage caps remain 640. The legacy HANDOFF.md remains invalid and stale,
measured 42 commits behind the starting head. Its previously checked ancestry passes, while its
unchanged-stage-source claim is false. Historical full validity and advisory checks were not all
repeated. The current completion plan and latest roadmap section describe the remaining goal.

## Complete module decoding

wire_decode_module_via_kel composes framing, checksum, auxiliary, opcode and debug readers.
Schema commands 57 through 63 select an optional shebang prefix and the declared frame, check
headers and section bounds, enforce runtime width maxima, validate extension metadata, compare
mirrored fields and select each chunk's opcode span. A separate schema instance handles debug
pools, while the coordinator retains framing state. All module fields are constructed from decoded
values. Rust transports requested bytes and constructs typed values without the reference loader.

Wire command 220 starts the existing incremental checksum state independently of canonical framing.
Commands 189 and 192 feed and finish that transaction. The old canonical unsigned reader is retained
for its existing API. The highest-command ceiling is now 220. The public instruction set is unchanged.

Reference behavior preserved by controls includes trailing input, prefixes across transport windows,
noncanonical and unaligned section offsets, overlapping sections, and unaligned chunk spans.
Every operand entry receives a parity check, including unused entries. Unknown unused pool tags
remain unobserved. Only selected opcode records are decoded. Empty chunk tables ignore opcode data.
Debug payloads become interpreted only at complete module construction, so present empty or malformed
pools can pass auxiliary decoding and fail here.

This API matches the low-level module_from_wire_bytes codec scope. Extension metadata consistency
is checked, but signature authentication, decryption and virtual-machine verification remain caller
responsibilities. Error variants and ordering for multiple defects are not matched. It is not wired
into the shipping runtime load path by this increment.

## Resources and evidence

The schema stage has 115 chunks and a 55,492-byte artifact. It uses 1,216 shared bytes and 4,160
private bytes. Its 28 new framing slots account for 896 private bytes. The largest legacy schema
input channel uses 147 of 640 occurrence rows. Wire has 561 chunks, a 163,044-byte artifact and
1,294 of 1,365 constant nodes, leaving 71 nodes against the required minimum reserve of 64.
Its legacy occurrence channel remains 638 of 640. The largest corpus blob is 23,753 bytes, leaving
25,399 bytes under its 49,152-byte cap. Parse remains the name maximum at 776 of 1,024 events.

All fourteen complete decoded modules re-encode byte-identically to reference artifacts.
Monomorphization controls pass. Inference independence covers 50,483 recorded expression types.
The runtime test drops the original module and runs decoded instructions, including a function
call, shared input, debug pools and persistent initialization. Consecutive calls return 12 and 17.

## Verification receipts

Runs use empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one. Stage runs use
KEL_SELFHOST_CACHE zero. No verification subject was edited while its run remained live.

- The initial capacity run fails because a new field used a reserved identifier. Session 40972
  exits 101 in tmp/order1-module-first.log. The next run catches a mutable local unsupported by
  the language. Session 51803 exits 101 in tmp/order1-module-capacity.log. Both were corrected.
- The first complete-loader controls catch the new checksum command's stale dispatch ceiling.
  Session 57731 exits 101 in tmp/order1-module-tests.log. The next run catches an incorrect test
  premise that every repaired offset mutation must be invalid. Session 68831 exits 101 in
  tmp/order1-module-tests-fixed.log. Both the dispatch and the fixture expectation were corrected.
- Seven module controls, two accounting checks and one node-budget guard pass in
  tmp/order1-module-comparison.log. Command cargo test --release --features self-host --test
  selfhost_wire_module_decode --test wire_private_state_accounting --test module_input_node_budget
  --no-fail-fast -- --nocapture. Session 26585 exits zero.
- Two direct protocol checks pass in tmp/order1-module-protocol.log. Command cargo test --release
  --features self-host --lib selfhost::wire_module_decode -- --nocapture. Session 96425 exits zero.
- All six artifact checks and three monomorphization checks pass in tmp/order1-module-artifacts.log.
  Command cargo test --release --features self-host --test selfhost_artifact_identity --test
  selfhost_monomorphize_identity --no-fail-fast -- --nocapture. Session 77865 exits zero.
- The final legacy capacity check passes all fourteen sources in tmp/order1-module-final-capacity.log.
  Command cargo test --release --features self-host --test selfhost_typecheck
  the_stage_is_measured_against_the_real_stage_sources -- --nocapture. Session 70132 exits zero.
- Four inference-independence checks pass in tmp/order1-module-inference.log. Command cargo test
  --release --features self-host --lib compiler::inference_independence -- --nocapture. The command
  returns exit zero directly without a continuing session.
- Nine no-default integration and accounting checks pass in tmp/order1-module-no-default.log.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_wire_module_decode --test wire_private_state_accounting --no-fail-fast.
  Session 27141 exits zero.
- Forty no-default wire unit checks pass in tmp/order1-module-no-default-unit.log. Command cargo
  test --release --no-default-features --features self-host --lib selfhost::wire_ -- --nocapture.
  Session 81977 exits zero.
- The full workspace runs 3,158 passes, one failed stale size assertion and five ignored across
  181 suites. Command cargo test --release --workspace --no-fail-fast. Session 29575 exits 101.
  Log tmp/order1-module-workspace.log. The sole failure expects the old 23,713-byte blob.
- The measured expectation and its margin comment are corrected to 23,753 and 25,399. The complete
  affected wire suite then passes 184 tests. Command cargo test --release --workspace --test
  selfhost_wire -- --nocapture. Log tmp/order1-module-wire-final.log. Session 16168 exits zero.
  Expected caught panics from mutation controls appear in this log and are followed by passing tests.
- Fresh final Clippy passes with actual Checking. Command cargo clippy --workspace --tests
  -- -D warnings. Log tmp/order1-module-final-clippy.log. Session 3977 exits zero.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-module-doc.log. Session 86185 exits zero.
- The no_std check passes with actual Checking. Command cargo check --no-default-features
  --features self-host. Log tmp/order1-module-nostd.log. Session 15707 exits zero.

The four documentation guards pass 24 tests after the handoff and plan updates. Command cargo
test --release --test documentation_links --test claimed_counts --test comment_citations
--test push_order_claims. Log tmp/order1-module-doc-guards.log. Session 12966 exits zero.
Formatting and diff checks pass. No verification process remains live at handoff.

No production code changed after the workspace run. Only the measured test expectation and
subsequent documentation changed. The entire workspace was not repeated after that correction.
Its root library ran 1,380 tests. The source inventory contains 1,581 integration test functions
across 153 files. Full feature matrices, minimum supported toolchains, Miri, cross-target builds,
advisory scanning and final integration gates remain required before completing Order 1.

## Next work

Remove encoder limits that reject sources the production compiler already accepts, then integrate
Keleusma serialization into shipping output. The unsigned shipping path still uses Module::to_bytes.
Do not narrow source admission to fit the encoder. The completion plan now records reproducible
source constructions for two measured blockers.

- Generate 170, 171 or 177 distinct functions f0 through fN-1, each taking x as Word and returning
  x + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8. Append fn main returning f0(0). The production compiler accepts
  all three. They contain 1,361, 1,369 and 1,417 roots. Only the first fits the encoder's 1,365-root
  limit. Reference artifact lengths are 54,772, 55,092 and 56,996 bytes.
- Generate fn main returning Text with a literal of 256, 257 or 60,000 ASCII x bytes. All three
  are accepted by the production compiler. The first encodes byte-identically. The latter two
  fail the constant name/input capacity. Reference artifacts are 1,244, 1,244 and 60,988 bytes.
  forest_input rejects individual constant names above 256 bytes before its aggregate input cap.

Every reference artifact in those six cases decodes and re-encodes byte-identically through the
new complete decoder. The final scratch receipt is tmp/order1-module-admission-decoder.log,
session 23381 exited zero. The harness is tmp/order1-admission-probe/src/main.rs, invoked with
cargo run --offline --release --manifest-path tmp/order1-admission-probe/Cargo.toml and the usual
wrapper/cache settings. This probes the production library API, not the command-line binary.
The initial scratch manifest had an incorrect relative dependency path and was corrected before
these executions. Earlier receipts are tmp/order1-module-admission-probe-fixed.log, session 37157,
tmp/order1-module-admission-boundaries.log, session 43371, and tmp/order1-module-admission-identity.log,
session 57301, each exited zero. Promote these cases into standing regression tests while fixing
the encoder. Other name-event, aggregate-input and forest limits still require coverage.

Retain all type obligations in ORDER1_COMPLETION_PLAN.md, including projection identity, nested
headers, binders, occurrences, nominal layouts, aliases, empty arrays, mixed tuples, casts and
enum capacity. Refresh stage identity, inference independence, monomorphization, shipping behavior
and the final integration gates after the remaining changes. Full Order 1 remains active.

## Shadow pilot and interline

Task v023-order1-module records the first stage parse diagnostic and three bounded completion claims.
All four labels are non-independent. The stage and selected-span claims are supported. The runtime
claim excerpt is unsupported because it omits compare, which establishes that the returned module
is independently decoded. The full test source supplies that provenance. This is a packet evidence
gap, and the pilot does not change development gates.

Exact payloads, request hashes and source hashes were reviewed in tmp/order1-module-pilot-export.json.
The bounded decoder increment is resolved. External evaluation was not attempted under the prior
automatic approval rejection of its destination and payload. No accuracy claim is made. Preserve
peer ownership of the tracked pilot checkpoint. No peer messages were sent.

The v0.2.3 inbox is empty. The interline checkpoint was exported and reviewed with no diff.
