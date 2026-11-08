# BRIEF — the reason the family was narrow has expired

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why now

The pre-registered mutation family is **arithmetic and bitwise only**, and the recorded reason is
explicit: *"no comparison, no branch. A comparison swap can change a loop's trip count, and the native
side carries no bound check, so a mutant could run forever and hang the suite."*

**Two filters built since then handle exactly that**, and both were forced by real failures rather than
anticipated:

| filter | what forced it |
|---|---|
| the mutant must remain **admissible** | a **SIGBUS** on `04_for_in.kel` |
| the mutant must not **fault** on the reference | a **SIGTRAP** afterwards |

An inadmissible mutant is refused before it is lowered; a faulting mutant is caught by running the
tolerant side first. **The restriction is a carried claim whose basis no longer holds**, and this
line's own handoff names the rule: *"Re-derive, do not carry."*

## What widening buys

More families produce more **killable** mutants. The census currently asserts that every killable
mutant is detected — a real property, but one resting on a narrow family at three sites. Comparison
and negation swaps exercise control flow, which arithmetic swaps largely do not.

**The census may get worse, and that would be the finding.** A newly-killable mutant going undetected
is exactly what the differential should be probed for.

## Wrong turns to avoid

- **Do not treat widening as pre-registration violation.** Amending a family is legitimate when the
  reason for its shape expires; it is illegitimate when done to flatter a result. The amendment must
  be recorded with its cause, as the `CheckedAdd` amendment was.
- **Do not assume the filters hold.** They were built for arithmetic mutants. **A hang is not a test
  failure — it is a suite that never finishes**, so the run must be watched and the family narrowed
  again if anything hangs. Do not claim safety that was not observed.
- **Do not conflate "newly undetected" with "the differential got worse."** A wider family reaching new
  code is *more* probing; an undetected killable mutant is a finding about the observable, not a
  regression.
- **Do not report a changed detection figure without the family it rests on.** The number is
  meaningless without saying what was mutated.
- **Do not silently drop a mutation kind that misbehaves.** If a kind has to be removed, say which and
  why.
- **Do not let the detection floor mask a change.** A ratio floor calibrated on the old family may pass
  trivially or fail spuriously on the new one; re-calibrate deliberately and say so.

## The second deliverable, and why it is not busywork

The operator has been absent for roughly ten increments. **All remaining capability work on this line
is blocked behind decisions only they can take** — measured, not assumed: the 4 unlowerable chunks sit
in exactly the 3 refused modules, and those are `Len` (structurally inadmissible), `Stream` (the
composite slot-reuse obligation), and the float entry ABI.

A single short index of *what needs deciding, what each costs, and what I will do if nothing is said*
is more useful to them than another depth increment. **It must not re-argue the cases** — those
documents exist — and it must not recommend where the earlier documents deliberately declined to.
