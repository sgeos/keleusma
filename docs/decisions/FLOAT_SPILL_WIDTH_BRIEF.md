# BRIEF — the float spill's WIDTH has no witness

**Filed 2026-09-18, against `c9c0932a`, backlog 0, suite 647.**

> ⚠ **STATUS, ADDED BY THE ARTIFACT AUDIT 2026-09-18.** A BRIEF DESCRIBES THE TREE AT FILING, BEFORE ITS OWN WORK LANDS. Every gap it states in the present tense was, by construction, still open when written. **LANDED `97156846` AS A NEGATIVE RESULT, AND ITS TITLE CLAIM IS NOW FALSE.** The float spill width HAS a witness as of `51542c99`: a composite built directly from the spilled operand and the reply detects a corrupted spilled width in both float configurations. It became expressible only once the resumed reply carried its declared width, which is why this brief could not find it.


## The gap, and it was found by measurement rather than by reading

The previous increment added two subjects that leave a float operand beneath a
yielded value, and then measured what they detect by corrupting each half of the
spilled `(Width, OperandKind)` pair separately:

| corruption | outcome |
|---|---|
| kind dropped to `Unknown` | **DETECTED** — refused with a mixed-kind message |
| width dropped to `Unknown`, kind kept | **NOT DETECTED** — both configurations still agree |

So the package now witnesses a float's KIND crossing a suspension and **nothing
witnesses its WIDTH**. The `Byte` subject in `stream_width_survival.rs` witnesses
width, but only for a byte, and only because a byte add needs a matched one-byte
pair. A float add does not care.

**That asymmetry is recorded but not closed**, and a width lost on the resume path
is documented in the emitter's own comment as *"a silently wrong number rather
than a fault"* — the worst failure class this package recognises.

## Where a float's width does matter

Composite packing. The emitter's composite arm states that the pack places each
operand **at the operand's own width**, and that a float carries `Width::Scalar(8)`
from `push_k`. So a value whose width was lost would pack at the wrong size or be
refused by the arm's exactness check.

The shape to build:

1. compute a float operand,
2. suspend with it beneath the yielded value, so it goes through the spill,
3. after the resume, consume it and **build a composite from the result**,
4. read a field back and compare tick by tick against the reference.

If the spilled width does not survive, the composite's field offsets or its packed
size differ, and the differential sees it — or the module refuses, which is also a
detection.

## Wrong turns, named

1. **Do not assume the subject reaches the spill because it contains a yield.**
   That is precisely the error corrected last increment, where 240 generated
   streams carried everything in LOCALS and touched no spill at all. The `yield`
   must be a SUBEXPRESSION with a computed operand already on the stack.

2. **Do not accept a passing test as the result.** The whole point is detection.
   **Corrupt the spilled width and require the new subject to fail.** If it passes,
   the subject does not depend on the width and must be replaced rather than
   explained. This is a pre-commitment, not an aspiration.

3. **A composite BODY beneath a yield is refused deliberately** and that refusal
   must stay — it is what holds the yield-escape line. The composite here is built
   **after** the resume, from a scalar that crossed. If the subject accidentally
   stacks a body beneath the yield, it will be refused, and that is the emitter
   being right rather than a gap.

4. **A refusal is a detection, not a divergence.** If corrupting the width makes
   the module refuse instead of miscompute, the subject still works. Say which it
   is rather than blurring them.

5. **The f64-versus-f32 constraint binds.** Operands, replies and yielded values
   must all be four-byte exact, operands included.

6. **If this adds a file, check the host-buffer census FIRST.** Last time a new
   file was green under every targeted run and turned the gate red there, because
   `-E binary(X)` cannot see a census in binary Y. Prefer adding subjects to a
   harness already declared.

7. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment, written before the work

- **If no expressible subject makes the float spill width observable, say so and
  stop.** That is a real result: it would mean the width is carried on that path
  but nothing downstream consumes it, which bounds the hazard rather than closing
  it. Recording that honestly is worth more than a subject that passes for
  unrelated reasons.
- **If the composite route turns out to refuse for an unrelated reason**, record
  the refusal and its cause; do not reshape the subject until something passes.
- **No emitter change in this increment.** This is an instrument, and the previous
  two increments both found that the claim about an instrument was wrong before
  the instrument was.
