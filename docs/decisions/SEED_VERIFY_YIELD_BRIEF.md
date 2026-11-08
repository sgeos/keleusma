# BRIEF — absorption 13, and seeding `verify_yield.kel`

**Written**: 2026-08-26, third loop iteration of the day. **For this line's own use.**

## The two goals and why this pair

**1. Absorption 13** — #282, radix-prefixed literals, **+1 function (`hex_val`) in `lexer.kel`**.
Third consecutive absorption to touch an Order-1 stage source, and **the first to touch a SEEDED
one.** `parse.kel` and `reconstruct.kel` moved in 11 and 12; `lexer.kel` is driven with a real seed
in both harnesses, so this one can move gate figures the previous two could not.

**2. Seed `verify_yield.kel`** — the increment the requirements document set up. Chosen over
`analyze.kel` and `codegen.kel` because it is measurably the smallest: `op_count`, `region_start`,
`region_end`, four parallel `[Word; 1536]` tables (`class`, `arg`, `mark`, `cay`), two outputs
(`out_fell`, `out_hy`). `analyze.kel` needs nine tables and eight scalars.

## What is already established, so it is not re-derived

- The name-resolved shared-slot route reaches these stages; the structure is named slots in an
  ordinary `shared data` block, not opaque marshalled state.
- `class` is enumerated in `analyze.kel`'s header: `0` plain, `1` If (`arg` = branch target), `2`
  Else, `3` EndIf, `4` Loop (`arg` = exit target), `5` EndLoop, `6` Break, `7` BreakIf, `8` Trap.
  `verify_yield.kel` uses the same encoding, `mark` = 1 for Yield, `cay` the fixpoint variable.
- `verify_yield.kel` caps at **8192** steps. `stage_differential` supplies **400** ticks;
  `corpus_differential` supplies **60**. **A cap is not a cost** — use `stage_differential`.

## Prior failures this work is specifically exposed to

**1. The truncated fold.** A seed that drives a stage which stops early compares a prefix while
reporting as seeded. Already made once here. **The seed builder must REQUIRE the verdict** — for
this stage, that `out_hy` or `out_fell` actually MOVES from what the buffer already held.

**2. The already-holds-the-answer trap.** The three `verify_*` stages were credited as seeded while
writing a verdict the buffer already contained, so the observable never moved. **An accepting
subject is vacuous by construction.** Seed a subject that makes the verdict change.

**3. Green on an unvalidated value.** Absorption 12's whole lesson. A stage that returns without
doing the work looks identical to one that did, unless something distinguishes them.

**4. Carrying a figure across a change that moves it.** Absorption 9 did this. Re-derive at the
absorption, and again after the seeding, since a new seeded stage changes gate counts.

**5. Subject-shopping.** If a subject does not drive the stage, report it; do not quietly try
another until one works.

**6. Reading a pipeline's exit status instead of the command's.** Four times in one day.

**7. Asserting a count without naming the command that produced it.** The "32 commits" figure.

## Specific wrong turns to avoid here

- **Do not edit `src/selfhost/`** or the other read-only files. If seeding needs something only the
  `v0.2.3` line can add, **that conclusion is the deliverable.**
- **Do not pin a distribution.** The `verify_types` precedent: report the figure, assert only that
  it was derived from runs actually performed over a non-empty set. A distribution assertion fails
  on ordinary corpus growth and teaches the next reader to delete it.
- **Do not assume the one-array `seed()` helper generalises silently.** It writes one length slot
  and one array slot; this stage needs four parallel tables against a single `op_count`. Widening it
  is a change to a helper other seeded stages depend on — `lexer.kel` and `parse.kel` — so their
  figures must be re-derived after, not assumed unchanged.
- **Do not report the stage as seeded if only the tables were written.** Written is not driven.
- **Do not let absorption 13 and the seeding share a measurement.** Two changes, both capable of
  moving gate counts. Measure between them or attribution is lost — the exact mistake absorption 9
  made and absorption 10 was split to avoid.
