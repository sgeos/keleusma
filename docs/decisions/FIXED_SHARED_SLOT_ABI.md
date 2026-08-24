# The `Fixed` shared-slot ABI — the scale, not the representation

**Status**: **open, and narrower than it has been recorded as being.** This document does not
resolve it. It replaces an open-ended question with a single specific one and prices the answers.

**Scope note.** Written by the `v0.3.X` line. The wire schema, `src/value_layout.rs`, and
`src/wire_schema.rs` belong to the `v0.2.3` line; **nothing here is a change to them.** The
measurements are pinned in `native_codegen/tests/fixed_shared_scale.rs`, which is this line's file.

## Why this was reopened

The operator ruled: *settle `Fixed` alongside the float ABI.* The `v0.3.X` native backend refuses a
`Fixed` shared data slot with the message

> `Fixed slot; fixed-point representation is unsettled`

**That message is imprecise, and the imprecision made the decision look bigger than it is.** The
representation is not unsettled. Measured, not assumed:

| Fact | Where |
|---|---|
| `ScalarKind::Fixed` is a signed Q-format integer of the runtime's **word width** | `src/value_layout.rs:75` |
| `size_in_bytes` returns **`word_bytes`** — 8 at a 64-bit word, 4 at 32-bit | `src/value_layout.rs:116`, pinned at `:581`–`:582` |
| the shared-slot tag for it is **4**, with `SHARED_SLOT_COMPOSITE_FLAG` clear | `src/value_layout.rs:155` |

A backend lowering a `Fixed` shared slot as a word-width signed integer at the stated offset would
**agree with the reference byte for byte**. Nothing about the bits is open.

## What IS absent: the scale

A `Fixed<N>` value is an integer scaled by `2^N`. **`N` is carried by the opcodes** —
`WordToFixed(frac_bits)`, `FixedToWord(frac_bits)`, `FixedMul(frac_bits)`, `FixedDiv(frac_bits)` —
**and by nothing in the layout descriptor.** `src/value_layout.rs:76` states it directly:

> The fraction-bit count is carried by the opcodes that produce or consume the value, not by the
> layout descriptor.

and `src/bytecode.rs:3398` gives the rationale:

> the type checker has already enforced fraction-bit compatibility at compile time, so the runtime
> only needs to confirm the operand is `Value::Fixed`.

**That reasoning is correct and it is scoped to inside the module.** Every producer and consumer of
an internal `Fixed` is type-checked against the same `N`, so the scale is a compile-time agreement
and the runtime genuinely does not need it.

**A shared data slot is not inside the module.** The host receives `word_bytes` of raw
two's-complement integer and has nothing to consult. `SharedSlotLayout` is
`{ offset: u32, kind: u8, len: u16 }` and `len` is documented as `0` for a scalar slot — so there is
no field where a scale could be hiding.

### The measurement that makes this an ABI finding rather than a struct observation

**Two modules differing only in `N` produce byte-identical shared-slot layouts.** `Fixed<16>` and
`Fixed<8>` — semantics differing by a factor of 256 — are **indistinguishable to the host**.

Pinned by `two_scales_produce_indistinguishable_host_visible_layouts`. A missing field is an
observation about a struct; two programs with different meanings and identical host-visible layouts
is an observation about the interface. **The surface admits the slot today** — a `Fixed<16>` shared
declaration compiles, passes the structural verifier, and receives a worst-case memory bound, each
step asserted rather than assumed — so this is live, not hypothetical.

## So the question is one question

Not *"how should fixed-point be represented"*, which is answered, but:

> **Where does the host-visible scale live, or is there deliberately none?**

## The options, with costs

### A — carry the scale in the slot descriptor, reusing `len`

`len` is `u16` and documented as `0` for a scalar slot, so a `Fixed` scalar slot could carry `N`
there. **No new field, no size change, no new opcode.**

- **Cost**: a semantics change to an existing field. It needs a schema note and a validation rule
  (`N < word_bits`), and it must be stated for the composite case too, where `len` already means
  body length.
- **Compatibility hazard, and it is real**: a v2 artifact compiled today with `Fixed<16>` writes
  `len = 0`, which under this reading reads back as **Q0**, i.e. a plain integer. That is a silent
  misreading rather than a rejection. It is mitigated by the fact that such artifacts were *already*
  unreadable by a host — there was no correct interpretation to lose — but "already broken" is a
  weaker argument than a version check, and it should be made deliberately rather than inherited.
- **Does not cover the composite case.** A `Fixed` field inside a flat composite shared slot has the
  same problem and no spare field at all; this option is a scalar-slot answer only.

### B — refuse `Fixed` in a host-visible position, permanently and at the SOURCE

The host marshals through `Word` and applies the scale itself.

- **Cost**: the surface loses a declaration it currently accepts, so this is a **breaking source
  change** requiring the usual authorization.
- **Benefit**: it is the only option that makes the guarantee *structural*. There is no
  under-specified value to misread because there is no such value.
- **Note**: this option makes the backend's current refusal correct **by specification** rather than
  by deferral, which is a materially better place for it to sit.

### C — fix one canonical host-visible Q format

E.g. every shared `Fixed` is Q(`word_bits`/2), converted at the boundary.

- **Cost**: **the worst failure mode of the three.** A program using a different `N` either needs an
  inserted conversion (silently changing precision) or is refused (which is option B with extra
  steps). A host reading a slot that *looks* right and is scaled wrong is exactly the class of bug
  an ABI exists to prevent.
- **Recorded for completeness. Not recommended.**

## What this line recommends, and the confidence attached

**Preference: B, then A, then C.** Stated as a preference and not a finding — the surface-breaking
call in B is an operator decision and the schema in A is the `v0.2.3` line's.

B is preferred because the property it gives is **structural rather than documentary**: A leaves a
correct-by-convention interface where a host that ignores the new `len` semantics reads a plausible
wrong number, whereas B leaves nothing to ignore. **The float ABI decision should be taken with this
one**, per the operator's ruling, and the two share this exact shape: a host-visible scalar whose
in-module representation is settled and whose boundary contract is not.

## What the `v0.3.X` backend does until it is settled

**Nothing changes.** `alloc_format_kind` keeps refusing the slot. The refusal was already correct;
only its stated reason was imprecise.

> **ACTION, for whichever line owns the message**: the refusal text should say *the host-visible
> fraction-bit scale is unspecified* rather than *fixed-point representation is unsettled*. The
> current wording sends a reader looking for a representation decision that was made long ago.

`native_codegen/tests/fixed_shared_scale.rs` pins the three facts. **If a scale ever becomes
recoverable, that file fails** — which is how the decision landing on the other line reaches this
one without anybody remembering to send a message.
