# BRIEF — the mixed-type half of the operator matrix

## Present goals

The scalar matrix drives **same-type** pairs and found `Fixed % Fixed`. Mixed
pairs were named as an open gap in the previous two hand-offs and never driven.
They matter because a promote-or-convert path is where a width or a scale is
silently lost, and because the operand-kind lattice widened last increment is
least exercised there.

## The measured result

**All 42 mixed pairs are refused by the reference compiler.** Three scalar types,
six ordered mixed combinations, five arithmetic operators plus two comparisons.

The cause was read rather than assumed: *"type error: cannot add Byte and Word"*,
*"cannot order Word and Fixed<32>"*. **Keleusma has no implicit numeric
conversion.** Verified three ways, which is the shape that would have prevented
the `Bool` mistake:

1. **The cause** — the message names both operand types.
2. **A control** — `Word + Word` compiles, so the rejection is about the pair.
3. **The workaround** — `(a as Word) + b` compiles, so the conversion is available
   and merely not implicit.
4. And the return type is **not** the cause: `-> Byte` and `-> Word` give the same
   message at the same span, which is the `a + b` expression.

**A fifth axis appeared while measuring.** `Fixed<16>` and `Fixed<32>` are also
mutually unassignable — *"cannot add Fixed<16> and Fixed<32>"*. The same nominal
type at different const parameters is a different type.

## What this means for the backend

The backend **never sees a mixed scalar pair from source**, so
`arith_result_kind`'s mixed-kind handling is unreachable through the compiler.
It remains reachable through hand-built bytecode and `Vm::new_unchecked`, so the
behaviour is worth recording, but it must be recorded as *unreachable from the
surface* rather than as *tested*.

This is a negative result and should be written as one. A matrix of 42 refusals is
valuable precisely because it converts "we never tried mixed operands" into "the
reference refuses all of them, for this stated reason".

## A CORRECTION I OWE THE RECORD

The previous increment's commit message, design-journal entry and brief describe
the `Fixed % Fixed` divergence as *"`4.0` for `200.0 % 7.0`"*. **Bare `Fixed` is
`Fixed<32>`, not Q16.16** — the compiler emits `FixedMul(32)` for it. The raw
operands used were `200 << 16` and `7 << 16`, which as Q32 values are
approximately `0.003052` and `0.0001068`, and the result `262144` is
approximately `0.000061`.

**The divergence, the mechanism and the fix are unaffected** — the integer
remainder of the raw words is `262144` under either reading, and the reference
traps regardless of scale. Only the decimal figures named in prose were wrong.
The commit message cannot be corrected without rewriting a pushed branch; the
journal and this brief carry the correction instead.

## Wrong turns to avoid

- **Recording a refusal without its cause.** Two increments ago an unknown type
  name was filed as a fact about comparisons. Read the message, check a control,
  and check that the obvious alternative explanation is excluded.
- **Claiming the mixed-kind code path is tested.** It is unreachable from source.
  Say unreachable, not verified.
- **Assuming a default.** `Fixed` is Q32 here. The default was assumed once
  already in this session and put wrong numbers into three artifacts.
