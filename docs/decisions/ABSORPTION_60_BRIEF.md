# BRIEF — absorption 60

## Stamp

**Computed against `4d5db105` (ours) and `f54594ce` (`origin/v0.2.3`), with
46 unabsorbed commits.**

That stamp is the rule added after absorption 59, whose brief predicted zero
conflicting files while `merge-tree` had computed one — the prediction was
**stale, not mistaken**, having been computed an iteration earlier against a
smaller backlog. A prediction inherits the tree it was computed against.
`prediction_stamp.rs` fails if the newest brief carries neither figure.

## What the backlog contains

46 commits touching **two `src/` files**, both in the self-hosted compiler:
`src/selfhost/mod.rs` (+328) and `src/selfhost/kel/parse.kel` (+37). Their
subjects are a self-hosted opcode matrix, a float-divergence cause, and a run of
process-document currency work.

**`src/bytecode.rs`, `src/vm.rs`, `src/compiler.rs` and `src/value_layout.rs` are
untouched.** `native_codegen/` and `examples/` are untouched.

## Prediction

1. **One conflicting file: `docs/process/REVERSE_PROMPT.md`.** Both lines wrote to
   it — this line filed reports 4 and 5, the other rewrote its own state. Resolve
   by keeping both sides' content, since it is a channel and not a shared
   artifact.
2. **No backend behaviour change.** The backend reads `bytecode.rs`,
   `value_layout.rs` and the module structures; none moved. The gate's failures,
   if any, will be in instruments that read the corpus, not in lowering.
3. **Corpus count unchanged at 74 modules, 1 refused.** No `.kel` file is added or
   removed; one is edited.
4. **ISA classification unchanged at 63 of 66.** This is the prediction most
   likely to be wrong, and deliberately stated: `parse.kel` is IN the corpus, and
   a stage source gaining a construct could emit an opcode currently classified
   `NotEmitted` — `Len` or `IsStruct`. If it does, `opcode_denominator.rs` fails
   and the reclassification is the finding.
5. **Test population unchanged**, `native_codegen/` being untouched.
6. **The opcode denominator parses `src/bytecode.rs` at test time.** Untouched, so
   the denominator stays 66. If that moves, the parse floor or the stale-row check
   fires.

## Discipline

**Measure alone.** No edits while the run is in flight. Absorption 40 was measured
with edits landing mid-run and this suite contains tests that READ SOURCE TEXT FROM
DISK, so "edits do not affect a running suite" is false here. The attribution was
argued rather than certain, and the whole point of the discipline is that an
attribution never has to be argued.

## Wrong turns to avoid

- **Resolving the channel conflict by taking one side.** It is two lines' outgoing
  notes in one file; dropping either loses a report or an answer.
- **Reading a figure move as a defect.** A corpus change legitimately moves corpus
  figures. The guards exist to force the change to be SEEN and classified, not to
  assert the old numbers were right.
- **Patching a guard's constant to make the gate green.** If the ISA
  classification moves, the move is the result; record what newly emits the
  opcode.
