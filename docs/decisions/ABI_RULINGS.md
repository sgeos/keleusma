# The operator's ABI rulings, 2026-08-29

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: recorded.** Two items are settled, three remain open, and one supposition in the ruling was
incorrect and is preserved here rather than silently corrected.

**Attribution matters more than usual in this document.** Where something is my inference rather than
the operator's words, it says so. A later reader cannot tell them apart from tone.

---

## Settled

### Float ABI — **Option A**, a real floating-point ABI

> *"Give float-typed entries a real double ABI: `f64_type`, FP registers, float opcodes lowered."*

**⚠ WIDTH IS MY READING, NOT THE RULING.** `Float` is `f32` **or** `f64`, selected by the
`narrow-float-32` feature; `ScalarKind::size_in_bytes` takes `float_bytes` as a parameter. "Double" is
incoherent in a build with no `f64`, so the only coherent reading is **the floating-point type matches
the runtime's float width**. Proceeding on that; it is an assumption and is flagged as one.

**This also settles the `Float` shared slot** (kind 5): with a real representation the slot is
IEEE-754 bytes at the stated offset, size `float_bytes`. The operator supposed this and was right.

**Scope, measured before building** — `native_codegen/tests/float_abi_scope.rs`:

| | |
|---|---|
| corpus modules carrying a float | **1** (`float_witness.kel`) |
| …via a **signature** | **0** |
| …via a **constant** | **1** |
| modules refused for a float | **1** |

**The ruling names the entry ABI; the corpus is blocked by a constant.** The entry-ABI change has
**no corpus witness**, so built alone it could not be verified against the corpus — only against
hand-built subjects. Option A as recorded covers both, so the ruling is not wrong, but **a reader
planning from the phrase "entry ABI" would build the wrong piece first.**

Gain if built: **66 → 67 modules**, and with float arithmetic the two conversion opcodes the ISA
census lists as UNPROVEN (`FloatToInt`, `IntToFloat`) resolve.

### String ABI — **Option B**, make the two embeddings agree

> *"Option B, with a note that strings need to be revisited in the future."*

**NOT IMPLEMENTABLE BY THIS LINE.** Option B changes marshalling in `src/`, owned by the `v0.2.3`
line and read-only here. Recording it is the whole of what this line can do.

**A tension raised and not overridden**: B is the most expensive of the three options, and the ruling
also anticipates revisiting strings. Ratifying the current shape, or refusing string-taking natives,
are cheaper holding positions. B is defensible — it is the option that avoids breaking embedders later
— but "do the expensive thing now and expect to revisit" was flagged rather than assumed away.

---

## Open

### `Fixed` — the ruling has three readings

> *"Every option ought to be a different type, so that fixed point numbers are conceptually something
> like generics. The compiler can then bake in assumptions about the number of fractional bits without
> needing to store or pass this information."*

**What this describes is already exactly how `Fixed` works in-module.** `Fixed<N>` is a
const-parameterised type, the type checker enforces fraction-bit compatibility at compile time, and
the runtime carries no `N`. The open question was never in-module.

**The compiler cannot bake anything into the host**, which is separately compiled. So:

| reading | meaning | relation to the recorded options |
|---|---|---|
| **(a)** | host is told `N` out-of-band and applies the scale; slot carries a `Word` | **this is Option B** |
| **(b)** | the toolchain emits host bindings carrying `N` | **a new option**, not in the record |
| **(c)** | each `N` is a distinct slot kind tag | **stores `N`**, contradicting the ruling's own words |

*"without needing to store or pass"* points at **(a)**, which would make the ruling Option B — the
recorded preference. **It was framed as distinct from the listed options, so it is not assumed.**

**The interop goal still governs and is still unstated.** (a) gives convention-based interop only; if
a foreign toolchain must read a slot correctly with no side agreement, only (b) can provide it.

### `Text` slot — **the ruling's supposition was incorrect**

> *"Presumably Float and Text are nominally addressed."*

**Float: yes. Text: no.** The string ruling settles the *static string literal* ABI — a
`{ i64 len, [n+1 x i8] }` global passed to natives. The `Text` **slot kind** is a different construct:
a fixed-size handle of **`2 * word_bytes`**. Settling literals does not settle a two-word handle in
host-visible memory.

**Preserved rather than silently corrected**, because a reasonable reader drew that conclusion once and
will again.

### `Opaque` — intent already met; the literal form conflicts with narrow builds

> *"Opaque seems like it ought to be a pass through pointer to data that the host allocates for
> itself."*

The current design is a fixed-size handle to a host-managed `Arc<dyn HostOpaque>`, sized `word_bytes`
— an index into host-side ownership, **not an address**. The stated intent (the host allocates and
owns it; Keleusma just carries it) **is already what the handle achieves.**

Taken literally as a raw pointer, two consequences:

- it **does not fit under `narrow-word-8` or `narrow-word-16`**, where a word is 1 or 2 bytes and a
  host pointer is 8. The handle design avoids this precisely by not being an address;
- ownership moves from a host registry to raw memory the virtual machine holds, touching the `no_std`
  and bounded-memory story.

### `Unit`

> *"Not sure what unit is."*

**That is a question, not a ruling**, so nothing is recorded as decided. `Unit` is the empty type,
**0 bytes**; a zero-byte shared slot conveys nothing.

**MY INFERENCE, not the operator's**: the honest resolution is a permanent refusal — a degenerate case
rather than a representation decision.

---

## What this line does next

**Nothing is implemented on an ambiguous ruling.** The float scope is measured and pinned; `Fixed`,
`Text`, `Opaque` and `Unit` stay open with the input captured above.
