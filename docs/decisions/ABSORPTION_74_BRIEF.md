# ABSORPTION 74 — the one that tests three recorded claims at once

**Computed against** `a4dc94e617963144e2b3f662634db8b4168e17db`, with **2 unabsorbed** commits on `origin/v0.2.3`
(`489fac4e`, `15923e9a`): the book's programs are now executed, with the check attached to
the branch. Figures derived at that tree.

## Predictions

1. **The merge is clean.** `merge-tree` was run in ITS OWN COMMAND before this brief —
   the absorption-72 correction, applied for the second time.

2. ✅ **THE OWNERSHIP CHECK FIRES, for the second time in nine absorptions.** Root `src/`
   and `tests/` move — `src/typecheck.rs` and `tests/book_example_coverage.rs` — **by
   their hand**. This line's commits still touch nothing there. Absorption 69 was the only
   prior firing, and 70 through 73 were vacuous, so this is evidence again rather than an
   absent check reading like a pass.

3. **Reach flips, and  is among the paths named.** This is the **first absorption to
   touch root ** since the reach set was widened to include it. ⚠ **State the reason
   honestly when it fires**:  is watched BROADLY, so the flag is correct by breadth,
   **not** because any test reads . The specific file this line's tests read is
   , which is untouched here.

4. ⚠ ** CHANGED, so the claim "continuous integration never builds
   this package" must be RE-MEASURED, not assumed.** It was recorded as zero mentions across
   fourteen jobs and is the load-bearing premise of the whole gate-provenance increment. A
   changed CI file is exactly the event that could falsify it.

5. ⚠ ** is the reference type checker, so the CORPUS POPULATION may move.**
   The differential compiles every corpus program with it and silently skips what does not
   compile. A typecheck change can therefore alter the 74-module count, the ISA census
   denominator, and the loader floor's headroom. **Re-derive the corpus figures after
   absorbing rather than trusting the state table.**

## Why this one is worth care

Three recorded claims are in its path at once: an ownership boundary, a CI premise, and a
corpus population. Two of the three are figures this line asserts in its state table.
