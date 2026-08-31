# Requirements addressed to the V0.2.X line — `Text<N>`, and the reference-kind sizing

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## PROVENANCE, FIRST, BECAUSE THIS IS A RELAY

**These are rulings the operator gave in session to the V0.3.X line on 2026-08-31, and the operator
has instructed that line to communicate them to you.** They did not reach you through your own
session.

**This is a relay and you should treat it as one.** Your line's recorded standard is that a ruling
read off another branch is not a ruling received, and you held the string ABI ruling back on exactly
that basis until the operator confirmed it directly. That standard is right and this document does
not ask you to abandon it. **Confirm with the operator before recording any of this as binding.**

What is different from the earlier case is only that the operator has asked for the communication
rather than the other line having decided to make it.

The operator's words are quoted below rather than paraphrased. Anything that is the V0.3.X line's
reasoning rather than the operator's is marked as such.

---

## The rulings, quoted

> *"Static `Text` decided. Dynamic `Text<N>` with a fixed buffer `N` is reasonable, and I do not see a
> better way of doing things. It is conceptually something like the `limit` loop."*

> *"Add the dynamic `Text<N>`. You will need to coordinate with the V0.2.X line."*

> *"`Text<N>` work is authorized with the understanding that the V0.2.X line is responsible for some
> of said work."*

> *"Use the address size."* … *"`Opaque` should be sized by `addr_bits_log2`."*

> *"Coordinate with the V0.2.X line if you do not own the file. V0.3.X work nominally builds on top
> of V0.2.X work, even if your requirements are driving said work."*

---

## Why this shape, which is the V0.3.X line's reasoning and not the operator's words

The question the operator asked was whether an obvious `Text` solution exists that survives host
retirement. **Static is easy and dynamic is hard, and the difficulty is a symptom.**

**A handle is the wrong shape for this language.** It implies storage of unbounded lifetime, which is
why the current design carries an epoch, and it puts worst-case memory beyond static reach. That
contradicts the ecosystem value proposition, which is definitive WCET and WCMU.

**A capacity-bounded value type fixes all of it at once.** `Text<N>` laid out flat as a length and an
`N`-byte buffer gives static worst-case memory by construction, removes the handle, the epoch and the
escape question, and makes concatenation total with a computable result capacity. **The const
arithmetic that needs already shipped in V0.2.1**, where const parameters support `+`, `-` and `*`
and are erased at monomorphization, so the analyses see no symbolic constant.

**The operator's own analogy is the precise one.** `for .. limit <const>` is a runtime range under a
static cap, and it exists because a bound the verifier can see is worth a surface restriction.
`Text<N>` is that same trade applied to storage rather than to iteration.

**And it dissolves a question rather than answering it.** The `Text` SHARED SLOT problem disappears,
because a bounded text is a flat composite and the shared-slot machinery already carries composite
bodies. No new slot kind is needed for the dynamic case.

---

## Requirements

**R1. `Text<N>` as a const-parameterised type**, `N` a capacity in bytes, following the existing B40
const-generic machinery.

**R2. A FLAT layout with no reference field**, for example a length word followed by `N` bytes. The
exact shape is yours. **What the V0.3.X line depends on is only that it is a flat composite carrying
no handle**, because the native backend already packs and reads flat composites and would need no new
representation at all. If it carries a reference field instead, that dependency breaks and the
backend work is no longer free.

> ### `Text<N>` RETIRES THE REASON `ScalarKind::Text` MUST BE TWO WORDS
>
> This was understated in the first version of this document, and it is the strongest argument for
> the design rather than a side effect.
>
> `TYPE_SYSTEM.md` already records the representation that would let a static text field be one
> reference, and records that it does not yet exist. It also records what today's two-word arena
> field costs: **a value that transitively contains a flat `Text` field cannot cross the yield
> boundary at all**, because the iteration `RESET` reclaims the arena.
>
> **A bounded `Text<N>` has no arena residency, no handle and no epoch.** So once dynamic text is a
> bounded composite, `ScalarKind::Text` has no dynamic case left to represent, the one-reference
> static form becomes reachable without splitting the kind, and the cross-yield restriction on
> text-bearing composites dissolves with it.
>
> **Hence the sequencing.** Changing `ScalarKind::Text` now would move bytes the typed verifier
> validates and the wire format carries, which is a `BYTECODE_VERSION` question and the operator's
> to authorize. Doing it now and again after `Text<N>` would spend that authorization twice.

**R3. Concatenation total, with the result capacity computed by const arithmetic.** `Text<A>` and
`Text<B>` yielding `Text<A + B>` is the natural form and is already expressible.

**R4. Static `Text` becomes ONE POINTER.** The string ABI ruling already settles the literal
representation as a length-prefixed global, so the length need not travel in the value and a fat
pointer is unnecessary.

**R5. `ScalarKind::size_in_bytes` gains an ADDRESS width.** It takes a word width and a float width
today and sizes `Opaque` as one word and `Text` as two words. Per the operator's ruling `Opaque` is
sized by `addr_bits_log2`.

> ### ⚠ R5 ORIGINALLY CARRIED A SECOND CLAUSE AND IT WAS WRONG. RETRACTED 2026-08-31.
>
> It read *"and R4 makes static `Text` one address as well"*, proposing that `ScalarKind::Text` be
> sized as one address. **The `v0.2.3` line caught it before building to it.**
>
> **`ScalarKind::Text` is the FLAT COMPOSITE FIELD kind, and a flat `Text` field is always dynamic.**
> `docs/spec/TYPE_SYSTEM.md` says so in as many words, and `src/marshall.rs` agrees: the field is a
> two-word arena reference, a data pointer and a byte length. A static string packed into a composite
> is PROMOTED into the arena, which is why.
>
> **R4's justification does not reach it.** The string ABI puts the length in the blob for a static
> LITERAL. An arena string has no such prefix, so its length lives in the field's second word and
> nowhere else. **Sizing that field as one address loses the length with nothing to recover it from**,
> which is a silent wrong read rather than a refusal, over the sites in this tree that declare a
> composite with a `Text` field.
>
> **R4 stands and governs the static literal's VALUE representation only. The clause that generalised
> it to the field kind drops out, and `ScalarKind::Text` stays two words.** `Opaque` is unaffected.
>
> **Do not spend a `BYTECODE_VERSION` authorization on this.** See the note under R2 below.

**R6. Record the `Opaque` trust boundary explicitly**, in the operator's terms: *"An `Opaque` is a
handle to memory the host has provided. The host is responsible for making sure this handle is valid
and otherwise safe to use. The Keleusma procedure never dereferences the handle, but it does pass
this handle back to the host."*

---

## Evidence the V0.3.X line can supply, measured rather than asserted

**The header already carries the field R5 needs.** `Module.addr_bits_log2` exists, is documented as
the address size the bytecode requires, is accepted when at most the runtime's own address width, and
is mirrored in the framing header. The features `narrow-address-8`, `narrow-address-16` and
`narrow-address-32` are selected independently of the word width.

**The layout does not use it, and that is latent rather than broken.** In the default configuration
the word width and the address width are both sixty four bits, so `Opaque` at one word and an address
agree and nothing notices. **This is the same shape as a defect this line already found and fixed**,
where a conversion was poison by the language definition and agreed with the reference only because
the hardware happened to saturate. It was reachable before anyone noticed.

---

## Two further items for the same recipient, since they are in your files

**The `f32` configuration is red in the workspace, and all of it is one cause.** Under
`narrow-float-32` the workspace runs 2056 tests with **2051 passed and 5 failed**. Three in
`narrow_vm` and two in `require_directive` fail because the tests construct targets whose float is
eight bytes, which an `f32` runtime correctly rejects with *"target float_bits_log2 = 6 exceeds
runtime maximum 5"*. **No failure is a runtime defect.** The sharpest illustration is
`wider_float_bytecode_rejected_by_f32_runtime`, which exists to check that rejection and dies on its
own setup because the rejection now happens earlier than it looks. The operator has ruled that the
configuration must stop being red and that tests must pass meaningfully rather than be cheated into
passing, so deriving the width from the build rather than pinning it is the repair.

**`keleusma::confine` is about to become load-bearing.** The operator ruled that the region planner's
soundness obligation is discharged by analysis, that an inconclusive verdict declines, and that your
line owns the analysis while the V0.3.X line consumes it. **Measured before relying on it**, over 69
modules the verdict answers at exactly the planner's key, 256 placements to 256 verdicts, and it
names the known escaping site correctly as `Escapes` because `Yielded` at iteration scope. From the
moment the planner consumes it, a wrong verdict is a miscompilation rather than a reporting error.
**That is a change in what the analysis is for, and you should know it before changing that file.**

---

## What the V0.3.X line will and will not do

It will not edit `src/`. Its share is the backend half of each item, and under R2 that share is close
to nothing, which is the point of the design. It will consume the address width once
`size_in_bytes` carries it, and it will consume the confinement verdict at iteration scope.
