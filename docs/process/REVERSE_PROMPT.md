# Reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
This increment started at 04c51522 on feat/v02-order1-next. The legacy handoff is
invalid-and-stale, with its stamp 49 commits behind that starting head. Appendix B
was read before tracked writes. The concurrent V0.3.X worktree and pre-existing
.codex/ and nvim.log remain untouched. No push, merge or publication was performed.

## Tuple construction and expression layouts

The parser now sizes tuple literals from source-derived expression layouts rather
than assigning eight bytes to every member. Boolean and Byte members occupy one
byte, Unit occupies zero, and nested tuple, array, struct and text members retain
their physical representation. The existing header descriptor and tuple-member
pools carry these facts. Bindings and aliases retain descriptors for projections.
Array-of-tuple headers use the nested header parser.

The expression observer tracks scalar literals, parameter and binding kinds,
operators, earlier call declarations, field and array projections, and scalar data
reads. A data field retains its declared kind, including across an indexed read.
Array literals now use the known element width. Projection updates replace the
expression root so a child composite cannot type its scalar parent. Nested field
tracking preserves a dot already consumed by the field scanner.

Body descriptors and layouts are released at function boundaries while declared
header layouts remain live. A 400-function tuple corpus checks reuse. The descriptor
pool remains 1,024 nodes with 32 tuple layouts and 256 members. Eight simultaneously
open literal tuple contexts are admitted and the ninth is explicitly refused.
Unknown expression layout has private diagnostic 38, separate from capacity and
source rejection. A call to a later declaration is still unavailable through this path.

Thirty valid sources preserve an independent true verdict and complete artifact
identity. Five programs execute to seven after Keleusma serialization and decoding.
Five invalid tuple programs require PipelineRejected. Six tests in
tests/selfhost_tuple_construction.rs cover these cases, descriptor reuse and the
remaining forward-call diagnostic. Public bytecode version stays 2 and opcode count
stays 66. Parser declaration and selected type-stage capacities stay 640.

The parser has 228 chunks and a 190,492-byte artifact. Its shared storage remains
336,016 bytes. Private storage is 1,170,848 bytes, an increase of 59,936 bytes. The
source is 377,878 bytes, within the existing 393,216-byte input capacity. Codegen,
verify_types and analyze resources remain unchanged from the relocation increment.

## Verification

Runs use empty RUSTC_WRAPPER, CARGO_HUSKY_DONT_INSTALL_HOOKS set to one and
KEL_SELFHOST_CACHE zero. Production and test sources remained fixed during live runs.
The roadmap was updated while a combined invocation was active. Because one codegen
test embeds that document, the codegen receipt is superseded by a fresh full-target run.

- The corrected focused tuple run passes six tests. Log tmp/order1-tuples-corrected.log.
  Session 17213 exits zero. Command cargo test --release --features self-host --test
  selfhost_tuple_construction -- --nocapture. This includes all thirty artifact cases,
  five executions, five independent negatives, the 400-function corpus and depth controls.
- All five boundary audits pass. Log tmp/order1-tuples-boundaries-corrected.log.
  Session 39247 exits zero. Command cargo test --release --features self-host --test
  selfhost_codegen boundary -- --nocapture.
- The combined final run is session 83041. Command cargo test --release --features
  self-host --test selfhost_tuple_construction --test selfhost_parse --test
  selfhost_codegen --test selfhost_typecheck --no-fail-fast. Log
  tmp/order1-tuples-verified-broad.log. Session 83041 exits zero with 152 codegen tests,
  91 parser tests and two ignored, six tuple tests and 103 type-checking tests passing.
- The fresh codegen run after the roadmap edit is session 80569. Command cargo test
  --release --features self-host --test selfhost_codegen --no-fail-fast. Log
  tmp/order1-tuples-codegen-final.log. Session 80569 exits zero with all 152 tests passing.
- The current all-stage artifact and monomorphization run is session 9401. Command
  cargo test --release --features self-host --test selfhost_artifact_identity --test
  selfhost_monomorphize_identity --no-fail-fast -- --nocapture. Log
  tmp/order1-tuples-verified-artifacts.log. Session 9401 exits zero with six artifact tests
  and three monomorphization controls passing. Every complete stage artifact passes
  Keleusma serialization and decoding comparisons. The artifact suite takes 577.18 seconds.
- Fifteen selected tests pass without default features. Command cargo test --release
  --no-default-features --features self-host --test selfhost_tuple_construction --test
  selfhost_data_relocation --test selfhost_source_initializers --no-fail-fast. Log
  tmp/order1-tuples-verified-no-default.log. Session 35370 exits zero.
- Four inference tests pass after erasing 58,198 recorded types across fourteen stages,
  preserving complete artifacts. Command cargo test --release --features self-host
  --lib compiler::inference_independence -- --nocapture. Log
  tmp/order1-tuples-verified-inference.log. Session 4431 exits zero.
- Fresh Clippy passes with actual Checking and warnings denied. Command cargo clippy
  --workspace --tests -- -D warnings. Log tmp/order1-tuples-verified-clippy.log.
  Session 46384 exits zero.
- The current documentation run is session 95052, using cargo doc --workspace --no-deps
  with RUSTDOCFLAGS set to -D warnings. Log tmp/order1-tuples-verified-doc.log.
  Session 95052 exits zero with actual Documenting.
- The current build without the standard library is session 28279, using cargo check
  --no-default-features --features self-host. Log tmp/order1-tuples-verified-nostd.log.
  Session 28279 exits zero with actual Checking.
- Resource measurements are in tmp/order1-tuples-verified-resources.log. The fresh
  remaining-boundary probe is in tmp/order1-tuples-verified-remaining.log. Their combined
  scratch command exits zero in session 96482. The source inventory is 1,623 integration
  test functions across 162 files. No library tests were added in this increment.

The four documentation guard targets pass 24 tests in tmp/order1-tuples-doc-guards.log.
Session 66459 exits zero. Command cargo test --release --test documentation_links --test
claimed_counts --test comment_citations --test push_order_claims. Formatting and diff checks
pass. The final receipt update is checked again before commit. All verification handles
have been collected to terminal results.

The complete workspace, full feature matrix, minimum supported toolchains, Miri,
cross-target builds, advisory scan and final integration gates have not been rerun.
The historical workspace run had 3,202 passes, three legacy adapter failures and five
ignored tests. Targeted repair receipts do not replace the final complete workspace gate.

## Failed runs and corrections

The original fourteen cases fail in tmp/order1-tuples-first.log, session 12583 exiting
101. The first implementation fixes thirteen and leaves the nested array header in
tmp/order1-tuples-implementation.log. The second implementation passes eighteen cases
in tmp/order1-tuples-second.log. The first broad attempt has two Rust test-assertion
compilation mistakes in tmp/order1-tuples-broad.log. The corrected broad attempt has
four codegen failures in tmp/order1-tuples-broad-fixed.log. Those identify struct tuple
members incorrectly receiving a generic composite sentinel. Restoring their separate
struct marker preserves the existing equality lowering.

The first extended attempt references the new data block before its declaration,
which the reference parser refuses. Log tmp/order1-tuples-extended.log. Moving the
data declaration before its first use repairs it. The next four-test controls pass in
tmp/order1-tuples-extended-fixed.log, and eleven nested-equality tests pass in
tmp/order1-tuples-equality-regressions.log.

The subsequent broad attempt passes 150 codegen tests and fails two boundary audits
in tmp/order1-tuples-final-broad.log. Both identify the same split nested-array indexing
regression. The source kept the outer array shape after an element read. Replacing
the root shape after indexing repairs it. The thirty-source corpus now includes the
witness. The final broad and artifact receipts above supersede these earlier runs.

## Remaining work

The complete Order 1 goal remains active. The fresh remaining probe confirms two
next witnesses.

```keleusma
fn main() -> Word { let t = (yes(), 7); t.1 }
fn yes() -> bool { true }
```

This needs return-layout facts from a later declaration. It receives private diagnostic
38 and is correctly classified as unavailable. The facts must remain source-owned.

```keleusma
fn make() -> [(bool, Word); 2] { [(true, 7), (false, 8)] }
fn main() -> Word { let xs = make(); xs[0].1 }
```

This still leaves three reconstruction roots. The array-of-tuple header now parses
nested members, but its returned array shape does not reach the call result. Preserve
the reconstruction cardinality guard. Calls returning nested tuples already pass.

Continue through remaining declaration, binding and occurrence coverage, alias and
cast obligations, enum capacity and final integration gates. Wildcard function
parameters remain explicitly unsupported. A finite tuple corpus does not establish
full source-type or language coverage. Audit every row of ORDER1_COMPLETION_PLAN.md
before marking the full goal complete.

## Shadow pilot and interline

The pilot task is v023-order1-tuples-04c5. Case identifiers share the prefix
v023-20261011-tuples-04c5 and end in triage, corpus, execution and scope. The triage
label is compiler_or_test. The three completion labels are supports. Evidence
includes immutable source excerpts and executed results. All labels are
non-independent self-review, with no independent accuracy claim.

All four request payloads were reviewed. External evaluation remains unavailable
under the previous automatic approval review rejection. No evaluation retry or
bypass occurred. The three-claim sampling limit is reached. The outcome is resolved for this tuple increment, with unavailable usage and elapsed values
recorded as null. The reviewed export is tmp/jev-shadow-tuples-04c5-final.json. All four
current-task requests and their immutable source hashes match the reviewed evidence.
The export contains 170 samples across 43 tasks. No provider judgment is claimed.

The assigned inbox is empty. No peer messages were sent. The tracked interline checkpoint
is exported and reviewed before handoff. The peer-owned tracked shadow-pilot checkpoint
remains untouched.
