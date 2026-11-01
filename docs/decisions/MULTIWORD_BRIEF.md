# BRIEF — a Multiword differential, and the multiply refusal pinned

**Filed 2026-09-23, against `a2afaca0`, backlog 0, suite 661.**

> ⚠ **STATUS.** Filed before its own work lands, and — unusually for this session —
> **after** the measurement that decides its shape. The premise below is observed,
> not expected.

## The gap, measured before it was written down

`Multiword<N, F>` — the multi-word fixed-point family — had **one mention across
every backend test and source file**, and it was incidental: a shape in
`len_producer_census.rs` that the reference REJECTS.

Probed with the real construction form, `(limb, limb) as Multiword<2>`:

| operation | result |
|---|---|
| index, add, subtract, compare, shift, per-limb bitwise, fixed-point add | **lower, zero refusals** |
| **multiply** | **REFUSED** — `NewComposite ... has an operand of unknown packed width` |

**So six operations lower and nothing compares them.** That is the shape that
produced this session's float findings.

## ⚠ TWO WRONG TURNS ALREADY TAKEN ON THIS, BOTH MINE, BOTH TODAY

1. **I guessed the construction syntax and got seven false "refusals".** `5 as
   Multiword<2, 0>` does not compile — *"cannot cast Word to Multiword<2>"* — and
   the parameter is a **LIMB COUNT**, constructed from a tuple of that many limbs.
   Had I written the brief from that probe, it would have claimed the reference
   rejects the whole feature. **The census subject I copied it from is itself a
   shape the reference rejects**, which is why it looked authoritative.

2. **I nearly concluded the feature was unsupported** on the strength of a single
   mention plus a failed probe. The correct reading came from the reference's own
   suite, `tests/multiword.rs`, which shows the canonical form.

## The design constraints, from this session's own scars

- **A `Multiword` is a BODY, not a scalar.** Its operands carry `Width::Body`, and
  confusing `Body` with `Scalar` writes a POINTER into a parent body — this line's
  recorded silent-wrong-answer class. The differential compares a `Word` extracted
  by indexing, so the body never crosses the harness boundary.
- **Feed a RUNTIME value, not only literals.** A wholly constant subject can be
  folded by the middle end, and the comparison would then establish nothing about
  the lowering. At least one limb must come from a parameter.
- **Do not assume the scale.** `Multiword<2>` is integer-valued; `Multiword<2, 16>`
  carries 16 fraction bits. A misread scale printed wrong decimals for `Fixed`
  once; the differential is unaffected but any figure written down is not.
- **Multiply is refused, so pin the refusal with its cause and a control** — the
  same treatment the float-reply gap got, so a change in either direction is
  noticed rather than discovered.

## Wrong turns still to avoid

1. **Do not widen `width_of_tag` or touch the emitter.** The multiply refusal is
   sound: it fails closed on a width it does not know. Closing it is a separate
   increment with its own differential, exactly as the float reply was.
2. **Do not compare only one operation.** Six lower; a differential over one of
   them would be labelled as covering the family.
3. **If a file is added, check the host-buffer census BEFORE committing**, and its
   list is compared as an ORDERED sequence — that caught me twice today.
4. **Do not create probe files in `tests/` while a gate runs.** The eleventh
   catalogue row is mine from this session.
5. **`src/` and `tests/` at the repository root are read-only to this line** — they
   were read to find the construction form, which is the intended use.

## Pre-commitment

- **Prove reach by perturbation**, and perturb something in the multi-word path
  rather than in scalar arithmetic.
- **If the middle end folds a subject to a constant, the subject is wrong** and gets
  a runtime operand, not a looser comparison.
- **No emitter change.** This is an instrument plus a pinned refusal.
