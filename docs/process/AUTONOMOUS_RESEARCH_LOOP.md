# Autonomous Research Loop Process

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

Distilled from one completed loop, twenty firings on 2026-05-21, covering V0.3.0, V0.4.0, V0.5.0, and three cross-cutting threads. Revised against the post-loop audit on 2026-05-22. Current version is 0.2.

## Purpose

A self-firing research loop authorises an artificial intelligence agent to advance design work autonomously while the operator is away from the keyboard. The loop is appropriate when the operator has a substantial backlog of resolvable open questions, has stated bounds within which autonomous work is acceptable, and prefers to return to a body of draft material rather than to an idle session.

The loop is not appropriate when the work requires interactive operator judgment, when the questions are insufficiently specified, or when an action could affect systems outside the project's working directories. Default to interactive sessions unless the criteria below are met.

The relationship to the standard interactive workflow described in [PROCESS_STRATEGY.md](./PROCESS_STRATEGY.md) is one of substitution. During an autonomous loop the per-task review checkpoint specified there is suspended for the duration. Operator review resumes when the loop stops.

## Preconditions

Before initiating a loop the operator should confirm the following.

1. A written backlog of distinct, resolvable design questions exists. Three to twenty items is the workable range.
2. Invariants that all output must preserve are stated explicitly. Examples from the inaugural Keleusma loop include "Worst-Case Execution Time and Worst-Case Memory Usage analysis must not break" and "the recursion-to-iteration-to-loop universal expressibility property must hold."
3. Output discipline is specified. Where work goes, which directories are public versus internal, whether commits are permitted, whether hardware interaction is permitted, and what review process applies on the operator return.
4. Stopping conditions are stated. Productive work exhausted, context budget straining, operator returns, plus any operator-specific conditions.
5. Pacing mechanism is chosen. The Keleusma loop used dynamic `ScheduleWakeup` with thirty-minute intervals between firings.

## Invariants the loop must respect

These are general defaults. Project-specific invariants override when they conflict.

- Output only to the directories the operator authorised. No edits outside that scope.
- No commits during the autonomous phase unless explicitly authorised. All work remains local and uncommitted, awaiting operator review.
- No hardware interaction with shared or remote systems unless explicitly authorised. Local development environment changes are acceptable when reversible.
- No actions visible to third parties. No pull request creation, no remote pushes, no external service interaction beyond what was authorised for the research itself.
- Reversible local actions are admissible. File creation in authorised directories, local builds, local tests, local web fetches, and read operations are all admissible.

## Output placement

All loop output goes into untracked directories at the project root. Two roots are in use, distinguished by sensitivity.

- `tmp/research/` for general, public-facing research. One subdirectory per topic when the topic generates more than a single document or carries a test project. One file per design question, named after the backlog identifier.
- `secret/research/` for material that touches internal use cases. This includes any framing, scenario, or worked example whose subject matter the operator does not want indexed, accidentally committed, or visible to third parties. Internal-use-case implications and scenario exploration belong here. Defence-adjacent framings, customer-specific scenarios, and any topic the operator flags as internal are the typical inhabitants. The two trees are otherwise structurally identical.

Each root carries a `STATUS.md` recording the backlog, the firing log, and the stopping condition status. A synthesis document at the end summarises the body of work for operator review.

The default is `tmp/`. Move material to `secret/` only when the topic itself is internal, not when the document happens to mention a sensitive technique in passing.

Each document should carry explicit uncertainty markers. The categories fact, inference, and hypothesis are the minimum. The verification activities listed in the firing protocol below feed the confidence labelling on each recommendation.

## Firing protocol

Each firing follows a consistent shape. The numbered steps are mandatory. The verification activities under step three are selected from a menu rather than performed all at once on every firing.

1. Re-read `STATUS.md` to determine current state.
2. Select the next backlog item by priority within the operator-stated ordering.
3. Perform the research work for that item. The work consists of a verification phase followed by a drafting phase. The verification phase draws from three activities, chosen based on the question.
   - **Web search.** Search for prior art, recent literature, authoritative documentation, and reference implementations of the technique under study. This is the default first step when the question depends on facts established outside the project. Use `WebFetch` to read the most authoritative source directly when a search result identifies one.
   - **Local-documentation reconciliation.** Read the relevant project documentation under `docs/architecture/`, `docs/spec/`, `docs/decisions/`, and `docs/guide/`. Identify any place the question intersects an existing decision, invariant, or design statement. A recommendation that contradicts the project's own published positions must surface the contradiction explicitly rather than glossing it.
   - **Test project.** When the question hinges on whether a technique compiles, runs, links, or interoperates, create a small Rust cargo project under `tmp/research/<id>_spike/`, implement the minimum that exercises the question, and run it. For internal-use-case items the equivalent path is `secret/research/<id>_spike/`. A successful build, a captured runtime output, and a reproducible recipe in the design document are the deliverable. The bundled `llvm_retcon_spike` directory under `tmp/research/` is a worked example of this pattern.
4. Write the resulting design document with explicit confidence labelling. High confidence when verified directly against an authoritative source or a working test project. Medium confidence when verified against a secondary or older source. Low confidence when verification was not possible within the firing budget.
5. Update `STATUS.md` with the firing outcome.
6. Evaluate whether stopping conditions have been met.
7. If not, schedule the next firing via the chosen pacing mechanism.

## Verification budget

The inaugural Keleusma loop produced eighteen design documents and zero empirical experiments or web searches during the autonomous phase. Post-loop web research surfaced one material correction and three lesser corrections. The single largest lesson is that verification budget per firing is the highest-leverage change.

Recommended budget per firing, measured against the loop's pacing interval rather than against an LLM-side compute estimate. A firing scheduled at thirty-minute intervals should produce roughly one document worth of work in that interval, partitioned as below.

- Approximately one third of the firing on verification. Web search, local-documentation reconciliation, and any test-project work all belong here.
- Approximately half on drafting the design recommendation.
- The remainder on cross-referencing prior loop documents, updating `STATUS.md`, and scheduling the next firing.

The verification step should produce explicit confidence labelling on the recommendation, per step four of the firing protocol above.

## Document length discipline

Aim for two hundred to four hundred lines per design document. The inaugural loop produced documents ranging up to fifteen hundred lines. The operator must read every document on return, so additional length carries cost.

A workable shape for each document.

- One paragraph stating the question.
- Two to four paragraphs reviewing alternatives, including findings from web search and local-documentation reconciliation.
- One paragraph stating the recommendation with confidence label.
- Worked example or test-project pointer where applicable.
- Open questions and deferred work explicit at the end.

If the analysis is genuinely complex the document can grow, but length should be deliberate rather than accidental.

## Cross-document consistency

After approximately one third of the backlog is resolved, a dedicated firing reviews the body of work for inconsistencies. Specific risks.

- Constants stated in one document conflicting with constants implied by another.
- Recommendations that compose poorly when read together.
- Documents that supersede earlier documents without explicit notation.

The consistency firing produces either a clean bill of health or a list of revision items. Either outcome is valuable. The output goes to `CONSISTENCY_AUDIT.md` in the relevant research root.

## Stopping conditions

The loop must stop when any of the following hold.

- The backlog is exhausted and all items have explicit recommendations.
- The operator has returned and the conversation context is interactive again.
- Context budget is straining such that continued firings would degrade quality.
- Operator-stated conditions specific to this loop are met.

When stopping, write a final note in `STATUS.md` explaining the reason. Do not silently terminate.

A common failure mode is continuing past the productive horizon. The inaugural loop's last few firings showed diminishing returns. Self-assessment of marginal value per firing should be honest. If the next firing would feel like filler, stop instead.

## Stopping is not the same as scoping out

If the loop discovers a load-bearing issue that requires operator judgment, escape early. Record the issue in `STATUS.md` and stop the loop. Do not attempt to resolve operator-level questions autonomously. Examples of operator-level questions.

- A recommendation requires choosing between two equally defensible options where the choice has long-term cost implications.
- A finding contradicts an existing strategy document and the resolution requires architectural judgment.
- A required prerequisite is missing from the operator-stated authorisation.

Surface and stop. The operator returns to a brief and a question, not to a body of work built on questionable premises.

## Operator return protocol

When the operator returns, the loop presents the work in a defined order.

1. The current state summary from `STATUS.md`.
2. The synthesis document if one was produced.
3. A list of material findings that warrant operator review before integration.
4. A list of open items deferred during the loop.

Do not summarise individual documents on return. The operator can read them. Direct the operator's attention to where their judgment is most needed.

## Anti-patterns

The following were observed or risked during the inaugural loop. Avoid in future loops.

- Generating documents without verification. The inaugural loop spent its budget on prose and not on prototypes or web searches. Every firing now budgets verification time explicitly, per the firing protocol above.
- Skipping web search when the question depends on external facts. The loop treats web search as a mandatory first step for such questions, not as an optional enrichment.
- Skipping local-documentation reconciliation. Recommendations that contradict the project's own published positions waste operator attention. Read what the project already says before recommending what it should say.
- Scope expansion. After exhausting the stated backlog the inaugural loop continued into adjacent topics. Some additions were valuable, others bordered on inventing work. Stop earlier when the stated scope is done.
- Confidence flattening. Treating high-stakes recommendations with the same epistemic weight as cosmetic recommendations is a disservice. Confidence labels per document are mandatory.
- Length over precision. Long documents impose review cost. Tighter documents respect operator attention.
- Silent termination. Always record why the loop stopped.
- Mixing tracked and untracked output. All loop output goes to `tmp/` or `secret/`. No edits to `docs/`, `src/`, or any other tracked directory during the autonomous phase.
- Placing internal-use-case material in `tmp/`. The `secret/` tree exists for that material; the placement decision is the operator's standing policy, not a per-firing judgement.

## Tooling notes

The following tools, from the Claude Code agent harness, are the standard set for a firing.

- `ScheduleWakeup` with the autonomous loop dynamic sentinel for pacing between firings.
- `TaskCreate`, `TaskUpdate`, `TaskList` for in-firing task tracking when the work decomposes naturally.
- `Write` and `Edit` for document production.
- `WebSearch` and `WebFetch` for the web-search verification step. These are mandatory tools for the loop, not optional. Every firing whose question depends on facts established outside the project must use them.
- `Read` and `Grep` for the local-documentation reconciliation step.
- `Bash` for creating and running test projects, restricted to actions within `tmp/` and `secret/` and to read-only operations elsewhere.

## Cross-references

- [PROCESS_STRATEGY.md](./PROCESS_STRATEGY.md) describes the standard interactive workflow, which the autonomous loop substitutes for during its run.
- [COMMUNICATION.md](./COMMUNICATION.md) describes the human-AI communication protocol that resumes when the loop stops.
- [TASKLOG.md](./TASKLOG.md) records material revisions to this document.
- [docs/guide/LLM_USAGE.md](../guide/LLM_USAGE.md) covers operator-facing guidance for AI sessions in this repository, including the read-AGENTS-first session protocol.

## Versioning

Material revisions bump the version and note the change in `docs/process/TASKLOG.md`.

| Version | Date | Change |
|---------|------|--------|
| 0.1 | 2026-05-21 | Initial draft distilled from the inaugural Keleusma research loop. |
| 0.2 | 2026-05-22 | Web search, local-documentation reconciliation, and test-project creation elevated to the standard verification menu in the firing protocol. `secret/research/` output destination added for internal-use-case material. Navigation header, cross-references section, and `Worst-Case Execution Time` and `Worst-Case Memory Usage` acronym expansions added. Specific R-id citations from the inaugural loop removed because the audit found they mischaracterised the underlying research items. Verification budget and pacing interval reconciled. |
