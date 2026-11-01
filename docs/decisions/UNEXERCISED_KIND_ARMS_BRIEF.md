# BRIEF — the unexercised kind arms, dispositioned

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## What the census reports, and what it conflates

`kind_arm_census.rs` measures which `(operation, scalar kind)` combinations the corpus produces: **32
of 40 unreached, 16 resolved to a named test, 16 "UNEXERCISED by corpus or by any named test."**

Its header states the hazard exactly: *"An accepted path that no test executes ships a plausible wrong
number."* **But "unexercised" is one label over at least three different situations**, and measuring
them apart changes what a reader should do:

| combination | measured |
|---|---|
| `GetField`/`GetTupleField`/`GetEnumField`/`GetIndex` × `Unit` | **unconstructible** — a `Unit` struct field does not compile |
| the same four × `Text` | the declaration compiles, **constructing one is REFUSED** (unknown packed width) |
| the same four × `Opaque` | **unconstructible** — no way to make the value |
| `shared slot × Fixed` | **lowers, and nothing drives it** |

**Only the last is the hazard the header describes.** The other twelve are unreachable, and reporting
them the same way makes the real one harder to see, not easier.

## The stale record found on the way

`fixed_shared_scale.rs` opens: *"`alloc_format_kind` in this backend refuses a `Fixed` shared data
slot."* **It does not.** `shared_scalar_width` maps `SCALAR_FIXED` to eight bytes, and a driven read
agrees with the reference — `Fixed(196608)` on both sides.

**The file's assertions are still true and still valuable**: `Fixed<16>` and `Fixed<8>` produce
identical host-visible layouts, so the ABI gap it pins — a host cannot recover the Q-format scale —
is real and open. **Only its header describes a world that no longer exists.** That is the fourth
record this session found outliving its subject.

## The wrong turns

1. **Do not retract the ABI gap with the refusal.** They are different claims: the scale is still
   invisible to a host, and that is what the file's assertions hold. Only the refusal is gone.
2. **Do not mark the twelve unreachable ones "covered".** They are not tested and cannot be; the
   honest label is that the shape does not exist, with what was measured to establish it.
3. **Do not drive a `Fixed` shared slot through a hand-written buffer and call it a differential.**
   Both sides must see the same bytes, and the reference needs `call_with_shared`.
4. **Do not assume the other three shared kinds behave like `Fixed`.** `Unit`, `Text` and `Opaque`
   shared slots were not measured here; say so rather than generalising from one.

## What done looks like

A `Fixed` shared slot is driven end to end and agrees; the census distinguishes unreachable from
refused from untested, with what was measured for each; and `fixed_shared_scale.rs` says the refusal
is gone while keeping the ABI gap it actually pins.

---

## OUTCOME — 2026-09-12

**Sixteen "unexercised" combinations were one label over three situations. Exactly one was the
hazard.**

| combination | measured | disposition |
|---|---|---|
| four flat reads × `Unit` | a `Unit` struct field does not compile | **unconstructible** |
| four flat reads × `Opaque` | no way to make the value | **unconstructible** |
| four flat reads × `Text` | the declaration compiles; **construction is REFUSED** for unknown packed width | **refused** |
| `shared slot` × `Unit`, `Text`, `Opaque` | the value cannot be made | unconstructible |
| **`shared slot` × `Fixed`** | **lowers; nothing drove it** | **now driven, and it agrees** |

The residue is 15 and the resolved count 17. **The number barely moved and the meaning did**: a reader
now sees one live gap closed rather than sixteen undifferentiated ones.

### The stale record found on the way

`fixed_shared_scale.rs` opened by stating that this backend REFUSES a `Fixed` shared data slot.
**It does not** — `shared_scalar_width` maps `SCALAR_FIXED` to eight bytes, and the driven read agrees:
`Fixed(196608)` on both sides.

**Its assertions were never wrong.** `Fixed<16>` and `Fixed<8>` still produce identical host-visible
layouts, so the ABI gap it pins — a host cannot recover the Q-format scale — is real and open. Only
the framing described a world that no longer exists.

> **Fourth record this session found outliving its subject**, after a repaired unsoundness still
> asserted, a repaired defect still recorded as open, and a report the other line had acted on. **The
> pattern is not carelessness about any one file. It is that nothing re-reads a header when the code
> under it changes.**

### What was deliberately not done

The three other shared-slot kinds were **not** separately measured; the brief said not to generalise
from one, and the table says they were not measured rather than implying they were.
