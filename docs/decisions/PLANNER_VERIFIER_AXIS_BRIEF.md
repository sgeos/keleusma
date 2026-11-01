# BRIEF — can a planner reproduce the verifier's liveness axis?

**Written**: 2026-08-27, eleventh loop iteration. **For this line's own use.**

## Where this stands

The arena thread has three findings: confinement does not explain the gap; **branch exclusivity
explains it completely** (11 of 11, zero residue); and the max-over-arms remedy is **safe on the
modules that need it and unsound on most of the corpus** (0 hazard sites among the exceeding
modules, 176 across the corpus).

The remedy's **third** precondition is still listed open: *whether the verifier's `peak_live` is
computed on an axis the planner can reproduce.*

## It is half-answered already, and saying so is part of this increment

**A forty-line syntactic walk over `If`/`Else`/`EndIf` reproduced `peak_live` EXACTLY on 11 of 11
exceeding modules.** That is direct evidence the axes align — a planner *can* compute the verifier's
figure, at least there.

**But all 33 of those sites are bare conditionals.** The corpus's other 176 sites sit inside loops,
where my walk **deliberately accumulates** rather than treating iterations as alternatives. **So the
agreement has only been tested on the easy half.**

## The measurement

Extend the `path-max` versus `peak_live` comparison from the eleven exceeding modules to **every
module with a construction site**, and report where they diverge.

- **They agree corpus-wide** → the axes align generally, and the third precondition is met on
  everything measurable here.
- **They diverge, and the divergences are loop-heavy** → the axes align on branches and part on
  loops, which names exactly where a planner would fail to reproduce the verifier.
- **They diverge unpredictably** → the walk is not modelling what the verifier models, and the
  earlier 11-of-11 agreement was luckier than it looked.

**The third outcome would weaken a published finding, which is precisely why it must be measured.**

## Direction of the comparison, because it is not symmetric

`path-max` is an **upper bound** on simultaneous liveness. `peak_live` is derived from the verifier's
`max_heap_bytes`, which is a bound the verifier is willing to certify. **Expect `path-max >=
peak_live`; a module where `path-max < peak_live` would mean the walk is missing liveness the
verifier sees**, and that is a defect in the walk rather than an interesting divergence. Report the
two directions separately.

## Prior failures this is exposed to

1. **Generalising from the easy half.** The 11-of-11 result is exactly that, and this increment
   exists to test it.
2. **Reporting a divergence as a defect when it is a modelling difference.** Say which.
3. **A vacuous instrument.** Six filters or guards have broken this session.
4. **Conflating populations** — exceeding-only against corpus-wide, on record repeatedly.
5. **Division by a zero site count**, which would silently produce a `peak_live` of 0 and a fake
   agreement. Modules with no sites must be excluded and the exclusion stated.
6. **Reporting a figure without the command that produces it.**
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not change `plan_chunk_region` or any read-only file.** Still a measurement.
- **Do not adjust the walk to make agreement better.** If it diverges, that is the result. Tuning an
  instrument until it matches the thing it is being compared against destroys the comparison.
- **Do not treat a module with mixed placements as a loop case or a branch case.** Report its
  placement mix alongside the divergence, so the reader can see which explains it.
- **Do not claim the third precondition met on a corpus-wide agreement alone.** Agreement here is
  evidence about this corpus; the same caveat that applied to the hazard cell applies again.
