# BRIEF — reading a `Fixed` back out, and one exclusion that must STAY

## The goal

Last increment widened `width_of_tag` so a `Fixed` operand has a width, which made a composite
carrying `Fixed` fields **pack**. Reading one back still refuses:

```
pair_sum: UnsupportedOp("GetField reading Fixed is not lowered")
```

Packing a field and reading it back are separate arms and only the first followed from the width.
That gap is demonstrated, is mine rather than the operator's, and is the obvious next thing.

## It is a FAMILY, not a single arm

Four sites carry the same `Int` / `Byte|Bool` / catch-all shape and drop `Fixed` into the catch-all:

| site | what it governs |
|---|---|
| `GetField` scalar read | a struct field in a flat body |
| `GetIndex` scalar read | an array element in a flat body |
| `slot_entry` | a **shared data slot** |
| `shared_scalar_width` | the same slot's width |

**Finding the family is the point.** The previous increment fixed one exclusion that had no stated
reason; the same shape repeats four times, and fixing only the one that happened to be demonstrated
would leave three behind.

## THE ONE THAT MUST STAY, AND WHY IT IS DIFFERENT IN KIND

**The data-slot exclusions are not the same question and must not be swept in.**

A composite body field is **internal**: the compiler packs it, the same program reads it, and nobody
outside sees the layout. Eight bytes is what the reference already does, so the backend agreeing is
a fact, not a choice.

A **shared data slot is HOST-VISIBLE.** Its layout is an application binary interface — the same
class of question as the string ABI (ruled provisional) and the float ABI (undecided, and blocking
two opcodes). `alloc_format_kind` says *"Fixed slot; fixed-point representation is unsettled"*, and
whether that is stale is **not** something to settle by writing whichever version compiles.

**So: lower the two body-access arms. Leave the two slot arms, and say why.** A brief that treats
four identical-looking exclusions as one problem would be making exactly the bundling error the last
increment found.

## Prior failures and specific wrong turns

- **A `Fixed` field is eight RAW bytes of Q-format bits** — identical handling to `SK::Int`: an
  unaligned eight-byte load pushed at `Width::Scalar(8)`. Do not zero-extend it, do not mask it, and
  do not scale it. The bits are the value; scaling lives in the opcodes that consume them.
- **Do not verify by "it lowers".** The previous increment's `Fixed` arithmetic was checked by a
  corpus program that EXECUTES. Restore the composite half of that program and let the differential
  drive it; a module that newly lowers must newly agree.
- **Beware the harness precondition just added.** Anything multi-function cannot use the
  hand-written differential — it lowers `chunks[0]` and runs the entry point. The corpus is the
  route.
- **One refusing chunk exempts a whole MODULE.** If the composite half still refuses for some other
  reason, it will silently take the arithmetic's execution down with it. Check the executed count
  moved, not just that the suite is green.
- **`Float` stays refused everywhere.** It is excluded by the module guard by every route; nothing
  here may create a path for it.

## What a good outcome looks like

A composite carrying `Fixed` fields packs, reads back, and **agrees with the virtual machine on an
executed corpus program**; the two slot exclusions remain with a stated reason distinguishing them
from the body ones; and the report says whether the opcode headline moved (it will not — these are
`GetField` and `GetIndex`, already lowered for other kinds).

**If the read disagrees, revert and report it.** That would mean eight bytes is right for packing and
wrong for reading, which is worth more than the coverage.
