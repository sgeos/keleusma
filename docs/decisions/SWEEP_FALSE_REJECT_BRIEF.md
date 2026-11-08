# BRIEF — a "rejected shape" that was a type-name error

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## What I filed this morning, and why it is wrong

`operand_variant_sweep.rs` carries a table of shapes *"the reference rejects"*, asserted so that their
absence from the variant matrix is a fact rather than an untested gap. One entry reads:

> `byte comparison to Bool` — `fn main(a: Byte, b: Byte) -> Bool { a < b }`

**The reference does reject it. Not for the reason the entry implies.** Keleusma's boolean type is
lowercase `bool`; `Bool` is not a type name at all. With the correct spelling, `Byte`, `Fixed` and
`Word` comparisons all compile and all lower.

**So a typo was recorded as a language property**, in the very table built to stop absences being
mistaken for facts. The irony is the point: the table's purpose is to distinguish "cannot" from
"untested", and this entry did the opposite.

## What that hid

**Comparisons were never swept per operand type.** The matrix covers arithmetic and bitwise operations
and stops there. Comparison is exactly the family where type-specific semantics have precedent — the
emitter notes that float comparison matches the reference and is deliberately NOT IEEE.

Measured now: `Byte`, `Fixed` and `Word` comparisons agree, including a byte pair straddling 127 where
a sign-extending load would invert the answer, and `i64::MIN` against `i64::MAX`.

## The wrong turns

1. **Do not delete the entry silently.** It was wrong in a way worth recording: the table exists to
   separate "cannot" from "untested" and this entry conflated them.
2. **Do not assume the remaining entry is sound because this one was not.** `Byte` subtraction with an
   overflow arm was measured separately and its rejection is real.
3. **Do not add comparisons without values that discriminate.** A byte pair below 128 agrees under a
   sign-extending load; the pair must straddle the boundary.
4. **Do not treat "all agree" as proof the family is sound.** It is evidence about the operand types
   and operators driven, which is what the sweep already says about itself.

## What done looks like

The false entry is corrected where it was made, with what was measured; comparisons are in the matrix
with discriminating values; and the surviving rejection entry is confirmed rather than inherited.

---

## OUTCOME — 2026-09-12

**The entry was wrong, and the shape it hid agrees.** `Byte`, `Fixed` and `Word` comparisons all
compile, all lower, and all match the reference — including a byte pair straddling 127, where a
sign-extending load would invert the answer, and `i64::MIN` against `i64::MAX`.

**The entry is corrected rather than deleted**, spelled as what it actually is: an unknown type name
is rejected, which is not a fact about comparisons. The wrong turn the brief named first was exactly
this, and it was worth naming: the table exists to separate "cannot" from "untested", and that entry
conflated them in the file built to prevent it.

### The surviving rejection was re-checked, not inherited

`Byte` subtraction with an overflow arm is still rejected, and for a real reason — the reference
states the outcome cannot arise. Only one of the two entries was wrong.

### Two small things the work turned up

- **`Bool` is a scalar in the sweep's terms**, and 0/1 is its flat form: the emitter's own comment says
  a comparison result "is 0 or 1 in an i64, which is the flat representation of the VM's tagged
  `Bool`." The converter had to learn that before comparisons could be driven at all.
- **The case count moved 21 to 27 while the test-function count did not.** Cases are data in a matrix,
  not test functions; the population guard's note says so, because a reader reconciling the increment
  against that number would otherwise go looking for a change that is not there.

### What is not claimed

Six comparison cases over three operand types is evidence about those, not a proof that the
comparison family is sound. The sweep already says this about itself, and adding rows does not change
it.
