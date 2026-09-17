# BRIEF — widen the generator, on the two axes that are safe

## Why now and not in the same increment

The narrow generator — 300 `Word` trees, one seed, depth 3 — has been through a
full gate. **Widening it in the same breath as writing it would have been the
mistake this session found twice**: an instrument that looks like coverage and
has never been shown to produce a finding. Its reach is now demonstrated, at the
lowering, so the base is worth building on.

## The two axes, and why only two

**Seeds.** The run costs 0.58 seconds for 300 programs. Several seeds cost
seconds, and a single seed is a single sample of the shape space — the cheapest
real strengthening available.

**`Byte` operands.** The fixed matrices cover `Byte` operators **one at a time**,
and this line has already found two genuine defects there: checked `Byte` multiply
and add returning untruncated values, where `200 * 100` gave 20000 instead of
`Byte(32)`. **That is the type with the worst track record, and composition over
it is untested.**

**`Fixed` is deliberately excluded**, and the reason is concrete rather than
timidity:

- `FixedMul` carries a scale, so magnitude grows far faster than in the integer
  case and a safe depth bound is materially harder to argue;
- **`Fixed % Fixed` is report 4** — the reference type-checks it and the virtual
  machine traps, and this backend now refuses it. A generator emitting it would
  hit the refusal assertion in the shared driver, not a comparison.

Excluding it is a scope statement, not a claim that `Fixed` composition is fine.

## The trap hazard, again, and sharper for `Byte`

`+`, `-`, `*` are checked. **A `Byte` product overflows far sooner than a `Word`
one**: with leaves bounded by 3, a depth-2 all-multiply tree reaches 81 and stays
inside a byte; **depth 3 reaches 6561 and does not.** So `Byte` trees must be
shallower than `Word` trees, and the bound must be derived from the type rather
than inherited from the existing constant.

Getting this wrong does not produce a failure — it produces a **SIGTRAP that kills
the test binary**, which reads as a harness fault rather than as a finding.

## Shape, taken from a form already proven in this tree

`driven_opcode_witnesses.rs` drives `fn main(a: Word, b: Word) -> Word { (a as
Byte) as Word }` through the shared two-argument driver. **Wrapping a typed
subexpression in a `Word` signature is therefore known to work**, and avoids
needing the operator matrix's own typed driver.

## Wrong turns to avoid

- **Letting the `Byte` depth be the `Word` depth.** Stated above; it is the one
  way this increment kills the binary rather than failing.
- **Reporting a seed count without a program count.** What matters is how many
  distinct programs were compared, and that floor must move with the widening.
- **Widening until the run is slow.** The gate's longest phase is 380s. This
  should stay under a few seconds; if it does not, cut seeds rather than
  accepting the cost quietly.
- **Assuming the widened generator still has reach.** It is a new instrument.
  **Perturb the lowering again** — the previous demonstration was for `Word`
  `bxor`, and says nothing about whether a `Byte` tree would catch a `Byte` defect.
