# BRIEF — the mutation sweep is stalled on cost per mutation, not on count

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: working brief.

---

## The situation

[`NATIVE_MUTATION_CENSUS.md`](./NATIVE_MUTATION_CENSUS.md) says **"no hole open"**. It was measured
2026-08-16 at `f1800820`, and `native_codegen/src/lib.rs` has changed in dozens of commits since. The
sweep mutates that emitter, so every verdict there is a property of the emitter as it stood.

**Re-running it was attempted and abandoned.** Round one ran **12h51m** and was killed on `CmpLe`, the
**5th of 25** mutations — roughly **60 hours** for round one alone, itself 25 of **53** pre-registered
mutations. The emitter was restored byte-identical and re-checked.

**The targeted-subset route was examined and rejected**: the opcodes named in the emitter diff cover
roughly **18 of 25**, not a handful.

## The assumption nobody has tested

Every analysis so far has attacked the **number of mutations**. **The cost is the product of two
terms, and only one has been attacked.** At 12h51m for five mutations, each costs about **2.5 hours**,
because each runs the whole corpus differential.

**A mutation to the `CmpLe` emitter can only be detected by a module containing `CmpLe`.** Every other
module is executed, compared byte for byte, and contributes nothing. If the differential for a
mutation ran only the modules that carry its opcode, the per-mutation cost could fall by an order of
magnitude, and 60 hours with it.

**This is a hypothesis about where the time goes, and the first task is to measure that** rather than
to build on it. If the time is dominated by JIT setup or by a handful of large modules that carry
every opcode, the saving evaporates and **the honest outcome is to record that and stop.**

## THE CORRECTNESS OBLIGATION, WHICH IS THE WHOLE RISK

**Scoping an oracle weakens it.** The differential is this line's correctness signal, and narrowing
what it compares is exactly the kind of change that makes a sweep cheaper and its verdicts worthless.

Two failure directions, and the second is worse:

- **A mutation recorded "not caught" that an excluded module would have caught.** Reports a hole that
  is not there; wastes work.
- **A mutation recorded "caught" on a scope so narrow the catch was incidental.** Reports coverage
  that is not there. **This is the direction that puts a false "no hole open" back into the census.**

**So no verdict may be quoted from a scoped run until the scoping is shown to lose no known catch.**
The check is available and cheap: take mutations the full sweep already caught, run them scoped, and
require the same verdict. A scoped run that disagrees with the full run on even one is disqualifying.

## Wrong turns, named

1. **Do not re-run the full sweep to "see how it goes".** It is 60 hours and it has already been
   killed once. The background command ceiling here is **ten minutes**; anything longer must be
   chunked, and two runs were lost tonight to exceeding it.
2. **Do not quote a figure from a scoped run before the equivalence check passes.** A cheaper
   instrument that has not been calibrated is not a cheaper instrument, it is a different one.
3. **Do not annotate the census while measuring it.** The staleness metric counts emitter commits
   since the last commit touching the census, so **annotating a stale document resets the metric that
   measures its staleness** — already recorded, already once broken. Compute against the pinned
   measurement commit `f1800820`.
4. **Do not pipe a long run through `tail`.** The previous sweep's output was buffered until exit, so
   progress was invisible and the continue-or-kill decision was blind for hours.
5. **Restore the emitter byte-identically and verify it**, as the previous attempt did. A sweep that
   mutates a file in place must leave it provably unchanged.
6. **Do not edit the tree while a measurement runs.** Done twice on this line.
7. **If the measurement says the idea does not pay, record that and stop.** A negative result here is
   a real result: it closes the cheapest-looking route to un-staling the census, and the census stays
   marked stale rather than being quietly leaned on.
