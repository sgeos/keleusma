# BRIEF — absorption 62

## Stamp

**Computed against `92cf5303` (ours) and `a362d349` (`origin/v0.2.3`), with
18 unabsorbed commits.**

The stamp exists because absorption 59's prediction was **stale rather than
mistaken**: its `merge-tree` had run an iteration earlier against a smaller
backlog, and both lines had rewritten the conflicting file in between. A
prediction inherits the tree it was computed against, so the tree is named.

> ⚠ **A MEASUREMENT SLIP CAUGHT WHILE COMPUTING THIS BRIEF, RECORDED BECAUSE IT
> IS THIS PROJECT'S OWN RECURRING CLASS.** The first conflict probe was written
> as `git merge-tree ... | head -20; echo "exit=$?"`. **`$?` there is `head`'s
> status, not `merge-tree`'s**, so the printed `exit=0` was evidence about the
> pager. Re-run with the status captured from the command itself: still clean.
> The verdict did not change; the evidence for it did.

## What the backlog contains

Eighteen commits, and **the shape is unusually narrow**:

| path | what changed |
|---|---|
| `compiler/src/main.rs` | a stale count dropped from the subproject banner |
| `tests/claimed_counts.rs` | upstream's own claimed-figure guard |
| `tests/selfhost_typecheck.rs` | a seventh reduction recorded as unavailable |
| `docs/process/` | `DESIGN_JOURNAL`, `HANDOFF`, `REVERSE_PROMPT`, `TASKLOG` |

**`src/` IS ENTIRELY UNTOUCHED. No `.kel` file changed. `native_codegen/` is
untouched.** That is the whole basis for the behaviour predictions below, and it
is checkable: the name-status listing above is the complete set.

One upstream commit deserves naming — `1244bf45`, *"guard the instruction-set
spec's opcode count against the source"*. **That is the other line independently
building the same instrument as this line's `opcode_denominator.rs`**, against the
same source of truth. It does not change the count; it pins it. Worth knowing
that both lines now hold an opcode-count pin, because a future divergence between
them is a real signal rather than a duplicate.

## Prediction — FIGURES

1. **No conflicting files.** `merge-tree` computes a clean tree
   (`e3c36b3210bb650945929d8ae6797d90b5a2294e`) at this stamp. **This is the
   sharper claim than it looks**: `DESIGN_JOURNAL.md`, `REVERSE_PROMPT.md` and
   `TASKLOG.md` are modified on BOTH sides, and absorption 60 conflicted on the
   third of those. The append-rather-than-overwrite resolution shape is what is
   being tested.
2. **No backend behaviour change**, because nothing this backend compiles against
   has moved.
3. **Corpus unchanged at 74 modules, 1 refused** — no `.kel` file changed.
4. **ISA unchanged at 63 of 66**, and the denominator still parses 66 from
   `src/bytecode.rs`, which is untouched.
5. **Test population unchanged at 563 functions in 130 files** —
   `native_codegen/` is untouched.
6. **Driven witnesses unchanged at 62 of 66.**

## Prediction — WHICH GUARDS TRIP

**None of them.** Named individually so a silent one is still a scored
prediction:

- **`corpus_fingerprint.rs` — silent.** No `.kel` changed, so no content hash
  moves. This was the guard absorption 60's figures-only brief missed.
- **`opcode_denominator.rs` — silent.** It parses `src/bytecode.rs`; untouched.
- **`handoff_figures.rs` — silent.** No row in the state table is predicted to
  move except `absorption`, which I will update by hand as part of the increment;
  that is an edit, not a trip.
- **`shared_channel_discipline.rs` — AT RISK, AND THE ONE TO WATCH.** It holds the
  shape of this line's addendum in `REVERSE_PROMPT.md`, and upstream rewrote that
  file. A clean textual merge does not by itself guarantee the addendum survived
  intact and attributed. **Predicted silent, with the lowest confidence of the
  five.**
- **`outstanding_reports.rs` — silent.** It re-runs five reproductions against the
  reference; `src/` is untouched, so all five must still reproduce. **If report 4
  stops reproducing, the other line has ruled on `Fixed % Fixed` and the correct
  response is to RETRACT, not to debug.** That is the guard's own instruction.
- **`upstream_premise_census.rs` and `comment_citations.rs` — silent.** Both scan
  `native_codegen/` only.

If any of these fires, the prediction was wrong and the reason belongs in the
outcome, not in a patched constant.

## Discipline

**Measure alone.** No edits while a run is in flight. Tests here READ SOURCE TEXT
FROM DISK, so "edits cannot affect a running suite" is false in this package, and
absorption 40's attribution had to be argued instead of being certain.

**The long phase needs the FOREGROUND.** Three consecutive background launches
were killed at 240-360s against a ~415s runtime.

## Wrong turns to avoid

- **Reading a killed phase as a pass.** It never reaches the frozen-tree check.
- **Treating a clean `merge-tree` as the end of the work.** The merge is the cheap
  part; the verdict is the measurement.
- **Skipping the gate because the backlog is docs-heavy.** The prediction that
  nothing moves is worth exactly as much as the run that confirms it, and
  "obviously inert" is the premise this file's own history keeps falsifying.
- **Reading a pipeline's exit status as the command's.** See the stamp note.
