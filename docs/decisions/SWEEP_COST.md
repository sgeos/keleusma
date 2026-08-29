# The sweep cost, measured — and a guard I removed on a number I had disclaimed

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: resolved by measurement, 2026-08-29.** The census is **restored to the everyday gate**; the
deep sweep **remains opt-in**. Both dispositions now follow a number rather than an inference.

## The error being repaired

The previous increment marked **both** mutation sweeps `#[ignore]`, removing their assertions —
including the detection floor — from the gate. **The justification was cost, and the cost was never
cleanly measured.**

1. Widening the family made them slow. **Measured: 1379s alone.**
2. Three optimisations followed — hoisting the reference runs out of the mutant loop, cutting the
   census to one site, cutting the deep cap from 16 to 8 — and **their combined effect was never
   measured**, because every attempt collided with a loaded machine.
3. The one full figure obtained, **4132s**, was **disclaimed in the same commit** as contaminated by a
   load average near 13.
4. **Slowness was then used as the reason to disable the guards anyway.**

Step 4 does not follow from a number rejected at step 3.

## The measurement, with a threshold fixed beforehand

**Acceptable was declared to be under 600s, before any number was known.**

| | time | load |
|---|---|---|
| both sweeps together | **712s** | ~5–6 |
| **census alone** | **206s** | ~5–6 |
| deep sweep alone | **710s** | ~5–6 |

**The pair failed the threshold I set, and I did not re-litigate it by pointing at the load average.**
Measuring them separately is a different question, not a second attempt at the same one: the deep
sweep is **essentially the entire cost**, and the census is cheap.

## Disposition

| sweep | runs in gate | why |
|---|---|---|
| `which_subjects_would_notice_a_wrong_backend` | **yes** | 206s, and it carries the detection floor and the non-vacuity checks over every module |
| `how_deep_does_the_undetected_set_go` | **no, opt-in** | 710s, over the threshold |

**What remains unprotected day to day**: a regression in the *depth* of mutation sensitivity — a
subject hiding a killable mutant beyond the census's single site. **The breadth property — every
killable mutant found at one site per module is detected — is asserted on every run again.**

## Two wrong intermediate readings, recorded because they were opposite

First I judged the deep sweep to dominate. **That was right.** Then, watching the gate, I concluded the
census dominated and said so — **wrong**, because libtest prints its over-sixty-seconds notice for
every long test running in parallel, so both appeared stalled. **Only the separate measurement settled
it**, and the correction cost three coverage reductions taken on a wrong premise, of which one (the
census's three sites to one) I have kept because it is cheap and the breadth gain came from the wider
family rather than from extra sites.

## An assertion of mine failed the same way this repository already recorded

The script restoring the census asserted "exactly one `#[ignore]`" and counted **two** — the second
being the text `` `#[ignore]` `` **inside a doc comment**. That is precisely the defect this line
documented when a scanner counted 33 skippable tests where the truth was 10. The check was rewritten
to match attribute lines only, and the file had been correct all along.
