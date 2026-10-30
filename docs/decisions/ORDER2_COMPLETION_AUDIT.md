# Order 2 completion audit

> **Navigation** [Completion plan](./ORDER2_COMPLETION_PLAN.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope and disposition

Order 2 is complete for the full first-pass validator scope on 2026-10-13.
The implementation, five feature configurations and documentation are verified. This audit preserves the roadmap's structural, typed, resource,
text-allocation, module orchestration and shipping requirements. Full-language
expansion remains Order 6.

The admission contract combines the checks represented by verify and
verify_resource_bounds. Native attestations and host value-slot width have an
additional public entry. Acceptance retains the previously selected typed
control-flow widening profile and the reference consistency gate.

## Requirement evidence

| Requirement | Current implementation and evidence | Disposition |
|---|---|---|
| Structural verdict | verify_structural.kel, verify_depth.kel and verify_yield.kel check targets, operands, block types, depth and productivity. Forward targets are checked before region walks. The detached 46-test structural corpus now invokes both legacy and fallible whole-module entries. All 46 pass. | Verified |
| Typed verdict | verify_typed.kel consumes complete parameter, native-return and enum facts. Existing typed tests and eight new controls cover projection forms, stride, locals, resume shapes, loop heights, layout reconciliation and transport capacity. The selected widening boundary is tested explicitly. | Verified for the first-pass profile |
| Complete stage corpus | Directory-derived inventories cover all fourteen current stage sources. Every module passes the fallible structural/typed verifier. Full resource admission agrees with the reference below, at and above its arena boundary. All fourteen complete artifacts match the reference and decode. | Verified |
| Call graph | analyze.kel selects traversal, completion order and cycle rejection. Rust services indexed opcode requests. Five controls cover all stage graphs, graph shapes, cycles, capacities and an intervention that changes only the stage's cycle decision. | Verified |
| Module summaries and admission | analyze.kel retains per-chunk stack, heap and text summaries and decides completion and stream arena admission. Independent interventions disable memoized heap use or admission and change the verdict for identical subject input. Missing summaries are errors. | Verified |
| Yield fixpoint and productivity | verify_yield.kel schedules rounds, retains membership, decides convergence and emits the productivity rejection flag. Separate interventions change membership and productivity decisions. | Verified |
| Text allocation | analyze.kel owns NotText, Known and Unbounded transitions, widening, saturating heap totals and boundary flags. Host storage transports abstract values. Resource summaries include text heap and transitive return flags. Differential controls cover calls, locals, branches, loops, natives and early saturation. | Verified |
| Resource arithmetic and host costs | Heap addition and multiplication saturate at the reference unsigned 32-bit ceiling. Loop-bound extraction checks arithmetic overflow and follows literal-array length aliases. Keleusma charges native costs per site or per unique external native according to attestations. Shared copy-out is supplied as a primitive opcode cost fact. Twelve resource tests include all-stage boundaries and subject widths. | Verified |
| Fallible admission | admit_module_via_kel combines structural, typed and resource results. The attested variant accepts native bounds and host value-slot width. Capacity, unfinished work, execution and protocol failures remain errors. Failure injection proves that unavailable analysis cannot become a completed verdict. | Verified |
| Shipping integration | keleusma validate selects rust or self-hosted. The Keleusma result is computed before the reference comparison. Disagreement and unavailable analysis return two, rejection one and acceptance zero. Four actual command tests include both validators on all fourteen stage boundaries and a concrete disagreement witness. | Verified |
| Compatibility and operator limits | Five feature configurations, minimum Rust and embedded compilation pass. Public bytecode version remains 2, instruction count 66, selected type capacities 640 and the depth limit 32. The analyzer uses a 64-bit host Word. Subject widths include 8, 16, 32 and 64 bits. | Verified |
| Recorded completion and documentation | The requirement register, roadmap, process records and public command documentation carry the implemented scope and limits. Citation, roadmap and public-link controls pass. | Verified |

## Named regression evidence, reconciled 2026-10-23

The following tests already existed at b1dbfe4f. The portable receipt records
selected individual PASS lines as well as the enclosing matrix. Reading an
implementation function alone is not an execution receipt. A file-level command
below runs its full test population, without relying on a remembered count.

| Acceptance row | Named test and source | Reproduction command |
|---|---|---|
| Structural verdict | `malformed_targets_are_rejected_before_region_walks` and `depth_and_productivity_failures_are_completed_rejections` in tests/selfhost_admission.rs | `cargo test --features self-host --test selfhost_admission` |
| Typed verdict | `scalar_and_nested_array_strides_reach_the_typed_stage`, `declared_local_assignment_checks_shape`, `yield_resume_shape_controls_field_admission`, `every_flat_projection_family_checks_the_extent`, `loop_back_edge_height_must_be_neutral`, `layout_reconciliation_checks_prefix_count_and_private_order`, `packed_call_shape_pool_accepts_its_exact_capacity` and `loop_termination_preserves_the_reference_post_loop_checks` in tests/selfhost_rejection_families.rs | `cargo test --features self-host --test selfhost_rejection_families` |
| Complete stage corpus | `every_current_stage_completes_the_fallible_verifier` in tests/selfhost_admission.rs and the per-stage tests in module `every_stage_matches_complete_resource_admission` in tests/selfhost_resource_admission.rs, one function until 2026-10-30, with the inventory check `resource_admission_covers_every_stage_source` | `cargo test --features self-host --test selfhost_admission --test selfhost_resource_admission` |
| Call graph | `stage_order_and_cycle_verdict_match_the_reference`, `current_stage_graphs_preserve_reference_order` and `cycle_verdict_depends_on_the_stage_with_identical_inputs` in src/selfhost/call_graph.rs | `cargo test --lib --features self-host selfhost::call_graph::tests` |
| Module summaries and admission | `admission_and_summary_use_depend_on_stage_decisions` in src/selfhost/resources.rs | `cargo test --lib --features self-host selfhost::resources::tests` |
| Yield fixpoint and productivity | `membership_and_convergence_are_stage_owned` in src/selfhost/yield_fixpoint.rs and `depth_and_productivity_failures_are_completed_rejections` in tests/selfhost_admission.rs | `cargo test --lib --features self-host selfhost::yield_fixpoint::tests` and `cargo test --features self-host --test selfhost_admission` |
| Text allocation | `text_lattice_matches_the_reference_over_control_flow_and_calls` and `text_heap_saturates_and_stops_at_the_same_instruction` in tests/selfhost_resource_admission.rs | `cargo test --features self-host --test selfhost_resource_admission` |
| Resource arithmetic and host costs | `heap_arithmetic_saturates_like_the_reference`, `iteration_counts_obey_the_reference_u32_boundary` and `native_attestations_preserve_per_site_and_per_chunk_charging` in tests/selfhost_resource_admission.rs | `cargo test --features self-host --test selfhost_resource_admission` |
| Fallible admission | `malformed_stage_responses_are_protocol_errors` and `failed_stage_execution_is_not_a_rejection` in src/selfhost/validation.rs | `cargo test --lib --features self-host selfhost::validation::tests` |
| Shipping integration | `command_distinguishes_valid_invalid_and_unavailable`, `command_checks_resource_capacity_without_running_the_stream`, `reference_disagreement_prevents_successful_admission` and `shipping_validator_reaches_every_current_stage` in keleusma-cli/tests/validator.rs | `cargo test -p keleusma-cli --test validator` |
| Compatibility and operator limits | `host_resource_stage_preserves_declared_subject_target_widths` in tests/selfhost_resource_admission.rs and `nesting_capacity_preserves_the_declared_boundary` in tests/selfhost_admission.rs, plus the five configurations and platform checks in the portable receipt | Commands above and the receipt's exact matrix commands |
| Documentation | `the_instructions_do_not_misstate_the_integration_test_counts` in tests/claimed_counts.rs, plus the documentation-link, citation and gate-correspondence binaries | `cargo test --test claimed_counts --test documentation_links --test comment_citations --test gate_ci_correspondence` |

## Explicit scope discrepancy

Recorded 2026-10-23 for strengthening item 8. The roadmap's whole-verdict gate
and this audit's selected widening profile are different contracts. The latter
establishes agreement on the fourteen-stage corpus and the named rejection
families. It does not establish equality with the reference for arbitrary
well-formed bytecode. At control-flow joins the Keleusma typed stage may forget
a scalar shape that the Rust verifier retains. The retained reference gate is
therefore part of shipping admission, not an optional comparison.

The earlier roadmap describes the widening trade-off as operator-selected.
That record is evidence of the implementation choice. It does not, by itself,
establish operator acceptance of narrowing the later completion gate. No new
acceptance is claimed here. Strengthening item 8 remained open for that decision,
while independent receipt work could proceed. The operator gave that acceptance on
2026-10-24, recorded below. Removing the reference gate or
promoting the first-pass result to whole-verdict equivalence remains unsupported.

The existing `reference_disagreement_prevents_successful_admission` test pins the
concrete cross-join witness. It asserts reference rejection, Keleusma acceptance,
self-hosted command exit 2 with a disagreement diagnostic, and Rust command
exit 1. Removing the shipping comparison would fail its exit assertion. Job
120536366163 executed that test successfully at b1dbfe4f. This closes the requested
pin in item 9 without claiming that the underlying precision gap has been repaired.

### Operator acceptance of the first-pass scope, 2026-10-24

The operator accepted the first-pass Order 2 scope on 2026-10-24. The ruling was
given first-hand in a Claude Code session in the V0.2.X worktree, as the answer to
a direct question, and was not relayed by a peer line. The accepted scope is
whole-verdict agreement with the reference on the fourteen-stage corpus under
the cross-join widening of the typed stage, with the shipping reference gate
retained. Equivalence for arbitrary well-formed bytecode is deferred to Order 6
and to success criterion 2 of the roadmap. Strengthening item 8 is closed by
this record.

Two facts were checked before the question was put. Both come from Git history.

1. The gate cell added in 18818f27 on 2026-07-15 reads "The self-hosted
   validator reproduces the whole `verify()` verdict (structural, typed,
   resource) for the stage corpus, diff-tested against the reference." Its
   scope is the stage corpus, not arbitrary bytecode.
2. The widening was recorded as "the operator's chosen trade-off" in 0136c8e5
   on 2026-07-16, when typed slice 2a landed. That record was written by an
   agent and is not a first-party ruling. The strengthening list's statement
   that the profile first appeared on the declaration day is therefore
   incorrect for the trade-off itself. Only the phrase "selected widening
   profile" first appeared then.

The reading that the gate text as written is met is an inference from those two
facts and from the stage-corpus tests named above. The acceptance does not
repair the precision gap pinned by `reference_disagreement_prevents_successful_admission`,
and it does not permit removing the reference gate.

## Host and trust boundary

The host decodes bytecode and transports opcode classes, operands, primitive costs,
widths and declared metadata. It allocates stage machines and buffers, services
indexed requests, selects regions from validated markers, sequences passes and
reports rejection or unavailable analysis. Keleusma performs the control-flow
walks, graph ordering, yield fixpoint updates, type and layout checks, text
transitions, resource summary storage and arena admission.

Reference verification of the subject remains in the shipping consistency gate
and differential tests. Bootstrapping the trusted analyzer stages still uses the
Rust compiler and safe virtual-machine constructor. The resource admission audit
covers the current reference memory model and finite-loop decisions. It does not
extend the separate timing-estimate interfaces or claim a trap-freedom verdict.

The typed stage widens shapes across control-flow boundaries under the selected
first-pass profile. A concrete cross-join scalar witness is accepted by the stage
and rejected by the more precise reference. The shipping command reports this
as disagreement and cannot report successful admission. Runtime guards remain
required. Independent review remains a prerequisite for reference retirement.

The bounded profile admitted 1024 chunks, 1536 opcodes per chunk, 256 local and
operand slots, 256 enum layouts and 12288 transported call shapes per chunk.
Those are the figures of 2026-10-13. Commit 7a058917 raised the opcode limit per chunk to
2048 on 2026-10-15, which is the value of `VERIFY_OP_CAP` in src/selfhost_host.rs.
Exhaustion is unavailable analysis. The existing 32-level depth limit is preserved.
The call-shape control accepts exactly 12288 entries and reports capacity at 12416.
Dead calls isolate that transport boundary from stack and opcode capacities.

The command executes no subject instructions and does not authenticate signatures.
Native attestations are empty in the offline command. Hosts with attested costs use
the library interface. The runner's security and deployment policies remain in force.

## Defects demonstrated and repaired

Earlier Order 2 controls repaired missed ninth call arguments, float constants
misclassified as unknown, enum-table truncation and module metadata lost when a
caller signature was absent. The fallible boundary added explicit unfinished-work
and storage outcomes and enforced the loop operand floor.

The resource increment repaired branch-prefix widening, analysis after terminated
branches and incompatible break heights. A later audit caught an error in the
pending loop change. When a terminated loop body has no break state, the reference
restores the entry state and checks subsequent instructions. The stage skipped
those checks and the complete admission entry accepted a malformed scalar
projection. The identical witness now rejects, and its valid control accepts.
The permanent control is in tests/selfhost_rejection_families.rs.

The extreme loop-count control also exposed unchecked subtraction in the Rust
reference. Checked subtraction prevents a debug overflow panic. The corresponding
debug test passes separately from the release suite.

## Verification receipts

The validator source tree remained fixed during the corrected gate. Each step
recorded an empty changed-subject list against
tmp/order2-corrected-subject-hashes.json. Full commands, exit codes and durations
are in tmp/order2-corrected-gate-results.json. Tests used release mode, an empty
RUSTC_WRAPPER and KEL_SELFHOST_CACHE set to zero. The full nextest ci profile was
used, with separate documentation tests. Unqualified log names in the table use
the prefix tmp/order2-corrected-.

| Check | Result and receipt |
|---|---|
| Workspace | 3281 tests pass, two skipped. Twelve documentation tests pass, three ignored. tmp/order2-corrected-workspace.log and workspace-doctests.log. |
| No default features | 329 tests pass. Documentation tests pass. tmp/order2-corrected-no-default.log. |
| Signatures | 2350 tests pass. Documentation tests pass. tmp/order2-corrected-signatures.log. |
| Signatures and shell | 2367 tests pass. Documentation tests pass. tmp/order2-corrected-broad.log. |
| Isolated self-host | 3031 tests pass, two skipped. Documentation tests pass. tmp/order2-corrected-self-host.log. |
| Wire configurations | 57 tests pass with all features and 20 without defaults. tmp/order2-corrected-wire-all.log and wire-no-default.log. |
| Detached compiler | 86 tests pass, including the extended 46-test structural corpus. Clippy and formatting pass. tmp/order2-corrected-detached.log and detached-clippy.log. |
| Workspace Clippy and formatting | Fresh all-target Clippy reports actual Checking and passes. It is repeated after the link-control addition in tmp/order2-handoff-clippy.log. Formatting passes in tmp/order2-handoff-fmt.log. |
| Documentation builds | Workspace documentation with signatures, encryption, shell and self-host passes with warnings denied. Wire and arena all-feature documentation also pass. tmp/order2-corrected-docs.log, wire-docs.log and arena-docs.log. |
| Minimum Rust and embedded target | Rust 1.88 checks tests with self-host. The thumbv7em-none-eabihf build passes. tmp/order2-corrected-msrv.log and no-std.log. |
| Book | 85 assertions pass with zero failures and floor violations. tmp/order2-corrected-book.log. |
| Debug arithmetic boundary | One extreme iteration-bound test passes in debug mode. tmp/order2-final-debug-iteration.log. |
| Shell scripts | ShellCheck exits zero. tmp/order2-final-shellcheck.log. |
| Link-gate correction | The initial link check reached generated copies under the ignored secret directory. The checker now prunes that local store. A new command control proves public broken links still fail and restored public links pass. tmp/order2-doc-link-controls.log and public-doc-links.log. Six citation checks, the roadmap consistency check and the new public-link control pass. Final link and formatting receipts are in tmp/order2-handoff-doc-links.log and tmp/order2-handoff-fmt.log. |

The first workspace run completed 3284 passes, one stale residency-pin failure and
five ignored tests. It is not a green receipt. Its remaining sequence was stopped
before the loop repair. Measured pins now use analyzer total 3215 and peak 367,
and typed-validator total 1438 and peak 395, preserving the existing threefold
fusion requirement. The corrected full workspace run executes those pins successfully.

The link-gate repair and its one new command test followed the full feature runs.
The validator implementation was unchanged. That new test and the final documentation
checks have separate receipts, rather than being added to the earlier test counts.
The source inventory is 1682 integration test attributes across 169 root files.
Workspace feature unification executes 1407 library tests, and isolated self-host
executes 1357. These counts describe this increment.

Memory-interpreter, dependency-advisory and unrelated platform checks were not repeated.
No unsafe arena code, dependency manifest, lockfile or platform implementation changed.
Remote continuous integration, merging and publication are not claimed.

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

Task v023-order2-resource-a12c contains its first diagnostic and three bounded claims.
The diagnostic is labelled compiler_or_test. Protocol failure and post-loop repair
claims are labelled supports. The resource-boundary claim is labelled unsupported
because its short excerpt omits proof that the limit is positive. The actual command
corpus explicitly asserts positive capacity and passes for all fourteen stages.
All labels are non-independent self-reviews. External evaluation remains unavailable
under the earlier automatic approval rejection. No provider judgment, independent
accuracy or avoided-reasoning claim is made. The increment outcome is resolved.
The reviewed export tmp/jev-shadow-order2-resource-a12c-final.json contains 193 samples
and 49 task records. All four current request digests and source hashes match.
The peer-owned tracked pilot checkpoint was not overwritten. Usage remains null.

## Next milestone

Order 3 adds trap analysis and a host mode gated on the agreed trap-free verdict.
