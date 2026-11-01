# BRIEF — the two producers that lose an operand's packed width

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The gap, measured before this brief

Four composite shapes compile and are then refused by `NewComposite` with *an operand of unknown
packed width*: a `bool` struct field, a `bool` array element, a `Fixed` tuple member and a `Fixed`
array element. **The refusal names an operand POSITION, not an opcode**, so reasoning backwards from
the packer would have guessed. The probe asked instead:

| source | producing opcodes |
|---|---|
| `[1, 2]` | `Const`, `Const` — **carries a width, and this lowers today** |
| `[true, false]` | `PushImmediate(1)`, `PushImmediate(2)` — **no width** |
| `[a as Fixed<16>, …]` | `WordToFixed(16)` — **no width** |

So the gap is **two producers**, not a composite-construction problem at all. `Const` already does the
right thing, which is why every integer composite in the corpus works and nothing noticed.

## Why the widths are facts rather than guesses

`Bool` is one byte in the canonical flat layout. `Fixed` is eight: a Q-format value is an `i64` of
fixed-point bits occupying a full slot, and the tree already records the measurement —
`struct { a: Fixed<16>, b: Fixed<16> }` packs at sixteen bytes, identical to a pair of words — as the
justification for `width_of_tag`'s `Fixed` case. **This increment makes two producers agree with a
width the tree already establishes elsewhere.**

**And the existing exactness check is the safety net**: the construction arm requires the operand
widths to account for the baked `byte_size` EXACTLY, so a width that is wrong refuses rather than
mispacking. That is why widening a width is a bounded risk here and why guessing one is not.

## What must NOT be widened

- **`Unit`.** `PushImmediate(0)` pushes a placeholder zero for a value that carries nothing and whose
  flat width is ZERO. Giving it a scalar width would be inventing a representation, and the comment
  at that arm already warns that the placeholder is sound only because nothing consumes it.
- **`None`.** Already refused; it needs an `Option` representation this backend has not settled.

## Prior failures to avoid repeating

- **Do not verify by acceptance.** A composite that lowers is not a composite that packs correctly.
  The oracle is execution against the virtual machine, and for a `bool` the discriminating question
  is whether the neighbouring field survives — a one-byte value written eight bytes wide overwrites
  its neighbour and no single-field test would see it.
- **Values that discriminate.** For `Fixed`, use values whose bit pattern differs from the integer
  reading; a fixed-point value that happens to equal its integer form proves nothing.
- **Confirm mutations APPLIED by printing the changed line**, and remember the two extension sites in
  the read arms are distinct — the same trap caught this session, where one mutation left a witness
  passing.
- **Expect the censuses to MOVE only if the corpus carries these shapes. Measure first**; the last
  two increments both predicted movement and measuring said otherwise.
- **Do not name a test in a comment before writing it.** The citation guard caught that twice today.

## The wrong turn most likely here

**Widening every width-losing producer at once because they look alike.** `Unit` is the
counter-example sitting in the same match arm, and it must stay unknown for a stated reason.

## Outcome, written after the build

**The gap was THREE producers, not the two the probe first found.** `Op::PushImmediate` and
`Op::WordToFixed` were the two it named. The third surfaced only when the tests were written: a
COMPUTED boolean, `a > 0`, still had no width, while a literal `true` now did — so `[true, false]`
lowered and `[a > 0, a > 5]` did not.

**The integer half of the comparison arm dropped the width while its FLOAT twin, six lines away, had
always set it.** No reason was recorded for the asymmetry, and it is the sort that survives because
each half reads correctly on its own. Found by a test failing, not by reading the code.

| producer | width now | why it is a fact |
|---|---|---|
| `PushImmediate(true/false)` | 1 | `Bool` is one byte in the canonical flat layout |
| `PushImmediate(Int)` | 8 | a `Word` is eight throughout this backend |
| `PushImmediate(Unit)` | **unchanged, unknown** | its flat width is ZERO and the pushed value is a placeholder |
| `WordToFixed` | 8 | the tree's own measurement, recorded at `width_of_tag` |
| integer comparison | 1 | it yields a `Bool`, and the float twin already said so |

**Evidence.** Six tests in `narrow_composite.rs`. The struct case reads the NEIGHBOUR back, because a
one-byte value written eight bytes wide overwrites what follows and **no test reading only the
boolean would see it** — confirmed by mutation: widening the boolean immediate to eight bytes fails
that test and **only** that test. Narrowing the fixed-point width to one fails both fixed-point
tests. The boolean cases exercise both branches so a value that is always true cannot pass by
accident, and the fixed-point values are chosen so the stored bit pattern differs from the integer
reading by a factor of 65536.

**`Unit` is pinned by consequence rather than by implementation**: a composite carrying a unit value
must be refused, and the test says what to do if that ever changes deliberately — rewrite the
reasoning in the arm, not delete the test.

**Censuses re-derived and UNMOVED**: 1072 of 1074 chunks, 89854 of 89940 opcode instances. Measured,
not predicted; the corpus carries none of these shapes. The kind-arm accounting moves from eight
combinations resolved to ELEVEN, with twenty-one still unexercised and not implied closed.

**An expired warning was corrected in passing.** The comparison arm carried *"the NaN path is
unexercised; no source construct produces a NaN today: the route is division, and `Op::CheckedDiv` is
not lowered"*. Division lowers, and the NaN path is exercised and was wrong on two predicates when
first tested. Kept as history rather than deleted, since the reasoning it records is why the defect
was caught.
