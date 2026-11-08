# Brief — check the requirement this line asserted against what actually landed

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02.**

---

## The goal set

| goal | state |
|---|---|
| **G20** verify requirement R2 against the landed `Text<N>` surface | **unblocked, and the subject of this brief** |
| `f16` | no oracle — the reference refuses widths 3 and 4 at load |
| publication | held |
| absorption | nothing unabsorbed |

**All three items that were blocked on the `v0.2.3` line at the start of this session are closed.**
There is no other unblocked capability work, and I am not going to invent some.

## The claim being checked, which is mine

`TEXT_N_REQUIREMENTS.md`, addressed to the `v0.2.3` line, states:

> **R2. A FLAT layout with no reference field** ... **What the V0.3.X line depends on is only that it
> is a flat composite carrying no handle**, because the native backend already packs and reads flat
> composites and would need no new machinery.

**That is a claim about how much work this line has to do**, told to another line so they could plan
around it. **It has never been checked against anything**, because nothing existed to check it
against. The type surface has now landed, so it does.

This is the same shape as the float-ladder claim earlier today: **an assertion made in a record,
checkable only later, and worth checking precisely because it was told to someone else.**

## The prediction

**Predicted: R2 is neither satisfied nor violated yet, because the LAYOUT has not been decided.** The
landed commit touches the parser, the type checker, monomorphisation, the layout pass and zero values
— **and not `value_layout.rs`**, which is where a flat layout would be defined. That is consistent
with their own description: *"the type surface, refused everywhere below it."*

**Falsifiers:**

1. A layout **does** exist — a `ScalarKind` sizing for text, or a `LayoutDescriptor` shape. Then R2 is
   checkable now, and the answer might be that it is violated.
2. The surface reaches this backend in some way that already requires work, making my "my share is
   small" claim wrong today rather than pending.

**If the prediction holds, the honest outcome is a short record saying the requirement is still
pending**, not an inflated one. A goal that turns out small should be reported small.

## The wrong turns

**1. Do not report a pending requirement as a met one.** "Nothing contradicts R2" and "R2 is
satisfied" are different, and the first is what a missing layout supports.

**2. Do not re-derive their design.** The layout shape is theirs to choose. This checks only whether
what exists is compatible with what this line said it needed.

**3. Do not treat a refusal as satisfaction.** The backend refusing `Text` at every route means the
requirement has not been *exercised*, not that it has been *met*.

**4. If R2 is already violated, say so plainly and tell them**, because they planned around my
statement that this line's share was small.
