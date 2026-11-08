# ABSORPTION 77 — three rkyv advisories cleared, and the lock moves under this line

**Computed against** `c569e31918703e91cf962a4d05241e08afc05d54`, with **2 unabsorbed** commits (`1ac2d8d5`, `5c54a263`): three
rkyv advisories cleared, and the dependency scan that had never run.

## Predictions

1. **The merge is clean.** `merge-tree` ran in its own command before this brief.

2. ⚠ **The ownership check is VACUOUS, a fifth time in twelve absorptions.** Zero root `src/`
   or `tests/` files arrive. 69 and 74 and 76 are the only firings.

3. **Reach flips through `docs/decisions` and `docs/process/REVERSE_PROMPT.md`**; no
   `native_codegen/` file arrives.

4. ⚠ **THIS ONE MATTERS IN SUBSTANCE, NOT FORM: `Cargo.lock` MOVES.** This package builds
   against `keleusma`, which depends on rkyv, so a lock change moves **this line's own
   dependency graph** — not merely a document. **The gate is re-run because the code being
   gated may differ**, which is a different reason from every other absorption this session,
   where re-running was about keeping the record current.

5. **The CI premise is re-measured, not assumed**: `ci.yml` changes again, and the premise
   behind the gate-provenance work is zero mentions of `native_codegen`. The job count is
   deliberately not predicted — it was removed from the claim last absorption after rotting
   from 14 to 25 to 27.

## What is NOT claimed

That the advisories affected this backend. Three were cleared upstream; whether any reachable
path here was ever exposed is **not** established by this absorption, and the brief does not
assert it.

## Outcome

**Predictions 1, 2, 3 and 5 held.** Clean merge; the ownership check vacuous a fifth time in
twelve; reach flipped through `docs/decisions` and `REVERSE_PROMPT.md`; and the CI premise
re-measured at **zero** mentions of `native_codegen`.

⚠ **PREDICTION 4 WAS WRONG, AND THE REASON IS STRUCTURAL.** It said the lock change moves this
line's dependency graph, so the gate had to re-run because the gated CODE might differ.
**`native_codegen/Cargo.lock` exists** — a detached workspace root carries its own lock — so the
root's lock change does not reach this package at all. The root diff is 39 lines and adds only
`syn`. The gate re-runs for the ordinary reason, keeping the record current, not because the code
differs.

**THIS IS THE THIRD CONSEQUENCE OF ONE FACT, REDISCOVERED THREE TIMES.** A detached workspace root
inherits nothing:

| consequence | when found |
|---|---|
| no `[profile.dev]`, so full debug info for 155 test binaries | 2026-09-26 |
| no `rust-version`, so no MSRV floor | 2026-09-27 |
| **its own `Cargo.lock`**, so the root's lock changes do not reach it | 2026-09-27, here |

**The reusable form is the fact, not its instances**: *a detached package shares nothing with the
parent workspace unless it declares it.* Ask that question once per absorption that touches
workspace-level configuration, rather than being surprised a fourth time.

**Also not claimed, as the brief said**: whether any reachable path here was exposed by the three
rkyv advisories. Cleared upstream; unexamined here.
