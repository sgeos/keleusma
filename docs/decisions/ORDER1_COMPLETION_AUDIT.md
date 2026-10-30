# Order 1 completion audit

> **Navigation** [Completion plan](./ORDER1_COMPLETION_PLAN.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)


**Accepted by the operator first-hand on 2026-10-28.** The operator accepted the Order 1
first-pass gate as recorded in this audit.
## Scope and verdict

Verified on 2026-10-13. Order 1 is complete for the first-pass compiler subset defined
by the roadmap. The shipping compiler processes all fourteen stage sources, derives
its type inputs from the Keleusma pipeline, rejects the required invalid-source corpus,
and emits complete artifacts through Keleusma. The explicitly listed loop outcome-arm
requirement is now implemented. Full-language support remains Order 6.

The reference compiler remains a mandatory comparison oracle. This verdict does not
claim full grammar support, retirement of Rust bootstrap compilation, or completion of
the separate validator milestone. Legacy extraction and bounded encoder interfaces retain
explicit boundary controls.

## Requirement evidence

| Requirement | Current evidence | Verdict |
|---|---|---|
| Complete stage corpus | `production_compiler_matches_every_complete_stage_artifact` and the directory inventory in tests/selfhost_artifact_identity.rs cover all fourteen current sources. Both full suites pass. | Verified |
| No reference scaffold borrowing | `self_host_compile_scratch` assembles chunks and module fields from pipeline records, reconstructed bodies, emitted instructions and resource analysis. Reference artifacts supply no fields to that construction. | Verified |
| Pipeline-owned type inputs | `try_call_type_inputs_from_pipeline` transports facts from Keleusma parser records and reconstructed nodes. The independent production path does not read the reference parser tree or recorded expression types. | Verified |
| Independent rejection | `production_types_reach_corpus_and_rule_form_census` covers the required negative corpus, valid controls and rule/form census. Shipping negatives must be `PipelineRejected`. Eleven outcome-specific negatives also pass with that result. | Verified |
| Stage-owned derived decisions | `derived_binding_verdict_depends_on_the_keleusma_decision` in src/selfhost/typecheck.rs holds transported inputs fixed and disables only stage-side derivation. Derived rejection changes to acceptance while direct rejection remains effective. Both full suites execute the control. | Verified |
| First-pass monomorphization | tests/selfhost_monomorphize_identity.rs inventories every stage, proves identity on all fourteen sources and retains a changing control. | Verified |
| Independence from recorded inference | All four controls in src/compiler/inference_independence.rs pass. Erasing 59,999 recorded types leaves all fourteen complete artifacts unchanged. | Verified |
| Complete Keleusma serialization | Framing, instructions, operands, auxiliary sections and integrity fields pass complete reference comparisons and Keleusma decoding in the all-stage artifact test. The production emitter uses bounded transport windows. | Verified |
| Shipping command | Every current stage compiled through `target/release/keleusma compile --compiler self-hosted`. All fourteen files match Rust reference compilation of the same source. The concrete scope regression compiles and its artifact executes to seven through the command. | Verified |
| Loop outcome lowering and bindings | tests/selfhost_loop_outcomes.rs covers 23 artifact fixtures, eight scope/execution fixtures, seven decoded execution cases, eleven independent rejections and the accepted eight-level versus refused nine-level nesting boundary. tests/for_limit.rs also executes all eight reference scope controls. | Verified |
| Compatibility and bounds | Five feature configurations, the detached compiler, minimum Rust checks and embedded build pass. Public bytecode version stays 2, the public instruction count stays 66 and selected type capacities stay 640. | Verified |
| System 1 shadow participation | The first diagnostic and three bounded claims were captured, previewed, self-reviewed and exported with checked hashes. The task outcome is resolved. External evaluation remains unavailable under the prior automatic approval rejection. | Local capture and review complete |

## Regression evidence and remaining coverage, reconciled 2026-10-23

The portable receipt below records the named tests that already ran at b1dbfe4f.
These controls refine the acceptance rows without treating output equality as a
proof that the implementation never reads reference data.

| Acceptance row | Named regression evidence | Remaining limit |
|---|---|---|
| Reference-scaffold independence | `scratch_assembly_matches_every_stage_with_the_reference_poisoned` in src/selfhost/reference_poison_tests.rs assembles all fourteen stages while the reference lexer refuses the marked user program, and requires zero poisoned lexings and complete artifact equality. `production_compiler_matches_every_complete_stage_artifact` and `all_stage_artifacts_are_independent_of_recorded_expression_types` remain. | The poison matches a marker carried on every column-zero function head and on a final line. A consultation of a fragment holding no copy would evade it. The shipping entry is not a subject because it compiles the user program with the reference by design. |
| Pipeline-owned type inputs | `production_type_verdicts_hold_with_the_reference_poisoned` in src/selfhost/reference_poison_tests.rs runs `type_check_source_via_kel` over the fourteen stages, the twenty ill-typed programs and the seven valid controls with the reference poisoned. Every verdict is unchanged and no poisoned lexing occurs. `pipeline_expression_extraction_reaches_every_stage`, `production_types_reach_corpus_and_rule_form_census` and `derived_binding_verdict_depends_on_the_keleusma_decision` remain. | The sixteen rule-form cells and eight rule-form controls are not repeated under poison. The fragment limit of the previous row applies. |
| Complete serialization | `production_compiler_matches_every_complete_stage_artifact` and `complete_artifact_corpus_covers_every_stage_source` in tests/selfhost_artifact_identity.rs | Complete artifact equality and decoding cover the current stage inventory. They do not prove unsupported source forms. |
| Shipping command | `the_self_hosted_command_compiles_every_stage_to_the_reference_artifact` in keleusma-cli/tests/self_hosted_stage_corpus.rs runs the built binary with `compile --compiler self-hosted` on every stage file and requires byte equality with same-source reference compilation. `the_same_source_compiles_identically_in_separate_processes` and `independent_type_rejection_reaches_the_command` remain. | Signed, encrypted, stripped, debug and cross-target outputs are not exercised by the corpus test. |
| Compatibility and bounds | The five configurations in the portable receipt, `self_hosted_construct_support_boundary` in tests/selfhost_codegen.rs and the explicit boundary controls cited in the numbered conditions | A matrix pass is not a universal capacity proof. Existing limits remain fixed. |
| Shadow pilot | The historical pilot record below describes local samples and unavailable external evaluation | No repository test can establish that historical private samples were captured or independently reviewed. Treat this as process evidence, not an implementation regression claim. |

Reproduce the artifact and type rows with
`cargo test --features self-host --test selfhost_artifact_identity --test selfhost_typecheck`
and the intervention with
`cargo test --lib --features self-host derived_binding_verdict_depends_on_the_keleusma_decision`.
Reproduce the command fixtures with
`cargo test -p keleusma-cli --test compile_reproducible --test self_hosted_refusal`.
The three controls added on 2026-10-24 are described in the next section.

### Reference-poisoning and command corpus controls, 2026-10-24

The no-borrowing and type-input rows previously rested on code inspection plus
output equality. Equality cannot distinguish an independent construction from
one that copies the reference. A test-only hook in `tokenize` in src/lexer.rs
now counts and refuses reference lexing of any text that carries an armed
marker. `tokenize` is the only constructor of reference tokens, so every
reference parse, type check and compilation of the marked user program passes
through it. Unmarked stage programs, which the pipeline legitimately bootstraps
through the reference, are unaffected. The hook exists only in test builds with
the self-host feature.

Three facts were established locally on 2026-10-24 at the parent commit
35a85ae1 plus the uncommitted increment, in the debug test profile.

1. `cargo test --lib --features self-host reference_poison_tests` passed five
   of five in 2999 seconds. The five are the inventory, a control showing that
   the poison stops the reference and spares unmarked text, a control showing
   that marking leaves every reference artifact unchanged, and the two
   intervention tests named in the table above.
2. With a reference read injected into `self_host_compile_scratch` and another
   into `try_call_type_inputs_from_pipeline`, both intervention tests failed on
   the first stage in 16 seconds. The injections were then removed. This
   establishes that the controls can fail, not that every possible leak is
   detected.
3. `cargo test -p keleusma-cli --test self_hosted_stage_corpus` passed one of
   one in 3505 seconds, running fourteen command processes four at a time.

A full local matrix then ran at commit 18162341, which carries these controls,
between 07:28 and 08:20 UTC on 2026-10-24. It used nextest in the Cargo release
profile with `--no-fail-fast`. This satisfies numbered condition 7 for that tree
in release only. Item 3 of the strengthening list notes that release does not
establish a debug pass.

| Command | Result |
|---|---|
| `cargo nextest run --cargo-profile release --workspace --no-fail-fast` | Exit 0, 3464 passed, 2 skipped |
| `cargo test --release --workspace --doc` | Exit 0, 12 passed, 3 ignored |
| `cargo nextest run --cargo-profile release -p keleusma --features self-host --no-fail-fast` | Exit 0, 3204 passed, 2 skipped |
| `cargo nextest run --cargo-profile release -p keleusma --no-default-features --no-fail-fast` | Exit 0, 331 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures --no-fail-fast` | Exit 0, 2458 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures,shell --no-fail-fast` | Exit 0, 2475 passed |

The workspace and self-host totals exceed the b1dbfe4f receipt by six and five,
which are exactly the tests this increment adds. The detached compiler result is
recorded in REVERSE_PROMPT.md.

These are local results and do not replace remote continuous integration. The
two intervention tests and the command corpus test are among the slower tests
in the suite, though below the slowest recorded test at 2.65 hours.

Remote continuous integration then ran pull request 498 at a90fc449 as workflow
run [40092978507](https://github.com/sgeos/keleusma/actions/runs/40092978507),
created 12:12 UTC and concluded successfully 13:58 UTC on 2026-10-24, with every
job passing. That tree is 18162341 plus the Order 2 acceptance record and a merge
of v0.2.3 at d7461857, which changed only the dev profile to opt-level 1 and the
CI job layout. The test jobs used the Cargo test profile, which inherits that
setting and keeps debug assertions. This closes the debug-pass gap noted above
for that tree, at opt-level 1 rather than 0. The pull request merged into v0.2.3
as 6db0724b.

| Job | Nextest summary |
|---|---|
| Test, the workspace with `selfhost_*` binaries filtered out | 2607 run, 2607 passed |
| Test, self-host feature, shards 1 to 4 | 793, 783, 760 and 870 passed, 3206 in total |
| Test, no default features | 333 passed |
| Test, signatures feature | 2460 passed |
| Test, broad features | 2477 passed |

The job logs were searched for the six tests this increment adds, so that a
green run is not mistaken for one that reached them. All five poison tests
passed in the Test job and again in the self-host shards. The command corpus
test passed in the Test job in 615 seconds. The workspace counts are not
comparable with the b1dbfe4f receipt, because the job layout changed between
the two runs.

## Numbered completion condition

The historical [completion condition](./ORDER_1_COMPLETION_CONDITION.txt) remains intact,
including its two withdrawn items.

| Item | Evidence and disposition |
|---|---|
| 1 Derived bindings | Literal and derived-operator binding comparisons in tests/selfhost_typecheck.rs pass. The production scoped path also types outcome bindings from an intrinsic Word source record. |
| 2 Explicit boundary | The legacy extraction boundary tests and `self_hosted_construct_support_boundary` remain present and pass. Unsupported syntax, unknown layout, capacity exhaustion and reference divergence remain distinct. |
| 3 Host does not supply the answer | The stage-decision intervention changes only Keleusma derivation with identical transported inputs. Its required changing, valid and direct-rejection controls pass. |
| 4 Withdrawn file-operand condition | The condition records that the proposed staged command does not exist. The existing compile command already accepts a file. |
| 5 Withdrawn sidecar condition | The condition records that no sidecar-producing command exists. The separate future design remains in PIPELINE_THEN_MONOLITH.md. |
| 6 No silent narrowing | Public signatures and operator limits remain fixed. Existing supported-source comparisons pass. The legacy module-forest buffer grows to 1,536 nodes, with boundary tests, while raw node output retains its 1,365-node bound. The 90-record batch remains bounded and all stages still pass the separate streaming path. |
| 7 Green verification | The full workspace and standalone self-host suites pass with zero failures. Formatting and deny-warning Clippy pass. Exact receipts follow. |
| 8 Recorded blockers | No implementation decision blocks Order 1. External pilot evaluation remains unavailable and has not altered any development gate. |

## Outcome semantics and resource changes

The parser, reconstruction stage and code generator own outcome lowering. LIMIT, OK and
BREAK dispatch in reference order regardless of source order. OK binds the completed count.
BREAK and LIMIT bind the exit index. An absent BREAK handler falls through to OK. An absent
LIMIT handler or false LIMIT guard traps. A false BREAK guard performs no handler body.

Handler scopes exclude the induction variable and completed body locals. Explicit bindings
may shadow outer names. A probe exposed a reference compiler mismatch with its type checker.
The reference resolved a finished loop's induction variable inside an outcome handler and
returned two where the outer binding required seven. The compiler now removes loop-local
names before dispatch while preserving their allocated slots. Word, Boolean, array and
generated-name collision controls execute correctly in both compilers.

Source-order local identities are mapped to emission-order slots. Handler bias is restored
across nested handlers. Loop side-table offsets are rebased for each function-head range.
Malformed outcome descriptors, exhausted descriptor storage and excessive context depth
have explicit reconstruction controls.

The parser has 1,399 constant nodes. Its growth exceeded the legacy module-forest buffer.
That buffer now stores 1,536 nodes independently of the raw node-output buffer. Its three
scratch arrays occupy 98,304 shared bytes. The three output regions reserve at most 61,440
bytes inside the unchanged 65,536-byte window. Tests accept the exact bound, include a nested
forest at that bound, and reject the next node. Raw node output still rejects beyond 1,365.

The parser has 585 fields against its 640-field limit and 389,535 source bytes against the
393,216-byte lexer source limit. These figures are historical and describe 2026-10-13. Commit
68bc27b1 raised the lexer source limit to 524,288 bytes on 2026-10-15, which is the value of
`LEX_SOURCE_CAP` in src/selfhost_host.rs. The legacy input census measures 983 names and 28,174 bytes
against unchanged limits of 1,024 and 49,152. The code generator has 92 functions and the wire
stage has 562. The old 90-record batch therefore refuses the current code generator, while
the streaming path remains required to accept every stage. No capacity assertion was removed.

## Final verification receipts

All commands below completed. Self-host checks used `KEL_SELFHOST_CACHE=0`. Cargo commands
used an empty `RUSTC_WRAPPER` and `CARGO_HUSKY_DONT_INSTALL_HOOKS=1`. Sources remained fixed
through live verification. Counts sum actual completed test summaries.

| Check | Result | Local receipt |
|---|---|---|
| Release workspace with no-fail-fast | 3,236 passed, zero failed, five ignored across 193 summaries | tmp/order1-outcomes-complete-workspace.log |
| Standalone self-host with no-fail-fast | 2,978 passed, zero failed, five ignored across 167 summaries | tmp/order1-outcomes-complete-selfhost.log |
| No default features | 329 passed, zero failed, two ignored | tmp/order1-outcomes-complete-no-default.log |
| Signatures | 2,350 passed, zero failed, three ignored | tmp/order1-outcomes-complete-signatures.log |
| Signatures and shell | 2,367 passed, zero failed, three ignored | tmp/order1-outcomes-complete-signatures-shell.log |
| Detached compiler | 86 passed, zero failed | tmp/order1-outcomes-complete-detached.log |
| Root and detached Clippy | Deny-warning checks pass with actual Checking output | tmp/order1-outcomes-complete-clippy.log and tmp/order1-outcomes-complete-detached-clippy.log |
| Root and detached formatting | Both checks pass | tmp/order1-outcomes-complete-fmt.log and tmp/order1-outcomes-complete-detached-fmt.log |
| Minimum supported Rust | Rust 1.88 checks core tests with self-host successfully | tmp/order1-outcomes-complete-msrv.log |
| Embedded build | thumbv7em-none-eabihf build passes | tmp/order1-outcomes-complete-embedded.log |
| Inference independence | Four controls pass, 59,999 types erased across fourteen stages | tmp/order1-outcomes-complete-inference.log |
| Actual command corpus | Fourteen complete artifacts match same-source reference compilation | tmp/order1-outcomes-complete-cli-corpus.log and tmp/order1-outcomes-complete-cli-reference.log |
| Actual outcome command | Compilation succeeds and execution prints seven | tmp/order1-outcomes-command.log and tmp/order1-outcomes-command-run.log |
| Documentation guards | 29 tests pass across links, counts, citations, push-order and gate correspondence | tmp/order1-outcomes-handoff-guards.log |

The first broad run found a shared-layout test-format issue. The next exposed the legacy
forest capacity and stale batch classification. Later full runs were stopped before the
reference scope repair. Their logs are retained. The first command comparison also used the
Rust command's native preamble, so it compared different inputs. The corrected comparison
uses the exact same stage source. Only the completed receipts above establish final green.

Memory-interpreter, dependency-advisory, book and unrelated platform checks were not repeated
in this increment. The corresponding earlier receipts remain recorded in TASKLOG.md. No
unsafe arena code, dependency manifest or lockfile, book source or platform code changed.
Remote continuous integration, merging and publication are not claimed or authorized here.

### Remote continuous integration, 2026-10-21

The commits from 89fc6aac to 36ebe64d, which carry this audit's evidence, first ran in
remote continuous integration through pull request 484 into v0.2.3. CI workflow run
39704059671 passed every job at 89aaba69, which is 36ebe64d plus the audit follow-up
commits. The pull request merged as 3e4f4984. The run tested the tree after the audit
follow-ups, not 36ebe64d itself, and the follow-ups changed wire.kel, three chunk-count
pins, the book catalogue and the CI job layout. See docs/process/TASKLOG.md.

### Portable successor receipts, reviewed 2026-10-23

The historical local receipts above remain declaration-time evidence. Their missing
commands or commit stamps have not been reconstructed from filenames. The following
successor evidence describes commit `b1dbfe4f258a3b9f8145c571da188999b3ad7526`,
after the merged audit repairs and handler increments. It does not retroactively
verify an earlier tree or certify subsequent edits.

[Continuous integration run 39989231681](https://github.com/sgeos/keleusma/actions/runs/39989231681)
completed successfully with all 31 jobs. The reviewed
[portable receipt](../process/verification/2026-10-23-b1dbfe4f.json) retains the
full commit, command scripts, job identifiers, summaries, named test passes and
counting rules. These are remote observations, not newly executed local tests.
Test commands below used the Cargo debug test profile, reported as unoptimized
with debug information. `--profile ci` selects nextest runner settings and does
not select a Cargo release build. All rows have zero failures. Exit status zero
is inferred from successful Actions steps under `bash -e`, without
`continue-on-error`. Successful steps do not print a numeric process status.

| Command at b1dbfe4f | Passed | Skipped or ignored | Exit | Job identifier |
|---|---|---|---|---|
| `cargo nextest run --profile ci --workspace --partition hash:1/4` | 842 | 2618 | 0 | [120536366483](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366483) |
| `cargo test --workspace --doc` | 12 | 3 | 0 | [120536366483](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366483) |
| `cargo nextest run --profile ci --workspace --partition hash:2/4` | 861 | 2599 | 0 | [120536366163](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366163) |
| `cargo nextest run --profile ci --workspace --partition hash:3/4` | 825 | 2635 | 0 | [120536366598](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366598) |
| `cargo nextest run --profile ci --workspace --partition hash:4/4` | 930 | 2530 | 0 | [120536366560](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366560) |
| `cargo nextest run --profile ci -p keleusma --features self-host --partition hash:1/4` | 790 | 2411 | 0 | [120536366531](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366531) |
| `cargo test -p keleusma --features self-host --doc` | 0 | 3 | 0 | [120536366531](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366531) |
| `cargo nextest run --profile ci -p keleusma --features self-host --partition hash:2/4` | 782 | 2419 | 0 | [120536366390](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366390) |
| `cargo nextest run --profile ci -p keleusma --features self-host --partition hash:3/4` | 757 | 2444 | 0 | [120536366667](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366667) |
| `cargo nextest run --profile ci -p keleusma --features self-host --partition hash:4/4` | 870 | 2331 | 0 | [120536366427](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366427) |
| `cargo nextest run --profile ci -p keleusma --no-default-features` | 331 | 0 | 0 | [120536366200](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366200) |
| `cargo test -p keleusma --no-default-features --doc` | 0 | 2 | 0 | [120536366200](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366200) |
| `cargo nextest run --profile ci -p keleusma --features signatures` | 2458 | 0 | 0 | [120536365955](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536365955) |
| `cargo test -p keleusma --features signatures --doc` | 0 | 3 | 0 | [120536365955](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536365955) |
| `cargo nextest run --profile ci -p keleusma --features signatures,shell` | 2475 | 0 | 0 | [120536366813](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366813) |
| `cargo test -p keleusma --features signatures,shell --doc` | 0 | 3 | 0 | [120536366813](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366813) |
| `cargo nextest run --profile ci -p keleusma-wire --all-features` | 53 | 0 | 0 | [120536366221](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366221) |
| `cargo test -p keleusma-wire --all-features --doc` | 4 | 0 | 0 | [120536366221](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366221) |
| `cargo nextest run --profile ci -p keleusma-wire --no-default-features` | 19 | 0 | 0 | [120536366221](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366221) |
| `cargo test -p keleusma-wire --no-default-features --doc` | 1 | 0 | 0 | [120536366221](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366221) |
| `cargo nextest run --profile ci -p keleusma-bench` then `cargo test -p keleusma-bench --doc` | 6 | 0 | 0 | [120536366296](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366296) |
| `cd compiler` then `cargo fmt --all -- --check` then `cargo clippy --all-targets -- -D warnings` then `cargo test` | 86 | 0 | 0 | [120536366455](https://github.com/sgeos/keleusma/actions/runs/39989231681/job/120536366455) |

The four workspace partitions execute 3,458 unique tests, and the four isolated
self-host partitions execute 3,199. Each population has two ignored tests. A shard's
skipped count includes tests assigned to other shards and must not be added to
other shards' skipped counts. Workspace documentation tests add twelve passes and
three ignored cases. The other core documentation commands execute zero cases
and ignore two or three examples as shown. A successful empty documentation run
is not counted as behavioral coverage.

The same receipt records exact commands for Clippy, formatting, documentation,
minimum Rust 1.88 checks, the embedded build and example verification. These are
successful build or tool checks, with no invented test count. Documentation uses
`RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links"`. The minimum Rust
job checks `--tests` with default features and does not reproduce the historical
self-host minimum-Rust command. The matching
[Book run 39989231699](https://github.com/sgeos/keleusma/actions/runs/39989231699)
also succeeded at the same commit.

The original all-stage command artifact comparison and private pilot records are
not recreated by a green workflow. The named artifact test exercises the shipping
library entry and serializer. Command tests exercise their documented fixtures.
Historical claims beyond those populations retain their original limits.

## Pilot record

Task `v023-order1-outcomes-b599` contains four samples. The diagnostic is labelled
`compiler_or_test`. The execution and rejection claims are labelled `supports`. The artifact
claim is labelled `unsupported` because its small payload omits the comparison implementation.
All labels are non-independent self-reviews. The three-claim limit was reached before the
later forest and scope repairs. No provider judgment or independent accuracy result is claimed.

The reviewed export is tmp/jev-shadow-outcomes-b599-final.json, with 182 samples across
46 tasks. All four current request hashes and immutable source hashes match. The peer-owned
tracked pilot checkpoint was not overwritten. Usage and elapsed values remain null.

## Next milestone

Order 2 is the full validator in Keleusma. Audit its complete structural, typed and resource
verdict against the reference before choosing its next implementation increment.
