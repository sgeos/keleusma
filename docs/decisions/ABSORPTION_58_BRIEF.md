# BRIEF — absorption 58

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11, before the merge.

## The prediction, recorded before the merge

11 commits. **Measured ALONE.**

| clause | prediction |
|---|---|
| conflicting files | **zero** — `merge-tree` computes a clean tree |
| conflicts in `src/` or `tests/` | zero |
| `src/` and `tests/` after the merge | byte-identical to `origin/v0.2.3` |
| corpus fingerprint | **stays green** |
| backend suite | **538 passed, 0 failed** (528 + 10), both float configurations |

## THE RISK, AND WHY IT IS SMALL — stated so the prediction cannot contradict it

**Absorption 57's brief predicted a green suite in the same document that named a corpus risk the
fingerprint guard must fail on.** If the risk had been real the prediction was impossible, and the
guard caught what the brief should have.

So this time the two are checked against each other. The changed set is three files:
`docs/process/DESIGN_JOURNAL.md`, `docs/process/GIT_STRATEGY.md`, and `tests/selfhost_typecheck.rs`.
**Not one is a corpus subject** — the corpus roots are `examples/scripts`, `src/selfhost/kel`,
`examples/rtos/scripts` and `compiler/kel`, and the incoming set touches none of them. **Zero `.kel`
files change.**

Therefore the fingerprint cannot fire, and predicting a green suite is consistent rather than
contradictory. **If it fires anyway, the premise above is what is wrong**, not the guard.

The incoming work is a self-hosted-compiler performance line — eliding inert expression-table rows and
de-duplicating occurrence facts. That is upstream of the bytecode this backend consumes only through
the `.kel` stage sources, which are unchanged here.

## The wrong turns

1. **Do not skip measuring it alone** because it looks small. The one absorption that was not measured
   alone is recorded as a failure, and its attribution had to be argued rather than read.
2. **Do not treat "docs only plus one test" as "no risk".** `tests/selfhost_typecheck.rs` is the other
   line's file and this line does not run it, but the ownership check must still be run rather than
   assumed.
3. **Do not report a green suite as the clearance.** The clearance is that the corpus fingerprint is
   unchanged and the ownership check is empty, both checked.

## What done looks like

The merge is in, every clause has an outcome recorded beside it, the ownership boundary is verified
rather than asserted, and any clause that missed is named.

---

## OUTCOME — 2026-09-11

| clause | predicted | measured |
|---|---|---|
| conflicting files | zero | **zero**, and `merge-tree` computed a clean tree in advance |
| conflicts in `src/` or `tests/` | zero | **zero** |
| `src/` and `tests/` after the merge | byte-identical to `origin/v0.2.3` | **identical**, and the check returns one file against the previous absorption's tree, so it is not vacuous |
| corpus fingerprint | stays green | **green** |
| backend suite | 538 passed, 0 failed | **538** (528 + 10), both float configurations, every half FROZEN |

**Every clause hit, and the fourth one hit for a stated reason rather than by luck.** The corpus
claim was established from the incoming file set — three files, none a `.kel` under any corpus root —
before the suite ran. That is the difference this brief was written to make: absorption 57 predicted a
green suite while naming a risk that made a green suite impossible, and the guard caught what the
brief should have.

> **A prediction is worth something only when its clauses are checked against each other.** Nothing
> automated does that; it is a thing a reader has to do, and this brief is where it was done.
