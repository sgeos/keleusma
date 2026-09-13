# BRIEF — how far the `Fixed` refusal actually reaches

## The finding

The previous increment refused `Op::Mod` and `Op::Div` on a `Fixed` operand and
described the fix as precise. **It was not.** `OperandKind::Fixed` was seeded at
chunk parameters only, so `(a + b) % (a + b)` still lowered the very `Op::Mod`
that `a % b` refused — neither operand being a direct parameter read.

Driving **sixteen** routes by which a `Fixed` value can reach that opcode found
**nine open**. I had named this gap in the previous hand-off and then had to
measure it to learn it was three times larger than I described.

> A passing check is evidence about the checker's reach before it is evidence
> about the tree.

## Why the lattice was wrong, not just incomplete

`OperandKind` was `Int | Float | Unknown`. **Every site that already knew a scalar
kind was discarding all of it but `Float`.** `StructField::Flat` and
`ArrayElem::Flat` carry a `ScalarKind` in the baked operand and the arms tested
`matches!(kind, SK::Float)`; a call result read the callee signature and took only
the float half; a shared slot read the layout tag and did the same. The lattice
was built for floats and never widened when `Fixed` began to matter.

## What is closed, and the one that is not

Closed: arithmetic results inherit `Fixed`; `FixedMul`, `FixedDiv` and
`WordToFixed` produce it; `FixedToWord` deliberately does not, the scale being
gone; flat struct fields and array elements map their declared kind; call results
take it from the signature; shared slots from the layout tag.

**Open, and not closable from here: a `Fixed` read back from a PRIVATE data
slot.** `DataSlot` carries a name and a visibility and **no scalar kind**, unlike
`SharedSlotLayout`. The declared type is not in the module.

The fail-closed alternative — refusing `%` and `/` on any operand of unknown
provenance — would refuse ordinary `Word` remainders on private-slot values, which
the reference runs correctly. **The coverage loss is worse than the recorded
residual**, so the route is pinned open by a test that fails when it closes, and
reported upstream as report 5.

## Wrong turns to avoid

- **Putting a parameter on one side of the test expression.** An early probe did,
  the refusal fired on that operand, and the route under test was never exercised.
  Every row that can avoid a bare parameter does.
- **Declaring a fix precise without driving its reach.** This is the second time
  in two increments. Measure the class, not the case that prompted it.
- **A `#[cfg(feature = ...)]` naming another crate's feature is always false.**
  Writing `#[cfg(feature = "floats")]` in this package silently deleted the
  `Float` arm and would have regressed float field reads inside a refactor that
  looked like a simplification. Clippy's `unexpected_cfg_condition_value` caught
  it; nothing else would have until a float test failed.
