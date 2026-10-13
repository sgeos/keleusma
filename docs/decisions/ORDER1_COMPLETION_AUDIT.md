# Order 1 completion audit

> **Navigation** [Completion plan](./ORDER1_COMPLETION_PLAN.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope and status

Audit date 2026-10-13. The defined goal is the first-pass compiler subset, including
production type rejection and complete artifact emission through Keleusma. The roadmap
explicitly defines a first pass as the subset used by the toolchain's own source.
Full-language support remains Order 6. The current corpus contains fourteen stage sources.

The stage-corpus requirements below are verified. The audit also found an explicit
Workstream A item that remains unimplemented, the loop outcome-arm form. Two valid
reference witnesses still receive Unsupported from the shipping self-hosted entry.
Order 1 is not complete. The final self-host suite passes after adding the
stage-decision intervention control.

Historical notes about broader syntax and incomplete legacy extraction APIs remain useful
boundary records. The production scoped input path is the subject of this audit. Its
independent verdict is checked before reference compilation. Unknown evidence and resource
limits remain distinct from source rejection. The reference comparison remains mandatory.
This audit does not claim full-language support or retirement of the Rust reference.

## Roadmap and completion-plan requirements

| Requirement | Authoritative evidence | Finding |
|---|---|---|
| Complete current stage corpus | `production_compiler_matches_every_complete_stage_artifact` and `complete_artifact_corpus_covers_every_stage_source` in tests/selfhost_artifact_identity.rs | Every current stage is inventoried. Pipeline modules are serialized and decoded through Keleusma and compared against complete reference bytes. Both full-suite runs pass. |
| No reference scaffold borrow | `self_host_compile_scratch` in src/selfhost/mod.rs | The function creates chunks and module fields from parser records, reconstructed bodies, emitted instructions and resource analysis. It never compiles the user program through the reference. |
| Pipeline-owned type inputs | src/selfhost/typecheck.rs and `try_call_type_inputs_from_pipeline` | Production inputs come from Keleusma parser records and reconstructed nodes. Rust interns identities and transports syntax facts. It does not read the reference program's inferred-expression table. |
| Independent rejection | `production_types_reach_corpus_and_rule_form_census` in tests/selfhost_typecheck.rs | Twenty negatives and seven positives, plus sixteen rule/form cells and eight controls, reach the independent path. Negative shipping results must be PipelineRejected. A reference refusal cannot satisfy the test. |
| Stage-owned derived decisions | `derived_binding_verdict_depends_on_the_keleusma_decision` in src/selfhost/typecheck.rs | Identical encoded inputs are run through the normal stage and a copy with only expression-type derivation disabled. A derived invalid argument changes from rejection to acceptance. A valid control remains accepted and a direct invalid argument remains rejected. |
| First-pass monomorphization | tests/selfhost_monomorphize_identity.rs | All fourteen stage sources remain unchanged. The inventory and changing control pass. Generics remain a documented boundary. |
| Independence from recorded inference | src/compiler/inference_independence.rs | Complete artifacts remain equal after the recorded table is cleared. The changing and admission controls pass. This proves the recorded-table intervention for the stage corpus. |
| Complete Keleusma serialization | `wire_module_artifact_via_kel` and the all-stage artifact test | Framing, instructions, operands, auxiliary sections and integrity fields are emitted through the Keleusma path. The comparison supplies no reference section bytes, offsets or lengths. The decoded module is compared again. |
| Shipping command | Fourteen actual command outputs in tmp/order1-cli-corpus.log and tmp/order1-cli-corpus-identity.log, plus backend dispatch and serialization in keleusma-cli/src/main.rs, plus keleusma-cli/tests/self_hosted_refusal.rs | The selected backend calls self_hosted_compile and the Keleusma emitter. Command tests check independent rejection, byte identity, decoded execution and large accepted sources. They pass in the workspace run. Each of the fourteen stage files also compiles through the actual command, and every emitted artifact matches the reference bytes. |
| Loop outcome-arm lowering listed in Workstream A | The roadmap names `for ... limit ... on { ... }` and its index binding. Fresh witnesses are recorded in tmp/order1-outcome-audit.log | Incomplete. Both an empty OK/LIMIT handler and a BREAK binding compile with the reference and remain unsupported in the self-hosted entry. This listed item is not discharged by the stage-corpus comparison. |
| Current verification | Receipt table below | All five feature configurations pass. Required compatibility, memory-safety, dependency and directly affected detached-package checks pass. The final self-host refresh includes the new test-only control. |
| Shadow pilot | Task v023-order1-frames-a7ef | The first diagnostic and three bounded claims were recorded and previewed. Reviews and a resolved frame-repair outcome are retained in the handoff. Provider evaluation remains unavailable under the recorded approval rejection. No independent accuracy claim is made. |

## Numbered completion condition

The historical [completion condition](./ORDER_1_COMPLETION_CONDITION.txt) is retained,
including its two withdrawn items.

| Item | Evidence and disposition |
|---|---|
| 1 Derived bindings | `the_declared_binding_rows_agree_between_the_pipeline_and_the_reference` includes literal-bound Word and Boolean locals. `the_derived_operator_rows_agree_between_the_pipeline_and_the_reference` compares operator relations by content and covers both Word and Byte operands. Both pass. |
| 2 Restated boundary | The extraction boundary tests and `self_hosted_construct_support_boundary` remain present and pass. Unsupported syntax, unknown layout, capacity and reference divergence retain separate meanings. |
| 3 Host does not supply the answer | The new stage-decision intervention uses unchanged production-derived inputs. Disabling the stage's derivation changes the invalid derived-binding verdict while direct rejection still works. |
| 4 Withdrawn file-operand condition | The condition itself records that its proposed staged command does not exist. The existing compile command already takes a file. No unreviewed phase-selection command was introduced. |
| 5 Withdrawn sidecar condition | The condition itself records that no sidecar-producing command exists. The separate future design remains in PIPELINE_THEN_MONOLITH.md. |
| 6 No silent narrowing | Public signatures, bytecode version 2, 66 public opcodes and selected 640-row type capacities remain fixed. Standing supported-source comparisons pass. Whole nested nominal arrays retain admission. Tuple-layout tests now distinguish interned reuse from actual layout and member-storage limits. |
| 7 Green verification | The complete workspace and standalone self-host configurations pass with zero failures. Formatting and deny-warning Clippy pass. The final refresh includes the added intervention. The named outcome-arm item remains open. |
| 8 Recorded blockers | No operator decision blocks the compiler implementation. External pilot evaluation remains unavailable and is recorded explicitly. It has not altered any development gate. |

## Remaining named work

The roadmap places loop outcome-arm lowering under Workstream A's first-pass list.
Its two current witnesses are small, valid programs.

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

Both return an unsupported-syntax diagnostic in the current pipeline. Preserve the
full objective and implement this named lowering and binding requirement before
claiming Order 1 complete. The existing for-limit reference tests define OK, BREAK,
LIMIT, guarded handlers and the default trap behavior.

## Verification receipts

Counts include the test summaries reported by each command. Ignored tests are shown
separately. Logs are local verification artifacts under the ignored tmp directory.

| Check | Result | Receipt |
|---|---|---|
| Release workspace | 3,227 passed, zero failed, five ignored across 192 summaries | tmp/order1-frames-workspace-final.log, session 96468 exit zero |
| Standalone self-host feature | 2,970 passed, zero failed, five ignored across 166 summaries, including the new intervention | tmp/order1-completion-selfhost.log, session 54094 exit zero |
| No default features | 329 passed, zero failed, two ignored | tmp/order1-full-no-default.log |
| Signatures | 2,349 passed, zero failed, three ignored | tmp/order1-full-signatures.log |
| Signatures and shell | 2,366 passed, zero failed, three ignored | tmp/order1-full-broad-features.log |
| Detached compiler | 86 passed, zero failed, formatting and Clippy pass | tmp/order1-detached-compiler-tests.log and companion lint/format logs |
| Minimum Rust versions | Arena checks on 1.85.0 and core default, no-default, signatures and test checks on 1.88.0 pass | tmp/order1-msrv-*.log, ending with core-tests-clean |
| Embedded build | thumbv7em-none-eabihf build passes | tmp/order1-frames-thumb-build.log |
| Miri with Tree Borrows | Arena has 64 passes and one ignored test. Null-text regression passes | tmp/order1-miri-arena-final.log and tmp/order1-miri-null-text-final.log |
| Dependency audit | 1,296 advisories loaded, 102 dependencies scanned, exit zero | tmp/order1-dependency-audit-final.log |
| Books | English and Japanese builds pass. Translation template matches. All 60 complete programs are covered by 85 assertions, with zero failures | tmp/order1-book-*.log |
| Shell scripts | ShellCheck passes | tmp/order1-shellcheck.log |
| Broad-feature documentation | signatures, encryption, shell and self-host documentation passes | tmp/order1-full-feature-doc.log |
| Wire feature variants | All-features and allocator-free checks pass | tmp/order1-wire-all-features.log and tmp/order1-wire-no-default.log |

No merge, publication or remote continuous-integration result is claimed. Release-only
platform jobs outside the directly affected compiler path remain governed by the release
process. This audit does not authorize publication or merging into main.
