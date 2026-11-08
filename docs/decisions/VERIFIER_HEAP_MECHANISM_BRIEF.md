# BRIEF — re-establish the mechanism, from the source rather than from a ratio

**Written**: 2026-08-27, twelfth loop iteration. **For this line's own use.**

## Why

Last iteration refuted a claim of this line's: that `path-max >= peak_live` holds *by construction*.
The derivation behind it — `peak_live = max_heap_bytes / (demand / sites)` — was invented here and
is not a count of anything. **What survived was a bare numeric agreement across 11 modules with no
established mechanism**, and that is a weaker position than the earlier summary implied.

**The job is to establish the mechanism by READING WHAT THE VERIFIER COMPUTES**, not by inferring it
from a ratio. That is available: `src/verify.rs` is readable here, and it is the authority.

## What reading it already shows

- **`src/verify.rs:992`** — `heap = heap.saturating_add(then_branch.heap_total.max(else_branch.heap_total));`
  with the comment *"Exactly one branch executes"*. **The verifier takes the MAX over branch arms.**
- **`:1016`** — a bare `if` with no `else` adds the then-arm.
- **`:1087`** — a loop adds `body_heap.max(break_heap)`, so a body counts once, not per iteration.
- **`:1863`** — a chunk's heap includes `max_invocations * per_call_wcmu` for its **callees**.
- **`:1774`** — `max_heap_bytes` is the **max over chunks** of that per-chunk figure.

**So the mechanism is real: the verifier models branch exclusivity and the backend does not.** The
11-of-11 agreement was not a coincidence — the path walk implements the verifier's own arm rule.

**And the walk's gap is now identifiable too**: it follows no calls. A chunk whose callee allocates
gets callee heap in the verifier's figure and nothing in the walk's, which is the obvious candidate
for the three `UNDER` modules.

## The measurement

Confirm the account rather than asserting it:

1. **Do the three `UNDER` modules have allocating callees, and do the eleven exceeding modules not?**
   If so, the discrepancy is explained and the agreement's mechanism stands.
2. If some `UNDER` module has no allocating callee, the account is incomplete and that module is
   named.

## Prior failures this is exposed to — and one committed yesterday

1. **Deriving a quantity and then naming it.** This is the failure being repaired. **Do not
   introduce another derived quantity without a check that can prove it impossible.**
2. **Replacing a refuted mechanism with a guessed one.** Last iteration explicitly declined to. This
   increment may only claim what the source states plus what a measurement confirms.
3. **Generalising from the easy half** — the 11 exceeding modules are the easy half by construction.
4. **A vacuous instrument.** Seven filters or guards have broken this session.
5. **Conflating populations.**
6. **Reporting a figure without the command that produces it.**
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not edit `src/verify.rs` or any read-only file.** Reading it is the point.
- **Do not re-assert the by-construction claim.** The walk under-counts where callees allocate; that
  is now known, so the inequality is false in general and must stay marked false.
- **Do not quietly restore the `peak_live` label.** If a quantity is needed, name it for what it is —
  a ratio of two figures on different axes — or do not compute it.
- **Do not treat "the verifier takes max over arms" as proof the backend could.** The verifier walks
  a call graph with invocation counts; whether a region planner can reproduce that is a separate
  question and last iteration's answer was no.
