# Reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
This increment started at b12545a7 on feat/v02-order1-next. The legacy handoff is
invalid-and-stale, with its stamp 47 commits behind that starting head. Appendix B
was read before tracked writes. The V0.3.X worktree and pre-existing .codex/ and
nvim.log remain untouched. No push, merge or publication was performed.

## Source private initialization

The parser now chooses each private field's initializer kind and value. Private
metadata 107 carries the kind, 108 a nonnegative value, and 109 the complement of
a negative value. The complement avoids the record protocol's minus-one sentinel.
The host validates the descriptors and constructs ConstValue entries without
borrowing a reference initializer or choosing a type-based default.

Word and Byte integer literals, Bool literals, a leading minus and parenthesized
scalar initializers retain their values. Grouped scalar type declarations are also
recognized. Scalar-array defaults repeat the stage-selected zero. Fixed and Float
default zeros are represented, with Float controls gated on the feature. This does
not add floating-point literal support to the lexer. Private composite initializers
and all shared initializers are rejected, matching the reference's admission rule.
Wrong literal kinds, Byte overflow and malformed scalar syntax are independent
source rejections. A one-element array remains distinct from a scalar.

Keleusma assigns shared slots before private slots while preserving declaration
order within each partition. Metadata 110 carries each runtime field's ordinal and
slot base. Function type extraction retains immutable address snapshots, shared
through copy-on-write Arc vectors. A later declaration cannot reinterpret an earlier
body's address as a different field type. Relocation of an already emitted body
across a later shared declaration remains open and is explicitly tested as unavailable.

The parser's own source now declares 525 data fields, beyond the former 512-field
table. The table and all four scans are widened to 640. Boundary controls derive the
limit from the stage, and a new scan-inventory check prevents loop limits from
lagging the table. The selected type-stage caps remain 640. The existing 24-bit data
address bound is checked before descriptor expansion. Its boundary control exercises
parser transport without allocating millions of runtime slots.

Fifteen valid initializer sources retain complete artifact identity. Eight cases
execute after Keleusma serialization and decoding, including negative one, Byte 255,
Bool true and interleaved visibility. The original counter now returns six. Eleven
invalid initializers require PipelineRejected from the shipping entry. The previous
unsupported-initializer pin has become a positive metadata control. The complete
module comparison remains in place.

The parser has 216 chunks, a 179,828-byte artifact, 336,016 shared bytes and 1,110,912
private bytes. Private storage grows by 37,440 bytes. The source is 368,876 bytes.
Other stage resources are unchanged. The legacy wire-input maxima are 901 names and
25,492 bytes, within the unchanged 1,024-name and 49,152-byte limits. Public bytecode
version stays 2 and public opcode count stays 66.

## Verification

Runs use empty RUSTC_WRAPPER, CARGO_HUSKY_DONT_INSTALL_HOOKS set to one and
KEL_SELFHOST_CACHE zero. Verification subjects were held fixed during live runs.

- The workspace run covers 188 suites with 3,202 passes, three failures and five ignored tests.
  Command cargo test --release --workspace --no-fail-fast. Log tmp/order1-source-init-workspace.log.
  Session 8231 exits 101. The failures are the missing metadata-110 case in the pipeline test
  adapter and two legacy join controls whose uncompressed scalar-node blob reached 50,252 bytes.
- Test-only repairs add the address-record case and use the existing compact scalar tail for
  the two stage-scale join controls. Both still compare complete NAMES and STRING_POOL payloads
  against the reference. The production streaming path already passed. The legacy grammar remains
  exercised by its other fixtures. No wire buffer was widened. All 193 tests in the affected
  targets pass in tmp/order1-source-init-legacy-controls.log. Session 99639 exits zero. Command
  cargo test --release --features self-host --test selfhost_pipeline --test selfhost_wire
  --no-fail-fast. The complete workspace was not rerun after these adapter changes.
- The corrected parser/code-generation run passes 152 code-generation tests, 91 parser tests
  with two ignored, and seven initializer tests. Log tmp/order1-source-init-parser-final.log.
  Session 56766 exits zero. Command cargo test --release --features self-host --test
  selfhost_source_initializers --test selfhost_parse --test selfhost_codegen --no-fail-fast.
- All 103 type-checking tests pass in tmp/order1-source-init-types-final.log, session 66892
  exiting zero. Command cargo test --release --features self-host --test selfhost_typecheck
  --no-fail-fast. The stage capacity and independent source-input corpus checks are included.
- Six artifact tests and three monomorphization controls pass in tmp/order1-source-init-artifacts.log.
  Session 26598 exits zero. Command cargo test --release --features self-host --test
  selfhost_artifact_identity --test selfhost_monomorphize_identity --no-fail-fast -- --nocapture.
  All fourteen complete stage artifacts pass shipping compilation, Keleusma serialization and
  decoding comparisons. The same targets pass in the workspace run.
- Twenty-one no-default tests pass in tmp/order1-source-init-no-default.log. Session 79410
  exits zero. Command cargo test --release --no-default-features --features self-host --test
  selfhost_source_initializers --test selfhost_text_boundary --test selfhost_type_transport
  --no-fail-fast.
- Four inference checks pass after erasing 56,921 recorded expression types across fourteen
  stages. Log tmp/order1-source-init-inference.log. Session 43743 exits zero. Command cargo
  test --release --features self-host --lib compiler::inference_independence -- --nocapture.
- Fresh final Clippy passes with actual Checking. Command cargo clippy --workspace --tests
  -- -D warnings. Log tmp/order1-source-init-last-clippy.log. Session 12293 exits zero.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-source-init-doc.log. Session 11035
  exits zero.
- The build without the standard library passes with actual Checking. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-source-init-nostd.log.
  Session 65282 exits zero.
- Resource measurements are in tmp/order1-source-init-resources.log. The resources scratch
  binary exits zero in session 5476. Reference parsing counts 525 data fields in
  tmp/order1-source-init-field-count.log, session 73194 exiting zero.

Earlier failures are preserved. The first thirteen-source comparison fails in
tmp/order1-source-init-first.log. A new parser function initially had an extra closing brace,
reported in tmp/order1-source-init-implementation.log. The first broad run exposed the 512-field
limit, an adapter missing metadata 110 and a statement-cap fixture using an invalid shared
initializer. After widening the table, two field scans still used 512 and caused LoopLimitExceeded.
The corrected four-scan inventory now pins their alignment with the advertised capacity.
These failures are in tmp/order1-source-init-broad.log and tmp/order1-source-init-broad-fixed.log.
The statement-cap fixture now uses valid shared data without an initializer.

An attempted process inspection using ps was denied by the sandbox. The confirmed live tool
handles were polled instead, and every verification handle was collected to a terminal result.
No production source changed after the workspace run. Two test adapters changed afterward,
followed by documentation. The root-library count is 1,390. The source inventory is 1,615
integration test functions across 160 files. The four documentation guards pass 24 tests in
tmp/order1-source-init-doc-guards.log, session 20526 exiting zero. Command cargo test --release
--test documentation_links --test claimed_counts --test comment_citations --test push_order_claims.
Final formatting and diff checks pass.


## Remaining work

The complete Order 1 goal remains active. Continue with mixed tuple construction,
general nested-header paths, binder and occurrence coverage, remaining alias and cast
obligations, enum capacity and final integration gates.

A remaining declaration-order witness is now named by a standing test.

```keleusma
private data d { n: Word }
fn early() -> Word { d.n = 1; d.n }
shared data s { n: Byte }
fn main() -> Word { early() }
```

The source is valid. Independent type checking remains true because early retains
its original declaration snapshot. Shipping refuses the stale address in early after
the later shared slot shifts the final private partition. Preserve the snapshot
protection while adding relocation or an equivalent source-owned solution. Do not
reinterpret the earlier private Word as the later shared Byte.

The existing tuple witnesses still require work.

```keleusma
fn main() -> Word { let t = (true, 7); t.1 }
fn pair() -> (bool, Word) { (true, 7) }
fn nested() -> Word { let t = ((1, 2), 3); t.0 .1 }
fn from_array(xs: [((Word, Word), Word); 2]) -> Word { xs[0].0 .1 }
```

Run these separately. Mixed literal layout and the nested reconstruction paths were
not changed here. Preserve the reconstruction cardinality guard. Wildcard function
parameters remain explicitly unsupported. Full feature matrices, minimum supported
toolchains, Miri, cross-target builds, advisory scanning and final integration gates
still require their complete audit before Order 1 can be marked complete.

## Shadow pilot and interline

The pilot task is v023-order1-source-init-b125. The first diagnostic is
v023-20261011-source-init-b125-triage, labelled compiler_or_test. Its original test source was
saved before later edits in tmp/order1-source-init-first-source.rs. Three completion samples
have suffixes execution, rejection and corpus. All are labelled supports. Every label has
independent set to false. No model accuracy claim follows from self-review.

The bounded initializer increment is resolved. The full Order 1 goal remains active. Provider
usage and elapsed time remain unavailable and are recorded as null. Later diagnostics were not
additional samples, following the first-diagnostic rule. The three-claim limit excludes separate
capacity and snapshot samples. External evaluation remains unavailable under the earlier automatic
approval rejection. No new service request or bypass was attempted.

The reviewed export is tmp/order1-source-init-b125-shadow-final.json. All four request hashes,
payload equality and source-snapshot hashes were verified. The peer-owned tracked pilot checkpoint
was not edited. Request hashes follow in diagnostic, execution, rejection and corpus order.

- dedf1a4f629e19b612cf82cd668a243979af9e0d525338f198cd8a56fbd451c7
- 68e82a2edf81ee4546129e4c4d33b299f230d18b2678adbd7d8c7b9c27c39dfe
- a271fb3438701a84ce0de1af8bb5c81c34d5ea8554483de80b977afd6541ae90
- 2c33f43f39a4a7ed9b83cb793df362f19f934410ae582f5842d876d84259aed2

The interline inbox is empty. Its checkpoint was exported and reviewed with no tracked change.
No peer message was sent. No verification process remains live at handoff.
