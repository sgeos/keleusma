# BRIEF — make the Workstream E gap CLOSEABLE from this line's own surface

## The recommendation, and why it is not the obvious one

The previous increment established that `auto_arena_capacity_for` sums four terms and **none is the
backend's composite region**, so a host doing exactly what the documentation says provisions nothing
for it. I recorded that as needing an operator ruling because closing it "means adding a term to
`auto_arena_capacity_for`", which lives in read-only `src/vm.rs`.

**That framing was too narrow, and it is worth saying so plainly.** Adding a term to the runtime's
sizing function is ONE way to close the gap and it does need a ruling. **But the backend knows its
own demand**, and a host can add that figure to whatever the runtime told it. **That half is
entirely on this line's surface and needs nobody.**

So: expose the missing term as a host-facing figure, document it as the supplement, and prove the
figure is sound. The operator's decision then becomes the narrower and better-posed one — *should
the runtime absorb this term* — rather than *is there a gap at all*.

## What "sound" has to mean here, or this is cosmetic

`region_total_bytes` already exists. A wrapper around it that changes only the argument list is
**decoration**, and this line has a standing rule against work whose payoff is tidying.

The content is the **soundness property**, which nobody has checked:

> The entry-rooted total must dominate every individual chunk reachable from the entry.

If the recursion under-counts anywhere — a callee whose own plan exceeds what the walk attributed to
it — a host sizing from the total would under-provision, and the backend writes at compile-time
offsets into that memory. **That is the check worth having.** The API is the delivery vehicle.

## Prior failures on this line, and the specific wrong turns

- **Do not write a second region planner.** The figure must come from `plan_chunk_region` and
  `region_total_bytes`, the ones the lowering itself uses. A parallel model measures the model.
  This line already resolved a 1032-vs-1027 discrepancy by restricting an EXISTING walker rather
  than writing a forbidden third one.
- **Do not assert something trivially true.** `supplement(m) == region_total_bytes(m, entry, 0)` is
  a tautology if the former just calls the latter. The non-tautological claims are: the figure is
  NON-ZERO for modules that build composites, and it DOMINATES every reachable chunk's own plan.
- **Do not conclude soundness from a green run over a corpus where every demand is zero.** Twenty-nine
  of sixty-three modules have a non-zero demand; assert that the dominating comparison actually
  reached some of them, and print the count.
- **Do not claim the host is now safe.** A figure a host must remember to add is weaker than one the
  runtime returns. Say which this is.
- **Scope, not just units.** The previous increment compared a transitively-folded verifier figure
  against a per-function ceiling and reported a violation that did not exist. Every comparison here
  must state what is rooted where.
- **Do not touch `src/vm.rs`, `src/verify.rs`, `src/bytecode.rs`, `src/selfhost/`, or
  `.github/workflows/`.** All read-only, and `verify.rs` additionally has an unresolved ownership
  question between the two lines.

## What a good outcome looks like

A host can compute the missing figure today, from this crate, without any runtime change; the
figure is shown to dominate what the lowering actually plans; and the operator's open item shrinks
from "there is an unaccounted pool" to "should the runtime absorb a term the backend already
publishes".

**If the dominance check FAILS, that is the better outcome** — it would mean the existing recursion
under-counts, which is a live defect in this line's own code rather than a documentation gap.
