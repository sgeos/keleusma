# Brief — absorption 47, and the first prediction to carry a build clause

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02.**

---

## The goal set

| goal | state |
|---|---|
| **G19** absorption 47 — the `Text<N>` type surface | **unblocked, and the subject of this brief** |
| `f16` | no oracle; the reference refuses widths 3 and 4 at load |
| publication | held |

`Text<N>` has been listed as blocked on the `v0.2.3` line all session. **Its first increment has
landed**, described as *"the type surface, refused everywhere below it."*

## Why this needs a brief when the last routine absorption did not

**Absorption 46 taught a specific lesson and this is the first chance to apply it.** That prediction
named test-level falsifiers and the absorption broke the build — a failure the instrument could not
express, because a test count is downstream of compilation. The `v0.2.3` line sharpened it: the
instrument is not merely unable to state the claim, **it cannot be reached at all**, so silence is the
only signal and silence is indistinguishable from not having run.

**So this prediction carries a build clause, checked separately**, and that is the change worth
recording rather than the absorption itself.

## The prediction

**Predicted, in two independent parts:**

1. **The package still BUILDS.** Checked on its own, because no observation at the test layer can
   distinguish a package that failed to build from one nobody built.
2. **Zero movement**: 461 passed, 0 failed, 89 binaries under default features and again under
   `narrow-float-32`.

**The reasoning for part 2, so it can be wrong for a nameable reason**: this backend refuses `Text` at
every route it can reach — the shared-slot resolver reports *"Text slot; string representation is
Workstream C"* — so no lowered path carries a text value.

**Falsifiers, named in advance:**

- **Build**: any compilation failure in the library or in a test target. Absorption 46 broke four call
  sites through two signature changes, and a new type surface can widen an enum that this backend
  matches on.
- **Movement**: any test asserting a composite offset, a flat-layout size, or a verifier-derived
  footprint. Those consume `value_layout` directly rather than through a lowered path, so the refusal
  does not protect them — and a new `ScalarKind` variant is exactly the kind of change that moves
  them.

## The wrong turns

**1. Measure the absorption alone.** Nothing else in flight, or the result cannot be attributed —
which the `v0.2.3` line sharpened into the better objection: a folded-in change makes a zero-movement
result **unfalsifiable**, not merely hard to attribute.

**2. If the build breaks, repair it minimally and record any concern rather than fixing it inline.**
That is what preserved absorption 46's measurement and what turned a deferred note into a real defect
found the next iteration.

**3. Enumerate the call sites; do not match by pattern.** The absorption 46 repair produced a
duplicate declaration, a missed site, and an unopened file, because a pattern match is a claim about a
set defined by what you thought of.

**4. Do not read "refused everywhere below it" as "cannot reach this backend."** Their refusal is at
their layer. A type surface still changes shared descriptors, and this backend reads those.

**5. The gate covered `1d49a102`, not this tree.** The steps that carry over are the ones nothing has
touched. Say which rather than implying the gate covers what came after it.
