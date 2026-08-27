# BRIEF — make the region planner's non-reuse enforced, not merely written down

**Written**: 2026-08-27, fifteenth loop iteration. **For this line's own use.**

## Why this, and why now specifically

Last iteration found that `NATIVE_LOWERING_INVENTORY.md`'s worst-case-memory-safety argument rested
on *"composite lowering does not exist"* — **which stopped being true.** The system is still safe,
but for a different reason the document never mentioned: **`plan_chunk_region` gives every static
site its own offset and never reuses.**

**That property is prose in FOUR places and asserted in ZERO.** `region.rs:8`, `region.rs:114`,
`composite_return_aliasing.rs:16`, `loop_composite_census.rs:21` all state it; nothing fails if it
stops being true.

**And this line has spent three iterations building the case for a reusing planner.** The arena-gap
work concludes that max-over-arms reuse would close an 11-module shortfall. **The pressure to change
exactly this property is one this line manufactured**, and the safety currently depends on nobody
acting on it carelessly.

> **A safety property whose only enforcement is a comment, in a codebase actively being pushed toward
> violating it, is the shape of defect worth spending an increment on.**

## The invariant, scoped precisely

**WITHIN a chunk, planned site ranges are pairwise disjoint** — not merely distinct offsets, since
two different offsets can still overlap if the sizes are large enough. Disjointness is the property
that makes non-reuse real.

## ⚠ DO NOT OVERCLAIM IT ACROSS CHUNKS

`composite_return_aliasing.rs` records a **known defect**: offsets are planned **per chunk from
zero**, so a callee writes its result at the same offset every call while a caller holds two live.
**One buffer, one offset, two live values.**

**So the cross-chunk case is already broken and documented.** A guard that implied whole-program
disjointness would be false, and worse, would look like coverage of a defect that exists. **State the
scope in the guard's own message.**

## Prior failures this is exposed to

1. **A guard that cannot fire.** Ten filters or guards have broken this session. **Demonstrate the
   predicate detects an overlapping layout**, not just that the corpus passes.
2. **Overclaiming scope.** The cross-chunk defect above is the trap.
3. **Pinning a figure corpus growth moves.** Assert the *property*, not a site count.
4. **Reporting a figure without the command that produces it.**
5. **Running the two suites in parallel** — invalidates the perf canary. Sequential.
6. **Confusing "no test fails" with "the property holds".** The corpus passing is evidence about the
   corpus; the guard's value is that a future planner change fails loudly.

## Specific wrong turns to avoid

- **Do not change `plan_chunk_region`.** This increment makes the current behaviour checkable; it
  does not alter it, and it must not quietly implement the reuse it is guarding against.
- **Do not assert distinctness when the property is disjointness.** Distinct offsets with overlapping
  ranges would pass a weaker check and still alias.
- **Do not delete or soften the four prose statements.** Add the enforcement and point the prose at
  it, so the next reader sees the property is now checked rather than merely claimed.
- **Do not treat this as closing the aliasing defect.** It is orthogonal: this guards within-chunk
  reuse, the defect is cross-chunk collision.
