# BRIEF — seeding `analyze.kel`, the second-to-last unseeded stage

**Written**: 2026-08-26, fourth loop iteration. **For this line's own use.**

## The goal

Seed `analyze.kel` in the Order-1 gate, taking unseeded from **2 to 1**. Nothing else is queued:
`v0.2.3` has zero unabsorbed commits and the tree is clean, so this iteration is one increment.

## What is already established, measured not assumed

- **The tick budget is a non-issue.** `analyze.kel` is `loop main(resume) -> Word { yield run() }`
  with `for step in 0..16384 limit 16384` **inside `run()`** — the same shape as `verify_yield.kel`.
  Checked directly this iteration rather than carried by analogy, which is the mistake that produced
  the wrong reading last time.
- **The route reaches it**: named slots in an ordinary `shared data` block, no accessor needed.
- **The encoding is in the file's own header**: `class` 0 plain / 1 If (`arg` = target) / 2 Else /
  3 EndIf / 4 Loop (`arg` = exit) / 5 EndLoop / 6 Break / 7 BreakIf / 8 Trap; `opk` 1 GetLocal /
  2 SetLocal / 3 Const / 4 CmpGe / 5 BreakIf / 6 CheckedAdd / 7 PopN / 8 EndLoop / 9 Loop / 0 other.

## ⚠ A correction to this line's own requirements document, found this iteration

It states `analyze.kel` takes **"nine parallel `[Word; 1536]` op tables"** and lists them. **There
are TWELVE**: `cost`, `class`, `arg`, `growth`, `shrink`, `heap`, `opk`, `slot`, `cval`, **`cint`,
`callee_slots`, `callee_heap`**.

**The cause is banal and worth naming: a truncated read reported as a complete list.** The block was
inspected with a line window that ended at `cval`, and the three slots below it were never seen.
**Nothing in the output said it was truncated** — which is exactly why the figure looked whole.

There are also **eight** input scalars (`op_count`, `stream_pos`, `reset_pos`, `local_count`,
`value_slot_bytes`, `arena_capacity`, `region_start`, `region_end`) and **five** outputs
(`out_wcet`, `out_stack_bytes`, `out_heap`, `out_reject`, `out_valid`).

## The verdict that must move

The unseeded buffer is all zeros, so `out_valid` is `0`. **A subject that yields a provable bound
must drive `out_valid` to 1 with a non-zero `out_wcet`.** A subject that cannot be bounded drives
`out_reject` to 1 — a *different* observable moving, and a legitimate second subject.

**Both directions move something here**, unlike the `verify_*` trio where the accepting direction was
vacuous. That is a property of this stage, not a general licence: check it, do not assume it.

## Prior failures this is exposed to

1. **The truncated fold** — a stage that stops early compares a prefix while reporting as seeded.
   **Require the verdict.**
2. **The already-holds-the-answer trap** — a verdict the zero buffer already contains proves nothing.
3. **Reading a number without establishing its scope** — the `0..16384` cap. Already corrected once
   this session; the same trap is available for `arena_capacity` and `value_slot_bytes`, which bound
   things whose units must be read rather than guessed.
4. **A truncated read reported as complete** — see above, committed this iteration.
5. **Subject-shopping** — report a subject that does not drive the stage; do not quietly swap it.
6. **Sharing a measurement between two changes** — nothing else is in flight, so this is low risk,
   but the seeding will move gate figures and they must be re-derived after.
7. **Running the two suites in parallel** — invalidates the perf canary. Sequential.
8. **Citing a name that does not exist** — the citation guard has fired on this twice today.

## Specific wrong turns to avoid

- **Do not edit `src/selfhost/`** or the other read-only files. A blocked conclusion is a deliverable.
- **Do not widen the shared one-array seed helpers.** `lexer.kel` and `parse.kel` depend on them.
  The `verify_yield` seeding added an additive builder; do the same.
- **Do not populate all twelve tables with zeros and call it seeded.** Zeros in `cost` mean a
  zero-WCET chunk, which may well produce `out_wcet = 0` — indistinguishable from not running.
  **The subject must make `out_wcet` non-zero.**
- **Do not infer `class` values from the opcodes.** The mapping is stated; use the stated one.
- **Do not declare the Order-1 gate met.** One stage would still be unseeded.
