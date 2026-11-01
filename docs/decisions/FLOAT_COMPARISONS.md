# Float comparisons: the reference says NaN equals everything

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: implemented and differentially verified for every value a source can produce. The NaN path
is written to MATCH and is UNEXERCISED**, and that is stated rather than left for a green test to
imply.

## The finding, made before writing anything

The reference compares floats with

```rust
x.partial_cmp(y).unwrap_or(core::cmp::Ordering::Equal)
```

**A NaN collapses to `Equal`** — equal to everything, rather than unordered. That is neither IEEE-754
behaviour nor LLVM's default. Emitting the obvious `fcmp oeq` would make `NaN == x` **true on the
reference and false natively**: a silent divergence, which is the class the float guard exists to
prevent.

## Matching it is small, and only because it was checked first

`olt`, `ogt` and `one` are already false when either operand is NaN, which is exactly what
NaN-as-Equal implies. With `uno = isnan(l) || isnan(r)`:

| predicate | when `uno` |
|---|---|
| `Eq`, `Le`, `Ge` | forced **true** |
| `Lt`, `Gt`, `Ne` | false — already what the ordered form gives |

Three predicates adjusted, three left alone. Had the semantics not been read first, the natural
implementation would have been wrong in three of six cases and silent about it.

## What is verified, and what is not

**Verified**: all six predicates against the reference, over seven probes each, with operands whose
fractional part decides the answer — so a comparison accidentally performed on the integer bit pattern
would disagree. A must-fire control confirms the probes discriminate rather than all giving one answer.

**NOT verified**: the NaN adjustment. **No source construct produces a NaN**, because the route is
division and `Op::CheckedDiv` is not lowered — it pushes three values and is a larger slice.

**It was still written to match rather than left to diverge.** Relying on NaN being unreachable is the
accidental protection this backend already lost once, when implementing float arithmetic removed the
block that had been supplied by the *absence* of an implementation. Its correctness rests on reading
the reference, not on a differential, and that is the weaker footing of the two.

## Scope

Comparisons joined the operand whitelist. Everything else that consumes a float and was not written
for one still refuses — **division in particular**, pinned in `float_differential.rs`.

**Censuses were not expected to move and did not**: no corpus module compares floats.
