# BRIEF — absorption 61

## Stamp

**Computed against `89b70dc6` (ours) and `be009a05` (`origin/v0.2.3`), with
15 unabsorbed commits.**

## What the backlog contains

Fifteen commits. **One `src/` file: `src/selfhost/mod.rs`** — the self-hosted
compile driver. Five upstream test files: `claimed_counts.rs`,
`float_opcode_without_floats.rs`, `selfhost_parse.rs`,
`selfhost_repeat_compile.rs`, `selfhost_typecheck.rs`.

**`src/bytecode.rs`, `src/vm.rs`, `src/compiler.rs` and `src/value_layout.rs` are
untouched. No `.kel` file changed. `native_codegen/` and `examples/` are
untouched.**

## Prediction — FIGURES

1. **No conflicting files.** `merge-tree` reports none. Absorption 60 conflicted
   on `REVERSE_PROMPT.md`; this line has not written to it since, and the
   resolution shape from then is guarded.
2. **No backend behaviour change.** Nothing this backend reads has moved.
3. **Corpus unchanged at 74 modules, 1 refused.**
4. **ISA unchanged at 63 of 66**, and the denominator still parses 66 opcodes
   from `src/bytecode.rs`, which is untouched.
5. **Test population unchanged** — `native_codegen/` is untouched.
6. **Driven witnesses unchanged at 60 of 66.**

## Prediction — WHICH GUARDS TRIP

**This section exists because absorption 60's brief did not have it.** That brief
predicted six figures, got all six right, and still missed
`corpus_fingerprint.rs`, which pins corpus CONTENT by hash rather than counts.
Predicting the figures is not predicting which guards a change trips.

- **`corpus_fingerprint.rs` will NOT fire.** No `.kel` file changed, so no hash
  moves. This is the sharpest prediction here, and the one most directly informed
  by the last absorption's miss.
- **`opcode_denominator.rs` will not fire** — it parses `src/bytecode.rs`, which
  is untouched.
- **`shared_channel_discipline.rs` will not fire** — this line's addendum is
  untouched and the other line's edits cannot fail it by construction.
- **`comment_citations.rs` and `upstream_premise_census.rs` scan
  `native_codegen/` only**, which is untouched.
- **`handoff_figures.rs` will not fire** unless a figure moves, and none is
  predicted to.

If any of these fires, the prediction was wrong and the reason goes in the
outcome, not into a patched constant.

## Discipline

**Measure alone.** No edits while a run is in flight. This suite contains tests
that READ SOURCE TEXT FROM DISK, so "edits do not affect a running suite" is false
here, and absorption 40's attribution had to be argued rather than being certain.

**The long phase needs the FOREGROUND.** Three background launches were killed at
240-360s against a ~415s runtime; the foreground completed it.

## Wrong turns to avoid

- **Reading a killed phase as a pass.** It never reaches the frozen-tree check.
- **Patching a guard's constant to make the gate green.** A figure that moves is
  the result; record what moved it.
- **Assuming no conflict means nothing to check.** The merge is the cheap part;
  the verdict is the measurement.

---

## OUTCOME

**Merged cleanly. Backlog 0. Every prediction held, figures and guards alike.**

| prediction | result |
|---|---|
| no conflicting files | **held** — `merge-tree` was right |
| no backend behaviour change | **held** — 587 tests, 0 failed |
| corpus 74 modules, 1 refused | **held** |
| ISA 63 of 66, denominator 66 | **held** |
| test population unchanged | **held** |
| driven witnesses 60 of 66 | **held** |
| **`corpus_fingerprint.rs` does NOT fire** | **held** — no `.kel` changed |
| the other four guards do not fire | **held** |

**The guard prediction is the part worth keeping.** Absorption 60's brief
predicted six figures, got all six right, and still missed the content-hash pin
firing. This brief predicted the guards explicitly and named the fingerprint as
the sharpest case. It stayed silent, as predicted, because no `.kel` moved.

## A FIFTH `LEAK`, AND THE FIRST REPEAT

`a_trapping_programs_native_side_dies_with_sigtrap` leaked again — the same test
as the fourth. That **sharpens** the earlier statement rather than confirming it:
the honest shape is **one test that leaks somewhat repeatably with an obvious
mechanism** (its purpose is to make a process die with `SIGTRAP`), **plus sporadic
one-offs elsewhere**. Not the whole-binary cause first guessed, and not a
shapeless scatter. Still untraced.

This is the second time the leak record has been revised as data accumulated,
which is what it was built for.

## COST

Six phases, both configurations, **all in the FOREGROUND**. Every phase completed
first time — against three consecutive background kills of the long phase in the
previous increment. Two extra phases were re-run after a comment-only edit to a
module header, because two censuses in this package READ SOURCE TEXT and a comment
is therefore not inert here.
