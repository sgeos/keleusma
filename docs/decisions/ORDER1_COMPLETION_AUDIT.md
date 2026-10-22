# Order 1 completion audit

> **Navigation** [Completion plan](./ORDER1_COMPLETION_PLAN.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

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
393,216-byte lexer source limit. The legacy input census measures 983 names and 28,174 bytes
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
