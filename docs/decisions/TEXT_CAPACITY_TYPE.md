# Dynamic `Text<N>`: a capacity-bounded mutable text buffer

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: authorized, design. Not implemented.

## The authorization, recorded with provenance

The operator authorized this directly, in session, on 2026-08-31, stating that the V0.3.X line
needs dynamic `Text<N>` and that the work is authorized. **This is a first-party ruling, not one
relayed through another branch**, which is worth stating because the string ABI ruling of two days
earlier arrived by relay and the relaying branch turned out to carry two contradictory records of it.

The operator also asked whether the work had been communicated. **It had not**, anywhere: neither
`v0.2.3` nor `v0.3.0` mentioned `Text<N>` in any document, source file, or stage source at the time
of this record.

When asked what `N` bounds, the operator chose a **fixed-capacity mutable buffer** over a
capacity-annotated immutable string, with the trade stated at the time: it overturns a documented
design property and pressures the instruction set. That is a decision on the record, not an
oversight.

## Ownership: this line builds it

`Text<N>` lands in `typecheck.rs`, `monomorphize.rs`, `value_layout.rs`, `compiler.rs`, `vm.rs`,
`bytecode.rs` and `marshall.rs`. `HANDOFF.md`'s ownership table names `src/` as **v0.2.3**, held
read-only by `v0.3.0`. So the V0.3.X line NEEDS this and the V0.2.X line BUILDS it, exactly as with
the string marshalling ABI.

## What text actually is today, measured rather than recalled

| claim | status |
|---|---|
| `Len` is the only text opcode | **true**, verified against `docs/spec/INSTRUCTION_SET.md` |
| `to_string`, `concat`, `slice`, `length` are bundled natives | **FALSE.** V0.2.0 removed them; only `println` is registered |
| `Op::Add` composes text at runtime | **FALSE.** No text arm exists in the virtual machine's `Add` |
| a script can compose text at all | **no**, except through host-registered natives |

**`src/verify.rs` and `src/text_size.rs` both still describe the removed machinery** as though it
were live, naming `Op::Add` on text operands and three bundled natives. Those comments are stale and
are corrected as part of this work.

The consequence is the shape of the feature. **Text composition was removed because it could not be
bounded. `Text<N>` is what allows it back.** `src/text_size.rs` carries a `TextSize` lattice of
`Known(n)` and `Unbounded` built for composition that no longer exists; it is infrastructure waiting
for this.

## Why the V0.3.X line needs it

`docs/roadmap/V0_3_0_SELF_HOSTING.md` records that the self-hosted resource analysis is a drop-in
replacement for the Rust verifier **only for text-free programs**, because "the one unmodelled WCMU
term is the text-size string-allocation heap". The self-hosted compiler is text-free by
construction: it takes source as `[Byte; N]` and uses three host natives rather than the `Text`
surface.

A declared capacity closes that. The lattice widens to `Unbounded` for text produced inside a loop,
inside a branch, or by a call; a value whose TYPE is `Text<N>` is `Known(N)` regardless of how it
was produced.

## Design

### The type

`Text<N>` is a distinct type from bare `Text`, which is retained unchanged for string literals and
host-returned dynamic strings. This keeps every existing program valid.

The precedent to follow is exact. `Multiword<N, F>` is already a const-parameterised built-in:
`TypeExpr::Multiword(ConstExpr, ConstExpr, Span)` in the abstract syntax tree becomes
`Type::Multiword(ConstDim, ConstDim)` in the type checker, where `ConstDim` is documented as "a
resolved const dimension of an array or `Multiword` type". `Text<N>` takes the same route with one
const parameter.

### Runtime representation: a FLAT composite, no handle

**A `Text<N>` is a flat composite carrying no reference field** -- for example a length word
followed by `N` bytes. B40 erases const parameters to literals at monomorphization, so `N` is known
to the compiler and the capacity need not be stored.

**THIS SECTION FIRST SPECIFIED A HANDLE, AND THAT WAS WRONG.** The original text made a `Text<N>`
the existing arena handle of data pointer, live length and epoch, reserving `N` bytes and exposing
the live prefix. It was corrected on the day it was written by the `v0.3.0` line's requirements
document, and their argument is better than the one it replaced:

> "A handle implies storage of unbounded lifetime, which is why the current design carries an
> epoch, and it puts worst-case memory beyond static reach. That contradicts the ecosystem value
> proposition, which is definitive WCET and WCMU."

Four things follow from flat that do not follow from a handle.

- **Worst-case memory is static by construction**, not recovered by an analysis. The bytes are in
  the body.
- **No epoch and no staleness.** There is nothing to dangle after a `RESET`.
- **The cross-yield prohibition does not apply.** A dynamic string cannot cross a yield boundary
  because it is an arena pointer reclaimed by the iteration `RESET`. A flat `Text<N>` carries no
  pointer, so a value containing one may be yielded -- which bare dynamic `Text` may not.
- **The `Text` shared-slot question dissolves.** A bounded text is a composite, and the shared-slot
  machinery already carries composite bodies, so the dynamic case needs no new slot kind.

The operator's own analogy is the governing one: `Text<N>` is `for .. limit <const>` applied to
storage rather than to iteration -- a runtime value under a static cap, accepting a surface
restriction to buy a bound the verifier can see.

**The `v0.3.0` line depends on exactly this and on nothing else about the shape.** Their native
backend already packs and reads flat composites, so a flat `Text<N>` costs them no new
representation. A reference field breaks that dependency.

### Where the bound comes from, which is the architectural point

**The bound is a property of the TYPE, not of the operation.** The worst-case-memory analysis reads
`Known(N)` from a value's declared type. It therefore does not matter, for bounding purposes,
whether an operation is an opcode or a native, and it does not matter that the lattice widens on
loops, branches and calls.

That decouples the resource question from the instruction-set question, and it is what makes a
zero-opcode implementation possible.

### Operations, and the instruction-set position

**The intent is zero new opcodes.** The operator's standing constraint is a rad-hard minimal
instruction set, currently 66 opcodes, preferring reuse over addition. The proposal is that
construction and mutation are provided as bundled operations with declared `Text<N>` signatures
rather than as new instructions.

**This is a policy reversal and is flagged as one.** V0.2.0 deliberately removed the bundled
text-utility library. The justification for reversing it is the one that motivated the removal:
those operations could not be bounded, and these can. The reversal is the operator's to confirm.

**If a new opcode proves unavoidable, that is an instruction-set decision requiring the operator's
authorization and this document must be revised before one is added.** The same applies to
`BYTECODE_VERSION`, which is currently 2 and moves only on authorization.

## The relayed rulings, ALL CONFIRMED 2026-08-31

The `v0.3.0` line relayed further rulings on 2026-08-31 at their `fea2d785`, having been instructed
by the operator to communicate them, and correctly flagged the relay as a relay. **This line's
standard is that a ruling read off another branch is not a ruling received**, so each item was put
to the operator directly. **All four were confirmed in session on 2026-08-31.** The relay carried
them accurately; the confirmation is what this line records as the receiving event.

| relayed item | status here |
|---|---|
| `Text<N>` authorized, fixed-capacity buffer | **first-party.** The operator stated it in this line's own session |
| R2, flat layout with no reference field | **adopted on merit**, independent of ruling status, because the argument stands on its own |
| R4, static `Text` becomes one pointer | **CONFIRMED** 2026-08-31 |
| R5, `ScalarKind::size_in_bytes` gains an address width; `Opaque` sized by `addr_bits_log2` | **CONFIRMED** 2026-08-31 |
| R6, record the `Opaque` trust boundary | **CONFIRMED** 2026-08-31 |
| `narrow-float-32` must stop being red | **CONFIRMED** 2026-08-31 |
| `confine` becomes load-bearing for the region planner | **CONFIRMED** 2026-08-31 |

R2 is marked differently on purpose. **It does not need to be a ruling to be right**, and adopting a
better design because a peer argued for it well is not the same as recording their relay as binding.

## R5's `Text` clause is RETRACTED, and the sequencing is the reason

R5 proposed sizing `ScalarKind::Text` as one address, on R4's reasoning that the string ABI puts the
length in the blob. **That is wrong and the `v0.3.0` line retracted it** at their `a89a713f` after
verifying the citations rather than conceding on trust.

`ScalarKind::Text` is the FLAT COMPOSITE FIELD kind. `TYPE_SYSTEM.md` states that such a field is a
two-word handle of arena data pointer and byte length, and states flatly that **a flat `Text` field
is always dynamic**, because a static string packed into a composite is promoted to an arena
`KStr`. R4's justification holds for a static literal and not for an arena string, which carries no
length prefix: its length lives in the field's second word and nowhere else. One address loses it,
and that is a silent wrong read rather than a refusal, across the eighteen sites in this tree that
declare a composite with a `Text` field.

**Option 1 is taken. R4 governs the static literal's value representation only, the "as well"
clause drops out, `ScalarKind::Text` stays two words, and no bytes move.**

### The argument for NOT escalating a version question now, which is theirs and is better than mine

I was prepared to take this to the operator as a `BYTECODE_VERSION` question. The `v0.3.0` line
supplied the reason not to, and it turns on something I had missed about this very feature:

**`Text<N>` removes the dynamic case from `ScalarKind::Text` altogether.** A bounded text has no
arena residency, no handle and no epoch. Once dynamic text is a bounded flat composite, the kind has
no dynamic case left to represent -- so the one-address form becomes reachable with no split into
static and dynamic flavours, and **the cross-yield restriction on text-bearing composites dissolves
with it**. `TYPE_SYSTEM.md` already anticipates that representation and records why it has not
happened: it had no story for a field that must hold a dynamic string. `Text<N>` is that story.

Changing the kind now moves bytes the typed verifier validates against the canonical layout and the
wire format carries. Doing it now and again after `Text<N>` would **spend the operator's
authorization twice for one net change**. Option 1 costs nothing and keeps that authorization intact
for the moment it buys the whole thing.

**`Opaque` sized by `addr_bits_log2` is unaffected by any of this** and proceeds independently.

## THE SETTLED SEMANTICS, ruled in session 2026-08-31

Everything below was decided directly by the operator and supersedes the open questions this
document previously carried.

### Static and dynamic are DIFFERENT TYPES, not one parameterised family

> *"Static text should essentially be a pointer to bytes in `.rodata`. It is dynamic text that needs
> to take the form `Text<N>`. Conceptually, this is similar to `limit` loops, just with space
> instead of iterations."*

**Static text** is a `.rodata` pointer: immortal, immutable, no capacity in its type. **Dynamic
text** is `Text<N>`: a flat buffer whose runtime length lives under a static capacity. The analogy
governs -- `for .. limit <const>` is a runtime range under a static cap; `Text<N>` is a runtime
length under a static cap. Space instead of iterations.

A literal is therefore STATIC text, not a `Text<N>`. It contributes its compile-time-known length to
a capacity computation, which is why `"ab" + "cd"` is `Text<4>`: concatenation's result cannot live
in `.rodata`, so it is dynamic and bounded.

### `N` counts CONTENT BYTES, with no terminator

Ruled after this document proposed counting a NUL. It does not, for three reasons the tree already
carries: a Keleusma string is length-delimited and an interior NUL is CONTENT, pinned by
`an_interior_nul_is_not_truncated`; the native ABI's trailing NUL is a C convenience explicitly
excluded from the length; and counting a terminator would make the type arithmetic `A + B - 1`
rather than the clean `A + B` that B40 const arithmetic already provides.

A trailing NUL may still be allocated in the LAYOUT for C hosts. It does not appear in the type.

### Overflow: refuse what is static, truncate what is not

**A statically-too-narrow assignment is a compile error.** After monomorphization the capacities are
literals, so `let r: Text<2> = "ab" + "cd"` is known to be wrong with full information. Silently
truncating there is a wrong answer where a refusal was available, which is the one thing the
conservative-verification stance exists to prevent.

**Runtime overflow truncates by default, with an optional arm.** This is not an exception invented
for text: it is the language's existing shape for a partial operation. `CheckedArmKind` already
gives checked arithmetic optional outcome arms over a wrapping default, and text overflow follows
it. Every operation stays total, and a program that cares handles the arm.

### Residence: locals are ordinary ephemeral values

> *"`Text` defined in the `.data` or `.rodata` region lives there. Locals live ephemerally in the
> arena. This is the same as any other value, except more bytes are nominally used."*

**There is no special residency rule and this document previously over-thought one.** A `Text<N>`
local is an ephemeral arena value exactly as any other flat composite is, occupying more bytes. A
`Text` in a `.data` or `.rodata` region lives in that region.

The reasoning that produced the over-thinking is still worth keeping, because the CORRECTION to it
is load-bearing. This document claimed the cross-yield prohibition on text-bearing composites
dissolves BECAUSE nothing would be arena-resident. **That was wrong.** The operator's account is
that an epoch-tagged arena value can already cross safely, since a post-`RESET` read resolves stale
rather than dangling -- residence was never the barrier, and `TYPE_SYSTEM.md` concedes as much by
saying prohibiting it structurally "is simpler" rather than necessary. What actually prevents
arena-resident mutable text is that the ephemeral region is write-once.

## Open questions, which belong to the operator

1. **The overflow rule.** A push past `N` must refuse at compile time where provable and do
   something definite otherwise. Trapping matches the language's secure-failure posture; saturating
   would silently truncate. Not decided here.
2. **The bundled-operation reversal**, above.
3. **Surface syntax.** Method form (`s.push(t)`) reads well and matches the authorizing example, but
   the language's method surface is trait-impl based. Whether `Text<N>` gains methods or free
   operations is a surface decision.

## A DEFECT THIS WORK UNCOVERED, AND WHICH IT WILL CLOSE

**`"ab" + "cd"` compiles, passes the verifier, and then always faults at runtime** with
`TypeError("cannot add KStr and KStr")`. Measured 2026-08-31.

The type checker still admits text concatenation that V0.2.0 removed from the virtual machine. It is
a clean trap rather than anything memory-unsafe, but the conservative-verification stance is that a
program the runtime cannot execute is rejected at the safe constructor, not at runtime. This is
residue of the removal, in the same family as the stale comments in `text_size.rs`.

R3 restores concatenation with a computed capacity, so the feature fills the hole rather than merely
fencing it. **Whichever lands first, the end state must be that no text expression both verifies and
always faults.**

## What must not happen

**Do not let the bound be inferred rather than declared.** The whole value of the type is that
`Known(N)` survives a loop, a branch and a call. An implementation that recovers the bound by
analysing operations reproduces the widening problem the type exists to remove.

**Do not silently reintroduce unbounded text.** If any path can produce a `Text<N>` whose live
length may exceed `N`, the worst-case memory bound is wrong rather than merely loose, and the
language's central claim is compromised.
