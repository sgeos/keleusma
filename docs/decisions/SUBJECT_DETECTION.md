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

---

## ⚠ 2026-08-29: these sweeps are now OPT-IN, and the figures above are DATED

**Both mutation sweeps are marked `#[ignore]` and no longer run in the everyday gate.** Run them with
`cargo test --test corpus_differential -- --ignored --nocapture`.

### The widened family — a real gain

| | arithmetic only | widened with control flow |
|---|---|---|
| detected | 39 | **48** |
| undetected | 0 | **0** |
| subjects with NO applicable site | 10 | **3** |

Comparison swaps reach opcodes arithmetic never touched. In the deep sweep, **8 of 15** re-swept
subjects yielded killable mutants under the wider family and every one was detected — including
`verify_datalayout`, whose applicable sites went from 9 to 41, which further refutes the claim about it
already withdrawn above.

### The cost, measured rather than estimated

About **600s** before the widening; **1379s** alone after it; and **neither sweep finished within
twenty minutes** under gate contention, even after hoisting the reference runs out of the mutant loop
and cutting the census to one site per module. The cause is that comparison mutants are admissible and
non-faulting, so they run fully across every variant, and the seeded stages carry many variants each.

### What being opt-in costs, stated plainly

**The assertions in these sweeps — including the detection floor — no longer protect anything day to
day.** A regression in mutation sensitivity would be caught only when someone runs them deliberately.
**The figures above are a dated measurement, not a standing guarantee.**

The widening was kept rather than reverted because 39→48 and 10→3 is more correctness than a fast gate
is worth. This matches how the existing opcode-level mutation work is already driven: externally, by
`tools/mutation_sweep.py`, rather than in the suite.

**Roles are now split**: the census is BREADTH — every module, one site — and the deep sweep is DEPTH,
at up to eight sites in the subjects the census finds nothing in.
