# Returning a composite: the caller-allocated return slot (`sret`)

**Status**: authorised by the operator, 2026-08-14. **Not yet implemented.**

A chunk returning a flat composite was refused by the native lowering because the body must
outlive the chunk that built it, and `plan_chunk_region` gives each chunk offsets in a region
the CALLER owns. Choosing where a returned body lives is host-visible surface, so it was held
as an operator decision alongside the string ABI.

## The convention

**The caller reserves space for the return value in storage it owns and passes the address as
a hidden trailing parameter. The callee writes the body there and returns that same address.**

This is the `sret` shape. It was chosen over the alternative considered — restore the caller's
region pointer on return and push the value onto it — for four reasons.

| property | `sret` | restore-and-push |
|---|---|---|
| region offsets stay STATIC | yes | no, reintroduces a bump pointer |
| copies on return | none | one, the body must move below the restored mark |
| ownership | unambiguous, caller's from the start | ambiguous during the window |
| survives MULTIPLE ARENAS | **yes** | **no** |

## Why it survives multiple arenas, which is the operator's stated direction

The operator expects future versions to have several arenas, so that **a caller and callee may
use different bump arenas**, and a returned value may need to live in some combination of the
caller's stack and heap.

**`sret` accommodates that without change, because the callee never names an arena.** It
receives a pointer and writes through it. Which arena, region or section that pointer came
from is decided entirely by the caller and is invisible across the call boundary.

Restore-and-push would have baked "caller and callee share one bump pointer" into the calling
convention, and would have broken precisely when the second arena arrived.

## The caveat, stated before it surprises anyone

**One `sret` pointer describes one CONTIGUOUS body.** Under the B28 flat representation that is
what a returned composite is, so the convention is sufficient today.

A future value needing genuinely split storage — a flat body in one place and a heap-resident
sub-object in another — would need a second pointer or a descriptor. That is a **change to this
convention rather than a use of it**, and should be recognised as such rather than bolted on.

## Cost, which is measurable and NOT yet measured

`sret` reserves per **call site**, not per live value: two call sites returning composites get
two slots even when their lifetimes never overlap. A stack discipline would reuse the space.

This is the price of static layout. The handoff records **23 of 826 chunks** returning a flat
composite, which suggests the blow-up is small — but "suggests" is not a number, and this
branch has been wrong about exactly that kind of inference three times.

**Measure total region bytes under per-site reservation against the current figures BEFORE
building on it.** If it blows up, that is a reason to revisit the choice, not to absorb it.

## Neither a new opcode nor a `BYTECODE_VERSION` change

This is purely a native calling convention. The bytecode already records that a chunk returns
a composite; nothing about the wire format or the instruction set moves. The rad-hard
minimal-ISA constraint is untouched.

## Implementation sequence

1. **Measure** the per-site region cost against today's figures. Report before proceeding.
2. Reserve a return slot per composite-returning call site in `plan_chunk_region`.
3. Pass its address as a trailing hidden parameter; the callee writes there and returns it.
4. Re-apply the tail-walk widening for the three rogue AI modules (thirty lines, described in
   `NATIVE_LOWERING_INVENTORY.md`).
5. **Differentiate all three against the virtual machine.** They declare no host natives, so
   there is nothing to stub. `lower_module` returning `Ok` is not verification.
6. Keep `codegen.kel` refused as the must-not-fire control — its refusal is delegated
   suspension, a soundness matter, and must not be widened by this work.
