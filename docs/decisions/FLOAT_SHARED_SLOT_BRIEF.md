# BRIEF — the float shared slot, which is a kind tag and a width guard

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## What is being built, and on whose authority

The `Float` **shared data slot**, kind tag 5. [`ABI_RULINGS.md`](./ABI_RULINGS.md) records the
operator's Option A ruling and states that it **also settles this slot**: with a real floating-point
representation the slot is IEEE-754 bytes at the stated offset, size `float_bytes`. That is the
settled half of a ruling already received. Nothing ambiguous is being decided here — `Fixed`, `Text`,
`Opaque` and `Unit` stay open and stay refused.

## Why it is smaller than the entry ABI was

**There is no representation change at this boundary.** The reference decodes a shared scalar through
`GenericValue::read_scalar_le`, whose `Float` arm at `float_bytes == 8` is `f64::from_le_bytes` over
the eight bytes at the slot's offset. The backend's operand stack already carries a float **as its
bit pattern in an `i64`**. So the load is the same eight-byte load a `Word` slot gets, and the store
is the same eight-byte store.

**The one thing that is not bits is the TAG.** A float read must push `OperandKind::Float`, or the
application binary interface is correct and the value unusable — every float operation downstream
refuses on an untagged operand. This is the same detail the entry ABI needed for a float parameter's
local, and it was not in that plan either.

## The write direction needs no kind check, and that is a conclusion

**The first draft of this brief specified one, in both directions, and it was wrong.** `Op::Call`
refuses a kind-versus-declaration disagreement because the bitcast to a floating-point parameter type
is a **representation** change and the wrong one yields a plausible number. Nothing converts at a
shared slot. The operand already is the bit pattern, so the same eight bytes are stored whatever the
tag says, and a guard could therefore prevent no wrong byte.

**What it would do instead is refuse a valid program.** A PRIVATE float slot's read is not
kind-tracked — the private path pushes untagged — so `s.x = h.f` arrives tagged `Int` while carrying
correct float bits. Under the guard that program is refused. Without it, it lowers and stores the
right bytes. It is pinned agreeing with the reference in `shared_data.rs` rather than left as an
assertion.

**The residual, named rather than left implicit**: a private float slot's value is unusable in
arithmetic, because the untagged read makes every float operation refuse at the operand. That
predates this increment and this increment does not close it.

## What stays refused, and none of it by omission

A float slot in a module whose `Float` is not eight bytes wide, refused loudly exactly as every other
float route does rather than approximated. `Fixed`, whose gap is the host-visible **scale** and not
the representation. `Text`, `Opaque`, `Unit`. Shared composite bodies.

## Prior failures to avoid repeating

- **Do not verify by acceptance.** A float slot lowered at the wrong offset or the wrong width
  returns a plausible number. The evidence is `shared_data.rs`'s oracle, which compares the host
  buffer **byte for byte** after both runs, and which exists because a store is an effect rather than
  a result.
- **Pick values that discriminate.** The byte comparison pins the exact bit pattern, so NaN, the
  infinities and signed zero are the interesting inputs. A test over small positive values proves
  little, which is the symmetry trap this package has fallen into three times.
- **Do not measure the corpus by grepping source text.** A crude `awk` over the corpus says no module
  declares a float in a data block. That instrument can be wrong, and the fourth instrument error on
  the sibling line was exactly this shape. The scope pin reads the module's **layout table**, which
  is the data itself.
- **Route 4 of `float_guard_routes.rs` rotates.** It currently asserts that READING a float slot
  refuses, and its own text instructs a reader to update the claim rather than delete it. The route
  enumeration is asserted, so the rotation is forced rather than optional.

## Prediction, recorded before building

**Censuses unmoved.** No corpus module declares a float shared slot, so this route has zero corpus
witnesses and the hand-built subjects are the entire population — the same shape the entry ABI had.
A movement in `isa_lowering`, backend coverage, or the corpus differential needs explaining rather
than accepting.

## Outcome, written after the build

**Landed, and the mechanism was as scoped: an eight-byte load, an eight-byte store, and a tag.**
`resolve_shared_scalar` admits `SCALAR_FLOAT_TAG` at an eight-byte `Float`, `shared_scalar_width`
gives it an eight-byte stride so a float array's contiguity proof works, and the read pushes
`OperandKind::Float`.

**THE SCOPE WAS WRONG ABOUT WHAT GATES THE OPERATION, AND THREE OF FOUR TESTS FAILED ON IT.** A
**whitelist** ahead of the opcode dispatch refuses any opcode consuming a `Float`-tagged operand
unless it is named float-aware, and neither `SetData` nor `SetDataIndexed` was named. This brief was
written from `resolve_shared_scalar`, which decides how a slot is ADDRESSED, not whether the opcode
may run. The requirement lived where the operand is admitted, and reading the consumer first is the
standing rule that would have found it.

The admission added is **positional**, like `Op::Call`'s: float-aware only when the target is a float
slot, because a blanket entry would also admit a float stored into a `Word` or `Byte` slot.

**A private slot is admitted on a separate ground** — its layout is this backend's own and the store
is a full-width bit copy — with the residual named: a private slot's read is not kind-tracked, so a
float stored there returns tagged `Int` and every float operation on it refuses. That is why
`s.x = h.f` moves a float correctly and `h.f + 1.0` does not lower.

**Evidence**: four tests in `shared_data.rs`, comparing the host buffer byte for byte over both
infinities, a negative zero and a NaN, all from runtime arguments so nothing is folded. Two
mutations, each confirmed applied by printing the changed line: a one-byte offset shift fails three
tests, and deleting the read's `Float` tag fails two.

**Route 4 of `float_guard_routes.rs` rotated** to assert that an eight-byte float slot lowers while
any other width refuses, the width made must-fire by overwriting the module's `float_bits_log2`. The
`skippable_tests.rs` pin records a RENAME rather than an addition; the population is unchanged at
ten.

**The prediction held.** No corpus module declares a float shared slot, now pinned by
`the_float_shared_slot_route_has_no_corpus_witness`, which reads the module's LAYOUT TABLE rather
than the corpus source text, with a non-vacuity assertion that the sweep saw shared slots at all.
