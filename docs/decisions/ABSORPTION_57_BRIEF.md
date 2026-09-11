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
