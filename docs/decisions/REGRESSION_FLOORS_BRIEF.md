# BRIEF — the figures this line reports have no floor under them

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The measured finding

Four figures go into the handoff, `REVERSE_PROMPT.md` and `TASKLOG.md` every increment. **A large
regression in any of them turns no test red.**

| figure | value | strongest guard | headroom |
|---|---|---|---|
| opcodes lowered | 61 of 66 | partition totality, non-vacuity — **structural only** | all |
| chunks lowerable | 1070 of 1074 | `compiled > 10 && total_ops > 1000` | all |
| opcode instances | 89841 of 89940 | same non-vacuity | all |
| differential executed and agreeing | 61 | `executed.len() >= 20` | 41 modules |

The existing assertions are **non-vacuity floors**: they catch "the corpus paths are wrong", which is
a different failure from "the backend got worse". Both are worth catching; only one is caught.

**It compounds.** `module_refusals` reports per chunk but the differential exempts per MODULE, so one
newly-refusing chunk removes a whole file from the correctness comparison. A lowering regression
therefore lowers the coverage figure *and* quietly shrinks the differential — and neither goes red.

## Why this is the same defect twice already found, generalised

This session found a census deriving opcode names from **English word order**, and a sweep reading
**35 modules where its consumers read 74**. Both were instruments, not products, and both were clean
only by accident. This is that class at the level of the numbers themselves: **the instruments report
faithfully and nothing checks the report against yesterday's.**

## Wrong turns to avoid

- **Do not equality-pin.** `corpus_differential.rs` says it plainly: a check that breaks on ordinary
  growth "teaches the next reader to delete the check". A deleted guard is worse than a loose one.
- **Prefer a RATIO where the denominator can move.** Chunk and instance counts grow with the corpus;
  a ratio floor survives growth in both directions, an absolute floor does not.
- **Do not set a floor at the current value.** It must have enough slack that a legitimate one-module
  corpus change does not go red, and little enough that a real regression does.
- **Do not treat the existing `>= 20` as adequate because it exists.** It tolerates losing two thirds
  of the correctness comparison. Tightening it is part of the work, not out of scope.
- **Do not claim a floor is a correctness guarantee.** A floor detects regression. It says nothing
  about whether the lowered code is right — that is the differential's job, and the census's own
  banner already warns that a LOWERS verdict is not a correctness claim.
- **Do not add floors to `spike_*` or `probe_*` files on the strength of their print/assert ratio.**
  Those are an exploratory genre that is *meant* to report. The ratio is not the finding; being
  reported to the operator is. Only figures that leave this repository in a handoff need a floor.
- **Do not widen anything to make a floor pass.**

## What good looks like

Each reported figure fails loudly if it regresses materially, passes if the corpus grows, and carries
in its message the value it was set against and why a floor rather than a pin.
