# BRIEF — can drift in prose be instrumented?

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## The gap this session named and did not close

**Six instruments fire on drift in code. None fires on drift in prose.** Four records were found
outliving their subjects in two days:

| record | what had changed |
|---|---|
| `NATIVE_BOUNDS_TRANSFER.md` | asserted the reference's memory bound unsound **four weeks after it was repaired** |
| `NATIVE_COMPOSITE_RETURN_ABI.md` | said "not repaired", and that a test carries the case as `#[ignore]`; repaired a month earlier, no ignored tests remain |
| `ORDER_1_VERIFY_TYPES_BRIEF.md` | said "REPORTED, not repaired"; the other line had ACTED on it |
| `fixed_shared_scale.rs` | opened by saying the backend refuses a `Fixed` shared slot; it lowers |

**Each was found by hand, one at a time.** The question is whether that can be mechanized.

## What was measured, before deciding

| matcher | population | usable? |
|---|---|---|
| status-claim phrases (`not repaired`, `still open`, `is refused`, `UNEXERCISED`, …) | **169 lines, 16 with a nearby date** | **no** — dominated by descriptive prose, not status |
| claims naming a test AND a state (`#[ignore]`, "`x.rs` … refuses/fails") | **32 lines** | **no** — dominated by recorded history |

**Both populations are mostly narrative.** *"`fixed_shared_scale.rs` opened by stating that this
backend REFUSES a `Fixed` shared data slot"* is a sentence ABOUT a corrected claim; a matcher cannot
tell it from the claim itself. Demanding a date on all 153 undated lines would be the unmaintainable
table this package has twice refused — the premise census measured a proposed widening at 29 rows to
132 and declined.

## The wrong turns

1. **Do not ship the general instrument anyway.** A census whose population is ten times its signal is
   the "looks like coverage while being none" failure this line names repeatedly.
2. **Do not conclude the gap is unreal.** Four records drifted; the mechanism is what is missing, not
   the problem.
3. **Do not claim the per-claim guards close it.** They close the four claims that are guarded. The
   next stale sentence will be somewhere else.
4. **Do not date-stamp prose as a substitute.** A date says when someone wrote it, not whether it is
   still true — the prediction stamp works because a prediction is compared against an outcome, and a
   description is not.

## What is actually available

**A guard per corrected record**, so the state cannot revert unnoticed. Two of the four already have
one — the bounds-transfer figure is asserted by its spike, and the `Fixed` shared slot is now driven.
The third is trivially checkable and missing. The fourth is upstream and already watched by
`outstanding_reports.rs`.

## What done looks like

The attempt is recorded with the measurement that defeated it; every record corrected this session has
something that fails if its subject reverts, or is named as unguarded; and the residual is stated —
prose drift is closed claim by claim, and the only general defence is re-reading a header when the
code under it changes.

---

## OUTCOME — 2026-09-12

**The general instrument was attempted and rejected on measurement, not on taste.**

Two matchers, both dominated by narrative rather than live status: 169 status-phrase lines with 16
dated, and 32 claims naming a test and a state. *"`fixed_shared_scale.rs` opened by stating that this
backend REFUSES a `Fixed` shared data slot"* is a sentence ABOUT a corrected claim, and no matcher
separates it from the claim. **A census whose population is ten times its signal reads as coverage
while being none** — the failure this package names repeatedly, and the reason the premise census
declined a widening from 29 rows to 132.

### Every record corrected this session now has a guard, or is named as unguarded

| record | guard |
|---|---|
| the bounds-transfer figures | `spike_bounds_transfer.rs` asserts at least one inversion and pins the population |
| the `Fixed` shared slot refusal | `fixed_shared_scale::a_fixed_shared_slot_reads_the_same_value_on_both_paths` drives it |
| the composite-return "#[ignore]" claim | **added here** — `this_file_carries_no_ignored_test`, proven to fire by inserting an `#[ignore]` and watching it fail |
| the `verify_types.kel` report the other line acted on | upstream, watched by `outstanding_reports.rs` |

### The residual, stated rather than closed

**Prose drift is closed claim by claim.** Nothing general exists, the measurement above says why, and
the next stale sentence will be somewhere none of these four guards look.

**Dating prose would not fix it.** A date records when someone wrote a thing, not whether it is still
true. The prediction stamp works because a prediction is COMPARED against an outcome; a description
has nothing to be compared against.
