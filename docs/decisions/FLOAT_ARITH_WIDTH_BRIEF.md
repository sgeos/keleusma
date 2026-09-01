# Brief: make float arithmetic honour the declared width

## The defect, stated precisely

The module header declares a float width. **Storage honours it and arithmetic does not.**

Three sites in `src/bytecode.rs` narrow a declared four-byte float through `f32` on the way into and
out of a composite field. **No site narrows an arithmetic result.** So a value rounds when it passes
through a composite field and does not when it stays on the operand stack, and the same expression
yields two answers depending on whether an intermediate was stored.

**It is not a `narrow-float-32` defect.** `check_runtime_widths` rejects only bytecode declaring
*wider* than the runtime and admits narrower deliberately. A stock build with no features, loading a
module that declares a 32-bit float, computes in `f64`. The feature is how it was noticed.

## Authorization

First-party, in session, 2026-08-31. The operator was asked whether "different data widths" meant
storage or arithmetic and answered: both. The V0.3.X line separately relayed an assignment; that
relay is not needed for this part and was not relied on.

## The construction, and why it is sound rather than merely convenient

Compute in the runtime's wide type, narrow the result to the declared width after **every**
operation. This is what the integer axis already does: `checked_arith_outputs` reads
`word_bits_log2` from the header and narrows each result to the declared width at runtime.

It is also forced below 32 bits. Rust has no `f16` on stable, confirmed against rustc 1.98.0, and no
`f8` of any kind, so there is no type to instantiate the machine at. The trait-per-width model
cannot reach those rungs at all.

**Soundness.** Computing in a wider format and rounding once per operation equals computing natively
at the target, provided the intermediate carries at least `2p+2` significand bits for target
precision `p`. `f64` has 53; an `f32` target needs 50, `f16` needs 24, E5M2 needs 8. Every rung
clears it, `f32` with a margin of 3.

This matters beyond correctness: the V0.3.X backend lowers natively at `f32` where LLVM has the
type. The two constructions differ, and the condition above is the reason their differential is
meaningful rather than coincidentally agreeing.

**Limits.** The condition covers addition, subtraction, multiplication, division and square root. It
does not extend to fused operations or transcendentals, which need their own argument. It assumes no
overflow or underflow of the intermediate, which `f64` makes remote.

## The scope, measured twice and corrected twice

**Fifteen sites construct a float value. TEN need narrowing.** I first believed there was a single
choke point at the generic helper, then said fifteen sites needed narrowing. Both were wrong, and
the second error is the more dangerous: five pointless edits would each look like a site that had
been reasoned about.

The five that do not need it: `Op::FloatToInt` produces an `Int`, the four `Float(0.0)` filler
pushes in the checked paths are exact literals, and the comparison arm produces an `Ordering`.

**And coverage of the ten was measured by mutation, not inspection.** The first test file passed
eleven tests while covering four sites. Removing each narrowing call in turn is the only thing that
showed which sites the tests reached.

## The wrong turn that will otherwise happen

**A missed site is invisible to every test that runs at the default width**, because declared and
runtime widths are equal there and narrowing is the identity. The suite is green today with zero
arithmetic narrowing anywhere.

So a green suite proves nothing about this work. The verification has to enumerate the operations at
a **declared width narrower than the runtime**, which is the configuration the defect lives in and
which no existing test exercises for arithmetic.

## Other specific wrong turns

**Do not change `pub type Vm` to use `f32` under the feature.** It is a public type, it does not fix
the stock build where the defect also lives, and the header-driven construction needs no such change.

**Do not narrow through an intermediate rung.** Narrow directly from the wide value. The rule is a
condition on the intermediate: at least `2p+2` bits. This forbids routing through bfloat16 to reach
binary16, which supplies 8 against a requirement of 24 and is a plausible choice on ML silicon.

**Do not confuse this with double rounding.** The likely defect is rounding *once at the end* rather
than after each operation. That is missing rounding, not double rounding, and at E5M2's three
significand bits it is a different function rather than a precision detail.

**`%` needs no rounding** but should still be routed for uniformity: a remainder of two
representable values is exactly representable, so narrowing is the identity there by construction
rather than by luck.

## Scope for this increment

Get the 32-bit rung right, leaving 16 and 8 as documented gaps needing software rounding. That flips
the V0.3.X line's one red test, `entry_abi_float.rs:163`, which they deliberately left red with the
cause named rather than driving it through the parameterised machine or picking values where the
widths agree.
