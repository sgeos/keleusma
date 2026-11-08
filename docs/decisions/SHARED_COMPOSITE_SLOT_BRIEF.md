# BRIEF — the shared composite data slot

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## What is refused

```
struct F { a: Word, b: Word }
shared data io { latest: F, n: Word }
```

> the operand is a flat composite body and this slot has no persistent pool entry: a one-word store
> would write the body's ADDRESS rather than copy its bytes

The refusal is correct and was deliberately general: it catches **any** body reaching a destination
that outlives the region, and the shared segment does.

## Why it is now implementable, and it is easier than the private case

`SharedSlotLayout` states everything:

| field | meaning |
|---|---|
| `offset` | byte offset within the host buffer |
| `kind` | with `SHARED_SLOT_COMPOSITE_FLAG` set, the low seven bits are the composite kind |
| **`len`** | **the flat body length in bytes, stated rather than derived** |

**The private pool needed a derived size and therefore a validated partition. This does not.** The
length is a field, so the increment is a copy at a stated offset of a stated length.

## The semantic difference worth stating rather than assuming

A private composite slot's initial value is `Unit`, so reading an unwritten one FAULTS, and that took
an initialisation word per slot. **The shared segment has no such notion**: the host owns the buffer
and initialises it by contract, and the runtime copies out whatever bytes are there and re-wraps them
as the declared composite kind. So a shared composite read is always well defined.

**No initialisation word belongs here.** Adding one would invent a fault the reference does not have.

## The wrong turns

1. **Do not reuse the private path's initialisation machinery.** It exists because `Unit` is not a
   body. The shared segment has no `Unit` state to represent.
2. **Do not derive the length from the next slot's offset.** It is a field. A second computation of a
   stated quantity is free to drift, which is the rule this line already applies to widths and pool
   offsets.
3. **Do not lower the INDEXED form on the strength of the direct one.** The existing shared-array path
   proves a range contiguous and uniform before striding, and a composite range needs the same
   treatment. If it is not done, refuse it with a reason.
4. **Do not weaken the general body refusal.** It must still fire for a destination that outlives the
   region and has no copy — the value-movement census's whole point. Narrow it to "no stated
   placement", not to "not private".
5. **Do not forget the censuses.** A copy and an address computation are exactly what two of them
   track, and a row that says REFUSED will become false.

## What done looks like

A shared composite slot is written by copy and read as a body, both agreeing with the reference; an
unwritten one reads as whatever the host put there, with no invented fault; the indexed form either
works or is refused with a reason; and the census rows that said "refused" say what is true now.
