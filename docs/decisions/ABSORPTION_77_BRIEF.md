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
