# What the standing stream refusals need

**Status: LARGELY CLOSED, 2026-09-09. Two of the three refusals this document framed are gone;
the third turned out not to be the question this document is about.** The framing below is kept
because its reasoning about the spill is what was implemented, and because **its central grouping
was WRONG and the correction is the useful part.**

> ⚠ **THIS DOCUMENT SAID THREE REFUSALS WERE ONE QUESTION — "what survives the return". Two were.**
>
> | refusal | outcome |
> |---|---|
> | operands stacked beneath a `yield` | **spill slice**, driven and agreeing over six suspensions |
> | operand stack non-empty at `Op::Reset` | **needed no storage at all** — the runtime TRUNCATES there, so discarding agrees |
> | a resume point that is also a branch target | **still refused, and it is a JOIN question** — the spill preserves entries across a RETURN and says nothing about two edges carrying different values into one block |
>
> The `Reset` case is the sharper lesson. A refusal had been added for it hours earlier, replacing a
> prose premise with a check — the right instinct, recorded as such. **But the check encoded an
> assumption never tested against `src/vm.rs`.** Replacing a comment with a check does not make the
> belief true; it only makes it visible.

**The three decisions this document said had to be settled first were settled as it recommended**:
one slice at the module ceiling rather than one per yield, sized from `MAX_STACK` — a figure the
backend already enforces with a refusal, so no second computation can drift from it; widths and
kinds recorded per site and never re-inferred; and a `Width::Body` operand refused, because carrying
a pointer into fixed-offset region storage across a suspension would defeat the yield-escape
refusal rather than honour it.

---

**Original framing follows. Status when written: OPEN. This document frames a decision and does not
take it.** It exists because the
alternative — inventing a layout while implementing one — is how a differential oracle
returns a wrong answer instead of a refusal.

## The refusals, and a correction to how they were grouped

A previous handoff on this line grouped three refusals as *"one question, not three — each is
what survives the return"*. **That was an inference stated as a finding, and measurement
separated one of them out.**

| refusal | what actually blocks it | needs a layout? |
|---|---|---|
| a yield with operands stacked beneath the yielded value | operand entries must cross a return | **yes** |
| a resume point that is also a branch target | two edges disagree about the operand stack | **yes, the same one** |
| a stream with more than one parameter | the shape faults on the REFERENCE | **no — see below** |

## The multi-parameter case is not a backend question

Measured in `native_codegen/tests/probe_multi_param_stream.rs`, driving
`loop main(a: Word, b: Word) -> Word { let r = yield a + b; yield r + b }` on the reference
with `a = 3, b = 10`:

| leg | value |
|---|---|
| `Yielded` | `13`, which is `a + b` |
| `Yielded` | `110`, which is `reply + b` — **slot 1 survives the suspension** |
| `Reset` | then `TypeError("Op::CheckedAdd ... got Int and Unit")` |

**The second parameter survives a suspension and does not survive the rewind.** `Op::Reset`
clears every local to `Unit`, `resume` writes only slot 0, and the next iteration's arithmetic
on slot 1 is a type error. The program compiles, passes the verifier, and faults on its second
iteration.

So lifting the native refusal would be lifting it on a program that does not work anyway.
**The question belongs to the reference**, and is reported there with three readings and no
verdict: a defect, an intended consequence of `Reset` semantics, or a shape the verifier
should reject outright. Only the third needs no runtime change.

> **A frame that was wrong, recorded because it is the reusable part.** The reasoning that led
> here was "clearing slots 1..n natively would AGREE with the runtime, since the runtime clears
> them too". It would not agree. **The runtime faults**, a fault is observable, and producing a
> value where the reference faults is the silently-wrong-answer class this backend exists to
> refuse.

## The real question: what survives a return

Locals live in the arena's ephemeral region and survive the return. **Operand-stack entries
live in SSA values and do not.** Both remaining refusals are that one fact:

- a `Yield` at operand depth `d > 1` loses the `d - 1` entries beneath the yielded value;
- a resume point that is also a branch target has an empty operand stack on the resume edge and
  depth `d` on the fall-through, which is the same loss seen as a disagreement.

**One mechanism answers both**: a slice of the ephemeral region holding the operand entries live
at each suspension, spilled before the return and reloaded at the resume point. The depth is
statically known — the emitter tracks it and the verifier computes it for every region — so the
slice is **statically sized** and the worst-case memory bound moves by a known constant. Nothing
dynamic is required, which is what makes it admissible under this project's value proposition at
all.

## What has to be settled before any of it is written

1. **One slice sized by the maximum depth over all yields, or one per yield point?** The first is
   smaller; the second makes each resume point's reload independent. A worst-case-memory versus
   clarity trade.
2. **How spilled widths are restored.** An operand carries a `Width` and an `OperandKind`.
   Spilling to `i64` slots loses neither *if* the reload restores the recorded pair — but the
   pairs must be **recorded per site rather than re-inferred**. Re-inferring is how a value comes
   back at the wrong width, and a value of the wrong width is a silently wrong number rather than
   a fault.
3. **Interaction with the yield-escape refusal.** A composite body sits at a fixed region offset
   and is already refused when it escapes its iteration. **A spilled POINTER to such a body is a
   second route to the same hazard** and must be refused on the same ground — not admitted because
   the pointer itself was preserved correctly.

## What would count as evidence

Not that the shapes lower. **A whole yielded sequence compared against the reference**, for a
subject whose operand stack is genuinely non-empty at the suspension, and for one whose resume
point is genuinely a branch target. Both currently refuse, so both would be new subjects rather
than existing tests turned green.
