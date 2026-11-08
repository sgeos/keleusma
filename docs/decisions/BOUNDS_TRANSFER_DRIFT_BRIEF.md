# BRIEF — the bounds-transfer figures, pinned

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## What was found

`NATIVE_BOUNDS_TRANSFER.md` records the sharpest result this line has about the project's premise:

> Measured across 20 stream chunks with both figures, **9 of 190 comparable pairs are inversions
> (4.7%)** — pairs the bytecode bound orders one way and the native code the other.

**The tree now measures 27 chunks, 24 of 351 pairs, 6.8%.** The conclusion is unchanged — inversions
exist, so the bytecode ordering is not a proxy for the native ordering — but every number in a
recorded decision document is stale, **and nothing would ever have said so**: `spike_bounds_transfer.rs`
prints its figures and asserts nothing about them.

## Why this matters more than a stale number usually would

This is not a coverage statistic. It is the measured refutation of a hope about **the project's
central claim** — that a program's worst-case execution time is statically bounded and that the bound
means something about the code that runs. A figure carrying that weight should not be able to move
unobserved.

This line already has the rule: **a printing test is a test that cannot fail.** Every other census
here pins its population precisely so a change announces itself.

## What the pin must and must not assert

- **It must assert the conclusion**, which is qualitative and robust: at least one inversion exists,
  so the proxy claim stays refuted. That survives corpus growth.
- **It must pin the figures** so drift is visible, in the same idiom as the other censuses: a recorded
  number, a failure message demanding the cause be established before the number is edited.
- **It must NOT assert an exact rate as a correctness property.** The rate is a property of the
  corpus, not of the backend. A test that fails because a stage gained a chunk is noise; one that
  fails silently *into* a wrong recorded figure is worse, which is the trade the recorded-number idiom
  already makes deliberately.

## The wrong turns

1. **Do not update the document's numbers and leave the original claim unmarked.** The 4.7% was true
   when written and is part of the record. Date both.
2. **Do not attribute the rise to this session's work without measuring.** The corpus grew by eleven
   chunks in absorption 57 alone, and the data-slot work touches modules with data segments rather
   than the stream chunks counted here. **Attribution is a measurement, not a plausible story.**
3. **Do not treat a higher rate as a worse result.** More inversions in a larger population says
   nothing about direction until the populations are compared.
4. **Do not quietly widen what the spike measures.** Its subject is stream chunks with both figures;
   changing that would make the new number incomparable with the recorded one.

## What done looks like

The figures are re-derived and recorded with their date, the original stays visible as the dated
record it is, the spike asserts its conclusion and pins its population, and the cause of the movement
is either measured or explicitly left open.
