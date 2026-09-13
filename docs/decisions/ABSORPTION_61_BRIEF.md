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
