# BRIEF — I measured the right number against the wrong model, one iteration ago

## What happened

The previous increment measured the Order-1 gate's strength and reported:

> **all ten agreeing stages are single-vector** … *"ten stages agreeing ONCE EACH is not ten stages
> shown to agree in general"*

**The measurement was correct. The interpretation was wrong, and it understated the gate.**

Every one of the twelve stage entries is a `Stream` taking one parameter — the **tick**. The harness
pins `seeds = 1` for a stream *deliberately*, and its own comment says why: the driver already varies
the tick across its iterations, so seeding it would change what the run MEANS rather than broaden it.

**So "single-vector" is true and "compared once" is false.** Measured: **600 result comparisons
across the ten agreeing stages, 60 per stage, min 60 and max 60.**

## Why this is worth a full increment rather than a quiet edit

The previous increment's whole finding was *a number read against the wrong population*. **I then
read a number against the wrong execution model in the same breath**, and shipped it into the resume
document as a headline. The failure is the same species, one level up: the first was a layout
coincidence, this was an unexamined assumption that "argument vector" is the universal unit of
comparison strength.

**It also errs in the rarer and more dangerous direction.** Most reporting errors inflate. This one
*understated* a gate the roadmap depends on — and an understated gate invites work that is not
needed, or a "not met" verdict the evidence does not support.

## What the fix has to be

1. **The gate block must report the measure that applies to a stream** — ticks compared — and say
   plainly that the vector count is not the strength measure here.
2. **The prior claim must be corrected where it was published**, not silently overwritten. It went
   into the resume document as a state-table row and a section heading.
3. **The error must be recorded**, because "vector count is the strength measure" is exactly the kind
   of assumption the next reader inherits.

## Prior failures and the specific wrong turns to avoid

- **DO NOT delete the vector-count line.** It is correct and it is the reason `SEEDS` widening does
  not apply here. Report BOTH, with the stream caveat attached to the first.
- **DO NOT now overclaim in the other direction.** Sixty ticks per stage is a real comparison
  surface, but every stage still gets ONE input program and ONE seeded segment. **The tick sequence
  varies the stage's internal position, not its input.** Say that; it is the honest residual.
- **DO NOT declare the Order-1 gate met.** This correction removes a reason to doubt it; it does not
  supply a reason to certify it. That call is not this increment's.
- **DO NOT assume 60 is a constant.** It came from a run. Report it from the run.
- **A guard on `min_c > 0` is the right shape** — an agreeing stage compared at zero points would be
  vacuous while counted. Do not pin 60.
- **Check whether the same wrong model leaked elsewhere.** If any other report treats vector count as
  the strength measure for a stream-entry module, it has the same defect.

## What a good outcome looks like

The gate block reports ticks compared alongside the vector count, with the stream caveat stated. The
resume document no longer says "compared once each" anywhere, and records that it did and why that
was wrong. **And the residual is stated: one input program per stage, sixty positions within it.**
