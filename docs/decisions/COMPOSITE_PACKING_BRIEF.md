# BRIEF — verify the packing my own fix made reachable

## The obligation this increment discharges

I changed `Op::Div`/`Op::Mod` to propagate `Width::Scalar(1)` for a matched `Byte`
pair. **The refusal I removed existed for a specific reason**, stated in the code
beside it:

> *"Refusing rather than guessing, since a wrong choice here is a silent wrong
> answer"*

and, on the masking helper:

> *"A `Fixed` must NOT be masked, and masking one would truncate it to eight bits
> while every later field offset still looked correct — a silent wrong answer of
> exactly the kind the `Width::Body` split exists to prevent."*

**The width is consumed by composite PACKING.** My evidence so far is 1500
generated scalar trees agreeing — and **a scalar differential never packs
anything**. The value is returned, not stored at an offset. So the riskiest
consequence of my own change is precisely the one nothing has tested.

**This is not a hypothetical worry.** The only genuine codegen defect this line
has ever found was composite-return aliasing, and three of the ten recorded
defects were composite-layout faults: a slot storing a body ADDRESS, a slot read
before write answering 0, a declared initializer never applied.

## What to generate

Programs that **store byte arithmetic into a struct field and read it back**,
including a field whose value came from `/` or `%` — the newly reachable case.
Compare against the reference, which packs by the canonical flat layout.

A field holding a byte quotient is the specific subject. A struct mixing a `Byte`
field with a `Word` field is the sharper one, because a wrong width there shifts
**every subsequent field's offset**, and the symptom appears in a neighbour rather
than in the field itself.

## Prior failures to avoid

- **Testing the value rather than the layout.** A single-field struct whose only
  field is the byte would return the right answer even under a wrong width,
  because nothing follows it. **The neighbour is the detector.**
- **A probe implicating itself, now six times.** Composite syntax must be checked
  against what the language actually accepts before concluding anything: the last
  probe assumed `let mut`, which does not exist here.
- **The trap hazard.** Byte arithmetic overflows quickly; the bounds already
  derived for byte trees apply unchanged, and a violated bound kills the binary
  with `SIGTRAP` rather than failing.
- **Reporting a green run without perturbing.** If the packing is right, show the
  comparison would catch a wrong width — perturb the emitter to push
  `Width::Unknown` again, or to mis-size the field, and watch it fire. **A green
  packing test that cannot fail is worth less than the refusal I removed.**

## The honest possibility

**The fix may be wrong.** If a composite built from a byte quotient mispacks, the
correct response is to revert the width propagation and restore the refusal, not
to patch the packing — the refusal was the conservative position and I am the one
who moved off it. Record that outcome as readily as the green one.
