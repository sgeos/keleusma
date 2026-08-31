# BRIEF — a float inside a composite, sized from the boundary this time

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why this needs no ruling, and why that is a claim rather than a convenience

**A composite body field is INTERNAL.** The compiler packs it, the same program reads it back, and
nothing outside the running program sees the layout, so the backend agreeing with the reference is a
**fact to be measured**, not an application binary interface to be chosen. That is the exact argument
`alloc_format_kind` already records for why `Fixed` is lowered in a body and refused in a shared
slot, and it is why this increment does not touch the open `Fixed`, `Text`, `Opaque` and `Unit`
questions.

## The boundary, MEASURED before this brief was written

`probe_float_composite.rs` was written first, deliberately, because this line has sized work from the
component being changed five times and been wrong every time. It reports:

| case | result |
|---|---|
| struct with a float field | refused at **`Op::NewComposite`**, by the float whitelist |
| the float field READ back | same refusal, at construction, before any read |
| tuple carrying a float | same |
| array of floats | same |
| control, the same struct with no float | **LOWERS** |

**The reference already lays a `Float` field out at eight bytes**: `struct { x: Float, n: Word }`
compiles at `byte_size: 16`, identical to a pair of words. So there is no packing decision to make —
only a width to stop calling unknown.

## What the change is, and it is three places

1. **The whitelist.** `Op::NewComposite` is not named float-aware, so a float operand refuses before
   the arm runs. The pack itself needs nothing: it places each operand at the operand's OWN width,
   and a float already carries `Width::Scalar(8)` from `push_k`. **It is a bit copy into the body.**
2. **`Op::GetField(Flat { kind })`** falls to a catch-all for a float kind. It needs an arm that
   loads eight raw bytes at alignment 1, exactly as `Int` and `Fixed` do, and **pushes the operand
   TAGGED `Float`**.
3. **`Op::GetIndex(ArrayElem::Flat { kind })`**, the same, for an array of floats.

There is no `SetField` or `SetIndex` opcode — a composite is constructed whole — which bounds the
increment.

## Prior failures to avoid repeating

- **The tag is the half that is forgotten.** It was missing from the entry ABI's plan and from the
  shared slot's, twice producing a correct layout whose values every float operation then refused.
  A read arm that does not tag is not finished.
- **Width guard, like every other float route.** Only an eight-byte `Float` is lowered; any other
  width is refused loudly rather than approximated, because the reference's body layout is sized by
  `float_bytes` and a wrong width mispacks silently.
- **Do not widen `width_of_tag`'s `Float` case as part of this.** That function types a declared
  PARAMETER, not a body field, and a float parameter's width already comes from the entry ABI's
  seeding. Widening it here would be a change whose justification lives in another increment.
- **Do not verify by acceptance.** A mispacked body returns a plausible number. The oracle is the
  differential against the virtual machine, with values whose bit patterns discriminate: the
  infinities, a negative zero and a NaN, produced from RUNTIME arguments so nothing is folded.
- **Confirm a mutation APPLIED before believing it.** Print the changed line. `\b` is a GNU extension
  and this is a Darwin box.

## Prediction, recorded before building

The instinct was that the censuses would MOVE, because `NewComposite`, `GetField` and `GetIndex` are
corpus opcodes. **Measured instead of assumed, and the instinct was wrong.** Over the 69 compiling
corpus modules there are **256 composite construction sites and ZERO float field or element reads**,
read from the instruction stream rather than from source text. So the censuses stay put and a
movement would be a regression rather than a gain.

**The 256 is an unplanned third confirmation of a figure the tree already carries.** The handoff
records 256 composite sites across 35 chunks, derived by the region planner's placements and by a
raw scan of the instruction stream. This sweep is a third method over the same population and it
agrees.

## Outcome, written after the build

**Landed in the three places the boundary named, and nowhere else.** The whitelist admits
`Op::NewComposite` as a bit copy into the body; `Op::GetField(Flat)` and `Op::GetIndex(Flat)` gained
float arms that load eight raw bytes at alignment 1 and **push the operand tagged `Float`**.

**Sizing from the boundary worked.** The probe was written before this brief, it named
`Op::NewComposite` as the blocker, and the implementation touched exactly what it named. That is the
first increment on this line in six where the plan and the work agreed.

**A harness was promoted rather than copied.** `composite_return_aliasing.rs` held the only
two-argument module driver that supplies the trailing region pointer. A second file needing it had
two options, and a private twin is how two tests come to answer one question differently without
saying so. It now lives in `common::vm_and_native_two_arg` with one caller each side.

**Evidence**: five tests in `float_composite.rs` over a struct field, a field read used in float
ARITHMETIC rather than merely returned, a tuple, an array whose stride is exercised by returning the
second and third elements, and a must-fire width refusal made reachable by overwriting the module's
`float_bits_log2`. Every value comes from runtime arguments and the set includes both infinities, a
negative zero and a NaN.

**Two mutations, each confirmed applied by printing the changed line**: halving the float element
stride from eight to four fails the array test; deleting the field read's `Float` tag fails four of
the five.

### The nested case was mis-stated, and measuring it corrected the statement

This section first read *"not covered: a float inside a NESTED composite body, which goes through the
`FlatNested` arms"*. **That was wrong in a way worth naming.** Nested bodies were never a separate
implementation: the outer read yields a body and the LEAF read goes through the very flat arms this
increment added, so nesting lowered the moment the flat case did. What was missing was **evidence,
not code** — and an accepted-but-unverified path is the more dangerous of the two shapes, because a
refusal is loud while a wrong float is a plausible number.

Measured: a struct holding a struct holding a float, an array of structs each holding a float, and a
nested read used in float ARITHMETIC all **agree with the reference** over the same discriminating
values. The tag mutation fails the nested tests too, so the coverage is not vacuous.

**Not covered, so a green file is not read as more than it is**: a composite carrying a float
reaching a data slot, and any `Float` width other than eight, which is refused.
