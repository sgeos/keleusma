# BRIEF — the cause is structural, and the "24-byte quantum" I reported was a coincidence

## What I published one increment ago, and what is wrong with it

The bound-transfer report gained a magnitude: shortfall 24 to 96 bytes, 504 total, **1.00x to 4.00x**
of the verified figure, and — computed as a greatest common divisor — *"every shortfall is a multiple
of 24 bytes"*. I labelled the divisor a **hint to check, not a cause**, and declined to write the
obvious story: *24 bytes of per-composite overhead the verifier does not model.*

**That restraint was correct, because the obvious story is FALSE.** Measured:

| module | `NewComposite` sites | each | backend | verified |
|---|---|---|---|---|
| `rogue_ai_chaser` | 2 | 24 | 48 | 24 |
| `rogue_ai_boss` | 4 | 24 | 96 | 48 |
| `rogue_player_ai` | 5 | 24 | 120 | 24 |
| `rogue_combat` | 4 | **16** | 64 | 16 |

**The backend figure is EXACTLY the sum of every static composite site's size.** `plan_chunk_region`
gives each `NewComposite` op its own slot and never reuses space; `region_total_bytes` sums those and
recurses through calls. The verified figure is the **peak concurrent liveness**.

So the relation is:

```
backend   = total_sites * size
verified  = peak_live   * size
shortfall = (total_sites - peak_live) * size
```

**And 24 is not a quantum.** `rogue_combat`'s composite is **16 bytes**, and its shortfall of 48 is
`3 x 16`. The greatest common divisor came out 24 only because the `rogue_ai` family dominates the
set and 48 happens to divide by 24. **A gcd is a gcd; it is not evidence of a unit.**

## Why this matters more than a tidier number

The published framing invited "a small fixed overhead a host margin already covers". **The real
relation is unbounded in the number of static composite sites.** A module with fifty sites and one
live at a time demands fifty slots from the backend and one from the verified figure. The 4.00x
observed is a property of this corpus, not a ceiling.

## Prior failures and the specific wrong turns to avoid

- **CORRECT THE DIVISOR CLAIM WHERE IT WAS PUBLISHED.** It is in the report, the resume document's
  state table, and the operator-decision list. A partial correction leaves the older half asserting
  confidently — this file's recorded failure mode, hit twice already.
- **DO NOT CALL THE BACKEND WRONG.** Never reusing region space is a legitimate strategy: it needs no
  liveness analysis and no runtime bookkeeping. The finding is that the two figures measure DIFFERENT
  THINGS, not that one is a defect.
- **DO NOT NOW RECOMMEND A FIX.** "Make the backend reuse slots" and "publish the term" are both
  design decisions with WCET and determinism consequences, and the decision is recorded as the
  operator's. Report the relation; stop.
- **STATE THE RELATION AS MEASURED ON FOUR MODULES, not proven.** It is an exact fit on every module
  checked, which is strong — but the check reads op sites and two published figures, and does not
  read the verifier's liveness computation. Say which.
- **DO NOT DROP THE MAGNITUDE.** The byte figures stay; they are correct. What changes is the
  interpretation attached to them.

## What a good outcome looks like

The report states the structural relation and shows the per-site arithmetic that supports it; the
divisor is retained but re-labelled as a coincidence of this corpus rather than a unit; every place
the old reading was published is corrected; and the observation that the gap grows with static site
count — rather than sitting at a fixed overhead — is stated plainly. **No recommendation, and the
decision stays the operator's.**
