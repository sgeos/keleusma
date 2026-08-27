# BRIEF — does confinement account for the arena-bound gap?

**Written**: 2026-08-27, eighth loop iteration. **For this line's own use.**

## The goal

Two measured facts have sat beside each other without ever being joined:

1. **`bound_transfer`**: **11 of 71** compared modules demand more arena than their verified heap
   figure. `backend = sites × size`, `verified = peak_live × size`, so
   `shortfall = (sites − peak_live) × size`. Recorded as *"Workstream E has no bound covering the
   backend's pool"* — a finding with no closure.
2. **`src/confine.rs`** answers, per construction site, *is this region unreachable once its
   enclosing scope ends?* — `Confined` / `CannotEstablish` / `Escapes`. It is a **public library
   predicate**, deliberately not wired into `verify()`.

**Confinement is exactly the property that would license reusing a region's space.** So the question
that has never been asked: **for the modules that exceed, are their sites confined?**

## Why this is worth an increment

The gap exists because **`plan_chunk_region` consumes no escape reasoning at all** — every static
site gets its own offset. That is also precisely why a wrong confinement verdict cannot miscompile
today: nothing reads it. So this measurement is **free of soundness risk** and can only inform.

Three possible outcomes, all publishable:
- **Mostly confined** → the gap is plausibly closable by reuse, and the size of the prize is known.
- **Mostly escaping** → the gap is real and reuse is not the answer; something else is needed.
- **Mostly `CannotEstablish`** → the analysis is too weak on this population to say, which is a
  finding about the analysis rather than about the gap.

## ⚠ The overclaim this must not make

**`Confined` does NOT mean `sites − confined` is the achievable demand.** Confinement says a region
is dead after its scope ends, which licenses reuse **across** scopes, not within one. Two confined
sites live in the same scope still need separate space.

**So the measurement bounds what reuse COULD reach; it does not compute a new bound.** State that
plainly, or the number will be read as a proposed figure.

The recorded direction of the existing bounds must also be preserved: **`Confined` is sound and every
disqualifier is an UPPER bound on escape**, so a count of confined sites is a **lower** bound on how
many are genuinely reusable.

## Prior failures this is exposed to

1. **Overclaiming from a suggestive number** — the whole hazard above.
2. **A vacuous instrument** — three filters broke this session. **Show it discriminates.**
3. **The already-holds-the-answer trap** — if every site in the corpus were `Confined`, a per-module
   count would look informative while measuring nothing. **Measure the corpus-wide distribution too,
   as the baseline the per-module figures are read against.**
4. **A truncated read reported as complete** — committed twice this session.
5. **Reporting a figure without the command that produces it.**
6. **Running the two suites in parallel** — invalidates the perf canary. Sequential.
7. **Pinning a figure that ordinary corpus growth moves** — the exceeding set moves with the corpus.
   Report; do not pin a distribution.

## Specific wrong turns to avoid

- **Do not edit `src/confine.rs` or any read-only file.** Using the predicate is fine; changing it
  is not.
- **Do not wire confinement into `plan_chunk_region`.** That is a code-generation change with real
  soundness consequences and it is not this increment. **Measuring is.**
- **Do not report a "new bound".** No bound is being proposed.
- **Do not treat `CannotEstablish` as `Confined`.** Its whole purpose is to be visibly separate.
- **Do not present the corpus-wide numbers and the exceeding-module numbers as the same population.**
  That conflation is on record here as a repeated failure.
