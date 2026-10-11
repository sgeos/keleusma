# Reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
This increment started at 8b118bf9 on feat/v02-order1-next. The legacy handoff is
invalid-and-stale, with its stamp 48 commits behind that starting head. Its ancestry
check passes, but its unchanged-stage-source claim and twelve-stage count are false.
Appendix B was read before tracked writes. The V0.3.X worktree and pre-existing
.codex/ and nvim.log remain untouched. No push, merge or publication was performed.

## Declaration-order relocation

Codegen now translates scalar and indexed data reads and writes from a function's
original field bases to the parser's final layout. Metadata 110 supplies the final
field bases. The host retains and transports those facts. Keleusma chooses the
translated operand, preserving field-relative offsets and indexed lengths.
Missing or ambiguous source ranges, invalid widths and addresses outside the
existing 24-bit space return an unavailable result. The stage has a 640-field map.

Original immutable address snapshots continue to govern independent type checking.
Relocation happens before reconstruction and multihead grouping. Collecting, fused,
full, scratch and shipping compile entries all reach the repaired path. The fused
entry uses a preliminary parser pass to collect layout facts while discarding
function records, then compiles one group at a time. It now pays for two parser
passes. Unchanged snapshots skip relocation. Host layout-record storage still
retains the address records emitted during parsing.

Seven source cases preserve complete artifacts, including scalar and indexed
reads/writes, explicit initializers, multiple private fields, a shared declaration
at end of file, and heads with different original snapshots. Three programs execute
after Keleusma serialization and decoding through five entry points, giving fifteen
runtime comparisons. Four private protocol tests check bounds, malformed maps,
indexed counts and recovery after refused requests. The former unsupported witness
in selfhost_source_initializers now requires successful artifact identity.

Codegen has 80 chunks, a 73,668-byte artifact, 58,424 shared bytes and 182,560 private
bytes. Shared storage grows by 15,392 bytes and private storage by 96 bytes. Its source
is 140,108 bytes. Parser, verify_types and analyze resources are unchanged from the
previous increment. Parser and selected type-stage capacities remain 640. Public
bytecode version stays 2 and opcode count stays 66.

## Verification

Runs use empty RUSTC_WRAPPER, CARGO_HUSKY_DONT_INSTALL_HOOKS set to one and
KEL_SELFHOST_CACHE zero. Verification subjects were held fixed during live runs.

- Four protocol tests pass. Command cargo test --release --features self-host --lib
  selfhost::data_relocation -- --nocapture. Log tmp/order1-relocation-protocol.log.
  Session 44452 exits zero.
- All 152 code-generation tests, two relocation tests and seven initializer tests pass.
  Command cargo test --release --features self-host --test selfhost_data_relocation
  --test selfhost_source_initializers --test selfhost_codegen --no-fail-fast.
  Log tmp/order1-relocation-targeted.log. Session 49521 exits zero.
- Nine pipeline tests and 184 wire tests pass. Command cargo test --release --features
  self-host --test selfhost_pipeline --test selfhost_wire --no-fail-fast.
  Log tmp/order1-relocation-pipeline-wire.log. Session 15831 exits zero.
- The two relocation and seven initializer tests also pass without default features.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_data_relocation --test selfhost_source_initializers --no-fail-fast.
  Log tmp/order1-relocation-no-default.log. Session 27198 exits zero.
- Four inference checks pass, erasing 57,089 recorded types across fourteen stages
  without changing their complete artifacts. Command cargo test --release --features
  self-host --lib compiler::inference_independence -- --nocapture.
  Log tmp/order1-relocation-inference.log. Session 47333 exits zero.
- Six artifact tests and three monomorphization controls pass in session 87605,
  which exits zero. All fourteen complete stage artifacts match the reference.
  Command cargo test --release --features self-host --test selfhost_artifact_identity
  --test selfhost_monomorphize_identity --no-fail-fast -- --nocapture.
  Log tmp/order1-relocation-artifacts.log. The artifact suite completes in 510.50 seconds.
- Fresh Clippy passes with actual Checking and warnings denied. Command cargo clippy
  --workspace --tests -- -D warnings. Log tmp/order1-relocation-clippy.log.
  Session 12539 exits zero.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-relocation-doc.log.
  Session 24717 exits zero.
- The build without the standard library passes with actual Checking. Command cargo
  check --no-default-features --features self-host. Log tmp/order1-relocation-nostd.log.
  Session 93721 exits zero.
- Resource measurements are in tmp/order1-relocation-resources.log. The resources
  scratch binary exits zero in session 70697. The source inventory now contains
  1,617 integration test functions across 161 files. Four private library tests were
  added. The historical 1,390-test workspace-library run has not been repeated.

The four documentation guard targets pass 24 tests in tmp/order1-relocation-doc-guards.log.
Session 39108 exits zero. Command cargo test --release --test documentation_links --test
claimed_counts --test comment_citations --test push_order_claims. Formatting and diff checks
pass. The final documentation-only receipt update is checked again before commit.

The baseline six source mismatches are preserved in tmp/order1-relocation-first.log,
with the first seven-case repair receipt in tmp/order1-relocation-first-implementation.log.
The current increment has not rerun the complete workspace or the full integration
matrix. The previous workspace run had 3,202 passes, three legacy adapter failures
and five ignored tests. The 193 affected tests pass again in this increment, but this
is not a replacement for the complete final workspace gate.

## Remaining work

The complete Order 1 goal remains active. A fresh probe in
tmp/order1-relocation-next-boundaries.log confirms the next tuple witnesses.

```keleusma
fn main() -> Word { let t = (true, 7); t.1 }
fn pair() -> (bool, Word) { (true, 7) }
fn nested() -> Word { let t = ((1, 2), 3); t.0 .1 }
fn from_array(xs: [((Word, Word), Word); 2]) -> Word { xs[0].0 .1 }
```

Mixed tuple literals still allocate sixteen bytes where the reference uses nine.
The nested cases leave two reconstruction roots. Preserve the cardinality guard.
The literal tuple parser currently counts every member as one Word and calls
word_tuple_layout for its let binding. The tuple header descriptor and postfix paths
already exist and should be extended without borrowing reference expression types.

Continue through general nested headers, binder and occurrence coverage, remaining
alias and cast obligations, enum capacity and final integration gates. Wildcard
function parameters remain explicitly unsupported. The fresh probe also records
an invalid tuple for-binder source returning parser diagnostic -931, detail 22.
This is distinct from a type verdict and still needs coverage analysis.

Full feature matrices, minimum supported toolchains, Miri, cross-target builds,
advisory scanning, shipping-command checks and final integration gates require
their full completion audit before Order 1 can be marked complete.

## Shadow pilot and interline

The pilot task is v023-order1-data-relocation-8b11. The first diagnostic case
v023-20261011-relocation-8b11-triage preserves six data-address disagreements and an
unchanged-address control. Its label is compiler_or_test. Three completion cases
with the same prefix end in protocol, runtime and inference. Each includes executed
results and relevant assertion source. Their labels are supports. All labels are
non-independent self-review and establish no independent accuracy claim.

Payload previews were reviewed. External evaluation remains unavailable under the
previous automatic approval review rejection. No evaluation retry or bypass occurred.
The three-claim sampling limit is reached. Further completion claims are not sampled.
The task outcome is resolved for this relocation increment, with unavailable usage and elapsed
values recorded as null. The reviewed export is tmp/jev-shadow-relocation-8b11-final.json.
All four current-task request digests and their immutable source hashes match. The export
contains 166 samples across 42 tasks. No provider judgment or accuracy comparison is claimed.
All verification handles have been collected to terminal results.

The assigned inbox is empty. No peer messages were sent. The tracked interline checkpoint
was exported and reviewed with no change. The peer-owned tracked shadow-pilot checkpoint
was not overwritten.
