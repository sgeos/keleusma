# VERDICT — the Text struct field refusal is CORRECT, and closing it is a workstream

**Read 2026-09-23 against `0214ba58`. No emitter change.**

The `Text` struct field was the third instance of `NewComposite` refusing an operand
of unknown packed width, and `text_surface.rs` deliberately left its closability
unsettled rather than guessing — because the float instance looked like a
`width_of_tag` change and was not, and the multi-word instance looked like the float
one and was not.

## The width IS knowable, which is why this needed reading and not assuming

`src/value_layout.rs` states it: the surface `Text` type is *"represented as a
fixed-size handle regardless of whether the underlying value is `StaticStr`
(rodata-resident) or `KStr` (arena-resident). The handle size is `2 * word_bytes` (a
pointer-or-offset plus a length or epoch field). The runtime distinguishes the two
cases through a discriminant in the handle; the layout pass treats both as the same
scalar shape."*

So a figure exists, and it is derivable from the module — exactly the shape that made
the float reply closable.

## And it still must not be supplied, because the BACKEND's representation differs

The emitter lowers a string constant as a **bare address**: it materialises a global
and takes its pointer as one word. Its own comment says why the width is withheld:

> *"Unknown width, deliberately. The operand is an address, and packing an address
> into a composite body as though it were a scalar is exactly the mistake
> `Width::Unknown` exists to make impossible."*

**One word against the canonical two, with no discriminant.** Supplying
`2 * word_bytes` would let `NewComposite` pack a one-word address into a two-word
field — a silent mispack, which is the one outcome this family's refusals exist to
prevent. The emitter elsewhere records the state plainly: *"Text slot; string
representation is Workstream C."*

## The verdict

**Correct, deliberate, and not closable by a width.** Closing it is the string
representation workstream: the backend would have to construct the same two-word
handle the reference does, discriminant included, and that is an ABI question rather
than a lookup.

## The family, now all three settled by reading

| instance | verdict |
|---|---|
| the resumed float reply | **CLOSED** — one site, the module's own `float_bytes`, and the representation already agreed |
| a `Multiword` multiply | **DECLINED** — the multi-write local rule is deliberate and needs a fixpoint to relax |
| a `Text` struct field | **DECLINED** — the width is knowable but the backend's one-word address does not match the canonical two-word handle |

**The common lesson is that a shared error message is not a shared cause.** Three
instances of one refusal had three different roots, and only one was a width that was
merely missing.
