# BRIEF — I disabled two guards on a number I had myself disclaimed

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The error being repaired

Last increment marked both mutation sweeps `#[ignore]`, removing their assertions — including the
detection floor — from the everyday gate. **The justification was cost. The cost was never cleanly
measured.**

The sequence is the problem:

1. Widening the family made the sweeps slow. **Measured: 1379s alone.**
2. I applied three optimisations — hoisted the reference runs out of the mutant loop, cut the census
   from three sites to one, cut the deep cap from 16 to 8 — and **never measured their combined
   effect**, because every attempt collided with a loaded machine.
3. The one full figure I did get, 4132s, I **disclaimed in writing** as contaminated by a load average
   near 13.
4. **Then I used slowness as the reason to disable the guards anyway.**

Step 4 does not follow from a number rejected at step 3. A guard that does not run protects nothing,
and removing one is exactly the kind of decision that needs the measurement it was denied.

## What to do

**Measure the clean cost on a quiet machine, then decide.** The load average is now 6.4 against nearly
13, with no test binaries running.

If the cost is acceptable, **restore both sweeps to the gate.** If it genuinely is not, the opt-in
disposition stands — but then it rests on evidence rather than on an inference from a rejected figure.

## Wrong turns to avoid

- **Do not measure under contention and call it clean.** Record the load average alongside the timing,
  or the number is worth what the last one was.
- **Do not restore them and skip re-running the gate.** Restoring changes what the gate does; the claim
  "both suites pass" must be re-established, not carried.
- **Do not tune the threshold to justify the outcome I want.** Decide what an acceptable gate cost is
  *before* seeing the number, and say what it is.
- **Do not narrow coverage further to hit the threshold.** Three coverage reductions have already been
  applied; a fourth taken to win an argument with the clock would be the same error compounding.
- **Do not treat "it was slow once" as a property of the tests.** The sweeps were measured slow *before*
  two of the three optimisations existed.
- **Do not quietly drop the `#[ignore]` doc comment if restoring.** It records a real episode; it should
  be corrected in place with the superseded reasoning visible, not deleted.

## The threshold, fixed before measuring

**Acceptable is under 10 minutes for the two sweeps together on a quiet machine.** The whole
`native_codegen` gate ran in roughly that order before the widening, and the loop runs it every
increment. Above that, opt-in stands and I say so.
