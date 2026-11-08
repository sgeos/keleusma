# BRIEF — the operand-variant agreement sweep

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## Why, and the evidence

Four defects in two increments hid behind one instrument's shape.
`backend_support_census.rs` is keyed by **opcode name** while support and semantics are decided by the
**operand**, and every `Checked*` row probed `Word`. It reported **0 refused** while:

- the `Fixed` variants of `CheckedMul` and `CheckedDiv` were refused outright, and
- the `Byte` variants of `CheckedMul` and `CheckedAdd` returned untruncated values.

That census also says plainly what it does not do: *"a pass here does NOT mean the emitted code is
correct."* **Support and agreement are different questions and nothing asked the second one per
variant.**

## What was already measured by hand

Probing the rest of the matrix found **no further divergence**: unchecked `Byte` add, subtract,
multiply, `band`, `bor`, `bxor`, the three `Byte` shifts, `bnot`, and `Fixed` add and negate all
agree. **That negative is the reason to build the sweep rather than a reason not to** — it is the
state today, established by hand, and nothing holds it.

## The wrong turns

1. **Do not make it a support test.** `lower_module` returning `Ok` is a fact about the compiler. This
   must EXECUTE both sides and compare, or it repeats the mistake it exists to correct.
2. **Do not let a non-compiling shape read as a pass.** Several combinations the reference rejects —
   `Byte` comparisons returning `Bool` in the obvious spelling, `Byte` subtraction with an overflow
   arm. Each must be recorded as "the reference rejects this" rather than skipped silently.
3. **Do not pick values that cannot distinguish.** `3 * 4` agrees under every wrong lowering that was
   just fixed. Each case needs at least one input that leaves the operand's natural range.
4. **Do not claim the matrix is complete.** It cannot be: the sweep knows the variants it was given.
   Say so, as the support census now does.
5. **Do not duplicate the differential's whole job.** This is about operand TYPE variation of the same
   operation, not about coverage of operations in general.

## What done looks like

A matrix of operation against operand type is driven on both sides and compared; a shape the reference
rejects is recorded as such; the population is pinned so a new variant announces itself; at least one
input per case leaves the operand's natural range; and the sweep states that it covers the variants it
was given rather than all that exist.

---

## OUTCOME — 2026-09-12

**21 driven cases, all agreeing.** The sweep found no new divergence, which is the expected result for
an instrument built immediately after the defects it would have caught were fixed. **Its value is
forward: the next variant that arrives is compared rather than assumed.**

### It caught something while being written

The `Word` control would not compile: **a `Word` overflow arm binds the high AND low halves —
`overflow(h, l)` — where `Byte` and `Fixed` bind one value**, because their middle slot is unused.

**The arm's ARITY varies by operand type**, not just the arithmetic. One more way the operand decides
the shape while the opcode name does not, discovered by writing the control rather than by reasoning
about it.

### The rejected shapes are recorded

`Byte` subtraction with an overflow arm, and a `Byte` comparison in the obvious `Bool` spelling, are
both rejected by the reference. They are asserted as rejected, so their absence from the matrix is a
fact rather than an untested combination — and if the reference ever admits one, the assertion says to
add it to the matrix rather than leave a shape this backend has never been driven on.

### What it does not claim

**It covers the variants it was given.** The matrix is hand-written, which is the same limitation the
support census now states about itself. A clean run is evidence about these combinations, not a proof
that no variant diverges.
