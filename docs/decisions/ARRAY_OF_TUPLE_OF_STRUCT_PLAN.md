# Array-Of-Tuple-Of-Struct Equality — Design Blueprint

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Blueprint for `[(P, Word); 2] == [(P, Word); 2]` — an ARRAY whose element is a TUPLE whose element
is a STRUCT — and the same shape as a struct field (`struct S { g: [(P, Word); 2] }`).

Status: **PARTIALLY ADDRESSED — the SILENT MIS-COMPILE is fixed; full support is still open.**

2026-08-02: rather than build nested support first, the admission hole underneath it was closed.
Four constructs were being admitted and silently mis-compiled by the flat array-equality family;
all four now DEFER cleanly (a short primitive compare that diverges loudly and is caught by the
CLI's reference cross-check) instead of emitting a wrong comparison. Boundary +4 Gap, 0 Ok.

That was the right order: the flat family had NO admission guard at all, so every step toward
nested support would have been built over a construct set that silently compiled wrong. The
remaining work below — actual nested support — is unchanged in shape and is now safe to attempt
incrementally, because anything not yet supported rejects loudly.

**The four fixed mis-compiles**, each verified against the reference with a control:
- `[(P, Word); 2]` as a parameter (83 wrong ops against 113 reference)
- `struct S { g: [(P, Word); 2] }` (73 against 128)
- `[M; 2]` where `M` nests a struct (33 against 93)
- `struct S { a: [bool;2], w: Word }` — **the most dangerous shape found: it diverged at the SAME
  op count as the reference (58 against 58), differing only in content.** A length-based or
  count-based check would never have caught it; only the byte-identical oracle did.

## The measured divergence

`struct P { x: Word } fn f(a: [(P, Word); 2], b: [(P, Word); 2]) -> bool { a == b }` —
reference 113 ops, `local_count` 12; self-hosted 83 ops, `local_count` 8. Identical through op 14,
diverging at op 15 with the now-familiar signature:

```
R 15 GetTupleField(FlatNested { offset: 0, size: 8, variant: Struct })   <-- descends
R 16 GetLocal(4)
R 17 GetTupleField(FlatNested { offset: 0, size: 8, variant: Struct })
R 18 SetLocal(6)   R 19 SetLocal(7)   R 20 Loop(34)
R 22 GetField(Flat { offset: 0, kind: Int })                             <-- compares P.x

S 15 GetTupleField(Flat { offset: 0, kind: Unit })                       <-- WRONG, compares P as a scalar
S 16 GetLocal(4)
S 17 GetTupleField(Flat { offset: 0, kind: Unit })
S 18 CmpEq
```

The struct-field case `struct S { g: [(P, Word); 2] }` diverges the same way (73 vs 128 ops).

This is the **same silent mis-compile class** as `struct S { t: (P, Word) }` (see
[`STRUCT_TUPLE_OF_STRUCT_PLAN.md`](./STRUCT_TUPLE_OF_STRUCT_PLAN.md)): admitted, then the struct
element is compared as one scalar. The program compiles, verifies, and runs on the wrong bytes.

## Why this is bigger than it looks — it is NOT a reuse of the previous increment

The obvious guess is that this reuses the per-frame accessor machinery just built. It does not.
**The array-of-struct/tuple equality path is a different family.** `array_of_tuple_eq_start`
(parse.kel ~2159) drives a FLAT drain: it emits `StructEqField` records for scalar element fields
only and closes with `ArrayOfStructEqBuild`. There is no nested-field form in that path at all — no
`StructEqNested` header, no `se_stk_*` frame stack, and codegen's per-element unroll
(`push_arr_of_struct_inner`) emits a fixed two-level scalar loop.

So closing this gap means either:

- **(a) Give the array-of-struct/tuple family a nested form** — a nested record in that drain plus a
  nesting-capable per-element emitter. Self-contained but duplicates concepts the `StructEqNested`
  family already has.
- **(b) Route an array-of-composite element through the `StructEqNested` frame machinery**, making
  the per-element extract just another frame with its own accessor (`GetIndex`). Strictly better
  long-term — it would also subsume array-of-deep-struct and array-of-array-in-struct — but it is a
  refactor of a working, byte-identical path, so the regression surface is the whole array-equality
  family.

**Recommendation: (b), but scoped as its own increment with (a) as the fallback** if the byte-identity
is shown structurally unable to express what the reference emits. The switch criterion is
CONVERGENCE, not an attempt count: a long red branch is fine while each step narrows the
divergence.

## A necessary but NOT sufficient first step

`step_tuple_type` (tuple parameters) and `step_struct_tuple_field` (struct fields) both record
`tup_estruct` for a struct element. The array-of-tuple element scanner — the `ps.arr == 3` branch in
`header_sig` — does **not**. That must be added, mirroring the other two (reset to 0, then search
`sd_name`), or the element's struct identity is unavailable to any drain.

**Verified 2026-08-02: adding that alone changes NOTHING observable** (the op streams were
byte-for-byte unchanged), because the flat array-of-tuple drain never consults `tup_estruct`. It was
therefore reverted rather than committed as dead code. Add it as part of whichever drain change
lands, not before.

## Verification

Fixtures, all measured DIVERGE today:

- `fn f(a: [(P, Word); 2], b: [(P, Word); 2]) -> bool { a == b }` (113 ops, `local_count` 12)
- `struct S { g: [(P, Word); 2] }`, `fn f(a: S, b: S) -> bool { a == b }` (128 ops)

Must stay byte-identical (the regression surface, and it is large): `eq/array_of_tuple`
(`[(Word, Word); 2]`), `eq/struct_arrayofstruct`, `eq/array_in_struct`, `eq/array_of_array`, and the
`!=` forms.

**Tighten the admission in the SAME change.** The prior increment's lesson: teaching a drain to
descend without a matching admission guard converts a shallow silent bug into a deeper one. An
element struct whose subtree is impure must defer, mirroring `struct_subtree_pure`.
