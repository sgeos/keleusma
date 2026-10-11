# Reverse prompt

## Current state

Continue the complete Order 1 goal and System 1 shadow pilot on line v0.2.3.
This increment started at cb033eb9 on feat/v02-order1-next. The legacy handoff is
invalid-and-stale, with its stamp 50 commits behind that starting head. Appendix B
was read before tracked writes. The V0.3.X worktree and pre-existing .codex/ and
nvim.log remain untouched. No push, merge or publication was performed.

This is a feature checkpoint with two failing tests. The repository permits feature
branches to commit on red. Keep both assertions and repair their causes or restore
the intended measurement. Do not merge this checkpoint into a version branch.
The full Order 1 goal remains active.

## Return-layout implementation

The parser now performs a token census that records the return-type range of each
source function. Scalar return kinds are known before the callee declaration is
parsed normally. Composite return types are parsed on demand by the existing
Keleusma header grammar. Cached descriptors are tagged with the current body
generation so temporary descriptor reuse cannot leave a live cache pointer.

Private record 111 announces a token seek. The driver restarts the lexer on a
backward seek and discards tokens on a forward seek, retaining at most eight tokens
in its window. Rust chooses no return type or layout. The appended shared
return_queries flag opts production feeds in. Legacy stage feeders and the existing
sequential cursor instrument leave it zero. The named fused feed now uses the
bounded token path. Shared-slot and six query-array capacities have explicit guards.

Ten valid source cases retain independent true verdicts and complete artifact
identity. Forward scalar, tuple, nested tuple, array and struct calls work in these
controls. Arrays of tuples returned from calls and direct indexing also work.
Four programs execute to seven after Keleusma serialization and decoding. Four
invalid uses reject independently. Collecting and bounded traces agree, with every
large cursor change covered by a seek record. Distant queries and a string containing
a fake declaration preserve all four lower-level compile entry points. The shipping
entry is covered by the ten-source corpus.

The parser has 233 chunks, a 194,916-byte artifact, 336,024 shared bytes and 1,367,904
private bytes. Shared storage grows by eight bytes and private storage by 197,056
bytes. Its final source is 383,333 bytes and produces 52,294 Keleusma tokens. The
collecting token cap remains 40,960. The source-byte cap remains 393,216. Public
bytecode version stays 2, opcode count stays 66, and selected type capacities stay 640.

## First repair priority

The new cache control exposed a guarded-head local-slot mismatch. The independent
source verdict is true and the reference compiles the program.

```keleusma
fn f(x: Word) -> Word when x == 0 {
    let a = pair();
    let b = pair();
    a.1 + b.1
}
fn f(y: Word) -> Word { let a = pair(); a.1 }
fn pair() -> (bool, Word) { (true, 7) }
fn main() -> Word { f(0) }
```

The shipping entry refuses chunk f at operation seven, with SetLocal 1 against
reference SetLocal 2. The standing failing test is
guarded_head_local_bindings_preserve_artifacts in tests/selfhost_return_layouts.rs.
It retains the complete artifact requirement. A separate cache test now passes and
checks exactly five seeks, comprising the census rewind and two body query round trips.
The scratch receipt tmp/order1-returns-cache-trace.log independently records the same.

Inspect reconstruct.kel step and build_multihead. Local records receive remap_base,
while the binary path passes a LetIn argument through unchanged. Head bases are
currently computed as the head ordinal plus one times the parameter count. A general
repair must also account for local allocations and guard/body temporaries before
later heads. Do not merely shift the first observed setter. The reconstruction and
code-generation stage sources were not changed by this return-layout increment.

## Second repair priority

The generic fixture in a_divergence_refusal_localises_and_does_not_assign_fault,
in tests/selfhost_typecheck.rs, no longer reaches the reference cross-check.

```keleusma
fn id<T>(x: T) -> T { x }
fn main() -> Word { id(1) }
```

Its message is now type checking unavailable because expression layout is unavailable
near token 12. The query sees the unknown return name T. Previously this fixture
reached a chunk-order divergence. Preserve a must-fire cross-check test. Determine
whether an unknown layout should be deferred until an operation needs it, or whether
the fixture must change while retaining actual cross-check coverage. Do not weaken
the assertion to accept any error.

## Verification

Commands use empty RUSTC_WRAPPER, CARGO_HUSKY_DONT_INSTALL_HOOKS set to one and
KEL_SELFHOST_CACHE zero. Verification subjects remained fixed during each live run.
Every handle listed below has a terminal result.

- The original ten-source comparison fails in tmp/order1-returns-first.log. Session
  88245 exits 101. The first implementation passes the ten sources in
  tmp/order1-returns-implementation.log, session 17681 exiting zero.
- The first expanded controls pass four return-layout and six tuple tests in
  tmp/order1-returns-controls.log, session 87153 exiting zero. Command cargo test
  --release --features self-host --test selfhost_return_layouts --test
  selfhost_tuple_construction -- --nocapture.
- The broad run passes 152 codegen tests, 91 parser tests with two ignored, five
  return-layout tests, six tuple tests and 102 type-checking tests. It fails the
  guarded-head artifact control and generic diagnostic fixture. Command cargo test
  --release --features self-host --test selfhost_return_layouts --test
  selfhost_tuple_construction --test selfhost_parse --test selfhost_typecheck --test
  selfhost_codegen --no-fail-fast. Log tmp/order1-returns-broad.log. Session 50770
  exits 101. At this point the cache and artifact assertions were still one test.
- The final return-layout target separates those assertions and reports six passes
  and one failure, guarded_head_local_bindings_preserve_artifacts. Command cargo test
  --release --features self-host --test selfhost_return_layouts -- --nocapture.
  Log tmp/order1-returns-final-controls.log. Session 19669 exits 101.
- The isolated generic diagnostic receipt is tmp/order1-returns-generic-diagnostic.log,
  exiting 101. The isolated original combined cache/artifact receipt is
  tmp/order1-returns-cache-diagnostic.log, session 32744 exiting 101.
- The no-default run passes five return-layout and six tuple tests and fails the
  combined guarded-head artifact control. Command cargo test --release
  --no-default-features --features self-host --test selfhost_return_layouts --test
  selfhost_tuple_construction --no-fail-fast. Log tmp/order1-returns-no-default.log.
  Session 89286 exits 101. This predates the test split.
- Six artifact tests and three monomorphization controls pass. Command cargo test
  --release --features self-host --test selfhost_artifact_identity --test
  selfhost_monomorphize_identity --no-fail-fast -- --nocapture. Log
  tmp/order1-returns-artifacts.log. Session 86714 exits zero. All fourteen complete
  stage artifacts pass Keleusma serialization and decoding comparisons. The artifact
  suite takes 474.24 seconds.
- Four inference tests pass after erasing 58,671 recorded expression types across
  fourteen stages. Command cargo test --release --features self-host --lib
  compiler::inference_independence -- --nocapture. Log tmp/order1-returns-inference.log.
  Session 30091 exits zero.
- Four shared-layout and capacity tests pass. Command cargo test --release --features
  self-host --lib selfhost_host::shared_layout_tests -- --nocapture. Log
  tmp/order1-returns-shared-layout-final.log. Session 49221 exits zero. The first run
  failed because its loop-limit reader requires the opening brace at the line end.
  Formatting that loop and declaring each query array on its own line repairs the
  guard and allows explicit checks of all six new arrays.
- The formatting-only stage change after the all-stage run preserves both the full
  Keleusma token stream and complete parser artifact. Scratch assertions compare the
  pre-format snapshot with the current file. Log tmp/order1-returns-format-identity.log.
  Session 78681 exits zero. No executable stage tokens changed after the all-stage run.
- Fresh Clippy passes with actual Checking. Command cargo clippy --workspace --tests
  -- -D warnings. Log tmp/order1-returns-clippy-final.log. Session 93176 exits zero.
- Documentation passes with actual Documenting and RUSTDOCFLAGS set to -D warnings.
  Command cargo doc --workspace --no-deps. Log tmp/order1-returns-doc.log.
  Session 62689 exits zero.
- The build without the standard library passes with actual Checking. Command cargo
  check --no-default-features --features self-host. Log tmp/order1-returns-nostd.log.
  Session 79489 exits zero.

The final test split is checked by another Clippy run with actual Checking in
tmp/order1-returns-clippy-split.log, which exits zero. The four documentation guard
targets pass 24 tests in tmp/order1-returns-doc-guards.log, session 89475 exiting zero.
Command cargo test --release --test documentation_links --test claimed_counts --test
comment_citations --test push_order_claims. Formatting and diff checks pass.

The complete workspace,
full feature matrix, minimum supported toolchains, Miri, cross-target builds,
advisory scanning and final integration gates remain outstanding. The source
inventory is 1,630 integration test functions across 163 files. No library test
functions were added. The full Order 1 goal is not complete.

## Shadow pilot and interline

Pilot task v023-order1-returns-cb03 contains four cases with prefix
v023-20261011-returns-cb03 and suffixes triage, corpus, execution and transport.
The triage label is compiler_or_test and the three bounded claim labels are supports.
All labels are non-independent self-review. Their claims concern the named passing
controls and do not claim a green increment. The outcome is unresolved, with missing
usage and elapsed values recorded as null.

Payloads were reviewed. External evaluation remains unavailable under the prior
automatic approval review rejection. No retry or bypass occurred. The reviewed export
is tmp/jev-shadow-returns-cb03-final.json, containing 174 samples across 44 tasks.
All four current-task request digests and their immutable source hashes match.
The three-claim sampling limit is reached. No provider judgment is claimed.

The assigned inbox is empty and no peer messages were sent. The tracked interline
checkpoint is exported and reviewed before handoff. The peer-owned tracked
shadow-pilot checkpoint remains untouched. After the two failing controls are repaired, continue the
remaining declaration, binding, occurrence, alias, cast and enum-capacity obligations
and perform the complete final audit in ORDER1_COMPLETION_PLAN.md.
