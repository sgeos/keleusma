# BRIEF — trust a multiply-written local when every write agrees

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Absorption 21 | yes |
| 2 | Lift the two composite refusals by certifying agreeing multi-write locals | yes |
| 3 | Prove the lift by execution against the reference, not by a coverage number | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 5 | Lower `Stream` | not in one increment |

## Rationale

The two remaining composite refusals are caused by the **multi-write local rule**: a local's packed
width is trusted only when the chunk writes it at most once, and a `for` loop's induction variable is
written twice — initialisation and increment. The rule is sound and its stated reason is correct: a
linear walk cannot see a back edge, so a local rewritten in a loop would be read at the width of the
textually earlier write.

**But "cannot see a back edge" only matters when the writes DISAGREE.** If every write to a local
records the same width, the width is that width whichever write reached the read, back edge or not.

**The circularity that would have forced a fixpoint does not exist here, and this was measured.**
`push_triple` pushes an arithmetic result at a literal `Width::Scalar(8)`, independent of its
operands. So the induction variable's two writes are a `Const` and an arithmetic result, both fixed,
and neither depends on the local being analysed. A previous draft of this plan assumed a monotone
dataflow analysis was required; **reading the arm removed the requirement.**

## The shape of the change

A pre-pass simulates the operand stack using the instruction set's **published** `stack_growth` and
`stack_shrink`, recording for each stack slot which instruction pushed it and **which of that
instruction's pushes it was**. For every `SetLocal`, that identifies the producer of the stored value.
A local is certified when every one of its writes has a producer whose width is a constant of the
instruction itself, and all those constants are equal.

## Prior failures to avoid repeating

1. **A heuristic walk gave a confident wrong answer** — "nearest preceding `GetLocal`" picked the
   loop condition's read. The published stack effects gave the right answer. **Use them again here.**
2. **A width guessed rather than derived mispacks silently.** A `Byte` and a `Word` are
   indistinguishable on the operand stack, so a wrong certification produces a plausible wrong value,
   not an error.
3. **An unverified widening was shipped once and had to be reverted.** The module-source differential
   now exists; use it.
4. **A passing test is weak evidence.** Show the new case FAILS without the change.
5. **Three recorded premises were false in consecutive increments.** Re-derive.

## Specific wrong turns to avoid

- **Do not certify from an instruction whose pushed width depends on its operands.** Only arms that
  push a literal width qualify. Anything else must yield no certification.
- **A multi-push instruction is not one slot.** `Checked*` pushes low, high and flag; only the low
  slot is the arithmetic result. Certifying on the instruction alone would label a flag as a word.
  Track which push a slot came from.
- **Do not treat an unclassifiable producer as agreeing with the others.** One unknown write must
  sink the whole local to unknown.
- **Do not read a coverage increase as correctness.** A wrong width raises coverage and mispacks. The
  evidence that counts is a program that runs and agrees with the reference.
- **Do not claim the rule's original reasoning was wrong.** It was right; this narrows when it needs
  to apply, and the documentation should say so rather than imply a defect was fixed.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
