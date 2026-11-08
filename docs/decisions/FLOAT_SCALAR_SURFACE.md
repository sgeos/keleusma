# The float scalar surface is complete, and what is not

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: scalar float arithmetic is implemented and differentially verified. The entry ABI — the
piece the operator's ruling names — is NOT built, and is deferred for a measured reason.**

## What is now lowered and verified against the reference

| | |
|---|---|
| constants | ✓ |
| `IntToFloat`, `FloatToInt` | ✓ — the conversion **saturates**, matching Rust's `as` |
| `Add`, `Sub`, `Mul` | ✓ |
| `Div`, `Mod` | ✓ — both total; `Mod` is `frem` |
| `Neg` | ✓ — `fneg` |
| all six comparisons | ✓ — matching the reference's **two different NaN conventions** |

Every one is checked by running the same program on the reference and on the lowered code.

## Two semantics that would have been wrong if assumed

**`Mod` is the TRUNCATED remainder**, carrying the sign of the dividend — Rust's `%` on `f64`, which is
`frem` and not a floored remainder. `-7.0 % 2.0` is `-1.0`, not `+1.0`. A probe with only positive
operands cannot tell the two conventions apart, so the differential uses negative dividends and a
must-fire control requires the positive and negative probes to have opposite signs.

**`Neg` needed its own branch.** The existing arm dispatches on WIDTH, and a float is eight bytes like
a `Fixed`, so without a kind check it would have negated the **bit pattern as an integer** — flipping a
mantissa bit rather than the sign.

## The entry ABI: deferred, with the reason

`lower_chunk` receives `chunk.param_types`, so parameter types are available. **The chunk carries no
RETURN type** — that lives in module-level `ChunkSignature`, which a single-chunk lowering never sees.

So the entry ABI cannot be done by halves: parameter types, return type, the prologue's bitcasts,
`Op::Return`, and `Op::Call` all have to land together, across both entry points. **That is a scoped
plan rather than a slice to squeeze in beside an absorption**, and recording the constraint means the
next attempt starts from it instead of rediscovering it.

The signature route of the module guard therefore stays closed, and is now the unsupported-opcode
subject in `float_differential.rs` — **the fourth in that file's succession**, after composite
construction and friends, division, and remainder.

## What is still absent, named so the surface is not read as finished

- **the entry ABI** — no corpus module has a float in a signature, so it also has no witness;
- **float shared slots** — an open ABI question the operator has not settled;
- **`f32`** — only the 8-byte width is lowered, and any other is refused rather than approximated;
- **floats inside composites**.

**Censuses were not expected to move and did not**: no corpus module negates or takes the remainder of
a float.
