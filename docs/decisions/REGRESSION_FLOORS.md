# The figures this line reports had nothing under them

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: closed, 2026-08-29.** Four figures now carry a regression floor. **Each floor was proven to
fire** by temporarily raising it above the measured value and observing the failure, then restoring
it — a floor whose reach is unproven is the defect it was written to prevent.

## What was actually guarded before

| figure | value | strongest existing guard | what that guard catches |
|---|---|---|---|
| opcodes lowered | 61 of 66 | partition totality, non-vacuity, extraction completeness | a broken **instrument** |
| chunks lowerable | 1070 of 1074 | `compiled > 10 && total_ops > 1000` | wrong **corpus paths** |
| opcode instances | 89841 of 89940 | the same non-vacuity check | wrong **corpus paths** |
| differential executed and agreeing | 61 | `executed.len() >= 20` | losing **two thirds** and no more |

Every one of these is a real check, and none of them is a check on the thing the figure measures.
**They hold just as well at 30 of 66 as at 61 of 66.**

## Why the fourth is the one that mattered most

`module_refusals` reports per **chunk**; the corpus differential exempts per **module**. So a single
newly-refusing chunk removes an entire file from the correctness comparison **without any refusal
being wrong**. A lowering regression therefore reaches the differential indirectly and quietly: the
coverage figure drops, the executed count drops, and neither was floored meaningfully.

The `KNOWN_VACUOUS` check immediately above the old floor exists to stop exactly this one module at a
time, and its own comment says that set "was 40-strong-looking coverage for months precisely because
nothing checked it". The floor and that check want the same thing; only one of them was tight.

## The floors, and why a floor rather than a pin

| figure | floor | slack |
|---|---|---|
| opcodes lowered | `>= 59` | 2 opcodes |
| chunks lowerable | `>= 99%` | ~0.6 points |
| opcode instances | `>= 99%` | ~0.9 points |
| differential executed | `>= 56` | 5 modules |

**Not equality pins.** `corpus_differential.rs` records what an equality check does when ordinary
progress breaks it: it "teaches the next reader to delete the check". Lowering *more* must stay free.

**Ratios where the denominator moves.** Adding one `.kel` source changes `chunks_total` and
`total_ops`, so an absolute floor there would fail on growth — the failure mode that gets guards
deleted. The opcode count and module count have stable denominators and take absolute floors.

Each failure message carries the value the floor was calibrated against, so a reader can tell a
regression from a stale threshold.

## What a floor does NOT establish

**Meeting a coverage floor says the backend emitted code, not that the code is right.** The census's
own banner already says a LOWERS verdict is not a correctness claim; these floors do not change that.
Correctness is the differential's job, which is why its floor is the one that was tightened rather
than merely added.

## What was deliberately left alone

Files whose stated purpose is to report — the `spike_*` and `probe_*` genre — were **not** given
floors on the strength of printing more than they assert. That ratio is not the finding; **being
reported to the operator is.** Manufacturing findings from a ratio is the error the corpus already
names in its own guard-design notes.

## The pattern this belongs to

Two defects found earlier in this session were in **instruments, not products**: a census deriving
opcode names from English word order, and a sweep reading 35 modules where its consumers read 74.
Both were clean by accident. This is the same class one level up — **the instruments reported
faithfully and nothing checked the report against yesterday's.**
