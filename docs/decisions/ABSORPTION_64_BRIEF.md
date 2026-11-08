# ABSORPTION 64 — the hostile-mutation execution rounds

## Stamp

**Computed against `da6e7fdba6e74f25d060d7a3a01dcb822177dd3a`, this line's HEAD, with
`unabsorbed` = 4.**

The count was **2** when this session's resume summary was written and **4** when
the prediction below was computed, roughly twenty minutes apart. That is the
reason the stamp exists and it is not a hypothetical: absorption 59's brief
predicted zero conflicts from a `merge-tree` run an iteration earlier against a
smaller backlog. **A prediction inherits the tree it was computed against**, so
this one names its tree rather than its hour.

## What is coming in

```
6937765c Merge pull request #450 from sgeos/test/hostile-execution-protocol
081a00fd test(verify): drive accepted mutants to the retained runtime guards
7214d591 Merge pull request #449 from sgeos/test/hostile-descriptor-tables
435b0350 test(verify): reach the descriptor tables, the coroutine paths, and the hot swap
```

Six files, 936 insertions, 35 deletions. The substance is `tests/hostile_module_mutation.rs`,
which grows by roughly 674 lines: the other line is driving accepted bytecode
mutants through to the retained runtime guards rather than stopping at load-time
acceptance.

## Prediction, recorded before the merge

1. **Zero conflicting files.** `git merge-tree --write-tree` against this HEAD
   returned a tree and no conflict report. It touches `docs/process/REVERSE_PROMPT.md`,
   which this line appends to, so a conflict there was the plausible one; the two
   lines edit disjoint regions of it.

2. **Zero movement in every backend figure.** Nothing arriving touches
   `native_codegen/`, `src/`, or the corpus. Suite count, test-file count,
   test-function count, corpus built and refused, and the opcode-lowering census
   should all re-derive **identically**. A move in any of them is a finding about
   this line's instruments, not about the merge.

3. **The ownership check goes empty.** `src/` and `tests/` currently differ from
   `origin/v0.2.3` by one file, `tests/hostile_module_mutation.rs`, entirely
   because that line advanced. After absorbing, the diff should be empty. **This
   is the non-vacuous direction**: it is non-empty now, so an empty result
   afterwards is evidence rather than an artefact of looking.

4. **The record guards stay green**, including `outstanding_reports.rs`, whose
   report six splits `REVERSE_PROMPT.md` on a heading this line wrote. If the
   other line's rewrite removed the appended section, that guard fails and the
   absorption has destroyed this line's outgoing channel — the exact hazard the
   unresolved operator question is about.

## Which guard trips if a prediction is wrong

Prediction 2 is watched by `handoff_figures.rs` and `test_population_guard.rs`.
Prediction 3 is checked by hand and stated here so the check is not optional.
Prediction 4 is watched by the four record guards. **Prediction 1 has no guard**
and is the one to verify by observation.

## Discipline

Measured **alone**. No increment edits land while the absorption run is in flight.
This suite contains tests that read source text from disk, so "edits do not affect
a running suite" is false here, which is why the rule is not merely tidiness.
