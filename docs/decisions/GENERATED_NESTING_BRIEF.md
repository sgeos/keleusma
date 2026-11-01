# BRIEF — generate nested composites, where the failure mode changes

## Why this surface, and why it is different from every other one today

Everything found today was **a refusal**: the backend declined to lower rather
than lowering wrongly. Safe, and costly only in capability.

**Nested composites are where that stops being true.** The emitter's own
documentation, on the `Width` enum:

> *"An eight-byte nested composite body and a `Word` are both eight bytes, and a
> single-field struct wrapping a word is exactly that shape — 80 of the corpus's
> constructions are `(8, 1)`. Storing one as a scalar would write the POINTER into
> the parent body while every downstream field offset still looked correct, which
> is **a silent wrong answer rather than a fault**."*

And one of the ten recorded defects was exactly that: **a composite data slot
stored as the body's ADDRESS.**

So this is the surface where the family's mistake produces a wrong number instead
of a refusal, it has burned this line once already, and **nothing generated has
ever exercised it.**

## What must vary

- **Nesting depth**, so a body inside a body is built and read through.
- **The `(8, 1)` shape specifically** — a single-field struct wrapping a word,
  which the documentation names as the ambiguous case, because its body is the
  same size as the scalar it could be confused with.
- **Scalar neighbours beside the nested field**, since a wrong width shifts them
  and the neighbour is the detector.
- **Computed values inside the nested body**, so the widths repaired today are
  exercised one level down.

## Prior failures to avoid

- **Testing only the nested value.** If the outer struct holds nothing but the
  nested body, an address stored instead of a body may still read back correctly
  through the same wrong indirection. **The neighbour is what catches it.**
- **Assuming a refusal means a gap.** Today's four gaps were all refusals worth
  repairing; a refusal here may equally be the conservative design working. **Read
  the message before concluding**, and do not repair a refusal into a mispack.
- **A probe implicating itself, six times so far.** Nested-composite syntax must
  be taken from something the tree already compiles.
- **Reporting a green run without perturbing.** The perturbation for this surface
  is specific: make a `Body` width be pushed as a `Scalar` of the same size and
  confirm the comparison catches it. **If it does not, the subjects are wrong**,
  because that is precisely the documented silent failure.

## What a green run would and would not say

**Would**: the generated nestings build and read back the same values as the
reference, at this commit. **Would not**: that `Body` and `Scalar` can never be
confused — only that these shapes do not confuse them.
