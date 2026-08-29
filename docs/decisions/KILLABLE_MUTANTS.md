# The undetected column was made of equivalent mutants

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: closed, 2026-08-29. This is a correction, the fourth in a row, and the largest.**

## What was wrong with the measurement

The census compared **`VM(original)`** against **`NATIVE(mutant)`** and recorded agreement as
"undetected". It never asked whether the mutation changed anything at all.

If `VM(mutant) == VM(original)`, the mutated site is not executed under these seeds, or its result
never reaches an observable. The mutant is **semantically inert**, and a *correct* backend must produce
that same answer. **No differential could ever detect it.** Standard mutation testing calls this an
**equivalent mutant** and excludes it; this census counted it as a subject failing to notice a wrong
backend.

## The result

| | before | after |
|---|---|---|
| detected | 39 | **39** |
| **undetected** | **11** | **0** |
| unmeasured, all mutants inert | — | **11** |

**All eleven were inert.** Every mutant that could have been caught was caught.

## What I published that was wrong

| claim | status |
|---|---|
| "eight of the ten self-hosted stages do not notice a mutated backend" | **withdrawn** — their mutants were inert |
| "`verify_datalayout` and `rogue_gear`, swept exhaustively, point at the observable" | **withdrawn** — they point at the **seeds**; every mutant that ran was inert |

The second is the one to note. It was stated confidently, with exhaustion offered as the reason it
could be trusted, and **exhaustion over inert sites establishes nothing**. Sweeping every site of a
module the seeds barely exercise produces a null result that means only that.

## What this does and does not establish

**Does**: within the sampled sites, **every killable mutant was detected**. That is now an assertion,
so a killable mutant going uncaught would be loud. It is a stronger property than the census had before.

**Does not**: that the differential is sound. Sites are capped, one pre-registered family is used, and
**3198 applicable sites were never exercised**. `codegen` and `parse` contribute 845 and 1015 sites
apiece and had 15 of 16 sampled mutants inert.

## The large inert counts are a statement about the SEEDS

`verify_datalayout` was swept exhaustively — nine of nine sites — and **not one mutant was killable**.
That is consistent with the harness's own `KNOWN_VACUOUS` record for that module, which was arrived at
independently: it "agrees while producing nothing". The inert column is a seed-coverage measurement
that fell out of a differential-strength measurement, and it should not be reported as the latter.

## The pattern, stated because it is mine

Four corrections in a row, every one finding the subjects better than first reported:

1. the driver ran subjects unseeded — 32/16 became 33/15;
2. three sites was too thin — became 38/12;
3. two copies of the site selection disagreed — became 39/11;
4. equivalent mutants were counted as failures — became **39/0**.

Each was found by looking harder at my own instrument rather than at the product. **The bias is toward
reporting the subjects as worse than they are**, and it is systematic rather than incidental: every
defect took the form of a measurement that could only understate.
