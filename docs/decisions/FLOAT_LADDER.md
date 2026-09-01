# The float ladder — `f64`, `f32`, `f16`, `f8`

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: ruled.** Received directly from the operator in session on 2026-08-31 by the V0.3.X line.
Not relayed, not read off another branch.

---

## The ladder

| rung | format | status |
|---|---|---|
| `f64` | IEEE 754 `binary64` | shipped |
| `f32` | IEEE 754 `binary32` | backend lowering built 2026-08-31; see [`F32_LOWERING_BRIEF.md`](./F32_LOWERING_BRIEF.md) |
| `f16` | **IEEE 754 `binary16`** | ruled, not built |
| `f8` | **OCP OFP8 `E5M2`** | ruled, not built |

## Why `E5M2` and not `E4M3`

**One semantic model at every rung.** `E5M2` keeps infinities, NaN and ordinary rounding, so the
ladder has no exceptional bottom. `E4M3` has **no infinities**, which would make division by zero a
per-format saturation rule and turn the last rung into a second semantic model.

**And `E5M2` shares `binary16`'s exponent field.** Both carry five exponent bits, so `f16` to `f8` is
a mantissa narrowing with the exponent untouched and `f8` to `f16` is exact. The ladder is one family
rather than three unrelated formats.

**Infinity earns its place, and this is the operator's stated reason.** A total language wants a
representable over-range outcome. `Fixed` has none: a sentinel steals a code point and is not
ordered, so comparisons stop meaning what they say. In `E5M2` a value beyond 57344 becomes infinity,
which compares, propagates and can be tested.

## What `f8` costs, recorded so nobody is surprised by it

**Three significand bits.** The relative step is **25 per cent at the bottom of each binade** and
about 14 per cent at the top. An earlier note in this line's correspondence said 12.5 per cent; that
is the BEST case, and the figure is corrected here.

**`Fixed<8>` remains the recommended default for control work**, and the two types are complementary
rather than competing. `Fixed<8>` gives uniform, exact resolution of roughly 0.4 per cent over a known
range. `f8` gives thirty orders of magnitude, 25 per cent steps, and an over-range value. **A
derating curve or a trip threshold wants `Fixed`.** Documentation that offers both without saying
which fits what will get `f8` used where it does harm.

## Arithmetic strategy, as ruled

> *"On architectures where an FPU is available, widen-calculate-truncate is preferred strategy.
> Minimizing rounding is ideal, but not worth spending excessive effort on. On architectures without
> an FPU, like the 6502, support just needs to be emulated."*

**Widen, compute in the wider type, narrow on the way back.** No mainstream hardware computes in
fp8, so this is what an accelerator does anyway. On a target with no floating-point unit at all the
whole surface is emulated, and the ruling explicitly accepts that.

### The one sub-decision that still has to be a single answer

**Not whether to minimise rounding, but what "narrow" DOES.** Round-to-nearest-even and
round-toward-zero differ by up to one unit in the last place, which at a 25 per cent step is not a
rounding nicety. The reference and the backend must do the same thing or the differential fails.

**It resolves for free.** LLVM's `fptrunc` and Rust's `as` both round to nearest, ties to even, so
**taking each toolchain's default makes the two sides agree at no cost**, which is exactly the
proportionate answer the ruling asks for. Recorded so it is a decision rather than a coincidence.

## Reserve the encoding space for a second eight-bit format

Pinning exactly one eight-bit format makes `float_bits_log2 = 3` unambiguously `E5M2`, which
dissolves an objection raised earlier: the header encodes a WIDTH and not a FORMAT, so two eight-bit
formats would have been indistinguishable.

**Reserve space for a discriminator anyway.** `E4M3` is what machine-learning silicon emits, so
interoperability may want it later, and reserving costs nothing now while adding it after the number
ships costs a `BYTECODE_VERSION` authorization.

## Preconditions, in order

1. **The runtime's arithmetic width must track its declared width.** Under `narrow-float-32` the
   module declares four bytes and the virtual machine computes in `f64`, because
   `pub type Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>` carries no `#[cfg]`. **Until that
   is fixed the differential oracle cannot validate any new rung**, at any width. Escalated by the
   `v0.2.3` line; the mechanism was verified on this branch rather than relayed.
2. **Each rung costs a full float differential surface.** `f32` surfaced four real defects on this
   line, every one a plausible wrong number rather than a fault. `f8` costs more than `f16` because
   emulation adds the narrowing question above.
3. **Linkage, for the ahead-of-time path.** On a target without hardware support LLVM lowers narrow
   float operations to compiler runtime calls. A linked C host will need those symbols, which is a
   packaging question the JIT path never asks. Worth checking when `f16` lands rather than at the
   link failure.
