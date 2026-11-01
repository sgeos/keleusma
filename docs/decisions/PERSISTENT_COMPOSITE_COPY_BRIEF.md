# BRIEF — the persistent composite copy

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11, the increment after the refusal.

## What is being closed

A private data slot holding a flat composite is refused, because the access lowered to a one-word
load or store and so carried the body's ADDRESS where the runtime copies its BYTES into the
persistent composite pool. The refusal is correct and is not the end state: `14_frame_log.kel` is a
corpus module that stops lowering because of it.

## The facts this rests on, each read from its producer rather than inferred

- **`private_composite_layout`** gives `(slot, offset)` for every private slot holding a flat
  composite body, single fields and array-of-composite element slots alike.
- **The pool is packed with ONE RUNNING TOTAL and no padding.** Read from `src/compiler.rs`, which
  does `total = total.saturating_add(body)` per entry in ascending slot order. **So a body's size is
  the gap to the next entry's offset, and the last entry's is the gap to
  `persistent_composite_bytes`.** This is exact arithmetic over a packed table, not an inference from
  a doc comment — but it is a DERIVED size, and it must be validated rather than trusted.
- **`required_persistent_capacity_for` = `private_count * size_of::<Value>()` + the pool**, and the
  runtime's `Value` is 32 bytes while this backend's private slot is `PRIVATE_SLOT_BYTES` = 8. **The
  two private regions are different memory images.** The backend may place its pool where it likes
  inside the capacity the host allocates, provided nothing overlaps.

## The wrong turns

1. **Do not take the size from the layout table without checking the table is a partition.** Offsets
   ascending, no overlap, and the last entry ending exactly at `persistent_composite_bytes`. A table
   that fails any of those must refuse, not produce a size. A derived length feeding a `memcpy` is
   the one place in this backend where being slightly wrong is a buffer overrun rather than a wrong
   answer.
2. **Do not place the pool by arithmetic that assumes the runtime's slot size.** Derive the base from
   the backend's OWN slot width and assert it clears the resume-state word, which sits at
   `required_persistent_capacity_for`. The relation holds because 8 is less than 32; an assertion is
   what keeps it holding if either number moves.
3. **Do not accept a write whose operand width is unknown.** The width is the only statement of how
   many bytes the operand points at. Unknown must refuse — the same defer-on-unknown discipline the
   typed verifier uses. And when the width IS known, **cross-check it against the derived size**: a
   disagreement means the derivation is wrong and must fail loudly rather than copy the smaller of
   the two.
4. **Do not lower the INDEXED composite case on the strength of the direct one.** An array of
   composites needs the element stride proven uniform from the table before `base + index * size` is
   admissible. No corpus module declares one. Refuse it and say so.
5. **Do not delete the discriminating test when it turns green.** It is the only subject that
   separates a copy from an alias; every other subject agrees under both. It should move from
   asserting a refusal to asserting agreement, keeping the runtime's pinned answer.
6. **Do not report the corpus refusal count going 2 -> 1 as the result.** The result is that a
   module which agreed by coincidence now agrees by construction.

## What done looks like

The discriminating subject agrees with the runtime instead of being refused, `14_frame_log.kel`
lowers and runs in the corpus differential again, the derived sizes are validated against the table's
own total, and an indexed composite slot is refused with a stated reason.

---

## CLOSURE — 2026-09-11, the same day it was drafted

**Every wrong turn in this brief was live at least once.**

- **1, validate the partition before deriving a size.** Implemented as three checks — ascending
  offsets, the last entry inside the declared total, a non-zero derived size — each refusing rather
  than copying. None has a corpus subject, and that is recorded next to them: the current compiler
  cannot produce a table that fails them, so the refusals are reasoned rather than driven.
- **2, do not place the pool by the runtime's slot size.** The base is `private slots x
  PRIVATE_SLOT_BYTES`, this backend's own figure, and the emitter refuses if that plus the pool would
  reach the resume-state word. The relation holds because 8 is less than 32; the check is what keeps
  it holding if either number moves.
- **3, refuse an unknown width, and cross-check a known one.** Both implemented. The cross-check is
  the more valuable half: the operand's width and the module's table are independent statements of
  one number.
- **4, do not extrapolate to the indexed case.** Refused, with a subject — the shape compiles on the
  reference, so the refusal is driven rather than hypothetical.
- **5, do not delete the discriminating test.** It kept its subject and swapped its claim. **Two
  OTHER tests were deleted, and they had to be**: they asserted the refusal, so leaving them would
  have meant a suite kept green by not implementing the copy.
- **6, do not report the count.** The count went back to 1. The journal entry leads with the
  mechanism and mentions the digit only to say what it cannot distinguish.

### The residual, stated plainly

The validation legs have no corpus subject and are therefore the least-tested code added here. They
fail CLOSED — every one refuses — so their failure mode is a lost lowering rather than a wrong answer,
which is the right direction for untested code to be wrong in.

## AUDIT OF THE COMPLETION CONDITION — 2026-09-11

Ten clauses. All ten met, which is a first on this line and deserves scepticism rather than
satisfaction.

**Why it landed differently from the last one.** The previous condition was written before the work
and pointed at the subject I had already decided was the risk; it passed while two defects sat
outside it. This one was written AFTER a defect had been found, so it describes a mechanism whose
failure mode was already known. **A condition written downstream of a real failure is not evidence
that condition-writing improved** — it is evidence that knowing the answer helps.

**The clause that did the most work is 4**, requiring the derived size to be validated before use. It
is the only clause that named a hazard rather than an outcome, and it is the one that turned into
three checks in the code.

**The clause that is weakest is 2, and it was closed rather than only recorded.** "Some subject reads
a composite slot in a later stream iteration than the one that wrote it" is satisfied by a subject
that would also pass if the pool were merely never overwritten. Survival across `Reset` and
never-being-rewritten are distinguishable only by a subject that writes the slot again in a later
cycle, and no clause required one. One now exists: it writes on some cycles and not others, so the
two properties fail on different elements of a single sequence. **Recording a gap found by an audit
and leaving it open is how gaps accumulate**, and this line has the record to prove it.
