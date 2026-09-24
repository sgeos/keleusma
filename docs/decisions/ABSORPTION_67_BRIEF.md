# ABSORPTION 67 — the CLI success-path tests

## Stamp

**Computed against `de2eef75db5e0c2442ec12390e04c6ef59e4448b`, this line's HEAD,
with `unabsorbed` = 2.**

## What is coming in

```
72eeae7f Merge pull request #455 from sgeos/test/cli-success-paths
07e860a7 test(cli): pin the success paths a coverage census found unexercised
```

Five files. The substance is a new `keleusma-cli/tests/strip_and_version.rs`, plus
`CHANGELOG.md` and the three process documents.

## Prediction, recorded before the merge

1. **Zero conflicting files.** `git merge-tree --write-tree` returned a tree and no
   conflict report. It touches `docs/process/REVERSE_PROMPT.md`, which this line
   appends to, so that is the plausible conflict; the two lines edit disjoint
   regions of it.

2. **Zero movement in every backend figure.** Nothing arriving touches
   `native_codegen/`, `src/`, or the corpus — it is confined to `keleusma-cli/` and
   documentation. Suite count, test-file count, test-function count, corpus built
   and refused, and the opcode-lowering census must all re-derive **identically**.

3. ⚠ **THE OWNERSHIP CHECK IS VACUOUS AGAIN, AND IT IS STATED RATHER THAN
   COLLECTED.** `src/` and `tests/` are **already** byte-identical to
   `origin/v0.2.3` before the merge, because the incoming commits touch neither. An
   empty result afterwards proves nothing about this absorption.
   **This is the second consecutive absorption where that check cannot fail**, and
   recording it as a hit both times would have manufactured two pieces of evidence
   from none. At absorption 65 it was non-vacuous — two differing files went to zero
   — and that is what made it evidence there.

4. **`outstanding_reports.rs` stays green.** It splits `REVERSE_PROMPT.md` on a
   heading this line wrote, and the other line modified that file again — the third
   consecutive time. It also watches the corpus arena figures published 2026-09-23,
   so a drift in either the channel or those numbers fails here.

## Which guard trips if a prediction is wrong

Prediction 2 is watched by `handoff_figures.rs` and `test_population_guard.rs`.
Prediction 4 is watched by `outstanding_reports.rs`. **Prediction 1 has no guard**
and is verified by observing the merge. **Prediction 3 has no guard and cannot be
verified**, which is the point of stating it.

## Discipline

Measured **alone**. This suite contains tests that read source text from disk.
