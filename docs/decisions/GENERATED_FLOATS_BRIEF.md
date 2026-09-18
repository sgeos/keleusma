# BRIEF — generated breadth over the float surface

**Filed 2026-09-18, against `8b218e2e`, backlog 0, suite 641.**

## What this closes, and what it is not

The three generators in this package — expressions, composites, nesting — emit
**zero float programs between them**. `generated_expressions.rs` says so in its
own header: *"nothing here says anything about the trap paths ... nor about
floats, composites or streams, which this generator does not emit."*

**That is an honestly stated exclusion, not a population narrower than its
description**, and the distinction matters. The two matrices closed earlier today
were making a false claim. This one is not. So this is a capability extension and
must be argued on value, not on correcting a record.

The value: **floats are now the least-covered scalar by EXECUTION breadth**, and
the float yield arm landed this session resting on four hand-written subjects.
Every hand-written float subject in the package was chosen by someone who already
had a hypothesis. Generated composition is the instrument for the defect nobody
hypothesised — which is the argument the expression generator was built on, and it
excluded floats.

## The constraint that dominates the design

`keleusma::vm::Vm` is `GenericVm<.., f64>` — **the reference runs at eight bytes
in BOTH configurations** — while the backend lowers at the configured width, `f32`
under `narrow-float-32`. A randomly generated float tree will produce values that
are not four-byte exact, and the two implementations will then differ
**legitimately**. Reported as a divergence, that is a false finding about the
emitter.

**This was already paid for twice today.** In `scalar_operator_matrix.rs` a
result-only exactness check let a phantom `float %: Disagree` through, because the
violated thing was an OPERAND. The fix is structural, not careful:

- **Generate only values that are exact in four bytes, by construction.** Integral
  leaves, and only `+`, `-`, `*`, so every intermediate stays an integer.
- **Bound the magnitude below the four-byte exact-integer limit**, which is
  `2^24 = 16777216`. The word generator uses leaves `0..=9` at depth 3, whose
  all-multiply worst case is `9^8 ≈ 43 million` — **that exceeds the limit**, so
  the float generator cannot simply copy its bounds.
- **Check it anyway**, on operands and on results, and fail loudly. Construction
  arguments rot when someone changes a bound.

## The wrong turns, named

1. **Do not include `/` or `%`.** Division leaves the integers immediately —
   `3.0 / 7.0` is inexact in both widths and differently inexact between them.
   Excluding them is a stated cost, like the existing generator's exclusion of the
   trap paths, not an oversight to be quietly fixed later.

2. **Do not reuse the word generator's depth and leaf bounds.** Its worst case is
   five orders of magnitude inside `i64` and **two orders OUTSIDE** the four-byte
   exact-integer range. Copying them is the obvious move and it is wrong.

3. **Do not add a tolerance.** The byte-identical differential is this line's
   correctness signal. An epsilon here would weaken the property the package rests
   on in order to paper over a harness artefact.

4. **Do not assume a green run means the floats were generated.** A generator that
   silently produced no float programs would pass. **Assert the population**: a
   distinct-program floor, and evidence that the emitted programs really contain
   float operations rather than integers with float type annotations.

5. **Do not let the trap hazard back in.** `+`, `-` and `*` are CHECKED for
   integers and the native side executes `llvm.trap` on overflow, killing the
   process with SIGTRAP. Float arithmetic is not checked the same way, but the
   magnitude bound above must still hold or the exactness invariant fails.

6. **`src/` and `tests/` at the repository root are read-only to this line.**

7. **One gate invocation covers ONE float configuration.** The script takes
   `--narrow`. A single run is not both.

## Pre-commitment, written before the work

- **If the generator finds a divergence, the first check is the exactness
  invariant, not the emitter.** If every operand and result is four-byte exact and
  they still differ, it is a backend finding and it gets reported as one.
- **If generated float breadth finds nothing**, that is the expected outcome and
  the increment still stands — but it must then say so plainly rather than being
  written up as validation. The word generator found nothing either, and its
  header says what a green run may claim.
- **If the exactness bound forces the trees so shallow that no composition is
  exercised**, stop and report that the surface is not reachable this way rather
  than shipping a generator that only tests single operators under a new name.
