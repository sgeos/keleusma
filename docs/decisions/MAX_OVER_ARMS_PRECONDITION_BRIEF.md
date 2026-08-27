# BRIEF — the soundness precondition for max-over-arms

**Written**: 2026-08-27, tenth loop iteration. **For this line's own use.**

## Where this stands

Two increments established the arena-bound gap and its cause:

- **Confinement does not explain it.** Within the only family that exceeds, exceeding members are
  *more* confined than compliant ones; the effect is family, not exceeding.
- **Branch exclusivity explains it completely** — 11 of 11, zero residue. The backend sums over
  static sites; the verifier peaks over live values; the sites sit on mutually exclusive arms.

The candidate remedy is a planner taking the **max across exclusive arms** rather than the sum. It
was named with three preconditions and **not adopted**. This increment establishes the first, and
it turns out the first and second are the same question.

## The hazard, stated precisely

**Outside a loop, exclusivity is total and escape is irrelevant.** Only one arm ever executes, so two
sites in different arms can share an offset no matter where their regions end up. **A region that
escapes to the caller is still the only one that was ever built.**

**Inside a loop, that stops being true.** Arm A allocates in iteration 1 and its region escapes into
a local that survives the iteration; arm B allocates in iteration 2 at the same reused offset —
**clobbering a region that is still live.**

> **So the precondition is not "are the sites confined" and not "are they exclusive". It is: ARE THE
> EXCLUSIVE SITES INSIDE A LOOP?**

That also resolves the apparent tension between the two prior findings. **Escape matters only in
combination with loop-carried reuse.** The exceeding modules' sites mostly escape — 33 of 36 — and
that is harmless if the exclusivity is loop-free.

## The measurement

For each exceeding module, determine whether any construction site sits inside a conditional that is
itself inside a loop.

- **None do** → the remedy is sound on this corpus, and the hazard is real but unexercised. The
  precondition becomes a stated guard rather than a blocker.
- **Some do** → the hazard is live, those modules are named, and the remedy needs the loop case
  handled before it could be adopted anywhere.

**Both outcomes are results.** The second is more interesting and must not be avoided by scoping.

## Prior failures this is exposed to

1. **Confirming a hypothesis on the example that produced it.** Committed once and caught; the
   measurement must cover the whole exceeding set.
2. **A vacuous instrument.** Five filters or guards broke this session. **Show it discriminates**: a
   site inside a loop-wrapped conditional must be distinguished from one in a bare conditional AND
   from one in a bare loop.
3. **Conflating populations** — corpus-wide against exceeding-only.
4. **Overclaiming a remedy.** Nothing is adopted. A clean result licenses "sound on this corpus",
   never "sound".
5. **Treating an absence as a proof.** If no module exercises the hazard, that is a fact about the
   corpus, not about the rule.
6. **Reporting a figure without the command that produces it.**
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not change `plan_chunk_region` or any read-only file.** This remains a measurement.
- **Do not conclude the remedy is safe from a corpus that never exercises the hazard.** Say which of
  the two was established.
- **Do not treat `BreakIf` as making a loop harmless.** A break changes which iterations run, not
  whether a region survives one.
- **Do not restrict the walk to the entry chunk.** The gap is measured module-wide; the precondition
  must be too, or the two are about different things.
