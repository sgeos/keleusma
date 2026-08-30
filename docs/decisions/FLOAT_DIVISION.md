# Float division, and the NaN semantics I had wrong

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: division implemented and verified. A comparison defect from the previous increment was found
and fixed — by the test that increment said it could not write.**

## Two corrections to my own recorded design

### 1. Division does NOT go through `CheckedDiv`

The previous increment recorded that float division "flows through `Op::CheckedDiv`'s three-value push
convention", and declined the slice on that basis. **That was wrong for the `/` operator.** The
compiler emits plain **`Op::Div`**, and the reference's float arm is a bare `x / y` with **no zero
check** — total, matching `fdiv` exactly. The lowering is a bitcast pair around one instruction.

The `CheckedDiv` reasoning was not wrong about `CheckedDiv`; it was wrong about which opcode `/`
emits. **It was read from the virtual machine's arm rather than from what the compiler produces**, and
compiling one line of source would have settled it.

`Op::Mod` on floats is still refused — the reference's float remainder was never checked here.

### 2. The reference has TWO comparison paths with DIFFERENT NaN semantics

| opcodes | path | NaN behaviour |
|---|---|---|
| `CmpEq`, `CmpNe` | **`PartialEq`** | ordinary IEEE — NaN equals nothing, so `!=` is **true** |
| `CmpLt`, `Gt`, `Le`, `Ge` | **`compare_op`** = `partial_cmp(...).unwrap_or(Equal)` | **NaN as Equal** |

The previous increment read only `compare_op` and applied NaN-as-Equal to `Eq`, `Le` and `Ge`. **`Eq`
was wrong**, making `NaN == x` true natively and false on the reference. And `Ne` needed the
**unordered** predicate, since `PartialEq::ne` is true for a NaN while `ONE` is false.

Corrected: `Eq` → ordered equal, `Ne` → **unordered** not-equal, `Le`/`Ge` → ordered with a NaN
override, `Lt`/`Gt` → ordered unchanged.

## The part that matters most

**That defect was written blind and declared as such.** The previous increment stated plainly that the
NaN adjustment rested on reading the reference rather than on a differential, because nothing could
produce a NaN — the route was division, and division was unimplemented.

**Implementing division made it reachable, and the very first NaN test caught the error.**

Two things made that work: saying the path was unexercised rather than letting a green suite imply
coverage, and writing the test the moment the feature that unblocks it landed. **Had the honesty been
skipped, the divergence would have shipped silently** — `NaN == x` is not a value anyone inspects.

## What is verified now

- **Division** over eight probes, including negatives.
- **Division by zero**: `+inf`, `-inf` and NaN, converted through the saturating cast to `i64::MAX`,
  `i64::MIN` and `0`. A non-vacuity check requires the three probes to give three different answers.
- **All six predicates against a NaN**, which is the test that could not exist before this increment.

## Still not built

The entry ABI, float shared slots, `Op::Mod` on floats, and `f32`.
