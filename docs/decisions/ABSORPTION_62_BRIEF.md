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

---

## OUTCOME

**Merged cleanly. Backlog 0. Every prediction held — and the gate found something
the absorption did not cause.**

| prediction | result |
|---|---|
| no conflicting files | **held** — three files auto-merged from both sides |
| no backend behaviour change | **held** — 590 tests, 0 failed, both configurations |
| corpus 74 modules, 1 refused | **held** |
| ISA 63 of 66, denominator 66 | **held** |
| test population 563 in 130 files | **held** |
| driven witnesses 62 of 66 | **held** |
| `corpus_fingerprint.rs` silent | **held** |
| `opcode_denominator.rs` silent | **held** |
| `handoff_figures.rs` silent | **held** |
| `shared_channel_discipline.rs` silent — *lowest confidence* | **held**; the addendum moved from line 2001 to 2076 as upstream grew above it, intact and attributed |
| `outstanding_reports.rs` silent | **held** — all five reports still reproduce, so the other line has not ruled on `Fixed % Fixed` |
| `upstream_premise_census.rs`, `comment_citations.rs` silent | **held** |

## ⚠ THE GATE FAILED BEFORE THE SUITE COULD RUN, AND NOT BECAUSE OF THIS MERGE

`cargo clippy --all-targets -- -D warnings` failed on an **unused binding** in
`differential_coverage.rs`. The `WITNESSES_ELSEWHERE` table carries a `driver`
column; the loop destructured it, and the only place the name appeared afterwards
was **inside a comment**, written as `` `{driver}` `` — which reads like an
interpolation and is not one.

**The absorption did not cause it, and that is provable rather than argued**: the
merge changed **zero** files under `native_codegen/`, and the offending line is
byte-identical in `076c42b1`, `2bc0f328`, `7a28fad2` and `e32ea387`.

**`2bc0f328` is the commit the handoff's state table cites** for *"590 tests, 0
failed, BOTH float configurations, every phase FROZEN — ASSEMBLED from six runs."*
The suite figure was true. **The clippy phase was not among the six assembled.**

### The failure class, which is new to this project's catalogue

**A gate run phase by phase, with the verdict assembled by hand, is only as
complete as the assembler's list.** The script `tools/backend-gate.sh` runs four
phases per configuration and reports one PASS or FAIL. Running the phases
separately — which the ten-minute ceiling forces for the long one — moves the
`fail=1` accumulation out of the script and into the operator's memory, and a
phase can then be dropped without anything saying so. **The assembled verdict has
no equivalent of `fail=1`.**

It sits alongside the eight ways already recorded in `CLAUDE.md`, and shares the
common property named there: the run did less than the person reading it believed.

### Repaired by making the field load-bearing

Not by prefixing an underscore. The driver name is now **reported in the
divergence message**, so the column is read by the code rather than only by a
reader. A field carried in a table and never read is a claim nothing checks.

## COST, AND A SECOND OPERATIONAL FINDING

Seven phases. **The narrow-configuration `corpus_differential` phase exceeded the
ten-minute foreground ceiling and was killed**, where the same phase under default
features completes in 427s. Split by test name it runs as 230s for the nine short
tests and 397s for `how_deep_does_the_undetected_set_go` alone.

**The handoff says the long phase must run in the FOREGROUND because background
launches were killed. Under `narrow-float-32` the foreground is not enough
either.** What worked: detach it with `nohup` to a log and wait on the log. Eight
phases, then, not six — and the eighth is a split forced by the configuration
being roughly a fifth slower.

The suite ran with the clippy repair present and uncommitted. **Attribution is
still certain**, by the same zero-files argument above; the frozen-tree check
confirms the content did not move mid-run.
