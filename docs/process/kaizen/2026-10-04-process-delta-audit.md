# Process delta audit through 2026-09-28

Measured 2026-10-04 by the kaizen line on `docs/kaizen-delta-20260928`.

## Conclusion

The trunk implemented parts of proposals one, two, four, six and seven from the
[original audit](./2026-09-08-process-audit.md#proposals). No proposal is demonstrated
as fully implemented within this review's scope. This is a classification of observed
mechanisms against the proposals, not a claim that the operator accepted those proposals.
The review establishes neither causal influence from the audit nor measured savings.

The bounded current-state sections and the verbatim gate-policy record are material
improvements. The first reduces the text needed to locate current state. The second
preserves the source of one family of operator rulings. Neither implements the full
mechanism originally proposed. Both should be credited before further work is proposed.

## Scope and evidence

The baseline is `802e72d3239b62c1dc90271a8d262932ca2ec68d`, the original audit's
2026-09-08 trunk measurement. The endpoint is
`89fc6aac87b94c4cad53392097386a857c51a8d1`, the last trunk commit dated 2026-09-28
in the locally available history. The audit and proposal wording are read from kaizen
commit `8e06e55fa9465378604b5752f0e80cfe1a0b7583`. This is a comparison against
the September 28 state, not a new baseline dated September 28.

Facts labelled measured come from commands executed over these immutable Git objects.
Facts labelled read come from the source or recorded accounts at those objects.
Adoption classifications and priorities are inferences. An absent mechanism means no
implementation was demonstrated in the inspected scope, not that every private session
or remote setting was searched.

The process-change population is `AGENTS.md`, `CLAUDE.md`, `docs/process/`, `scripts/`,
`.github/` and `.cargo-husky/`. Measured change is 18 files, 11,599 inserted lines and
501 deleted lines, a net increase of 11,098 lines. This reproduces the approximate
eleven-thousand-line statement as net growth. It does not measure semantic change,
reading cost or improvement. Tests and the roadmap were inspected separately as
supporting mechanisms and are outside that line-count population.

[Raw measurements](./evidence/2026-10-04-delta-measurements.txt) retain the population,
commands, source anchors and endpoint identifiers. [Verification evidence](./evidence/2026-10-04-delta-verification.txt)
records the test invocations and outcomes. No remote refresh, workflow-history survey,
private-conversation review or new literature review was performed. Native and proof
mailboxes were searched for kaizen references at the locally available tips, with none
found. Their broader implementation states were not re-audited. The September 29
compiler feature work is outside the comparison.

## Proposal disposition

Every row is an inference from the named evidence. Partial implementation does not
mean adoption of the proposed rule.

| Proposal | Classification | Observed change and remaining gap |
|---|---|---|
| 1. Bound the resume surface | Partial | The handoff and reverse prompt now mark current sections and tell readers where to stop. Startup and compaction instructions defer to the handoff's validity procedure, and native startup routes to its brief. No 200-line enforcement, separate history archive or monthly journal rotation was demonstrated. The task log remains a large startup input. See the measurements below and [communication rules][communication]. |
| 2. Derive every figure | Partial | The count guard adds feature-set, structural-number and other checks. It has 13 test functions at the endpoint against six at baseline. The documents still restate figures, the integration-count guard deliberately permits drift, and no general figure producer with a governed-document inventory was found. See [count guard][counts]. |
| 3. Give each lesson a mechanism or habit label | Not demonstrated | The root instructions expand their catalogues of misleading verification and unreachable subjects. Individual mechanisms also landed. No single incident ledger with required mechanism-or-habit fields and a schema check was found. Narrative catalogues do not establish that every lesson has been classified. See [instructions][instructions]. |
| 4. Require a negative control for every checker | Partial | The new gate-correspondence guard has explicit controls that reject unsupported claims while allowing the narrowly scoped native label. Those controls execute in this review. The pull request template is unchanged and does not request a control. No universal adoption rule was found. See [gate guard][gate-guard] and [template][template]. |
| 5. Filter documentation work and shard the test job | Not demonstrated | The main workflow still runs an unpartitioned default workspace suite. It has no documentation-only job selection or job timeout setting. New advisory, shell and example coverage strengthens verification, but does not implement this latency proposal. The existing pull request cancellation policy predates the baseline. See [workflow][workflow]. |
| 6. Centralize rulings and batch decision packets | Partial | The new gate-policy record carries dated operator quotations and explicitly distinguishes them from the assistant requirement they supersede. It links the policy to affected commits. It covers gate policy only. No shared all-rulings authority or mandatory packet-at-boundary protocol was demonstrated. The record itself says no conversation permalink or exported transcript is attached. See [policy][policy]. |
| 7. Give roadmap criteria executable status | Partial | The roadmap now connects more local claims to tests, including region completeness and the remaining trap-analysis obligation. These are useful narrower instruments. No unified reporter for the five success criteria or newly defined publication-hold lifting criterion was found. Release procedures and the explicit publication approval requirement already existed. See [roadmap][roadmap]. |
| 8. Authorize bounded defaults with intervals | Not demonstrated | The existing instruction to continue obvious work is not a per-decision default with an operator-authorized interval. No such authorization was identified in the reviewed delta. Silence cannot supply it. |
| 9. Trial fresh-context review at merge | Not demonstrated | A new readiness audit reviews setup and handoffs. It is not the proposed thirty-day code-review trial. No trial register, disposition series or merge policy requiring an isolated reviewer was found in the inspected process and workflow surfaces. Private review activity remains unverified. |
| 10. Reduce absorptions or use one trunk | Not demonstrated | Parallel-development changes reconcile channel ownership and retain the two-line arrangement. No new absorption schedule or trunk-consolidation decision was found. This review does not remeasure absorption counts or coordination cost. See [parallel rules][parallel]. |

## Bounded reading improved while total volume grew

These are measured byte and line counts from Git blobs. Lines use Python
`bytes.splitlines`. Current prefixes end immediately before the first exact history
heading named in the raw evidence. A prefix count is not a measurement of what an
agent actually reads.

| Document | Baseline bytes | Endpoint bytes | Baseline lines | Endpoint lines |
|---|---|---|---|---|
| Handoff | 93,081 | 140,195 | 1,439 | 2,178 |
| Reverse prompt | 19,096 | 197,510 | 307 | 3,088 |
| Task log | 434,064 | 534,969 | 2,165 | 3,767 |
| Design journal | 1,056,884 | 1,371,861 | 11,096 | 16,625 |

The endpoint handoff's current prefix is 155 lines and 9,128 bytes. The reverse
prompt's current prefix is 22 lines and 1,201 bytes. Those two prefixes are below
the audit's proposed 200-line limit, but the full documents are not. The communication
policy explicitly distinguishes current-state visibility from bounded file size and
chooses to preserve history in place. Moving history is therefore a remaining operator
decision, not an omitted mechanical cleanup.

The handoff also adds a commit-distance measurement and explains why ancestry alone
does not detect staleness. This improves the original validity procedure. It provides
information to a reader rather than a hard freshness threshold. This review does not
rerun that handoff's historical runtime or advisory gates.

## Mechanisms strengthened, with important limits

Measured execution of the endpoint's count guard passes all 13 tests. Read evidence
shows that the integration-file count tolerates a difference of ten and the integration
test count tolerates twenty percent. A green result therefore establishes the selected
bounded comparisons, not exact agreement of every number in every document.

Measured execution of the gate-correspondence file passes all five tests, including
its three explicit regression controls. The check compares normalized command shapes.
Its own source disclaims equivalence of the work those commands perform. This review
does not promote its pass to a complete coverage proof or a current green build.

Read evidence shows additional mechanisms beyond the ten proposals. The workflow now
includes dependency-advisory scanning, shell analysis, host-example checks and a library
undefined-behaviour check. The book examples are also attached to the version-branch
workflow. These are source-level observations here. Their remote executions and
effect on incident frequency were not measured.

## Policy propagation remains incomplete

The new policy record and root instructions settle the once-per-prompt and gate
boundaries. They should not be presented to the operator as undecided proposals.
However, the pull request template still asks for the full local workspace test run,
and the unchanged pre-push hook commentary describes the local release gate as
mandatory before a version-branch merge. Both differ from the clarified targeted-local
policy. The autonomous-loop document also retains a Claude-specific attribution step
and a stronger coverage claim than the newer guard establishes.

These are read contradictions between current-facing surfaces. The superseding policy
resolves authority, but a reader can still encounter the stale instructions first.
Inference is that a small policy-propagation review offers a better next increment than
rebuilding mechanisms that already exist. No peer-owned file is corrected by this audit.

## Verification and limits

The endpoint was exported with `git archive` to an isolated scratch directory.
Four document-oriented Rust test files were compiled directly with `rustc --test`
and edition 2024, with `CARGO_MANIFEST_DIR` set to that snapshot. The two files gated
on `compile` were compiled with that feature configuration. All 26 intended cases
passed with zero failures or ignored cases, comprising 13 count checks, five gate
checks, two link checks and six citation checks.

The initial invocation omitted the feature configuration and ran zero cases in the
count and link files. Those two empty runs were rejected as verification. Corrected
runs checked their expected case counts explicitly. Both attempts are retained in the
evidence. This is a newly observed instrument error in the audit itself.

Direct compilation executes these standalone document checks but bypasses Cargo,
dependency builds and its feature-resolution machinery. It is not a workspace gate.
The final audit documents passed 14 checks on this feature worktree, comprising two
link checks, six count checks and six citation checks, using the same direct compilation
method. Whitespace, report prose style and the measured disposition totals also passed
their local checks. External
links, all runtime suites, remote continuous integration and adoption effectiveness
remain unverified. No new operational rule or operator authorization follows from
this report.

## Recommended next increment

Reconcile the current-facing copies of the already settled gate and attribution
policies through a proposal to the owning line. Identify each conflicting instruction,
its replacement and the scope of a check before requesting integration. This concerns
propagation of an existing ruling, not reopening it.

Keep the remaining ten-proposal decisions separate from that correction. In particular,
the archive layout, governed figure inventory, lessons schema, universal control rule,
workflow cost changes and cross-line ruling authority still need their recorded rulings.
Neither a deadline nor inferred approval should be introduced by the kaizen line.

[communication]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/docs/process/COMMUNICATION.md
[counts]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/tests/claimed_counts.rs
[instructions]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/CLAUDE.md
[gate-guard]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/tests/gate_ci_correspondence.rs
[template]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/.github/PULL_REQUEST_TEMPLATE.md
[workflow]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/.github/workflows/ci.yml
[policy]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/docs/process/GATE_POLICY.md
[roadmap]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/docs/roadmap/V0_2_X_ROADMAP.md
[parallel]: https://github.com/sgeos/keleusma/blob/89fc6aac87b94c4cad53392097386a857c51a8d1/docs/process/PARALLEL_DEVELOPMENT.md
