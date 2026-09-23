# ABSORPTION 65 — the fault census, the CLI input tests, and the spec trap claims

## Stamp

**Computed against `0f8f795a` and RE-DERIVED against `6f3fc5e4`, this line's HEAD after the push, with
`unabsorbed` = 9.**

**Re-derived rather than trusted.** The figures were first computed at `0f8f795a`; by
the time the brief was filed HEAD had moved to `6f3fc5e4` through the push of eight
commits. Re-running `merge-tree` gave the same answer — still nine unabsorbed, still
no conflict — and the ownership diff is still two lines. **A prediction that
re-derives unchanged still gets its stamp updated**, because that is the only
evidence anyone looked.

The backlog was **0** when this line last absorbed and **9** now: the machine's
linker was broken for five days and the other line kept working through it. **A
prediction inherits the tree it was computed against**, which is why the stamp
names one rather than a date.

## What is coming in

```
6ed00165 docs(process): record that the blocker is cleared and everything is locally verified
8d30f2e9 Merge pull request #453 from sgeos/test/spec-trap-claims
8f68c77c test(spec): guard the trap-or-reify claims against the implementation
a37759be Merge pull request #452 from sgeos/test/cli-bad-input
dfc07557 docs(book): resync the generated chapter, and correct a second wrong mechanism
0cffdffc docs(spec): correct CheckedMod, and record that workstream C is lopsided
9a21d79c test(cli): exercise the shipping binary against bad input
cecba960 Merge pull request #451 from sgeos/feat/runtime-fault-census
2a7f68f7 test(verify): size workstream C by measuring which faults occur with no Trap
```

Twelve files. Three new test files — `tests/runtime_fault_census.rs`,
`tests/spec_trap_claims.rs` and `keleusma-cli/tests/bad_input.rs` — plus
documentation, the book, the instruction-set spec and `CHANGELOG.md`.

**`6ed00165` is independent corroboration of this line's blocker diagnosis.** The
other line hit the same broken linker and recorded it cleared, so "machine state,
not a defect in the tree" was not this line's inference alone.

## Prediction, recorded before the merge

1. **Zero conflicting files.** `git merge-tree --write-tree` returned a tree and no
   conflict report. It touches `docs/process/REVERSE_PROMPT.md`, which this line
   appends to, so that was the plausible conflict; the two lines edit disjoint
   regions of it.

2. **Zero movement in every backend figure.** Nothing arriving touches
   `native_codegen/`, `src/`, or the corpus. Suite count, test-file count,
   test-function count, corpus built and refused, and the opcode-lowering census
   must all re-derive **identically**. A move in any of them is a finding about this
   line's instruments, not about the merge.

3. **The ownership check goes empty.** `src/` and `tests/` currently differ from
   `origin/v0.2.3` by the three test files that line added. After absorbing, the
   diff should be empty. **Non-vacuous in the right direction**: it is non-empty
   now, so an empty result afterwards is evidence rather than an artefact.

4. **`outstanding_reports.rs` stays green, and it is the one at real risk.** It
   splits `REVERSE_PROMPT.md` on a heading THIS line wrote, and the other line
   modified that file across nine commits. If the appended section is gone, the
   guard fails and the absorption has destroyed this line's outgoing channel —
   exactly the hazard the unresolved operator question concerns.

## Which guard trips if a prediction is wrong

Prediction 2 is watched by `handoff_figures.rs` and `test_population_guard.rs`.
Prediction 3 is checked by hand and stated here so the check is not optional.
Prediction 4 is watched by `outstanding_reports.rs`. **Prediction 1 has no guard**
and is verified by observing the merge.

## Discipline

Measured **alone**, after the seven stacked commits are verified and pushed. This
suite contains tests that read source text from disk, so "edits do not affect a
running suite" is false here — and absorbing nine commits on top of six unverified
ones would make any failure's attribution an argument rather than an observation.
