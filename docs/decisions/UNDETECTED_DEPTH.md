# Is it the site, or the subject?

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: measured, 2026-08-29. The previous increment's figure is corrected, in the same direction as
the two corrections before it — the subjects are better than I reported.**

## The corrections

| reported | why it was wrong |
|---|---|
| **32 detected, 16 undetected** | the census drove subjects at seed 0 with no stage seed |
| **38 / 12** | three mutation sites per module was too thin a basis |
| **39 / 11** | two copies of the site *selection* disagreed; unified |

Of the 11 undetected at three sites, **6 are detected at sixteen**: `piano_roll_3`, `piano_roll_4`,
`reconstruct`, `verify_depth`, `verify_structural`, `verify_types`. **For those it was the site, not
the subject.**

## What remains, with its denominator

| subject | applicable sites | tried | real comparisons | detected deeper |
|---|---|---|---|---|
| `verify_datalayout.kel` | **9** | **9 — exhaustive** | 5 | no |
| `rogue_gear.kel` | **1** | **1 — exhaustive** | 1 | no |
| `piano_roll_9.kel` | 39 | 16 | **16** | no |
| `codegen.kel` | 845 | 16 | 15 | no |
| `parse.kel` | 1015 | 16 | 15 | no |

**Only three of the ten self-hosted stages remain undetected**, down from eight.

## Which explanation the evidence supports, and which it does not

The two candidates were **the site** (the sampled ops are not on a path the seeds execute) and **the
subject** (the compared observable does not reflect the computation).

- For **`verify_datalayout` and `rogue_gear` the sampling explanation is excluded**: every applicable
  site was mutated, real comparisons were produced, and none differed. **These point at the
  observable.**
- For **`codegen` and `parse` the evidence is weak in both directions.** 16 of 845 and 16 of 1015
  sites is a thin sample; nothing here distinguishes the two explanations for them.
- `piano_roll_9` produced a comparison for **every** mutant tried (16 of 16) but covers 16 of 39 sites.

**3198 sites beyond the cap were not exercised, and the test prints that.** A cap that is not printed
reads as exhaustive.

## What is NOT established

- **Not that the observable is weak in general.** It is supported for two subjects by exhaustion and
  unsupported for two others by thin sampling.
- **Not that a subject "detects nothing".** Everything here is against one pre-registered family —
  checked and plain add/subtract, multiply, bitwise-and, shift.
- **Six subjects produced ZERO comparisons** (`faulty`, `04_for_in`, `11_signed`, `rogue_dungen`,
  `rogue_game`, `wire`). Their `no` means *nothing ran*, not *nothing was noticed*. `wire.kel` is the
  striking one: 929 applicable sites and not one usable mutant.

## The defect this increment fixed in its own instruments

**The same query existed twice, twice.** First the probe: the census and the deep sweep each had a
copy, handling a faulting mutant differently. Then the site *selection*: one picked the middle as
`len / 2`, the other `(total - 1) / 2`, which differ for some lengths. They disagreed about
`verify_typed.kel` — in one list and not the other.

Both are now single functions. **A disagreement between two copies of "the same query" is invisible
unless something compares them**, and nothing did.
