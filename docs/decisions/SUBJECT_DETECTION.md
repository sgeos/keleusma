# Which subjects would notice a wrong backend

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: measured, 2026-08-29.** The reference runs the original module and the native side runs a
**mutated clone** — a simulated backend defect. A subject that still agrees did not distinguish it.

## Result

| outcome | count |
|---|---|
| **detected** the mutant | **38** |
| did **not** detect it | **12** |
| unmeasured — no mutation site | 10 |
| unmeasured — mutant inadmissible | 2 |
| unmeasured — mutant faults on both sides | 4 |

**The unmeasured classes are not failures to detect**, and folding them in would manufacture a finding.

## The twelve, and the part that matters

`piano_roll_3`, `piano_roll_4`, `piano_roll_9`, `rogue_gear` — and **eight of the ten self-hosted
stages**: `codegen`, `parse`, `reconstruct`, `verify_datalayout`, `verify_depth`, `verify_structural`,
`verify_typed`, `verify_types`.

**These are seeded.** `STAGE_SEEDED` carries ten stages, so this is not the known "a stage reading an
unseeded segment sees zeros" effect — they run on real input and still agree under mutation. They are
also the modules the V0.3.0 self-hosting goal depends on most.

## Scope, stated so the result is not read as more than it is

- **Against ONE pre-registered family** — checked and plain add/subtract, multiply, bitwise-and, shift
  — at **up to three sites** per module. A different family or site might be detected.
- **Within `corpus_differential` only.** `stage_differential.rs` is a separate harness that seeds
  **both** sides. **Whether it detects these mutants is a separate question and has not been asked.**
  "Undetected here" is therefore not "uncovered".
- **Nothing was deleted or exempted.** Coverage present before this measurement is present after it.

## Three corrections made to this measurement, each caught before publication

1. **The mutation family was wrong.** It listed only `Add`/`Sub`/`Mul` and found a site in 4 modules of
   65, because Keleusma is total and the corpus emits `CheckedAdd`. **The test's own non-vacuity
   assertion caught it**, and the family was amended before any subject had been classified.
2. **The driver was weaker than the harness it describes.** The first run drove every subject at seed 0
   with no stage seed and reported the stages as undetecting — an artefact of the driver, not a
   property of the subjects. Corrected to the sweep's own seeding: **33 detected, 15 undetected.**
   This is the **fifth** narrower-population error on this line.
3. **One site per module was too few.** Sampling three (first, middle, last) gave **38 and 12**.
   Sampling more was decided *after* seeing the undetected list, and that direction is the honest one:
   **more sites can only move subjects OUT of the undetected column**, so this strengthens the
   measurement against the finding rather than toward it.

**A published figure was corrected.** The previous increment reported **32 detected, 16 undetected**
from the unseeded single-site run. The corrected figures are **38 and 12**.

## Two filters that are correctness points, not conveniences

- **An inadmissible mutant is not a wrong backend.** It is a program the runtime would refuse. Lowering
  one killed an earlier run with **SIGBUS**, which became `BACKEND_ADMISSIBILITY.md`.
- **A mutant that faults is not one either.** Checked arithmetic is supposed to trap and both sides do;
  natively that arrives as a process-killing signal. Measured as **SIGTRAP**.

The tolerant side runs first, which is the ordering `corpus_differential` already documents for its own
sweep — arrived at here independently before that note was read.

## The floor

Reported figures get a floor, on the principle established one increment earlier. **60% of measured
subjects must detect**, calibrated against 38 of 50, as a ratio because the measured population moves.
