# VERDICT — the Multiword multiply refusal is CORRECT, and will not be closed

## Why this one, and why it is well-prepared

`Multiword` multiply is refused with **exactly the class I closed earlier today**:

> `NewComposite ... has an operand of unknown packed width`

For the resumed float reply the premise recorded in the handoff — that closing it
needed an emitter change *"affecting every composite carrying a float field"* — was
**wrong**, and re-reading `width_of_tag` said why: it takes a TAG, and the width was
not on one. The fix was **one site**, using `float_bytes` which the module already
carries, and `width_of_tag` was left untouched.

**The same question applies here**, and it must be answered by reading the emitter
rather than assumed: which operand reaching `NewComposite` has an unknown width in
the multiply expansion, and is that width available at the site?

## What makes this different from the float case, and it may kill the increment

The float reply's width came from the MODULE header. A multi-word multiply's
intermediate may have a width that genuinely is not available — a partial product
can be wider than either operand, and the reference's own expansion decides that.
**If the width is not derivable at the site, the refusal is correct and the
increment is to say so**, not to supply a guess. That is the whole reason
`width_of_tag` refuses rather than guessing.

## Wrong turns to avoid

1. **Do not widen `width_of_tag`.** Its refusal is right; the float fix worked
   precisely because it did NOT touch that function.
2. **Do not supply a width the reference does not use.** The canonical layout is
   `flat_byte_size(word_bytes, float_bytes, addr_bytes)`; a width that agrees with
   it is a measurement, anything else is a guess that mispacks silently.
3. **The differential must gain a multiply subject in the SAME increment.** A
   lowered multiply with nothing comparing it is the state that hid a panic on the
   float path.
4. **Revert if the differential disagrees.** A composite packing differently from
   the reference is worse than a refusal, because the refusal is honest.
5. **The pinned refusal test must be replaced by a driven subject, not deleted** —
   its own panic message says so.

## Pre-commitment

If the width is not derivable at the site from something the module carries, **stop
and record that the refusal is correct**, with the reason. That is a real result and
it is the likelier of the two.

---

# The verdict, after reading the emitter

**The kill condition fired**, and it was the outcome this note called the likelier
of the two. The chain, read rather than guessed:

1. `x * y` expands into per-limb `BitAnd`, `BitXor`, `Shr` and `Checked*` over
   temporaries. **The backend has no multi-word opcode at all**, so there is no
   multiply arm to fix.
2. Those temporaries are locals the chunk writes **more than once**.
3. `certified_local_widths` trusts a multi-write local only when every write's width
   is fixed **by the instruction rather than by its operands** — deliberately, so
   that no fixpoint is needed. It certifies `Const` and the `Checked*` family.
4. `BitAnd`, `BitXor` and `Shr` results are **operand-dependent**, hence
   unclassifiable, and *"one unclassifiable write sinks the local"*.
5. The local reads back `Unknown`, and `NewComposite` refuses it.

## Why this is not the float-reply case

That one was **one site** using `float_bytes`, a figure the module header already
carried, and `width_of_tag` was left untouched. **Here there is no such figure.**
The width pass is a linear walk that cannot see a back edge, and its own
documentation states that declining to trust a disagreeing multi-write local *"costs
coverage and cannot mispack, which is the correct direction for a decision that is
otherwise silent."*

Certifying operand-dependent sources needs the fixpoint that design explicitly
avoids. **That is a different and much larger change than this note anticipated**,
and nothing observed suggests it is warranted: the cost is capability, never a wrong
answer.

## What was done instead

The refusal is pinned in `multiword_differential.rs` with its cause, an `add`
control on the identical shape, and now the traced chain — so a future reader meets
the reason rather than re-deriving it. **No emitter change.**
