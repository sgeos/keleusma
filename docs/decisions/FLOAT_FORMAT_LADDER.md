# The float format ladder, and the header field that cannot name it

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: the plan is stated by the operator and recorded here. Nothing is implemented. One
gap in the wire format is identified and is **cheap to close before publication and expensive
after**.

## Provenance

Stated by the operator directly, in session, on 2026-08-31, to this line. The operator noted a
belief that the `v0.3.0` line had already documented it. **It had not.** A search of every
remote branch for `E5M2`, `E4M3`, `OFP8` and `OPF8` returned no match, so this file is the
first record. The operator's own wording carried a hedge, and the hedge was correct.

This matters because a plan believed to be written down is a plan nobody writes down.

## The plan as stated

The runtime is to support a ladder of float formats, narrowing from the present 64-bit and
32-bit pair:

| Declared width | Format |
|---|---|
| 64 | IEEE 754 binary64 |
| 32 | IEEE 754 binary32 |
| 16 | IEEE 754 binary16 |
| 8 | OFP8 E5M2 |

With 8-bit Q-format fixed point available at the low end as a separate option, riding on the
`Word` axis rather than the float axis.

Both the storage width and the arithmetic width follow the module header. The operator
confirmed that reading on 2026-08-31.

## The gap: the header records a width, not a format

`HeaderRecord` in `src/wire_schema.rs` carries `float_bits_log2` as a bare log2 width. There is
no format discriminant anywhere in the header.

**A width does not determine a format at either of the two new rungs.**

- At 8 bits, OFP8 standardises **two** formats, E5M2 and E4M3. They differ in exponent and
  mantissa split and are not interconvertible without loss.
- At 16 bits, IEEE binary16 and bfloat16 are both in wide use and differ likewise.

The plan names E5M2 and IEEE binary16, so the mapping is unambiguous **today**, by convention
rather than by encoding. It stops being unambiguous the moment a second format at either width
is wanted, and at that point the header must change.

The header has a spare `flags: u8` byte that is a candidate site. Whether the discriminant
belongs there or in a new field is a design question this document does not settle.

**Why the timing is the point.** `BYTECODE_VERSION` is 2 and nothing has been published at 2.
Header changes accumulate under the current number for free until the next publication and cost
a version 3 afterwards. Deciding whether the format is encoded or conventional is therefore a
pre-publication decision, and it is grouped with the `ScalarKind::Text` change that `Text<N>`
enables for the same reason.

## What the ladder forces, and it is not a preference

**Rust has no `f16` on stable.** Confirmed against rustc 1.98.0, which rejects the type as
unstable. There is no `f8` of any kind and there will not be one.

The `Float` trait currently works by pairing each declared width with a real Rust type, with
`impl Float for f32` at `BITS_LOG2 = 5` and `impl Float for f64` at 6. **That model cannot
reach 16 or 8**, because there is no type to implement the trait for.

So the ladder can only be built the way the `Word` axis is already built. Compute in the widest
type the build offers, then narrow the result to the declared format after each operation. For
words this already exists as `checked_arith_outputs` in `src/vm.rs`, which reads
`word_bits_log2` from the header and narrows to the declared width at runtime.

This converts the float-width question from a design preference into a constraint. Header-driven
narrowing is not the more elegant of two options for floats. Below 32 bits it is the only option.

## Specific wrong turns to avoid

**Do not chain narrowing steps.** Rounding a 64-bit intermediate to the declared format must go
directly from the wide value to that format. Narrowing through an intermediate rung, as in a
64-to-32-to-16 chain, is a double rounding and produces results that differ from correct single
rounding. This is the most likely way to get a plausible-looking implementation that is wrong in
the low bits.

**Do not treat rounding after each operation as an optimisation to skip.** At E5M2 the mantissa
is two bits, giving roughly three significant bits. The difference between rounding per
operation and rounding once at the end is not a precision nicety at that width. It is the whole
semantics of the format.

**Do not assume the storage path already covers this.** Storage narrowing exists and arithmetic
narrowing does not. Three sites in `src/bytecode.rs` write and read a declared 4-byte float
through `f32`, and no site narrows an arithmetic result. The present consequence is that a
32-bit float rounds when it passes through a composite field and does not when it stays on the
operand stack.

**Do not read the existing narrow presets as ready.** `Target::embedded_16` and
`Target::embedded_8` both declare `has_floats: false`. Adding the two new rungs changes what
those presets mean, rather than filling in something they already promised.

## Open questions, none of them settled here

1. Whether the format is encoded in the header or fixed by convention at each width.
2. What a native registered as `fn(f64) -> f64` observes when it serves a module declaring an
   8-bit float. Rounding at the boundary is required or the ABI launders precision, which is the
   same shape as the string ABI question.
3. How the per-operation narrowing cost enters the calibrated cost models in `keleusma-bench`.
   The cost is deterministic and therefore bounded, so this is calibration rather than a threat
   to the worst-case-execution-time claim.
4. Whether 8-bit Q-format fixed point needs anything beyond the existing `Word` axis, which
   already reaches 8 bits.
