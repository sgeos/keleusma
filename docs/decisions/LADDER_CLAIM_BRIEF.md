# Brief — measure the claim this line asserted about the float ladder

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01.**

---

## The goal set

| goal | state |
|---|---|
| **G12** measure the `f32` runtime-symbol cost on the bare-metal target | **unblocked, and the subject of this brief** |
| absorption 45 | available at `5d7755a1`, three commits; routine |
| **`f16`** | **still blocked, and not for the reason the handoff gives** — see below |
| publication | held |

### `f16` is blocked, and the handoff's reason has expired

The handoff says `f16` is *"ruled; behind the arithmetic width above"*. **The arithmetic width landed
in absorption 44, so that reason no longer holds — and `f16` is still blocked, for a different one.**

The differential oracle is this line's only correctness signal. It compares native execution against
the reference virtual machine. **The `v0.2.3` line refuses float widths 3 and 4 at load**, so a
binary16 module never runs on the reference side. **There is no oracle for `f16`, so an `f16`
lowering could not be validated even though LLVM has the type.**

Building it would produce code no measurement on this line can check. **That is worth stating
explicitly, because "the blocker cleared" is exactly the inference a reader makes when a named
blocker is struck through.**

## G12: a claim was asserted and never measured

`NARROW_TARGET_LINKAGE.md` says, of `thumbv8m.main-none-eabihf`:

> *"So `f32` and `f16` buy native instructions rather than merely narrower storage, on the target the
> ecosystem value proposition is written for."*

**That was an inference, not a measurement.** What was measured is that a single `f64` addition pulls
six runtime symbols there. The `f32` cost was never measured at all — the sentence reasons from the
`f64` figure and from the target having a single-precision floating-point unit.

**It is measurable now**, because the `f32` rung went green in absorption 44.

## The prediction, with its falsifier

**Predicted: an `f32` program emits ZERO compiler-runtime symbols on
`thumbv8m.main-none-eabihf`**, against six for the same program at `f64`.

**Falsifier**: any `__`-prefixed symbol in the emitted object for an `f32` program on that target.

**If it fires, the recorded claim is wrong as written and gets corrected rather than softened.**
"Buys native instructions" would become "reduces the runtime dependency from six symbols to N", which
is a weaker and still useful statement — but it is a different statement, and the record currently
makes the stronger one.

## The wrong turns

**1. Do not measure at `f64` and subtract.** The claim is about `f32`, so the probe must declare a
four-byte float. A module compiled at the default width and lowered to the narrow target measures the
wrong thing.

**2. Do not read the IR.** Compiler-runtime calls are synthesised in code generation and appear
nowhere in the intermediate form. This line has made that mistake twice today. **Read the object.**

**3. Do not conclude anything about `f16`.** Nothing implements it, so any statement about its symbol
cost is speculation. The recorded claim covers both rungs and **only one of them is measurable**;
say so rather than letting the measured half carry the unmeasured one.

**4. Do not report a symbol count without its target and its width.** Six different figures are
available here — two targets by three widths — and a bare number is the day's recurring error.

**5. Do not edit test sources while a suite runs.**
