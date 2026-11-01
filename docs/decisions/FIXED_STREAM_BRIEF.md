# BRIEF — a Fixed stream differential

**Filed 2026-09-23, against `34218286`, backlog 0, suite 655.**

> ⚠ **STATUS.** Filed before its own work lands, like every brief here. The gap it
> states in the present tense is open at filing by construction.

## The gap, found by probing rather than by a record

Four `Fixed` stream shapes were probed and **all four lower with zero refusals**:
a reply added to the parameter, a reply MULTIPLIED by it, a spilled `Fixed` operand
carried across a suspension into a `FixedMul`, and the degenerate tail-position
form.

**Nothing compares any of them against the reference.** That is the exact state the
float stream path was in before its defect surfaced, and this package recorded the
lesson at the time: *"the backend lowers these streams, the reference runs them, and
no test compared the two."* A panic was the lucky outcome there; a wrong number on
the same path had nothing watching it.

## Why `Fixed` is the interesting case and not just the next one

**A `Fixed` IS an `i64` at the LLVM level**, so the machine type cannot discriminate
it — the precise hazard that made the resumed float reply wrong. And the resumed
reply is currently kinded `Int` for everything that is not a declared `Float`, so a
`Fixed` reply carries `OperandKind::Int`.

That it nevertheless lowers suggests the scale rides on the OPCODE —
`Op::FixedMul(frac_bits)` carries the fraction count — rather than on the operand
kind. **If so, the kind genuinely does not matter here, and that is worth
establishing rather than assuming.** Either the values agree, which bounds the
hazard, or they do not, which is a finding.

## The hazards, and one is inherited

**No exactness constraint.** `Fixed` is exact integer arithmetic on `i64` bits in
both implementations. There is no configured width to disagree about.

**Unchecked overflow, and a stream feeds back.** `Op::Mul` is unchecked, so an
overflow is a wrong number rather than a trap, and **both implementations wrapping
identically would agree and prove nothing**. A reply multiplied by the parameter
makes the bound compound per tick, exactly as it would have in the float stream
generator — which is why that one restricted the reply to entering additively. A
hand-written differential can instead use few ticks and small values, but **the
bound must be asserted, not argued.**

## Wrong turns, named

1. **Do not reuse the float stream drivers unmodified.** They pass `Value::Float`.
   A `Fixed` stream needs `Value::Fixed` and lowers with `i64` parameters, so the
   signature differs from the float one and matches the scalar one.

2. **Do not copy the float exactness machinery.** There is no configured width here,
   and a guard against an absent hazard obscures the real one.

3. **Do not assume the reply's kind is irrelevant because the module lowers.**
   Lowering is not agreeing. **Perturb the reply's kind and see whether the values
   move**; if they do not, say that the kind is not load-bearing on this path rather
   than leaving it implied.

4. **Compare the whole sequence, not a final value.** The float stream perturbation
   diverged at the loop-back ticks while both sequences ended identically.

5. **A bare decimal literal is a FLOAT.** `Fixed` literals are `(N as Fixed)`, and a
   mixed pair is refused by the reference — which would read like a backend refusal.

6. **If a file is added, check the host-buffer census before committing.** A
   targeted run cannot see a census in another binary.

7. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment

- **If the sequences agree, the increment still stands**, and the write-up says the
  kind is not load-bearing on this path — measured, not assumed. That is the
  expected outcome for a backend that has produced no incorrect result in weeks.
- **If they disagree, check the magnitude bound first.** An overflow on one side
  only is the likeliest cause and it is a harness defect.
- **No emitter change in this increment.** It is an instrument.
