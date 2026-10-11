# Reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
This increment started at 1e178bd0 on feat/v02-order1-next. The legacy handoff is
invalid-and-stale, with its stamp 46 commits behind that starting head. Ancestry passes,
and its twelve-stage and unchanged-stage claims remain contradicted. Appendix B was read
before tracked writes. The concurrent V0.3.X worktree and pre-existing .codex/ and nvim.log
remain untouched. No push, merge or publication was performed.

## Layouts and complete module comparison

The parser retains declared member names and resolves struct and tuple layouts monotonically.
A forward dependency can become known after its referenced declaration. The bound follows the
existing 64-struct and 32-tuple tables. A reversed chain of 64 structs resolves successfully.
A forward struct with an initial Byte field uses the corrected nested-field and trailing-field
offsets, matches the reference artifact and returns twelve from host-supplied values.

Sixteen named signature fixtures match complete reference artifacts. They cover nested
parameters and returns, tuple members, array fields, enum fields, forward declarations,
empty structs, Float signatures, arrays of nominal values and an 80,008-byte tuple shape.
Signature sizes retain the unsigned 32-bit format range. Field-access operand bounds are
separate. The Fixed and Text physical scalar kinds are now also supplied by Keleusma.
Their type-name identifiers and the Float identifier are seeded as identifier plus one,
so an absent name cannot alias identifier zero. The three shared fields are appended.

The shipping comparison now checks every Module, Chunk and DataLayout field. Exhaustive
Rust destructuring makes a newly added field require a comparison decision. Thirty-one
independent mutations are detected, with an unchanged-module control. The permanent fourteen-stage
artifact test now calls self_hosted_compile, including independent type rejection and the
complete metadata comparison, before Keleusma serialization and decoding.

## Empty arrays and text boundaries

Private type tag 7 retains an empty literal's array shape and zero length while leaving its
element unknown. It consumes no structural-pool entry. Zero-length element constraints remain
compatible with any element type. Array projection checks distinguish a known array with an
unknown element from a known non-array value.

Ten source verdicts agree with the reference. Six previously missed rejections now require
PipelineRejected from the shipping entry. They cover argument, return, comparison, alias,
annotation and branch-length mismatches. Unused empty arrays and aliases remain valid and
execute as seven. Empty-array equality retains complete artifact identity and the reference
runtime comparison fault. Do not report that expression as successfully returning true.
Direct indexing of an empty literal remains a separate reconstruction gap, while a let-bound
empty array index retains the reference's valid type verdict.

The stronger metadata comparison exposed missing Text and Fixed metadata in existing admission
tests. Keleusma now records whether the entry body's identifiers use a Text-bearing parameter,
and whether its structural return type contains Text. Body usage follows the reference's
syntactic-name convention, including shadowing, and excludes head guards. Metadata 106 carries
the two bits after the body. Named unused Text parameters remain admitted.

The text-flow mode in analyze.kel interprets opcode facts and computes abstract stack/local
textness, per-chunk return and yield bits, and the ephemerality decision. The host stores values
and schedules callees using the existing topological-order helper. It consumes no reference
text-analysis result. Storage grows fallibly with the actual stack and local counts. No new
source-size or local-count cap was added. Text allocation bounds remain outside this Boolean
analysis. The native-operand convention and cycle fallback match the reference analysis.
Protocol controls reject malformed commands and fields and demonstrate subsequent recovery.

Wildcard function parameters were already rejected by the shipping opcode comparison. The
parser had skipped the underscore token and then treated Text as the parameter name. It now
reports the existing unsupported-pattern diagnostic explicitly. Discard-pattern lowering
remains open. A named unused Text parameter remains a positive control.

Public bytecode version remains 2, public opcode count remains 66 and selected type capacities
remain 640. Current measured resources follow.

- parse has 206 chunks, a 174,436-byte artifact, 336,016 shared bytes and 1,073,472 private
  bytes. The source is 363,141 bytes. This increment adds 24 shared bytes and 30,272 private
  bytes to the preceding parser tree.
- analyze has 19 chunks, a 17,964-byte artifact, 147,696 shared bytes and 53,984 private bytes.
  The text protocol adds 136 shared bytes and 64 private bytes. Its fused compilation retains
  a peak group of 324 records while total records grow to 1,877.
- verify_types has 44 chunks and a 23,404-byte artifact. Its 111,824 shared bytes and 82,240
  private bytes are unchanged. The empty-array tag adds no pool storage.
- Code generation remains at 79 chunks and a 71,772-byte artifact.
- The legacy bounded wire-input maxima are 873 names and 24,662 bytes, both in parse.
  The existing limits stay at 1,024 names and 49,152 bytes.

## Verification

Runs use empty RUSTC_WRAPPER, CARGO_HUSKY_DONT_INSTALL_HOOKS set to one and
KEL_SELFHOST_CACHE zero. No verification subject was edited during a live run.

- The final workspace run covers 187 suites with 3,197 passes, one failure and five ignored
  tests. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-signatures-workspace-final.log. Session 26757 exits 101. The only failure is
  the stale analyze total-record pin, measured at 1,877 instead of 1,538. Peak residency
  remains 324. The test-only correction passes all 152 code-generation tests in
  tmp/order1-signatures-fused-pin-fixed.log. Session 69344 exits zero. Command cargo test
  --release --features self-host --test selfhost_codegen --no-fail-fast.
  The full workspace was not rerun after that pin correction.
- The permanent shipping artifact test passes all fourteen stage sources in the workspace run.
  The same run passes monomorphization controls and all current source and protocol regressions.
- Twenty-seven selected no-default tests pass. Command cargo test --release --no-default-features
  --features self-host --test selfhost_nominal_signatures --test selfhost_empty_arrays --test
  selfhost_text_boundary --test selfhost_tuple_projections --test selfhost_type_transport
  --no-fail-fast. Log tmp/order1-signatures-text-no-default.log. Session 84869 exits zero.
- Four final inference checks pass, erasing 56,408 recorded expression types across fourteen stages.
  Command cargo test --release --features self-host --lib compiler::inference_independence
  -- --nocapture. Log tmp/order1-signatures-final-inference.log. Session 86052 exits zero.
- Text-flow differential, protocol recovery and cycle/native controls pass three tests in
  tmp/order1-signatures-text-protocol.log. Command cargo test --release --features self-host
  --lib selfhost::text_flow -- --nocapture. Session 5296 exits zero. They pass again in the
  workspace. All 31 metadata mutations pass their library control in both workspace runs.
- The text/parser/driver run passes five driver checks, 91 parser checks with two ignored,
  and three text-boundary checks. Log tmp/order1-signatures-text-parser.log. Session 59756
  exits zero. Command cargo test --release --features self-host --test selfhost_text_boundary
  --test selfhost_parse --test selfhost_driver_parity --no-fail-fast.
- The analysis stage self-compiles identically in tmp/order1-signatures-analyze-selfcompile.log.
  Command cargo test --release --features self-host --test selfhost_codegen
  self_host_compiles_analyze_kel_byte_identically -- --nocapture. Session 7010 exits zero.
- Fresh final Clippy passes with actual Checking. Command cargo clippy --workspace --tests
  -- -D warnings. Log tmp/order1-signatures-last-clippy.log. Session 78752 exits zero.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-signatures-final-doc.log.
  Session 85023 exits zero.
- The build without the standard library passes with actual Checking. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-signatures-final-nostd.log.
  Session 35448 exits zero.
- Final resources are measured in tmp/order1-signatures-final-resource-measurement.log. The
  resources scratch binary exits zero directly. The residency scratch binary reports peak
  324 and total 1,877 in tmp/order1-signatures-residency.log, session 91744 exiting zero.

Earlier failures remain in ignored logs. The first nominal comparison has ten disagreements
in tmp/order1-signatures-first.log. The expanded metadata comparison exposed five array-header
mismatches in tmp/order1-signatures-array-probe.log. All sixteen final signature fixtures pass.
The first shipping corpus run in tmp/order1-signatures-artifacts.log fails on the new helper's
value-plus-conditional expression. Giving the conditional its own binding restores self-compilation.
A prior workspace run in tmp/order1-signatures-workspace.log has 3,189 passes and three failures
across 186 suites, exposing Text and Fixed metadata and ephemerality gaps. Those were repaired
before the final run. The first empty-equality runtime expectation was wrong and became an
explicit reference-fault control. The comparison fixture required a mutated private variable, and the text-flow fixture required
an explicit Text tail after yield. Neither invalid fixture was treated as compiler evidence.

No production source changed after the final workspace run. Only the measured test pin and
subsequent documentation changed. The root-library workspace count is 1,390. The source inventory
is 1,608 integration test functions across 159 files. The four documentation guards pass 24 tests in
tmp/order1-signatures-doc-guards.log, session 2355 exiting zero. Command cargo test --release
--test documentation_links --test claimed_counts --test comment_citations --test push_order_claims.
Final formatting and diff checks pass.


## Next work

The complete Order 1 goal remains active. Prioritize source private initializers, then mixed
tuple construction and general nested-header paths, followed by the remaining binder,
occurrence, alias, cast and enum-capacity obligations and final integration gates.

The new comparison catches a concrete initializer defect. This valid source has identical
chunk operations but a different initial state.

```keleusma
private data d { n: Word = 5 }
fn main() -> Word { d.n = d.n + 1; d.n }
```

The raw pipeline module contains Int zero and executes as one. The reference contains Int five
and executes as six. Shipping compilation now refuses the mismatch as Unsupported, naming
private initializers. The standing unresolved_private_initializers_cannot_escape_the_shipping_comparison
test ensures this metadata difference cannot escape again. Replace its unsupported expectation
with positive artifact and execution controls when the producer is repaired. The default-zero
version is a passing control. Preserve the complete comparison while fixing source extraction.

The scratch receipt is tmp/order1-signatures-initializer-probe.log. The binary is
tmp/order1-type-probe/src/bin/source_initializers.rs. Run cargo run --offline --release
--manifest-path tmp/order1-type-probe/Cargo.toml --bin source_initializers, with CARGO_TARGET_DIR
set to tmp/order1-admission-probe/target and the wrapper/cache settings above. It reports
observations and exits zero. Inspect header_field_skip and commit_field in parse.kel and
assemble_private_init in src/selfhost/mod.rs. The latter still produces default zeros.
Do not import initializer values from the reference module to close this gap.

Retain the earlier mixed-tuple and general-header witnesses.

```keleusma
fn main() -> Word { let t = (true, 7); t.1 }
fn pair() -> (bool, Word) { (true, 7) }
fn nested() -> Word { let t = ((1, 2), 3); t.0 .1 }
fn from_array(xs: [((Word, Word), Word); 2]) -> Word { xs[0].0 .1 }
```

Run these as separate programs. The current receipt tmp/order1-signatures-remaining-final.log
confirms that mixed literals still use sixteen bytes instead of nine, and that the nested literal
and array-header forms leave two reconstructed roots. Session 6909 exits zero because the probe
reports observations rather than asserting agreement. Its seven observations include two reports
for each reconstruction failure and the known unsupported non-range loop. They are not seven
independent defects. Use the remaining binary in tmp/order1-type-probe with the same Cargo recipe.
Do not erase the reconstruction cardinality guard. The ordinary projected nominal-signature
witnesses from the preceding handoff are now fixed. The empty-array argument mismatch is also
fixed. Full-language support remains Order 6, and the complete Order 1 audit remains required.

Full feature matrices, minimum supported toolchains, Miri, cross-target checks, dependency
advisory scanning and final integration gates have not all been established by this increment.

## Shadow pilot and interline

The pilot task is v023-order1-nominal-layouts. The first record attempt reused the completed
v023-order1-signatures task identifier and was refused. Preview of that unrecorded case also
failed. A fresh task and case identifier were then used with the same first diagnostic.

The first diagnostic is v023-20261011-nominal-layouts-triage, labelled compiler_or_test.
Three completion excerpts cover the module mutations, the shipping artifact corpus and the
empty-array verdicts. Their suffixes are metadata, corpus and empty, and all are labelled
supports. Every label has independent set to false. No model accuracy claim follows from these
self-reviews. Later failures were not additional diagnostic samples, following the first-diagnostic
sampling rule. The three-claim limit excludes separate text-flow and resource samples.

The bounded increment outcome is resolved. The full Order 1 goal remains active. Provider usage
and elapsed time are unavailable and recorded as null. External evaluation remains unavailable
under the earlier automatic approval rejection. No new service request or bypass was attempted.

The reviewed export is tmp/order1-nominal-layouts-shadow-final-20261011.json. Request hashes,
payload equality and source snapshots were checked for all four current-task cases. The original
nominal test source was reconstructed from the recorded edit in
tmp/order1-signatures-first-source.rs. Its digest exactly matches the captured source snapshot,
0b282ef998f33517e9daf38bbbb4570b3f314072194a780528e4283f47b003d9. The later source expansion
did not rewrite that snapshot. The peer-owned tracked pilot checkpoint was not edited.
Request hashes follow in diagnostic, metadata, corpus and empty-array order.

- efe4d64facf10a0049069d0abc3c73a3dbd546599e9e25e798a7a8c15b9e258d
- ad0792cc499d2397e586735b1be25517a90a1ce1d51b3914026430c7d50b455e
- 5e1665d915c0de976d34dd1434dadd219216f7abc26ba132b2e481c0eb5bf5ef
- 648cd7bd32cba96e84343034fe48d09682c9b77ef7e83c3cf7e48159ed067d70

The interline inbox is empty. The checkpoint was exported and reviewed with no tracked change.
No peer message was sent. No verification process remains live at handoff.
