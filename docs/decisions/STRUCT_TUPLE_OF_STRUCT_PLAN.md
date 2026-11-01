# Struct-Field-Tuple-Of-Struct Equality — Design Blueprint

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Blueprint for closing the `struct { t: (P, Word) }` equality gap in the self-hosted compiler: a
struct field that is a TUPLE whose element is itself a STRUCT. For example
`struct P { x: Word }`, `struct S { t: (P, Word) }`, `fn f(a: S, b: S) -> bool { a == b }`.

Status: **COMPLETE — implemented, byte-identical, boundary 67 -> 69 Ok (and +1 deliberate Gap).**
Implemented 2026-08-02. The diagnosis below is retained because it is the record of a
silent-mis-compile bug, not merely a coverage gap.

**Outcome versus plan.** The stage split held, with one addition the scouting missed: parse.kel
needed a SECOND edit, because `step_struct_tuple_field` never recorded `tup_estruct` at all (only
`step_tuple_type`, for a tuple PARAMETER, did), so a struct element of a struct FIELD's tuple had no
recorded identity for the drain to find. The first edit alone changed nothing observable — the new
branch was unreachable. reconstruct.kel needed NOTHING, as predicted. codegen.kel needed the
per-frame accessor, as predicted, and cost four lines: the suffix extract takes its accessor from
`es_acc[top - 1]` (the PARENT frame), since it reads the child out of its parent container.

**The admission guard proved load-bearing, not defensive.** Once the drain could descend, an element
struct containing a tuple, array, or enum was descended into and then mis-lowered — recreating the
very bug being fixed, one level deeper. `struct_subtree_pure` on the element makes those defer.
Pinned by `struct_tuple_of_impure_struct_element_defers` and one deliberate `Gap` boundary case.

## Why this one is higher-risk than its size suggests

**The admission ADMITS it and the drain then emits a silently WRONG comparison.** This is the same
class the 3-level struct increment was caught by, and it is only visible through the byte-identical
oracle — the program still compiles, verifies, and runs. It just compares the wrong bytes.

`struct_eq_kind` (parse.kel ~2343) defers a tuple-typed field only when some element's `tup_ekind`
is `>= 100`:

```
if structdefs.sd_ftuple[fidx] > 0 {
    stmt.sq_found = 1;
    let t = structdefs.sd_ftuple[fidx] - 1;
    for g in 0..tupledefs.tup_ecount[t] limit 64 {
        if tupledefs.tup_ekind[tupledefs.tup_estart[t] + g] >= 100 { stmt.sq_scan = 0; }
    }
}
```

A STRUCT element has `tup_ekind == 0` (`scalar_kind_of` of a struct name is 0/Unit) and carries its
identity in `tup_estruct` instead, which this scan never consults. So the field is admitted as a
plain nested tuple, and the sub-field drain then treats the struct element as a scalar.

## The measured divergence (the byte-identity target)

For `struct P { x: Word } struct S { t: (P, Word) } fn f(a: S, b: S) -> bool { a == b }` the
reference emits 59 ops with `local_count` 8; the self-hosted pipeline emits 44 with `local_count` 6.
They agree through op 11 and diverge at op 12.

Reference, extracting the struct ELEMENT out of the tuple and recursing (note the accessor):

```
R 11 Loop(50)
R 12 GetLocal(5)
R 13 GetTupleField(FlatNested { offset: 0, size: 8, variant: Struct })   <-- tuple accessor
R 14 GetLocal(4)
R 15 GetTupleField(FlatNested { offset: 0, size: 8, variant: Struct })
R 16 SetLocal(6)          <-- r2' then l2', monotonic (+2 temps)
R 17 SetLocal(7)
R 18 Loop(32)
R 19 GetLocal(7)
R 20 GetField(Flat { offset: 0, kind: Int })                             <-- struct accessor
R 21 GetLocal(6)
R 22 GetField(Flat { offset: 0, kind: Int })
R 23 CmpEq  … R 31 EndLoop(19)
R 32 Not / If / Const(0) / Break(50) / EndIf        <-- the element's negate-break block
R 37 GetLocal(5)
R 38 GetTupleField(Flat { offset: 8, kind: Int })   <-- the trailing scalar element
```

Self-hosted, comparing the struct element as if it were a scalar:

```
S 12 GetLocal(5)
S 13 GetTupleField(Flat { offset: 0, kind: Unit })   <-- WRONG: kind Unit, no descent
S 14 GetLocal(4)
S 15 GetTupleField(Flat { offset: 0, kind: Unit })
S 16 CmpEq
```

## Where each stage is wrong

0. **The exact parse.kel edit is already written out below.** `se_pop_cascade` (~2480) advances
   `se_subcur` when the stack empties, which is correct for a tuple sub-field too, so the pushed
   frame needs no cascade change.

1. **parse.kel — the `se_subistuple` sub-field drain (~2593).** The struct-field-is-a-tuple branch
   emits a scalar record unconditionally, ignoring `tup_estruct`:

   ```
   if stmt.se_subistuple == 1 {
       stmt.se_subcur = stmt.se_subcur + 1;
       Node::StructEqNestedField() as Word + (tup_eoffset[fidx] + tup_ekind[fidx] * 65536) * 64
   }
   ```

   It must mirror the sibling `sd_fstruct` branch (~2597): when `tup_estruct[fidx] > 0`, emit the
   sentinel header `(tup_eoffset[fidx] + (100 + sd_bytesize[s]) * 65536)`, allocate `r2` then `l2`
   monotonically, and PUSH a frame. **The existing `se_stk_*` frame machinery already fits** — the
   element IS a struct, so its sub-fields read `structdefs.sd_*` exactly as a nested struct field
   does. This stage is expected to be small.

2. **reconstruct.kel — probably NO change.** The recursive `seb` grammar from the 3-level struct
   increment already accepts a nested sub-field block `[off, 100+size, subcount, r2, l2, field*]` at
   any depth. Verify against the depth-1/2 fixtures first; only if the bytes differ does this need
   work.

3. **codegen.kel — the real change: a PER-FRAME ACCESSOR.** The `es_*` reverse-DFS emitter
   hardcodes `getfield` for a nested sub-field's extract. Here the extract of the struct element out
   of its parent TUPLE must be `GetTupleField` (R13/R15) while the extract of `x` out of `P` stays
   `GetField` (R20/R22). So each emit frame must carry an accessor variant (Tuple vs Struct) chosen
   by the PARENT container's kind, and the `seb` block must thread that variant through.

   This is precisely the "per-frame accessor/variant" that an earlier handoff predicted for
   tuple-in-tuple and that turned out to be unnecessary there. It is genuinely required here.

   **NO new record encoding is needed, and therefore this is NOT an inter-stage-protocol change**
   (which would be a hard stop per the loop's stop list). Verified 2026-08-02: the header a struct
   field that is a tuple emits already carries FlatNested **variant Tuple (0)** — parse.kel ~2699
   omits the variant term, which encodes 0 — so reconstruct and codegen already receive the parent
   block's kind. The emitter can therefore CHOOSE the accessor from the enclosing `seb` block's
   existing variant rather than from a new sentinel: variant Tuple → `GetTupleField`, variant
   Struct → `GetField`. Thread the parent variant down the `es_*` frame stack; do not invent a new
   sentinel range, and do not add a record or node kind.

4. **Admission.** Widen `struct_eq_kind`'s tuple branch to consult `tup_estruct` and require
   `struct_subtree_pure` of the element struct, so a deeper or mixed element still defers rather
   than being admitted and mis-lowered.

## Byte-identity pivots (the usual traps)

- **Monotonic slot order.** Temps allocate depth-first, `r2` before `l2`, +2 per composite element,
  never rewound — matching the reference `next_slot`. The measured `local_count` target is 8.
- **Constant interning order.** `false` then `true` per composite compare; deeper levels add no new
  constants once the first nested field has interned both.
- **Recursion is forbidden** (verifier R4), so the descent stays an explicit stack, never a
  self-call.

## Verification

Depth-1/2 fixtures byte-identical FIRST after each stage edit (a regression there means the
generalization changed existing output — a stop), then the three new fixtures, all measured
DIVERGE today and all of which must become IDENTICAL:

- `struct P { x: Word } struct S { t: (P, Word) }` (59 ops, `local_count` 8)
- `struct P { x: Word } struct S { t: (P, P) }` (74 ops)
- `struct P { x: Word } struct S { t: (P, Word), w: Word }` (69 ops)

Then the boundary case as `SOk`, the whole self-compile suite, and the FULL
`scripts/release-gate.sh` before the no-ff merge.

## Related

The same per-frame-accessor machinery, once built, is what several remaining gaps need
(array-of-tuple-of-struct, and mixed subtrees where the parent container kind varies). See the
measured queue in [`../process/AUTONOMOUS_IMPLEMENTATION_LOOP.md`](../process/AUTONOMOUS_IMPLEMENTATION_LOOP.md)
and the frontier map in [`../process/REVERSE_PROMPT.md`](../process/REVERSE_PROMPT.md).

## The concrete parse.kel edit (stage 1)

Replace the `se_subistuple == 1` branch's unconditional scalar emission with a `tup_estruct` check
that mirrors the sibling `sd_fstruct` branch (~2597). Note it must NOT advance `se_subcur` — the
cascade does that when the pushed frame pops:

```
if stmt.se_subistuple == 1 {
    if tupledefs.tup_estruct[fidx] > 0 {
        let s = tupledefs.tup_estruct[fidx] - 1;
        stmt.se_stk_start[0] = structdefs.sd_fstart[s];
        stmt.se_stk_count[0] = structdefs.sd_fcount[s];
        stmt.se_stk_cur[0] = 0;
        stmt.se_stk_r2[0] = ps.pcount + stmt.slot_count;
        stmt.slot_count = stmt.slot_count + 1;
        stmt.se_stk_l2[0] = ps.pcount + stmt.slot_count;
        stmt.slot_count = stmt.slot_count + 1;
        stmt.se_stk_phase[0] = 1;
        stmt.se_sp = 1;
        Node::StructEqNestedField() as Word
            + (tupledefs.tup_eoffset[fidx] + (100 + structdefs.sd_bytesize[s]) * 65536) * 64
    } else {
        stmt.se_subcur = stmt.se_subcur + 1;
        Node::StructEqNestedField() as Word
            + (tupledefs.tup_eoffset[fidx] + tupledefs.tup_ekind[fidx] * 65536) * 64
    }
} else {
```

`se_sp = 1` (rather than an increment) is correct and matches `sd_fstruct`: this branch is only
reached with the frame stack empty, since a non-empty stack is handled by the earlier `se_sp > 0`
path.
