# ABSORPTION 68 — the run-tasks scheduler behaviour tests

## Stamp

**Computed against `1a7baf43de8c45c6c0a61ea1675a243db75c7888`, this line's HEAD,
with `unabsorbed` = 2.**

## What is coming in

```
2df884aa Merge pull request #456 from sgeos/test/runtasks-scheduler-behaviour
04d5f52a test(cli): guard the run-tasks scheduler behaviours that had never executed
```

Six files. A new `keleusma-cli/tests/runtasks_scheduler.rs`, plus `CHANGELOG.md`,
`docs/architecture/RUN_TASKS.md` and the three process documents.

## Prediction, recorded before the merge

1. **Zero conflicting files.** `merge-tree --write-tree` returned a tree and no
   conflict report. It touches `REVERSE_PROMPT.md`, which this line appends to, so
   that is the plausible conflict; the two lines edit disjoint regions.

2. **Zero movement in every backend figure.** Nothing arriving touches
   `native_codegen/`, `src/`, or the corpus — it is `keleusma-cli/` and
   documentation. All figure guards must re-derive identically.

3. ⚠ **THE OWNERSHIP CHECK IS VACUOUS FOR THE THIRD CONSECUTIVE ABSORPTION.**
   `src/` and `tests/` are **already** byte-identical to `origin/v0.2.3` before the
   merge. An empty result afterwards proves nothing, and recording it as a hit three
   times running would manufacture three pieces of evidence from none. It was
   non-vacuous at absorption 65 — two differing files went to zero — and that is
   what made it evidence there.

4. **`outstanding_reports.rs` stays green.** It splits `REVERSE_PROMPT.md` on a
   heading this line wrote, and the other line has now modified that file in four
   consecutive absorptions. It also watches the corpus arena figures published
   2026-09-23.

## Which guard trips if a prediction is wrong

Prediction 2 is watched by `handoff_figures.rs` and `test_population_guard.rs`.
Prediction 4 by `outstanding_reports.rs`. **Prediction 1 has no guard** and is
verified by observing the merge. **Prediction 3 has no guard and cannot be
verified**, which is the point of stating it.

## Discipline

Measured **alone**, and nothing else touches the tree while its guards run.

## Outcome, recorded after the merge

**Every prediction held, and the vacuous one stayed vacuous.**

1. **Zero conflicting files.** Six files, 362 insertions, one deletion.
2. **Zero movement in every backend figure**; both figure guards pass unchanged.
3. **The ownership check read empty before and after**, establishing nothing — as
   predicted, for the third consecutive absorption. Recorded as an ABSENT check.
4. **`outstanding_reports.rs` stayed green.** This line's appended section and its
   report six survive a FOURTH consecutive rewrite of `REVERSE_PROMPT.md` by the
   other line — still by disjoint editing rather than by any mechanism.

Backlog zero.

## A note on the vacuous check, now that it has repeated three times

Three absorptions running, the ownership check could not fail, because the other
line has been working in `keleusma-cli/` and documentation. **The honest reading is
that this line's isolation from `src/` and `tests/` is currently untested, not
confirmed.** The check becomes evidence again the moment either line touches those
directories; until then it is a standing assertion with no observation behind it.
