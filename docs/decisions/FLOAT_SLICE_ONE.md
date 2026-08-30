# Float slice one: the kind channel, and a round trip that agrees

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: the lowering exists and is differentially verified. The module-level float guard is
UNCHANGED, so nothing float-carrying is accepted through `lower_module` yet.** That ordering is
deliberate and is the safe direction.

## The prerequisite, which measuring found and typing would not have

`width_of_declared_shape` collapses `WireShape::Scalar { kind }` to `Width::Scalar(size)` and
**discards the kind**. A `Float` and a `Word` are both eight bytes, so **no float arithmetic could be
lowered until an operand's kind survived** — `Op::Add` is emitted for `Byte`, `Fixed` *and* `Float`.

Starting from the operator's phrase *"entry ABI"* would have built the wrong piece first and met this
mid-flight.

## What was built

**A kind channel beside the width channel**: `OperandKind::{Int, Float, Unknown}`, tracked per
operand-stack entry and per local, seeded by the **producing** opcode rather than threaded from
signatures. The stack stays homogeneous `i64` and a float lives on it as its bit pattern; the tag is
what a width cannot carry. The alternative — a stack of value enums — touches 46 pop sites for nothing
the optimiser does not already give.

Lowered: a float **constant**, `IntToFloat`, `FloatToInt`, and `Op::Add`/`Sub`/`Mul` on two float
operands.

**Every one of these refuses rather than guesses**: a mixed Int/Float pair, an unknown kind, and any
float width other than 8 are refused, because reading an integer's bits as a double is a plausible
wrong number rather than a fault.

`Module::float_bits_log2` is **carried**, not assumed — `Float` is `f32` under `narrow-float-32`, so a
hard-coded double would be the wrong type in a build with no `f64`. Only 8 is lowered; anything else is
refused. **That the FP type follows the runtime width is my reading of the ruling, recorded as an
assumption.**

## The verification, which is a differential and not an acceptance

`float_differential.rs` runs the witness's exact shape — `w as Float`, `+ 1.5`, `as Word` — on the
reference and on the lowered code, over ten probes including negatives, and **they agree**. It goes
through `lower_chunk`, which does not carry the module guard.

A must-fire control checks the program computes something rather than returning its argument, so the
agreement cannot be vacuous.

## Two errors of mine, both caught inside the increment

**The kind was lost across the local round trip.** `local_kinds` was declared and never wired, so
`GetLocal` pushed `Int` where a float had been stored. **The refusal caught it** — "one side is a float
and the other is not" — rather than a bitcast quietly reinterpreting.

**Then I read the kind AFTER popping.** `pop` lowers the depth and `kind_at` is relative to it, so the
check asked about the wrong operand — the diagnostic showed `depth=0` with
`local_kinds=[Unknown, Float, Float]`, proving the save was fine and the read was wrong. **The rule is
written at `SetLocal`** — *"Read the width BEFORE popping"* — and the arm I wrote broke it.

## What is NOT done, named so it is not assumed

- **The entry ABI.** No corpus module carries a float in a signature, so it has **zero corpus
  witnesses** and could only be verified against hand-built subjects.
- **Float shared slots**, division, comparisons, and `f32`.
- **The module guard is not relaxed.** Relaxing it is the next decision, and it needs the constant
  route's differential to cover the corpus witness end to end rather than a hand-built chunk.

---

## The hazard implementing this CREATED, and the whitelist that closes it

**Implementing float operations removed an accidental protection.** A module whose float arises from
`as Float`, with no float constant and no float in a signature, was previously refused **only because
no float operation existed**. `float_guard_routes.rs` names that exactly — *"a property of what is
unimplemented, not a guard"* — and it stopped being true the moment the operations were written. The
module-level guard does not cover such a module: it scans signatures, constants, native shapes and
data slots, and this shape has none of them.

**`Op::Div` is the sharp case.** Float dispatch was added to `Add`/`Sub`/`Mul` only, so a division
would have been an *integer* division of a double's bit pattern — a plausible wrong number rather than
a fault.

**A blacklist would have to name every arm that must refuse, and missing one is silent.** So the
protection is a **whitelist**: an opcode that consumes a float operand and was not written for one is
refused. Moves are exempt because they copy bits without interpreting them.

**The first formulation was wrong and a control caught it.** It checked the top two stack entries
rather than the operands the opcode POPS, which refused `Op::Const` for a float sitting *below* it.
The operand count now comes from `keleusma::verify::op_depth_effect`, which returns `(required,
delta)` — the documented API, rather than a per-opcode guess of mine.

Both directions are pinned: float division refuses, and the supported round trip still lowers.

## A pin of mine went red, correctly, and told me what to do

A pin in `unproven_opcodes.rs` asserted that `IntToFloat` was refused BY NAME. It is not, any more.
**Its own failure message said what to do** — *"If the backend now lowers one, it is no longer unproven
and needs an execution witness rather than this test"* — and the witness exists, so it was updated
rather than deleted, and **renamed**, because a test called `..._is_refused_...` that asserts the
opposite is exactly the stale label this line keeps finding.

**The first correction of it was WRONG and the pin caught that too.** It asserted the module-level
guard still refused the subject. **Measured: it does not.** That guard scans signatures, constants,
native shapes and data slots, and `(x as Float) as Word` has a float only as a local. The corpus
witness `float_witness.kel` is a *different* subject — it carries a float CONSTANT — which is why the
census still lists the conversions as UNPROVEN while this subject lowers. Conflating the two is what
the correction got wrong.
