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

---

# The corpus DID force this, and I said it did not. `10_multbyte.kel` is the counterexample.

2026-08-14. The section above states that no multi-chunk composite return exists in the corpus,
so `sret` was authorised but not yet forced. **That was wrong.**

## The defect, in three lines

```text
fn mk(x: Word, y: Word) -> [Word; 2] { [x, y] }
fn main(a: Word, b: Word) -> Word { let p = mk(a, b); let r = mk(b, a); p[0] + r[0] }
```

With `a = 3, b = 4` the answer is `3 + 4 = 7`. **Natively it is 8**, because `p[0]` reads `4`:
the second call's body overwrote the first's.

## The mechanism

`plan_chunk_region` gives every flat site in a chunk a distinct offset, and plans **per chunk
from zero**. `mk` therefore writes its result at the same region offset on every call, while
the caller holds two of those results live at once. One buffer, one offset, two live values.

**A single composite return is correct**, and so is a caller composite beside one callee
result — both are pinned as passing tests in `composite_return_aliasing.rs`. That is exactly
why the corpus looked clean: nothing was wrong until a caller kept two alive, and no test kept
two alive.

## How it surfaced

`corpus_differential.rs` reported `10_multbyte.kel` returning `1` on the virtual machine and
`0` natively, on its first run. That module calls `add_2` and `sub_2`, each returning
`[Word; 2]`, from a `main` that also builds four arrays of its own. It had lowered
"successfully" since composites landed and **had never been executed**, so nothing contradicted
the no-multi-chunk-return claim.

## What `sret` fixes, and why it is now forced

Under the caller-allocated return slot the caller reserves a **distinct slot per CALL SITE**,
so two calls to one callee write to two places and cannot alias. The convention was the right
answer before this was known; it is now also a repair rather than a provision for the future.

The per-call-site region-cost measurement recorded above is still owed and is still step one.

## Status

**Reported and pinned, not repaired.** `composite_return_aliasing.rs` carries the failing case
as `#[ignore]` with the reason, plus the two boundary cases that pass. The `#[ignore]` is a
pinned defect awaiting a repair, not a skipped test, and `10_multbyte.kel` remains in
`KNOWN_DISAGREEMENTS` where the set-equality assertion keeps it visible.

---

# Step one, taken at last: `sret` costs 4.9% of region bytes

`native_codegen/tests/probe_sret_cost.rs`, 2026-08-14. This document made measuring the
per-call-site cost step one when the convention was authorised, and it had never been taken.

| | |
|---|---|
| composite-returning call sites, whole corpus | **13**, across 5 modules |
| region bytes today | 4576 |
| bytes `sret` adds | **+224** |
| growth | **4.9%** |

| module | region now | `+sret` | sites |
|---|---|---|---|
| `09_big_numbers.kel` | 96 | 32 | 2 |
| `10_multbyte.kel` | 192 | 32 | 2 |
| `piano_roll_0.kel` | 1280 | 24 | 1 |
| `piano_roll_1.kel` | 1280 | 24 | 1 |
| `rogue_dungen.kel` | 16 | **112** | 7 |

## The verdict: proceed

**4.9% is not a blow-up**, and the concern this measurement existed to test — that per-SITE
reservation would multiply where per-live-value would not — does not materialise at corpus
scale. Thirteen sites in fifty-five modules is a thin surface.

**`rogue_dungen` is the outlier and reads worse than it is.** Its `+112` is 800% of a 16-byte
current region, which is arithmetic on a small base rather than a scaling problem. In absolute
terms it is a hundred and twelve bytes.

The figure is an **upper bound** on what any liveness-aware reuse could achieve, since a slot
is reserved per site whether or not two sites' values are ever live together. Nobody needs to
pursue that reuse on these numbers.

## What this does not license

The measurement clears the COST. It says nothing about the implementation being correct, which
`composite_return_aliasing.rs` and `corpus_differential.rs` are for. And it does not license
widening the convention: one `sret` pointer still describes one contiguous body, and a case
needing split storage remains an operator decision.
