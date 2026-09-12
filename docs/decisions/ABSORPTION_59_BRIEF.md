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

---

## ⚠ THE CONFLICT CLAUSE WAS FALSIFIED BEFORE THE MERGE RAN

**Predicted zero conflicting files; `merge-tree` computes ONE — `docs/process/REVERSE_PROMPT.md`.**

Recorded here rather than corrected in place, because the prediction is the artefact and editing it to
match the outcome would destroy the only thing it was for.

**The cause is this line's own activity, not a surprise about theirs.** The clean tree was computed
one iteration earlier, when the backlog was four commits and neither line had touched that file since
absorption 57. Both lines have rewritten it since — this line at the end of the report-guard
increment, and the other line in the seven commits now incoming.

**The prediction was stale, not mistaken**, which is a distinction without much comfort: it was
carried forward from a measurement taken against a different backlog, and the current one was printed
by the same command that wrote the brief. **A prediction is only as current as the tree it was
computed against**, and nothing marked it with the backlog it belonged to.

The other three clauses stand as written and are measured below.

---

## OUTCOME — 2026-09-12

| clause | predicted | measured |
|---|---|---|
| conflicting files | zero | **one** — `REVERSE_PROMPT.md`, and the miss was recorded BEFORE the merge ran |
| conflicts in `src/` or `tests/` | zero | **zero** |
| `src/` and `tests/` after the merge | byte-identical to `origin/v0.2.3` | **identical**, the check returning two files against the previous absorption's tree |
| backend suite | 545 passed, 0 failed | **545** (535 + 10), both float configurations, every half FROZEN |

**Three of four hit. The one that missed was stale rather than wrong**, and the difference is worth
keeping: the `merge-tree` that produced it was run one iteration earlier against a four-commit
backlog, and both lines rewrote the conflicting file in between.

> **A prediction inherits the tree it was computed against, and nothing here stamped it with one.**
> That is the same class as every other staleness this session found — a figure separated from the
> measurement that produced it — arriving this time inside the discipline meant to catch it.

### The risk was dismissed by reading, and that is the part worth repeating

`src/selfhost_host.rs` holds the shared-slot layouts this line's stage differential seeds through, so
a change there could move what the harness writes and where. **The diff is a panic message and its
comment** — the stage now records two measured causes for a step-budget exhaustion where the old text
named one "usual cause" nobody had measured.

Dismissing it required reading five lines. **Predicting first and reading afterwards is how absorption
57 produced a prediction its own risk made impossible.**
