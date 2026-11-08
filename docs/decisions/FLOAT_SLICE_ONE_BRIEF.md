# BRIEF — the float slice, and the prerequisite that decides its shape

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The prerequisite, found by measuring rather than by starting to type

`width_of_declared_shape` collapses `WireShape::Scalar { kind }` to `Width::Scalar(size)` — **it
discards the kind.** A `Float` (tag 5, 8 bytes) and a `Word` (8 bytes) are therefore
indistinguishable inside the backend, though the boundary shape carries the distinction.

So the obstacle is **not missing information**. It is information the internal model throws away, and
**no float arithmetic can be lowered until an operand's kind survives**. Had I started from the
operator's phrase "entry ABI", I would have built the wrong piece and discovered this mid-flight.

## What the one refused module actually needs

```
let f = w as Float;      // IntToFloat
let scaled = f + 1.5;    // float CONSTANT, and Add on FLOAT operands
scaled as Word           // FloatToInt
```

All three, and the `Add` is the one that forces the kind channel: `Op::Add` is emitted for `Byte`,
`Fixed` **and** `Float`, and today lowers unconditionally to `build_int_add`.

## The design, and why this shape

The operand stack is `widths: Vec<Width>` holding **`IntValue`**. A float is a `FloatValue`, so either
the stack becomes an enum of value kinds — **46 pop sites** — or it stays homogeneous `i64` with each
entry **tagged**, bitcasting at float operations.

**Tagged and homogeneous.** It is local to the 19 `push_w` and 6 `width_at` sites, and LLVM folds the
bitcasts. The producing opcode seeds the kind — `IntToFloat` pushes Float, a float `Const` pushes
Float — so this is local dataflow rather than signature threading.

## Wrong turns to avoid

- **Do not relax the float guard until the differential agrees.** A half-implemented float path that
  is *accepted* rather than refused is precisely the hazard the guard exists to prevent, and it would
  be worse than the current refusal. Relax only the shapes actually lowered, and only after they
  agree with the reference.
- **Do not widen `Width::Scalar(8)` to mean "float or word".** That is the collapse being repaired.
  The kind must be a distinct thing, so an unknown kind can stay unknown and refuse at the use, as
  widths already do.
- **Do not build the entry ABI this increment.** No corpus module carries a float in a signature, so
  it has **zero corpus witnesses** and could only be verified against hand-built subjects.
- **Do not assume `f64`.** `Float` is `f32` under `narrow-float-32`. The FP type must follow the
  runtime's float width — my reading, recorded as an assumption, not the operator's ruling.
- **Do not let a bitcast silently reinterpret.** A bitcast is only correct when the kind tag is right;
  if the tag is unknown the operation must refuse, not guess. This is the same discipline the width
  model already applies.
- **Do not report a coverage gain that the differential has not confirmed.** Lowering the module is
  not the same as computing the same answer.

## What good looks like

The witness module lowers and **agrees with the reference**, the two conversion opcodes stop being
UNPROVEN, and every float shape not implemented is still refused rather than accepted.
