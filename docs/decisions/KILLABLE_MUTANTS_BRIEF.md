# BRIEF — I have been counting equivalent mutants as failures

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The defect, in my own measurement

The census compares **`VM(original)`** against **`NATIVE(mutant)`** and records agreement as
"undetected". It never asks whether the mutation changes anything at all.

If **`VM(mutant) == VM(original)`** — the mutated site is not executed under these seeds, or its result
never reaches an observable — then the mutant is **semantically inert**, and a *correct* backend must
also produce that same answer. **No differential could ever detect it.**

Counting such a mutant as "the subject failed to notice a wrong backend" conflates two unrelated
things:

| | |
|---|---|
| **an equivalent mutant** | nothing to notice; says nothing about the differential |
| **a killable mutant that went unnoticed** | the observable does not capture a real semantic difference |

Only the second is a finding. Standard mutation-testing discipline calls the first an **equivalent
mutant** and excludes it; my census has not.

## Why this is likely the whole story for `codegen` and `parse`

Those modules have **845** and **1015** applicable sites. A seeded run exercises a fraction of them, so
most sampled sites are probably inert. That predicts exactly what was observed — many comparisons, no
differences — **without any weakness in the differential.**

**It is a prediction, not a conclusion.** The measurement decides it.

## The fix is nearly free, which is the embarrassing part

`probe_mutants` **already runs `VM(mutant)`** for the fault filter and throws the result away. The
killability test is that discarded value compared against `VM(original)`. No extra execution.

## What a killable-but-undetected mutant would mean

If a mutant is killable and the native side still agrees with `VM(original)`, then the two sides ran
**semantically different programs** and the harness saw no difference. That is the observable being too
narrow — precisely the question the previous increment could not separate from sampling. **This filter
is what separates them.**

## Wrong turns to avoid

- **Do not report the new number as a discovery.** It is a **correction**, the fourth in a row, and all
  four found the subjects better than I first said. Say so.
- **Do not drop the equivalent mutants silently.** They must be counted and printed as their own class,
  or the denominators stop adding up and a reader cannot see how much was excluded.
- **Do not conclude "the differential is fine" if the undetected set empties.** An empty set under one
  pre-registered family at capped sites is weaker evidence than it looks; say what was not exercised.
- **Do not compare `NATIVE(mutant)` against `VM(mutant)`.** That tests whether the backend faithfully
  compiles a mutated program, which is not the question. The reference is `VM(original)` — a correct
  backend versus a wrong one.
- **Do not let the two censuses drift again.** They now share a probe and a site selection; the
  killability filter belongs inside the shared probe, not in one caller.
- **Do not widen anything** to make a mutant killable.

## What good looks like

Per subject: mutants tried, equivalent (excluded), killable, and of the killable how many were
detected. The undetected column then contains only mutants that **could** have been caught.
