# BRIEF — finish the float scalar surface, and defer the entry ABI honestly

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## What remains of the scalar surface

Implemented: constant, both conversions, `Add`/`Sub`/`Mul`/`Div`, all six comparisons. **Missing:
`Neg` and `Mod`**, both of which the reference defines and both of which are total:

| opcode | reference | LLVM |
|---|---|---|
| `Neg` on a float | `-x` | `fneg` |
| `Mod` on a float | `x % y`, Rust's truncated remainder | `frem` |

No traps, no zero check, no flag convention. These are the last two pieces of scalar float arithmetic
and they are small.

## Why the entry ABI is NOT this increment, stated rather than skipped

It is the piece the operator's ruling names, and it is deferred for a measured reason:
**`lower_chunk` receives `chunk.param_types` but the chunk carries NO return type** — the return lives
in module-level `ChunkSignature`, which a single-chunk lowering never sees. So the entry ABI needs the
parameter types, the return type, the prologue's bitcasts, `Op::Return`, and `Op::Call` **together**,
across both entry points. That is a scoped plan, not a slice to squeeze in beside an absorption.

**Deferring it is a decision, not an oversight**, and it should be recorded as one so the next
increment starts from the constraint rather than rediscovering it.

## Wrong turns to avoid

- **Do not assume `frem` matches.** It does — Rust's `%` on `f64` is the truncated remainder with the
  sign of the dividend, which is `frem` — but that is checked, not assumed, and the differential must
  include **negative dividends**, where a floor-style remainder would differ.
- **Do not forget the whitelist.** `Neg` and `Mod` must be added, or they will refuse at the operand
  even once lowered. `Mod` is currently the unsupported-opcode subject in `float_differential.rs`, so
  **that subject retires and needs a successor**.
- **Do not let the `Neg` arm reach the integer path.** It has its own width-based refusal that would
  otherwise fire, or worse, negate the bit pattern as an integer.
- **Expect censuses not to move.** No corpus module negates or takes the remainder of a float.
- **Do not claim the scalar surface is "complete" without naming what is still absent**: the entry
  ABI, float shared slots, `f32`, and composites containing floats.

## What good looks like

`Neg` and `Mod` agree with the reference including on negative operands; the unsupported-opcode
subject moves to something still refused; and the entry ABI's deferral is on record with its reason.
