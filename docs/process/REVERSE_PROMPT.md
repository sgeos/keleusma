# Reverse prompt

## Current state

Continue the complete Order 1 goal and the System 1 shadow pilot on line v0.2.3.
The branch is feat/v02-order1-next. This increment started at 788b0e6f.
The legacy handoff is invalid-and-stale. Its stamp is 45 commits behind that starting head,
its ancestry check passes, and its twelve-stage and unchanged-stage claims are contradicted.
Appendix B was read before tracked writes. The concurrent V0.3.X worktree, pre-existing
.codex/ and nvim.log remain untouched. No push, merge or publication was performed.

## Tuple identity and layouts

The parser emits source element ordinals independently of packed tuple byte offsets. The type
stage follows structural tuple cells to derive the exact selected type. Named struct identity,
nested tuple members and array dimensions therefore survive projection. Metadata 104 carries
the source ordinal, metadata 105 carries stage-computed header layouts, and expression row 19
queries tuple elements. These are private protocols. Bytecode version stays 2 and the public
instruction set stays at 66 opcodes. The selected type-stage capacities stay at 640.

Tuple parameter and return headers now have an iterative scanner for named types, grouped types,
unit, nested tuples and arrays with literal dimensions. Equal layouts reuse descriptors.
Eighty repeated parameter headers and eighty repeated return headers retain admission.
Depth 32 succeeds and 33 reports an unavailable diagnostic. Thirty-two distinct layouts succeed
and 33 reports an unavailable diagnostic. The depth test drives the stage directly because the
reference parser has a separate recursion limit of 24. Fixed-size zero-length declarations and
out-of-range tuple ordinals are source rejections. An empty array literal remains a separate issue.

Projection layout metadata restores chained tuple, array and struct reads and let aliases.
The private code-generation operand marker 4 distinguishes nested Tuple from scalar extraction
and decodes to the existing public Tuple kind. Nested equality uses the existing frame walker.
A new four-shape comparison exposed a struct inside a third-level tuple being emitted as a scalar.
The tuple frame now descends into that struct. Equal and unequal nested tuple inputs execute as
true and false. Six additional host-input cases produce reference artifacts and return seven.

The historical code-generation harness now uses the bounded token feed while retaining its own
record collector. The previous resident seeder overflowed on the larger parser source. A function
inventory assertion prevents reference chunks from silently surviving absent parser output.
The code-generation self-compilation inventory is 79 functions. A former Boolean-array deferral
fixture now requires complete artifact identity through the shipping entry.

Measured resources from the current sources follow. Parser shared storage stays at 335,992 bytes.
Its private storage grows from 725,184 to 1,043,200 bytes, an increase of 318,016 bytes.
The 9,938 added private value slots cost 32 bytes each. They are not eight-byte packed words.
The parser has 198 chunks, a 165,924-byte artifact and 356,356 source bytes.
Code generation has 79 chunks and a 71,772-byte artifact. The type stage has 43 chunks,
a 22,580-byte artifact and 82,240 private bytes. Its two added scalar slots cost 64 bytes.
The header workspace bounds are 32 frames, 1,024 descriptors, 32 tuple layouts and 256 members.
Tuple equality still uses the existing bounded equality machinery. These finite controls do not
establish arbitrary nesting or complete composite support.

## Verification receipts

Runs use empty RUSTC_WRAPPER and CARGO_HUSKY_DONT_INSTALL_HOOKS set to one.
Stage runs use KEL_SELFHOST_CACHE zero. Verification subjects were held fixed during live runs.

- The final workspace run executes 184 suites with 3,182 passes, two failures and five ignored
  tests. Command cargo test --release --workspace --no-fail-fast. Log
  tmp/order1-tuple-workspace.log. Session 31817 exits 101. Both failures are guards described below.
- The declaration-record guard omitted numeric alternatives beside named constants. It now
  extracts each numeric alternative and retains guards. A new control removes the range and
  removes its guard separately. Named constants remain outside this textual check.
  The wire capacity guard now pins the measured 839 names instead of 776. Its 1,024-name and
  49,152-byte bounds did not change. The maximum legacy input blob remains 23,753 bytes.
- Both affected targets pass after these test-only changes. Command cargo test --release
  --features self-host --test selfhost_driver_parity --test selfhost_wire --no-fail-fast.
  Log tmp/order1-tuple-guards-fixed.log. Session 40621 exits zero, with five parity tests
  and 184 wire tests. The full workspace was not rerun after these guard changes.
- Eight tuple tests pass in the workspace run and again without default features. They contain
  fourteen verdict comparisons, six artifact/runtime cases, source-ordinal and metadata controls,
  repeated-header admission, nested equality execution, four nested-struct artifact shapes
  and capacity boundaries. Negative verdict cases require PipelineRejected at the shipping entry.
- All fourteen complete stage artifacts match in tmp/order1-tuple-final-artifacts.log, with six
  artifact tests and three monomorphization controls passing. Command cargo test --release
  --features self-host --test selfhost_tuple_projections --test selfhost_artifact_identity
  --test selfhost_monomorphize_identity --no-fail-fast -- --nocapture. Session 51490 exits 101
  solely because the new depth fixture incorrectly called the reference parser at depth 32.
  That fixture was corrected and passes in tmp/order1-tuple-header-capacity-fixed.log,
  session 92630 exiting zero. The workspace subsequently passes all three targets.
- Four inference checks pass in tmp/order1-tuple-final-inference.log. Command cargo test --release
  --features self-host --lib compiler::inference_independence -- --nocapture.
  Session 41029 exits zero. Erasing 55,276 recorded types preserves all fourteen artifacts.
- All fourteen legacy type-input channels fit in tmp/order1-tuple-final-capacity.log.
  Command cargo test --release --features self-host --test selfhost_typecheck
  the_stage_is_measured_against_the_real_stage_sources -- --nocapture.
  Session 99371 exits zero. Wire remains tightest at 638 of 640 occurrence rows.
- All fourteen stages also pass self_hosted_compile and complete reference artifact comparisons
  in tmp/order1-tuple-shipping-stages.log. Session 69205 exits zero. Run cargo run --offline
  --release --manifest-path tmp/order1-type-probe/Cargo.toml --bin shipping_stages with
  CARGO_TARGET_DIR set to tmp/order1-admission-probe/target. The probe inventories the directory,
  calls the shipping entry and compares complete bytes against independently compiled reference
  modules. Its final line reports fourteen stages and zero gaps.
- Twenty-four tests pass without default features in tmp/order1-tuple-no-default.log.
  Command cargo test --release --no-default-features --features self-host --test
  selfhost_tuple_projections --test selfhost_type_transport --test selfhost_composite_annotations
  --no-fail-fast. Session 48027 exits zero.
- Fresh Clippy after the guard repairs passes with actual Checking. Command cargo clippy
  --workspace --tests -- -D warnings. Log tmp/order1-tuple-final-guards-clippy.log.
  Session 6365 exits zero. Earlier current-code Clippy also passed in session 29068.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-tuple-doc.log. Session 53559 exits zero.
- The build without the standard library passes with actual Checking. Command cargo check
  --no-default-features --features self-host. Log tmp/order1-tuple-nostd.log.
  Session 80769 exits zero.
- Resource receipts are tmp/order1-tuple-resources.log and
  tmp/order1-tuple-baseline-resources.log. Both scratch resource binaries exit zero directly.
  The baseline parser source comes from git show 788b0e6f:src/selfhost/kel/parse.kel.

Earlier failures are retained in ignored logs. The first eight-case tuple comparison has three
gaps in tmp/order1-tuple-first.log. The first broad parser/code-generation run in
tmp/order1-tuple-parser-codegen.log exposed omitted metadata, resident token-buffer overflow
and nested equality failures. The former malformed transport kind 19 became valid and its
negative control moved to 20. An attempted zero-length fixed-array valid fixture was wrong
and became a rejection case. A deeper struct-in-tuple failure is preserved in
tmp/order1-tuple-deep-equality.log and its repair in tmp/order1-tuple-deep-equality-fixed.log.

No production source changed after the full workspace run. Two test files changed for the guard
repairs, followed by documentation updates. The source inventory is 1,599 integration test
functions across 156 files. The workspace root-library run passes 1,386 tests.
The four documentation guards pass 24 tests. Command cargo test --release --test
documentation_links --test claimed_counts --test comment_citations --test push_order_claims.
Log tmp/order1-tuple-doc-guards.log. Session 49043 exits zero. A trailing blank line reported
by the diff check was removed before the final formatting and diff checks.


## Next work from the remaining-source probe

The complete Order 1 goal remains active. Prioritize the nominal signature gap, then tuple literal
layouts and general header paths, then the remaining independent rejection obligations.

The probe is tmp/order1-type-probe/src/bin/remaining.rs. Run cargo run --offline --release
--manifest-path tmp/order1-type-probe/Cargo.toml --bin remaining with CARGO_TARGET_DIR set to
tmp/order1-admission-probe/target and the wrapper/cache settings above. Its final log is
tmp/order1-tuple-remaining-probe-final.log. Session 76137 exits zero because this diagnostic
program reports observations rather than asserting agreement. It covers thirteen sources.
Its ten observations are not ten independent defects, since two sources have both an extraction
failure and a shipping refusal. The non-range for-loop case is a known unsupported surface.

The following two valid programs pass the shipping chunk-body comparison but their complete
artifacts differ from the reference. Their parameter signatures are Top instead of the stated
flat shapes. The shipping comparison checks chunk names, operations, constants and local counts,
so it does not catch this metadata difference. Repair signature derivation and strengthen the
shipping comparison to cover assembled metadata while preserving valid-source admission.
Do not describe shipping success as artifact identity.

```keleusma
struct Q { x: Word }
struct P { q: Q }
fn main(p: P) -> Word { p.q.x }
```

The reference parameter is Flat Struct with size 8.

```keleusma
struct Q { x: Word }
struct P { q: Q }
fn main(t: (P, Word)) -> Word { t.0.q.x }
```

The reference parameter is Flat Tuple with size 16. In header_named, any zero scalar-kind struct
field makes the whole layout unknown, even when sd_fstruct identifies a previously declared
nested struct. Investigate transitive layout knowledge rather than substituting a byte-size guess.
A struct with a scalar-array field is a passing control in the same probe.

Mixed tuple literals are still lowered as all-Word layouts. Both sources below receive valid
independent type verdicts but shipping rejects NewComposite size 16 against reference size 9.

```keleusma
fn main() -> Word { let t = (true, 7); t.1 }
fn pair() -> (bool, Word) { (true, 7) }
```

The probe executes these as separate sources, naming the second function main.
Nested tuple literals and an array of nested tuple parameters also remain unavailable.
The spaces between projections avoid the reference lexer's numeric-token ambiguity.

```keleusma
fn main() -> Word { let t = ((1, 2), 3); t.0 .1 }
fn from_array(xs: [((Word, Word), Word); 2]) -> Word { xs[0].0 .1 }
```

Each separate source leaves two reconstructed roots. The second probe source names its function
main. Do not erase the reconstruction cardinality guard to admit either source.

An empty array argument is independently accepted while the reference rejects it.

```keleusma
fn f(xs: [Word; 2]) -> Word { 7 }
fn main() -> Word { f([]) }
```

An unused empty array literal is valid and retains artifact identity. Blanket empty-array rejection
would therefore narrow admission. Three casts from tuple, bool and nominal struct to Word are
correctly rejected in this finite probe. That does not establish complete cast coverage.

Preserve the completion plan's other binder, occurrence, alias, enum-capacity and general type-input
obligations. Refresh current corpus and shipping evidence after further changes. Full feature
matrices, minimum supported toolchains, Miri, cross-target checks, advisory scanning and final
integration gates remain unverified here.

## Shadow pilot and interline

The local pilot task is v023-order1-tuple. Its first diagnostic is
v023-20261011-tuple-triage, labelled compiler_or_test. Three completion samples are
v023-20261011-tuple-verdicts, v023-20261011-tuple-artifacts and
v023-20261011-tuple-equality. The verdict and equality excerpts are labelled supports.
The artifact excerpt is labelled unsupported because it contains equal byte counts and a
passing test name without the byte-comparison assertions. The actual artifact result is
established separately by the inspected test source and the executed artifact target.
All labels have independent set to false. No accuracy claim follows from self-review.

The bounded tuple repair outcome is resolved. The full Order 1 goal is still active.
Provider usage and elapsed time are unavailable and recorded as null. The first diagnostic
and three claims follow the pilot sampling rule. Later failures were not additional samples.
External evaluation remains unavailable under the earlier automatic approval rejection.
No new service request or bypass was attempted.

The reviewed export is tmp/order1-tuple-shadow-final-20261011.json. Four current-task request
hashes and payload equality were checked against the captured documents. Claim source hashes
match. The first diagnostic log hash matches, while its test source was extended after capture.
That historical source hash was preserved. The peer-owned tracked pilot checkpoint was not edited.
Request hashes follow in diagnostic, verdict, artifact and equality order.

- 69c473c0b18c7b2ffbe25c036bff173ee7443440cb016330eff1f3e7cd9ebe66
- a21b7afef1de0ac1c500a4ed286def485bd495ac7f3b063ca28a95911970e4b1
- 8373d1d0644f3b71532374e9a5fe91ff046cddbe09bb1c2762ba717d61e90379
- 29deea04bf527cd88e264dff2cc2c5df08f86105a03cc3075d1a0b0a068f8609

The interline inbox is empty. The checkpoint was exported and reviewed with no tracked change.
No peer message was sent. No verification process remains live at handoff.
