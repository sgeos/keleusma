# BRIEF — absorption 59

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12, before the merge.

## The prediction, and its risk, checked against each other

7 commits. **Measured ALONE.**

| clause | prediction |
|---|---|
| conflicting files | **zero** — `merge-tree` computes a clean tree |
| conflicts in `src/` or `tests/` | zero |
| `src/` and `tests/` after the merge | byte-identical to `origin/v0.2.3` |
| backend suite | **545 passed, 0 failed** (535 + 10), both float configurations |

## THE RISK I EXPECTED, AND WHY IT DOES NOT APPLY

The incoming set touches `src/selfhost_host.rs` — described in `CLAUDE.md` as *"the shared-slot
layouts the stages are seeded through"*. **This line's `stage_differential.rs` seeds every stage
through those layouts**, so a layout change would move what my harness writes and where.

**Read before predicting: the change is a panic message and its comment.** No layout, no offset, no
type. The stage records two measured causes for a step-budget exhaustion instead of one, because the
old message named a "usual cause" nobody had measured.

**So the green prediction is consistent with the risk rather than contradicting it** — which is the
specific failure absorption 57's brief committed, predicting a clean suite in the same document that
named a corpus risk the fingerprint guard must fail on.

The other changed file is `tests/selfhost_typecheck.rs`, which belongs to the other line and which
this line does not run.

## The wrong turns

1. **Do not skip measuring it alone.** The one absorption not measured alone is recorded as a failure
   whose attribution had to be argued rather than read.
2. **Do not treat "only a message changed" as "nothing changed".** The ownership check still runs, and
   the suite still runs; the claim is that the risk is dismissed by reading, not that verification is
   unnecessary.
3. **Do not let a dismissed risk go unrecorded.** A risk named and then measured away is evidence that
   the naming was done; a risk silently dropped is indistinguishable from one never considered.

## What done looks like

The merge is in, every clause has an outcome recorded beside it, the ownership boundary is verified
non-vacuously, and the dismissal of the named risk is recorded with what was read to dismiss it.
