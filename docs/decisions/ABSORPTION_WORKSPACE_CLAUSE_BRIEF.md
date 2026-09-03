# Brief — my absorption predictions have never mentioned the workspace

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02.**

---

## The goal set

| goal | state |
|---|---|
| **G21** re-run the combination, and fold the check into the absorption routine | **unblocked, and the subject of this brief** |
| `f16` | no oracle — the reference refuses widths 3 and 4 at load |
| publication | held |
| absorption | nothing unabsorbed |

## The gap, which is in my routine rather than in the tree

`UNTESTED_COMBINATION_BRIEF.md` established the shape earlier today: **`src/` and `tests/` are
byte-identical to `origin/v0.2.3`, and the corpus is not.** Six added `.kel` files, one modified, an
updated index, read by six workspace tests. **Their gate pairs their source with their corpus; this
branch pairs their source with a different one.**

That was run and passed. **Then absorption 47 changed seven `src/` files, and the check was not
re-run** — because **every absorption prediction I have written names only `native_codegen` figures.**

| | |
|---|---|
| gate covered | `1d49a102` |
| HEAD | `35757cce` |
| `src/` changed since the gate | **7 files, 191 insertions** (absorption 47) |
| last workspace run | `03e4917f`, before absorption 47 |

**So the combination has silently recurred**, and nothing in my routine says so. Each absorption
leaves workspace coverage stale and the prediction is silent about it.

## This is the build-clause lesson a second time

Absorption 46 taught that a prediction phrased as test counts cannot express a build failure, so the
prediction needs a **build clause checked separately**. **The same defect has a second instance**: a
prediction phrased as `native_codegen` counts cannot express workspace staleness, because the
workspace is not in the population the figure ranges over.

**Not a new species — the same one, in a second place.** A figure that is correct about its own
population, silent about a population it never covered, and read as though it covered both.

## The prediction

**Predicted: the workspace passes on this tree under default features**, and the figure is at or above
the 2720 measured before absorption 47, since that absorption adds tests and removes none.

**Falsifier**: any failure. That would mean this branch breaks something the other line's gate covers
on their corpus — a release blocker rather than a branch curiosity, given the back-merge plan.

**A green result confirms an expectation rather than resolving a doubt**, and the value is the routine
change rather than this run.

## The wrong turns

**1. Do not describe this as a gate run.** It is one configuration of one suite. The gate also runs
formatting, linting, documentation, links and the subprojects.

**2. Do not re-run the whole gate reflexively.** It costs ninety minutes and the marginal question is
narrow: does their new `src/` still pass against this corpus. Answer the narrow question, say it is
narrow.

**3. Report the figure with its population**, since several are available and they differ.

**4. The routine change is the deliverable.** If this run is green and nothing is recorded, the same
gap recurs at absorption 48.
