# BRIEF — audit this session's own artifacts against what it later established

**Filed 2026-09-18, against `c081515d`. Backlog 0. Suite 648 at the last gated

> ⚠ **STATUS, ADDED BY THE ARTIFACT AUDIT 2026-09-18 — AND THIS FILE WAS THE ONE IT
> MISSED.** Landed `c090dae6`. **The audit was blind to its own brief.** It gave a
> status line to eight briefs and there were nine; the ninth is this one, the brief
> of the audit itself.
>
> **Its population figure was also wrong.** The commit message says *"seventeen
> decision documents were filed today"*; derived from the session's first commit the
> true figure is **nineteen**, nine briefs and nine completions plus
> `FLOAT_YIELD_SCOPE.md`. The count came from a log range starting at the wrong
> commit and was asserted rather than derived.
>
> **This is the third census blind to its own subject in one session**, and the
> other two are recorded in the handoff. The lesson is not to be more careful: it is
> that a population must be DERIVED from a stated rule and printed, so the reader
> can see what was covered. The audit checked documents against the tree and never
> checked its own list against the tree.

commit. THE TOOLCHAIN CANNOT LINK.**

## Why this work, and why now specifically

`cc` exits 69 — the Xcode licence has not been agreed — so **nothing links**. No
test binary builds, no gate runs, no push succeeds. `cargo check` and
`cargo clippy` still work, because neither links.

**That rules out the increment that was next.** A `Fixed` generator written now
could be type-checked but never executed, and an instrument whose reach cannot be
proven is exactly what this session has spent the day finding and correcting. It
would be the worst possible thing to add while blind.

**What it does not rule out is the work whose entire verification is reading.**
Seventeen artifacts were filed in `docs/decisions/` today. Briefs are durable: a
future session reads them as the reasoning behind a change. **A brief whose
premise a later increment falsified will mislead a reader who has no way to know.**

## The two already confirmed, both mine, both filed today

1. **`GENERATED_FLOAT_STREAMS_BRIEF.md`** states the generator reaches the operand
   spill — *"everything beneath a yielded value is spilled to an ephemeral slice as
   `(Width, OperandKind)` pairs"*. **Measured false the same day**: all 240
   subjects carry values in locals, `deep` is zero, the spill loop never runs. I
   corrected the test file's header and the handoff **and left the brief alone.**

2. **`FLOAT_SPILL_WIDTH_BRIEF.md`** is titled *"the float spill's WIDTH has no
   witness"* and says *"nothing witnesses its WIDTH"*. **Closed by `51542c99`**: a
   composite built directly from the spilled operand and the reply detects a
   corrupted spilled width in both configurations.

That two of seventeen are already known wrong, inside a session whose recurring
finding is exactly this, is the argument for checking the other fifteen.

## Wrong turns, named

1. **Do not delete a falsified claim.** This project's convention is to correct in
   place and say what changed, because the correction is the useful record. A
   silently edited brief teaches nothing and cannot be audited.

2. **Do not attempt a general prose-drift census.** The handoff records that one
   was tried and **rejected on measurement** — two candidate matchers returned 169
   and 32 lines, both dominated by narrative, where a sentence about a corrected
   claim is indistinguishable from the claim. That reasoning stands. **This audit
   is bounded to a known population of seventeen files from one session**, which is
   why it is tractable where the general form is not.

3. **Do not add a guard for this and claim it works.** A guard needs execution to
   prove its reach, and nothing executes. Writing one now would be adding an
   unproven instrument, which is the thing this increment exists to avoid.

4. **Do not claim a gate ran.** `cargo check` and `cargo clippy` are not the gate.
   They catch a type error and a lint; they run no test. **Say precisely which was
   run**, and record that the commit is not gate-verified.

5. **Do not correct a claim that was TRUE WHEN WRITTEN and is framed historically.**
   A brief saying *"the gap that correction exposes"* is a record of a moment, not
   a present-tense assertion. Only claims that a reader would take as currently
   true need a correction note.

6. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment, written before the work

- **Every artifact gets checked, not only the two already known.** A census keyed
  to what you already noticed finds nothing you had not already noticed — this
  file's own handoff says so about an earlier guard.
- **If an artifact is fine, that is recorded too**, so the audit's population is
  visible rather than only its hits.
- **If the count of falsified claims exceeds two, say so plainly in the handoff.**
  The session's own close block says four records overstated the tree; a higher
  number changes that record and it must be corrected rather than left flattering.
