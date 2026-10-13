# Reverse prompt

## Current state

Continue the complete Order 1 goal and System 1 shadow pilot on line v0.2.3.
This work started at a7ef30ed on feat/v02-order1-next. Its two red controls are now
repaired. The legacy handoff remains invalid-and-stale, with its stamp 51 commits
behind the starting head. Appendix B was read before tracked writes. The V0.3.X
worktree and pre-existing .codex/ and nvim.log remain untouched. No push, merge or
publication was performed.

All five feature configurations now have passing full-suite receipts. The final
standalone self-host refresh includes the new stage-decision intervention. The
[completion audit](../decisions/ORDER1_COMPLETION_AUDIT.md) records the requirements
and evidence. It found an explicit Workstream A item still unsupported, loop outcome
arms. The full Order 1 goal remains active. Do not infer completion from the green
stage corpus or redefine the milestone to omit that named item.

## Implemented frame and admission repairs

Reconstruction retains relative slots for each head's guard and body. The first word
of a head_parts entry is now its parameter count. Keleusma code generation allocates
parameter-copy windows and schedules guard/body phases in order. Later heads start
after prior allocations. Let bindings are allocated after their initializers, and each
logical-right-shift temporary is allocated after its operand. Parser slot identities
map to physical slots, with a 2,048-entry map matching the bounded emitter. Emitted
uses establish the frame high-water mark. A separate forest_temps counter computes
nested equality strides without changing allocation state. Private work tags use
radix sixteen. No public opcode or artifact version changed.

Thirteen artifact controls and four decoded execution cases pass. They cover multiple
heads, guard and body temporaries, loops, matches, nested shifts and later bindings.
The inherited guarded-head witness passes. Unknown generic return layout is deferred
until needed, restoring the original generic divergence control. A tuple-member use
still reports unavailable layout. The reference comparison and error distinction stay.

The full workspace uncovered three additional issues. Tuple interning invalidated an
old exhaustion fixture. Its replacement uses distinct shapes to test 32 layouts,
64 literal members and 256 member entries, plus a repeated-shape reuse control. Whole
nested nominal arrays pass through without requiring an unused element layout. Legacy
wire-input maxima are pinned to the measured 962 names and 27,409 bytes, below unchanged
1,024-name and 49,152-byte caps. Test feature gates now match the production self-host
API they use. Codegen tests run in the self-host and workspace configurations. Two
wire controls and their helper declarations are locally gated. Other wire tests still
run without self-host.

Current stage resources are in tmp/order1-completion-resources.log.

- Parse has 233 chunks, a 195,012-byte artifact, 336,024 shared bytes and 1,367,904 private
  bytes. Its source is 383,694 bytes, below the 393,216-byte lexer source cap.
- Codegen has 90 chunks, a 76,700-byte artifact, 58,424 shared bytes and 248,320 private
  bytes. Its source is 142,127 bytes.
- Reconstruct has 34 chunks and a 33,068-byte artifact.
- Public bytecode version stays 2, public opcode count stays 66 and selected type
  capacities stay 640.

## Next required work

The roadmap explicitly lists for-limit outcome-arm lowering under Workstream A's
first-pass tasks. These valid witnesses remain unsupported, measured in
tmp/order1-outcome-audit.log. Both compile through the reference.

```keleusma
fn main(n: Word) -> Word {
    let i = 7;
    for i in 0..2 limit 2 {} on { ok => {}, limit => {} }
    i
}
```

```keleusma
private data d { hit: Word }
fn main() -> Word {
    for i in 0..5 limit 5 { if i == 2 { break; } }
    on { ok => {}, break(bi) => { d.hit = bi; }, limit => {} }
    d.hit
}
```

The current result is Unsupported with parser name ids five and six respectively.
Use tests/for_limit.rs and the reference compile_loop_outcome_dispatch for semantics.
OK bindings receive the completed count. BREAK and LIMIT bindings receive the exit
loop variable. A missing BREAK handler falls through to OK. An absent LIMIT handler,
or its false guard, traps. Preserve guard, scope and index-binding behavior and the
conservative verifier. Reuse existing public opcodes.

The parser currently finalizes a loop in step_for_emit before recognizing an outcome
block. Its forst context is stacked to depth eight. Reconstruct stores twelve fields
per ForLimit, and codegen emits the bare post-loop reclassification and trap path.
Extend the stage-owned representation and all affected record consumers together.
Do not replace source-owned decisions with a Rust AST walk.

The audit also preserves the original first-pass boundary. Full-language support is
Order 6. Historical legacy-extractor limitations are not proof that the production
scoped path has the same limitation. Re-derive each claimed gap and maintain explicit
boundary tests. Audit the new outcome-arm work before declaring Order 1 complete.

## Verification receipts

Commands use empty RUSTC_WRAPPER, CARGO_HUSKY_DONT_INSTALL_HOOKS set to one, and
KEL_SELFHOST_CACHE zero for self-host verification. Subjects were held fixed during
live runs. Every handle below is terminal. The new intervention is test-only.

| Check | Command and receipt |
|---|---|
| Workspace | cargo test --release --workspace --no-fail-fast. tmp/order1-frames-workspace-final.log. Session 96468 exits zero. 3,227 pass, zero fail, five ignored across 192 summaries. This precedes the added intervention test. |
| Final self-host | cargo test --release -p keleusma --features self-host --no-fail-fast. tmp/order1-completion-selfhost.log. Session 54094 exits zero. 2,970 pass, zero fail, five ignored across 166 summaries, including the intervention. |
| No default | cargo test --release -p keleusma --no-default-features --no-fail-fast. tmp/order1-full-no-default.log. 329 pass, zero fail, two ignored. |
| Signatures | cargo test --release -p keleusma --features signatures --no-fail-fast. tmp/order1-full-signatures.log. 2,349 pass, zero fail, three ignored. |
| Signatures and shell | cargo test --release -p keleusma --features signatures,shell --no-fail-fast. tmp/order1-full-broad-features.log. 2,366 pass, zero fail, three ignored. |
| Detached compiler | cargo test --release --manifest-path compiler/Cargo.toml --no-fail-fast. tmp/order1-detached-compiler-tests.log. 86 pass, zero fail. Its Clippy and formatting checks pass in the companion logs. |
| Current Clippy | cargo clippy --workspace --all-targets -- -D warnings. tmp/order1-completion-clippy.log. Session 51596 exits zero with actual Checking. |
| Minimum Rust versions | Rust 1.85.0 arena default and no-default checks pass. Rust 1.88.0 core default, no-default, signatures and test checks pass. Logs tmp/order1-msrv-*.log, ending with core-tests-clean. Toolchains were installed without changing the default. |
| Embedded target | cargo build -p keleusma --target thumbv7em-none-eabihf. tmp/order1-frames-thumb-build.log exits zero. |
| Miri | Tree Borrows arena tests have 50 unit, six integration and eight documentation passes, with one ignored. The null-text regression passes. Logs tmp/order1-miri-arena-final.log and tmp/order1-miri-null-text-final.log. |
| Dependency audit | cargo audit. tmp/order1-dependency-audit-final.log exits zero after refreshing 1,296 advisories and scanning 102 dependencies. |
| Books | English and Japanese builds pass. The extracted translation template matches. Book examples cover 60 complete programs with 85 assertions and zero failures. Logs tmp/order1-book-*.log. |
| ShellCheck | shellcheck scripts/*.sh. tmp/order1-shellcheck.log exits zero. |
| Broad docs | cargo doc -p keleusma --no-deps --features signatures,encryption,shell,self-host with deny-warning RUSTDOCFLAGS. tmp/order1-full-feature-doc.log exits zero. |
| Wire variants | Both all-features and no-default wire-package tests pass in tmp/order1-wire-all-features.log and tmp/order1-wire-no-default.log. |
| Actual command corpus | Every one of the fourteen .kel stages compiles through target/release/keleusma compile with --compiler self-hosted. tmp/order1-cli-corpus.log, session 57545 exit zero. Every output matches complete reference bytes in tmp/order1-cli-corpus-identity.log. |

All fourteen stage artifacts and monomorphization controls pass inside the full suites.
The current inference intervention erases 58,865 recorded types with complete artifact
identity. Log tmp/order1-completion-inference.log, four tests passing. The new unit test
in src/selfhost/typecheck.rs disables only the Keleusma expression-type decision while
keeping production-derived input unchanged. Derived rejection changes to acceptance,
while direct rejection remains effective. Its focused receipt is
tmp/order1-derived-decision-control-final.log and the final full self-host run includes it.
The initial test mutation removed an adjacent helper and failed to bootstrap. That is
preserved in tmp/order1-derived-decision-control.log and was repaired before the full run.

Earlier failures remain in tmp/order1-frames-first.log, controls.log, edges.log,
final-broad.log and workspace.log. The first full workspace had 3,224 passes and three
failures, now repaired and followed by the complete green rerun. Minimum-version test
compilation first exposed missing feature gates and then unused gated helpers. Both
are fixed without suppressing warnings. Miri setup and the advisory cache initially
hit sandbox write restrictions, then completed through approved narrow escalations.
An initial relative book destination escaped the workspace and was corrected to an
absolute path under tmp. No source was changed to bypass these environment failures.

The documentation guard targets pass 24 tests in tmp/order1-frame-handoff-guards.log,
session 69748 exiting zero. Command cargo test --release --test documentation_links
--test claimed_counts --test comment_citations --test push_order_claims. The final
receipt update is checked again before commit.

The root workspace library run counted 1,394 tests before the new intervention. The
final isolated self-host library run counted 1,345 tests. The source inventory contains
1,633 integration test functions across 164 files. Release-only platform jobs outside
the affected compiler path and remote continuous integration are not claimed here.

## Pilot and handoff

Task v023-order1-frames-a7ef has four cases with prefix v023-20261011-frames-a7ef and
suffixes triage, corpus, execution and generic. Triage is labelled compiler_or_test.
The three bounded claims are labelled supports. All labels are non-independent
self-review. Their sources and exact request hashes were reviewed. The frame-task
outcome is resolved, with unavailable usage and elapsed values recorded as null.
The full Order 1 goal remains active because of the outcome-arm item.

The reviewed export is tmp/jev-shadow-frames-a7ef-final.json, containing 178 samples
across 45 tasks. All four current-task request digests and immutable source hashes
match. External evaluation remains unavailable under the previous automatic approval
review rejection. No retry, provider judgment or independent accuracy claim occurred.
The three-claim sample limit was reached before the final audit intervention.

The assigned inbox is empty. No peer messages were sent. The tracked interline checkpoint
is exported and reviewed before handoff. The peer-owned tracked pilot checkpoint
remains untouched. No unfinished verification process should be resumed from the
older tmp/order1-verification-resume.json, whose two handles have now exited zero.
