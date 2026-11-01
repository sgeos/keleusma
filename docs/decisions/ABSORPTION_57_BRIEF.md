# BRIEF — absorption 57, and the width-discipline census

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11, before the merge.

## Part one: absorption 57

25 commits from `origin/v0.2.3`. **Measured ALONE**, with the prediction below committed first.

### The prediction, recorded before the merge

| clause | prediction |
|---|---|
| conflicting files | **exactly two** — `docs/process/REVERSE_PROMPT.md` and `docs/process/TASKLOG.md` |
| conflicts in `src/` or `tests/` | **zero** |
| `src/` and `tests/` after the merge | **byte-identical** to `origin/v0.2.3` |
| backend suite | **519 passed, 0 failed** (509 + 10), both float configurations |

### THE NAMED RISK, and it is not the usual one

Previous absorptions named a risk in `src/`. **This one's risk is in my CORPUS.** The incoming set
touches `src/selfhost/kel/wire.kel` and `src/selfhost/kel/verify_types.kel` — **and those files ARE
corpus modules of this backend**, swept by the censuses, the differential, and the mutation tables.

So the plausible breaker is not a runtime behaviour change but a **corpus population change**: a new
construct in either file can move the site counts, the chunk counts, the refusal set, or the coverage
figures, and several of those are pinned to exact numbers.

**A green suite would clear this only by coincidence of coverage.** What clears it is naming which
corpus-derived figures were re-derived and what they moved to.

## Part two: the width-discipline census

### Why this, and why now

Three defects in two increments, and **two of the three have one shape**: a correct operation applied
across a boundary it does not hold across.

- zeroing the non-parameter locals — right on entry, wrong on re-entry;
- a word-sized store into a data slot — right for a scalar, wrong for a composite.

The second is the one with an unexamined class behind it. **Every place the emitter moves an operand
as a word is a place a `Body` operand would be moved as its address.** The data-slot site was found
because a citation was checked, not because anything looked for it.

`pointer_offset_census.rs` is the deliberate instrument for ADDRESS arithmetic. There is no
counterpart for VALUE movement, and the defect that surfaced was in value movement.

### The wrong turns

1. **Do not build it in the shape of the data-slot defect.** The class is every word-sized move of an
   operand, not every data-slot access. A census that enumerates data slots would have found the one
   site already known.
2. **Do not treat "a body address stored as a word" as always wrong.** A local holding a composite
   stores the address by design, and so does an operand slot. The question is never "is it a word
   move" but **"does the destination outlive the region the address points into"**. Locals are
   cleared at `Reset`; a data slot is not.
3. **Do not count sites and call it coverage.** The pointer census says plainly that it counts sites
   and cannot prove any one is guarded. The same limit applies here and must be stated, not implied
   away.
4. **Do not assume the census is complete because it compiles.** Non-vacuity first: it must find the
   sites already known, including the one that was a defect.

---

## OUTCOME — 2026-09-11, recorded against the prediction above

| clause | predicted | measured |
|---|---|---|
| conflicting files | exactly two, `REVERSE_PROMPT.md` and `TASKLOG.md` | **exactly those two** |
| conflicts in `src/` or `tests/` | zero | **zero** |
| `src/` and `tests/` after the merge | byte-identical to `origin/v0.2.3` | **identical**, and the check returns 12 files against the previous tree, so it is not vacuous |
| backend suite | 519 passed, 0 failed | **519 ran, 1 FAILED** — `corpus_fingerprint` |

### THE FOURTH CLAUSE MISSED, AND IT CONTRADICTED THE RISK I NAMED IN THE SAME DOCUMENT

**A green suite and a fired corpus risk cannot both happen.** This brief named the risk precisely —
the incoming set changes `wire.kel` and `verify_types.kel`, which are corpus subjects — and then
predicted a clean run two lines later. `corpus_fingerprint` exists to fail when the corpus content
moves. **If the named risk was real, the prediction was impossible.**

The prediction should have read: *the fingerprint guard fires, names those two files, and every
derived figure is re-run and re-stated.* That is what happened.

> **A prediction that cannot be reconciled with the risk written beside it is not a prediction, it is
> two documents.** Nothing checks a brief against itself, and this one needed it.

### The named risk, cleared by evidence rather than by a passing suite

**The chunk population moved +11, measured directly** by compiling both versions of each changed
file: `wire.kel` 486 to 492, `verify_types.kel` 28 to 33. The corpus file population is unchanged at
74 — nothing added, nothing removed, confirmed by the diff and by an entry count.

**Every figure the guard named was re-run, and none moved:**

| figure | value |
|---|---|
| refusal set | 1 |
| ISA lowering census | 63 of 66, over 74 compiled modules |
| module-level coverage | 98.6%, 69 modules, 1 refused |
| chunks fully lowerable | 1084 of 1085 (99.9%) |
| opcode instances | 90800 of 90845 |
| interprocedural residual, yield-escape cost | unchanged, suites green |

**One arithmetic does NOT close, and it is left open rather than forced.** The `1070 of 1074` figure
elsewhere in the tree is a calibration dated 2026-08-29, not a measurement of the pre-absorption tree.
Adding the measured +11 to it does not reach 1084; `14_frame_log.kel` becoming lowerable earlier the
same day accounts for two of its chunks, and the remainder was not measured. **Stating an arithmetic
that does not close as though it did is a worse outcome than leaving it named and open.**
