# The unre-derivable-count triage

**Status**: complete for the stated population. 2026-08-27, `v0.3.X` line.

## Why, and the population

Recorded open and **declined twice**. What forced it: **2026-08-26 produced four confirmed instances
of the class in this line's own artifacts** — a count reproducible by none of nine measures, a
truncated read reported as a complete list, a correctly-measured number attached to the wrong axis,
and an assertion weaker than the property it named.

| | count |
|---|---|
| comment lines in `native_codegen/{src,tests}` | 9058 |
| carrying a multi-digit number | 382 |
| naming a command, a `file:line`, or "measured" | 38 |
| unmarked | 344 |
| unmarked and undated | 281 |
| **unmarked, undated, counting MODULES / CHUNKS / OPCODES / SITES / TESTS / STAGES** | **29** |

**The 29 was the target and the 281 was NOT**, because the wider set is dominated by definitional
constants (`0..=255`) and by figures sourced to a *named function* rather than a path —
`ty_max_steps()` is 1801 is perfectly re-derivable.

## The selector was proven to discriminate before its output was believed

Seven cases, one accepted and six rejected for six *different* stated reasons: dated, already marked,
not a state-of-tree count (twice), not a comment, and marked by a re-derivation verb. **A count from
a filter never shown to reject anything is not evidence**, which this line has now learned three
times in one day.

### ⚠ Two vacuous filters were written while sizing this, and both were caught by disbelieving a number

1. A first pass reported **8621 of 8621** comment lines as carrying a re-derivation marker. Cause:
   `grep -n` prefixes each line with `file.rs:<digits>`, so the "names a `file:line`" test matched
   **grep's own output**.
2. The fix then reported **0 of 29**, because the same prefix now matched the *exclusion*.

**The same artefact broke the filter in both directions.** The lesson is not "escape the prefix": it
is that **a filter and its input must not share a namespace**, which is the self-inclusion shape
already recorded twice on this line.

### A known false positive, stated rather than fixed

The selector reads **one line at a time**, so a figure dated on the *preceding* line is not seen as
dated. `corpus_differential.rs:3419` — *"19 modules widen, 482 pairs at `SEEDS = 24`"* — carries
"Measured 2026-08-20" on the line above and is a **false positive**. Left as a known limitation
because widening the window trades this for a harder-to-see false negative.

## Disposition

**Corrected — live claims measurably stale** (9 sites):

| where | was | now |
|---|---|---|
| `src/lib.rs:5` | "46 of 66 opcodes" | **60 lower, 2 refused, 3 unproven** |
| `corpus_differential.rs:2244` | "19 exempt" | **14** |
| `probe_agreement_depth.rs:3` | "34 modules executed and agreeing" | **59** |
| `probe_stage_vacuity.rs:3` | "40 modules executed and agreeing" | **59** |
| `probe_nesting_and_breaks.rs:48–56` | 1032 chunks / 64 modules; other file 1027 / 57 | **1079 / 74**; other **1074 / 67** |
| `spike_bounds_transfer.rs:21–31` | 1027 chunks / 57 modules; other file 1032 / 64 | **1074 / 67**; other **1079 / 74** |

**The two chunk-walker notes were corrected TOGETHER**, because each quotes the other's figure and
correcting one alone would leave the other looking current — the recorded failure mode of updating a
figure in isolation.

> **AND THE FINDING THEY CARRY SURVIVED THE CORRECTION INTACT.** The gap between the two walkers was
> **5 chunks** in August and is **5 chunks** now, across a corpus that grew both totals by about
> fifty. **The numbers being stale never made the claim stale.** That distinction is stated at both
> sites so a reader meeting the new figures does not read them as a discovered defect.

**Annotated as dated and deliberately not re-derived** (2 sites): `region.rs:199` and
`probe_piano_roll.rs:180`, both citing *"239 sites"*. **No instrument reports a corpus-wide site
total**, so there is no cheap re-derivation path. Their role is **motivation** — why a case deserves
a test — and the test is correct at any denominator.

**Historical narrative, needing no change** (the remainder): each already describes a past
measurement in its own words — *"A first attempt … classified 32 modules as vacuous"*, *"this spike
was written when 39 opcodes lowered; the set has since moved"*, the mutation-round tables. **A
sentence that dates itself is not stale**, and rewriting these would destroy the record of what was
believed when.

## Deliberately not done

**No guard pins any of these.** Most are prose about a corpus that grows on ordinary absorption;
pinning them would manufacture failures on growth and teach the next reader to delete the guard.
That is the opposite of the cross-tree guards this line does pin, which exist to *announce* a change
and say so.
