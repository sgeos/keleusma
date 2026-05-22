# Autonomous research loop process

Status. Draft. Distilled from one completed loop (20 firings, 2026-05-21) covering V0.3.0, V0.4.0, V0.5.0, and three cross-cutting threads. Refinements welcome.

## Purpose

A self firing research loop authorises an artificial intelligence agent to advance design work autonomously while the operator is away from the keyboard. The loop is appropriate when the operator has a substantial backlog of resolvable open questions, has stated bounds within which autonomous work is acceptable, and prefers to return to a body of draft material rather than to an idle session.

The loop is not appropriate when the work requires interactive operator judgment, when the questions are insufficiently specified, or when blast radius would be high. Default to interactive sessions unless the criteria below are met.

## Preconditions

Before initiating a loop the operator should confirm the following.

1. A written backlog of distinct, resolvable design questions exists. Three to twenty items is the workable range.
2. Invariants that all output must preserve are stated explicitly. Examples from the inaugural Keleusma loop include WCET and WCMU analysis must not break, and the recursion to iteration to loop universal expressibility property must hold.
3. Output discipline is specified. Where work goes, which directories are public versus private, whether commits are permitted, whether hardware interaction is permitted, and what review process applies on the operator return.
4. Stopping conditions are stated. Productive work exhausted, context budget straining, operator returns, plus any operator specific conditions.
5. Pacing mechanism is chosen. The Keleusma loop used dynamic ScheduleWakeup with thirty minute intervals.

## Invariants the loop must respect

These are general defaults. Project specific invariants override when they conflict.

- Output only to the directories the operator authorised. No edits outside that scope.
- No commits during the autonomous phase unless explicitly authorised. All work remains local and uncommitted, awaiting operator review.
- No hardware interaction with shared or remote systems unless explicitly authorised. Local development environment changes are acceptable when reversible.
- No actions visible to third parties. No pull request creation, no remote pushes, no external service interaction beyond what was authorised for the research itself.
- Reversible local actions are admissible. File creation in authorised directories, local builds, local tests, local web fetches, and read operations are all admissible.

## Output structure

The Keleusma loop established a workable structure that the operator may adopt or adapt.

- `tmp/research/` for general public output.
- An access-restricted location for sensitive framings the operator does not want indexed or accidentally pushed.
- One document per design question, named after the backlog identifier.
- A `STATUS.md` at the research root recording the backlog, the firing log, and the stopping condition status.
- A synthesis document at the end summarising the body of work for operator review.

Each document should carry explicit uncertainty markers. The categories fact, inference, and hypothesis are the minimum.

## Firing protocol

Each firing should follow a consistent shape.

1. Re-read `STATUS.md` to determine current state.
2. Select the next backlog item by priority within the operator stated ordering.
3. Perform the research work for that item. Allocate time within the firing for empirical verification, not only for prose drafting. This was a notable gap in the inaugural loop.
4. Write the resulting document.
5. Update `STATUS.md` with the firing outcome.
6. Evaluate whether stopping conditions have been met.
7. If not, schedule the next firing via the chosen pacing mechanism.

## Empirical verification budget

The inaugural Keleusma loop produced eighteen design documents and zero empirical experiments. Post hoc web research surfaced one material correction (R4.1 LLVM coroutine intrinsic family) and three lesser corrections. The most significant lesson from the inaugural loop is that empirical verification budget per firing is the highest leverage change.

Recommended allocation per firing of approximately one hour wall clock.

- Twenty minutes for empirical verification. Web search, source tree audit, a small prototype, or a reproducible measurement.
- Thirty minutes for drafting the design recommendation.
- Ten minutes for cross referencing prior documents, updating `STATUS.md`, and scheduling the next firing.

The empirical verification step should produce explicit confidence labelling on the recommendation. High confidence when verified directly against authoritative source. Medium confidence when verified against secondary or older source. Low confidence when verification was not possible within the firing budget.

## Document length discipline

Aim for two hundred to four hundred lines per document. The inaugural loop produced documents ranging up to fifteen hundred lines. The operator must read every document on return, so additional length carries cost.

A workable shape for each document.

- One paragraph stating the question.
- Two to four paragraphs reviewing alternatives.
- One paragraph stating the recommendation with confidence label.
- Worked example where applicable.
- Open questions and deferred work explicit at the end.

If the analysis is genuinely complex the document can grow, but length should be deliberate rather than accidental.

## Cross document consistency

The inaugural loop did not perform consistency checks between documents. After approximately one third of the backlog is resolved, a dedicated firing should review the body of work for inconsistencies. Specific risks.

- Constants stated in one document conflicting with constants implied by another. The inaugural loop's R3.4 per function inference bounds were not cross checked against R4.2 mangling format budgets.
- Recommendations that compose poorly. The inaugural loop did not verify that R5.3 separate compilation and R5.4 mutex analysis are mutually coherent.
- Documents that supersede earlier documents without explicit notation.

The consistency firing should produce either a clean bill of health or a list of revision items. Either outcome is valuable.

## Stopping conditions

The loop must stop when any of the following hold.

- The backlog is exhausted and all items have explicit recommendations.
- The operator has returned and the conversation context is interactive again.
- Context budget is straining such that continued firings would degrade quality.
- Operator stated conditions specific to this loop are met.

When stopping, write a final note in `STATUS.md` explaining the reason. Do not silently terminate.

A common failure mode is continuing past the productive horizon. The inaugural loop's last few firings showed diminishing returns. Self assessment of marginal value per firing should be honest. If the next firing would feel like filler, stop instead.

## Stopping is not the same as scoping out

If the loop discovers a load bearing issue that requires operator judgment, escape early. Record the issue in `STATUS.md` and stop the loop. Do not attempt to resolve operator level questions autonomously. Examples of operator level questions.

- A recommendation requires choosing between two equally defensible options where the choice has long term cost implications.
- A finding contradicts an existing strategy document and the resolution requires architectural judgment.
- A required prerequisite is missing from the operator stated authorisation.

Surface and stop. The operator returns to a brief and a question, not to a body of work built on questionable premises.

## Operator return protocol

When the operator returns, the loop should present the work in a defined order.

1. The current state summary from `STATUS.md`.
2. The synthesis document if one was produced.
3. A list of material findings that warrant operator review before integration.
4. A list of open items deferred during the loop.

Do not summarise individual documents on return. The operator can read them. Direct the operator's attention to where their judgment is most needed.

## Anti patterns

The following were observed or risked during the inaugural loop. Avoid in future loops.

- Generating documents without verification. The inaugural loop spent its budget on prose and not on prototypes. Future loops should budget verification time explicitly.
- Scope expansion. After exhausting the stated backlog the inaugural loop continued into adjacent topics. Some additions were valuable, others bordered on inventing work. Stop earlier when the stated scope is done.
- Confidence flattening. Treating high stakes recommendations with the same epistemic weight as cosmetic recommendations is a disservice. Confidence labels per document are mandatory.
- Length over precision. Long documents impose review cost. Tighter documents respect operator attention.
- Silent termination. Always record why the loop stopped.

## Tooling notes

The inaugural loop used the following tools.

- ScheduleWakeup with the autonomous loop dynamic sentinel for pacing.
- TaskCreate, TaskUpdate, TaskList for in firing task tracking when work decomposed naturally.
- Write and Edit for document production.
- WebSearch and WebFetch were available but were not used during the autonomous phase. Future loops should incorporate these into every firing where external facts are load bearing.

## Versioning

This document is version 0.1. Material revisions should bump the version and note the change in `docs/process/TASKLOG.md`.

| Version | Date | Change |
|---------|------|--------|
| 0.1 | 2026-05-21 | Initial draft distilled from the inaugural Keleusma research loop. |
