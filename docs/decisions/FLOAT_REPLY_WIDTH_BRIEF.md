# BRIEF — give the resumed float reply its declared width

**Filed 2026-09-18, against `246ddbc2`, backlog 0, suite 648.**

> ⚠ **STATUS, ADDED BY THE ARTIFACT AUDIT 2026-09-18.** A BRIEF DESCRIBES THE TREE AT FILING, BEFORE ITS OWN WORK LANDS. Every gap it states in the present tense was, by construction, still open when written. **LANDED `51542c99`, AND THE GAP IS CLOSED.** Its opening — *"A float reply cannot be packed into a composite"* — describes the tree before the work. It packs now, and the subject is driven value by value against the reference.


## The gap, and why it is smaller than the handoff assumed

A float reply cannot be packed into a composite:

> `NewComposite ... has an operand of unknown packed width: operand 2 of 2`

The resumed value is pushed with `width_of_tag(TypeTag::Float)`, which returns
`Unknown`. The handoff recorded this as needing *"an emitter change affecting every
composite carrying a float field"*, because the obvious fix is to change
`width_of_tag`.

**That reading is wrong, and re-reading the function says why.** `width_of_tag`
takes a TAG and nothing else. A float's packed width is `float_bytes`, which is
**not on the tag** — it comes from the module header. So the function is right to
refuse: it genuinely cannot know. Its own comment says a body width for a float
field *"would be a guess, which is the one thing this function exists not to do."*

**The resume-push site is not in that position.** It already has `float_bytes` in
scope — it uses it two lines earlier, in `float_to_bits`, to convert the parameter.
So the fix is local to one site and `width_of_tag` is untouched.

## Why `Width::Scalar(float_bytes)` is the right width, measured

- `float_bytes = 1 << module.float_bits_log2 >> 3`. **Measured: 8 by default and 4
  under `narrow-float-32`**, taken from the module's own header rather than from a
  cargo feature.
- The canonical layout function is `flat_byte_size(word_bytes, float_bytes,
  addr_bytes)` — **the reference's own packing is parameterised by exactly this
  figure.** So a float field's packed width IS `float_bytes`, and the backend
  agreeing with it is agreeing with the canonical layout rather than guessing.

## Wrong turns, named

1. **Do not change `width_of_tag`.** It cannot see the module. Widening it would
   make it guess, which is the thing its comment says it exists not to do, and it
   would affect every caller including ones with no module in hand.

2. **Do not hardcode 8.** It is 4 under `narrow-float-32`, and the gate runs both.
   A hardcoded 8 would pass the default gate and mispack in the other — the exact
   asymmetry that broke `Op::Neg`.

3. **The differential is the oracle, not the refusal lifting.** A module that
   lowers is not a module that packs correctly. The success criterion is that the
   composite's field reads back equal to the reference's, tick by tick.

4. **Do not assume this closes the spill-width gap.** It closes a REPLY width. The
   spilled operand's width still has no witness, and if this makes the
   direct-composite subject lowerable, that subject becomes the route to test it —
   which is a follow-on, not this increment's claim.

5. **The f64-versus-f32 constraint still binds**, operands included.

6. **Run the whole gate, once per configuration**, and check the host-buffer census
   if a file is added. A targeted run cannot see a census in another binary.

7. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment, written before the work

- **If the differential disagrees after the change, revert it.** A composite that
  packs differently from the reference is worse than a refusal, because the refusal
  is honest.
- **If the change requires touching a second site, stop and report the shape.** The
  claim of this brief is that it is one site; if that is false, the claim was wrong
  and the increment should be re-scoped rather than widened quietly.
- **Verify the new capability is really exercised**, not merely permitted: the
  composite field must be read back and compared, and the subject must fail if the
  width is wrong. A test that only checks "no refusal" would pass against a
  mispacking backend.
