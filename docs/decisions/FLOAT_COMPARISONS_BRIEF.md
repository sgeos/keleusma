# BRIEF — the reference says NaN equals everything, and LLVM does not

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The finding, made before writing anything

The reference compares floats with

```rust
x.partial_cmp(y).unwrap_or(core::cmp::Ordering::Equal)
```

**NaN collapses to `Equal`.** That is not IEEE-754 unordered behaviour and not LLVM's default. Emitting
the obvious `fcmp oeq` would make `NaN == x` **true on the reference and false natively** — a silent
divergence, exactly the class the whole float guard exists to prevent.

## What matching actually requires

`fcmp olt`, `ogt` and `one` are already false for NaN, which is what NaN-as-Equal implies. **Only
three predicates need adjusting**: with `nan = isnan(l) || isnan(r)`,

| predicate | value when `nan` |
|---|---|
| `Eq`, `Le`, `Ge` | **true** |
| `Lt`, `Gt`, `Ne` | false — already what the ordered comparison gives |

So the work is small, and it is only small *because* the semantics were checked first.

## The honesty problem this creates, and how to handle it

**No source construct produces a NaN today.** Division is the route, and `Op::CheckedDiv` is not
implemented — it pushes three values and is a larger slice. So the NaN adjustment will be **written to
match and not exercised by any differential**.

That is acceptable **only if stated**. Writing the ordered comparison instead, on the grounds that NaN
is unreachable, would repeat the exact mistake of last increment: relying on an accidental protection
that disappears the moment the neighbouring feature lands. **Write it to match; say it is unexercised.**

## Wrong turns to avoid

- **Do not emit the natural `fcmp` and call it correct.** The reference is unusual here, and matching
  the reference is the contract, not matching IEEE.
- **Do not claim the NaN path is verified.** No probe can reach it yet. State that plainly rather than
  letting a green differential imply coverage it does not have.
- **Do not implement division to make NaN reachable.** That is a different slice with a three-value
  push convention, and bundling it would make both harder to verify.
- **Do not relax the operand whitelist.** Comparisons become float-aware; everything else still fails
  closed at the use.
- **Expect the censuses NOT to move.** No corpus module compares floats, so a movement would mean
  something unexpected happened and needs explaining rather than celebrating.
- **Check the pins that name the comparison opcodes** before assuming they still hold.

## What good looks like

Float comparisons agree with the reference on every value a source can produce, the NaN adjustment is
present and documented as unexercised, and everything else that could consume a float still refuses.
