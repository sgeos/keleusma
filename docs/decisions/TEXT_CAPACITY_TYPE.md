# Dynamic `Text<N>`: a capacity-bounded mutable text buffer

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: authorized. The value representation and the literal cast are implemented as of 2026-10-30. Five bundled operations are implemented as compiler intrinsics behind the `text` cargo feature as of 2026-10-31. See [the literal-cast increment](#the-literal-cast-increment-landed-2026-10-30) and [the bundled operations](#the-bundled-operations-as-compiler-intrinsics-landed-2026-10-31). The `ScalarKind::Text` collapse was investigated on 2026-10-31 and not performed. See [the collapse investigation](#the-scalarkindtext-collapse-investigated-2026-10-31-and-stopped).

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

**The overflow rule was question 1 here and IS NO LONGER OPEN.** It was ruled in session on
2026-08-31 and the ruling is recorded under **THE SETTLED SEMANTICS** above: refuse what is static,
truncate what is not, with an optional arm following `CheckedArmKind`. This list continued to
present it as undecided after the ruling landed, which is a worse failure than an ordinary stale
document: a resuming session reading this list would believe the central semantic choice was still
its to make, and would either re-ask a question the operator has already answered or implement the
opposite of the ruling. Corrected 2026-09-03. **Where this list and the settled section disagree,
the settled section wins**, because it records a direct ruling and this list records the state
before it.

Two questions remain genuinely open.

1. **The bundled-operation reversal**, above.
2. **Surface syntax.** Method form (`s.push(t)`) reads well and matches the authorizing example, but
   the language's method surface is trait-impl based. Whether `Text<N>` gains methods or free
   operations is a surface decision. **Partly ruled on 2026-10-30**, see below. Construction is the
   literal cast with compile-time refusals and the operations are free functions. How a native
   signature can be generic over a capacity is still open, and it blocks the operations.

**Both are ruled as of 2026-10-31.** The operator chose compiler intrinsics, option (b), over
generic natives and per-capacity natives. That restores the bundled operations without answering
the generic native signature question, which stays open for natives alone. The surface is the
literal cast and free functions under `text::`, and the comparison operators stay refused. See
[the bundled operations](#the-bundled-operations-as-compiler-intrinsics-landed-2026-10-31).

## The literal-cast increment, landed 2026-10-30

### The ruling

The operator ruled first-hand in session on 2026-10-30, choosing option (a) in these words.

> *"A, the case for literals with compile time refusals."*

Option (a) as it was put to the operator was an `as` cast for a literal, `"abc" as Text<8>`,
refused at compile time when the literal is longer than `N`, with free functions for the
operations. The operations themselves are not part of this increment, because how a native
signature can be generic over a capacity is undecided.

### What landed

- **The type is admitted with a positive literal capacity** after monomorphization, as a `let`
  local, a function parameter and return, a tuple element, a struct field, an array element, an
  `Option` payload, an enum payload, and a `private` data field. A capacity may be a const
  parameter, and monomorphization resolves it before the compiler sees it.
- **`"..." as Text<N>` is the one construction.** The operand must be a string literal written at
  the cast site. A literal longer than `N` bytes, counted in UTF-8 bytes, is a compile error that
  names both lengths. The type checker refuses it, and the compiler repeats the comparison at
  emission as a backstop. An interior NUL is content and counts toward the length.
- **The emitted value is one pooled constant.** It is `ConstValue::Tuple(Int length, Array[Byte;
  N])`, the shape `layout_pass` and `zero_value` already describe, with the bytes zero padded to
  `N`. This is the path the compiler already takes for a compile-time composite constant, a const
  data field. One `Op::Const` replaces the `N + 3` instructions a byte-by-byte construction would
  emit, so the execution cost does not grow with `N`, and the load allocates nothing in the arena.
  The padding is part of the value, because a later equality operation compares whole bodies.
- **The bound follows the type.** Measured on 2026-10-30, a struct holding a `Word` and a
  `Text<N>` is one flat allocation of `word_bytes + word_bytes + N`, and raising `N` from 8 to 40
  raises the worst-case memory bound from `auto_arena_capacity_for` by exactly 32 bytes. The
  pooled body of a bare literal is `word_bytes + N` bytes of const pool, outside the arena. The
  verifier, `verify()`, accepts every module in the tests.
- **A `Text<N>` value crosses a yield**, as the flat representation predicts, and is pinned by a
  test that reads the yielded body.
- **No opcode, no wire format change and no `BYTECODE_VERSION` change.** The instruction set stays
  at 66 opcodes and the version stays at 2.

### What stays refused, and why

- **`Text<0>`** is not a legal type, for the reason a zero-length array is not. A capacity that is
  still symbolic after monomorphization is refused rather than sized by a guess.
- **Every other cast** to or from `Text<N>`, including the identity cast and a cast between two
  capacities, is a type error. A static literal is not implicitly a `Text<N>`.
- **Arithmetic, comparison, field reads, indexing, iteration and destructuring** are type errors.
  Comparison is refused for any type that contains a `Text<N>`, because the general composite
  rule would otherwise admit `t == t` and `t < t`, and both faulted at runtime when measured. A
  `let` tuple pattern over a `Text<N>` is refused for the same reason.
- **A `shared` data field** is refused. The host writes the shared buffer directly and nothing
  checks that a host-written length is at most `N`, so admitting it would let a host produce the
  one value this type exists to rule out. A `private` field is written only by script code.
- **A native signature naming `Text<N>`** is refused. Nothing marshals a flat `Text<N>` body across
  the host boundary, and admitting a concrete capacity there would answer the open question about
  generic native signatures by accident.
- **A length surface** is not built. The language has no `len` surface to extend, and adding one
  belongs to the operations increment. The tests observe the length from the host by reading the
  flat body.
- **The self-hosted compiler** does not lower `Text<N>`. The shipping entry `self_hosted_compile`
  refuses any program naming it as unsupported before a stage runs, after first consulting the
  reference so that a program the reference rejects reports that rejection. Before this refusal
  the four positions measured reached four different outcomes, a signature divergence, a parser
  stage rejection, unavailable type evidence, and one silent success.

### What remains

The operations, restored as a bundled library of free functions, wait on the generic native
signature question. With them come the length surface, equality over whole bodies, the runtime
truncation default with its optional arm following `CheckedArmKind`, and the end of the
`"ab" + "cd"` defect recorded above. A `shared` field needs a host-side type and a length check at
the boundary. The self-hosted stages need the type before the shipping refusal can be lifted.

**Updated 2026-10-31.** The operations, the length surface and equality landed as intrinsics and
are recorded in the next section. The `"ab" + "cd"` defect had already been closed by a type
checker refusal pinned in `tests/text_composition_hole.rs`, whose diagnostic now names
`text::concat`. The truncation arm, the `shared` field and the self-hosted stages remain.

## The bundled operations as compiler intrinsics, landed 2026-10-31

### The ruling

The operator ruled first-hand in session on 2026-10-31, as relayed to this line in the task that
built this increment. The operations are compiler intrinsics over `Text<N>`, option (b), chosen
over generic natives and over one native per capacity. They are conditionally includable behind a
cargo feature named `text`, and a build without the feature refuses their use at compile time. The
surface is the literal cast and free functions, so `==` and `<` on `Text<N>` stay refused and
`text::eq` is the comparison.

### What existed before V0.2.0 removed it

Commit `afc6079f` removed the bundled text library. It registered four natives, `to_string`,
`length`, `concat` and `slice`, and `Op::Add` carried a text arm. `length` and `slice` counted
Unicode scalar values, `slice` faulted on a negative or reversed range, and every result was an
arena string with no static bound. That last property is why they were removed.

### The operations

`A`, `B` and `N` are operand capacities, read from the operand types after monomorphization.
`wb` is the target word width in bytes. The instruction counts and costs exclude evaluating the
operands, and were measured on 2026-10-31 on the 64-bit host target with
`verify::wcet_whole_chunk` and `verify::wcmu_whole_chunk`, at capacities 1, 2, 4, 8, 16 and 32. Each
formula reproduces all six measurements exactly.

| operation | result | instructions | WCET | arena WCMU |
|---|---|---|---|---|
| `text::len(t: Text<N>)` | `Word` | 1 | 2 | 0 |
| `text::capacity(t: Text<N>)` | `Word`, the literal `N` | 2 | 2 | 0 |
| `text::concat(a: Text<A>, b: Text<B>)` | `Text<A + B>` | `7(A + B) + 3A + 15` | `15(A + B) + 5A + 27` | `2(A + B) + wb` |
| `text::eq(a: Text<A>, b: Text<B>)` | `bool` | `7 min(A, B) + 9` | `12 min(A, B) + 13` | 0 |
| `text::slice(t: Text<N>, start: Word, end: Word)` | `Text<N>` | `16N + 47` | `29N + 62` | `2N + wb` |

The arena figure for a constructed result counts the byte array and the enclosing tuple, which
the verifier charges separately. Concatenation and slicing also push one operand-stack value per
result byte before the two `NewComposite` instructions, so their operand stack peak grows by
`A + B` and `N` value slots respectively, which the verifier counts in the stack term.

### Semantics

- **Lengths and positions count bytes**, as `N` does. This differs from the removed `length` and
  `slice`, which counted Unicode scalar values. A slice that splits a multi-byte UTF-8 sequence
  yields the bytes it names.
- **Concatenation cannot overflow.** The result capacity is the sum of the operand capacities, so
  no truncation arises and no outcome arm is needed.
- **Slicing is total.** The range `[start, end)` is clamped, `start` into `[0, len]` and then `end`
  into `[start, len]`. An empty, reversed or out-of-range request yields a shorter or empty text
  rather than a fault. The result capacity is the operand capacity, so no truncation arises.
- **Equality compares live content** and admits operands of different capacities, so
  `"ab" as Text<2>` equals `"ab" as Text<8>`.

### Lowering

All five lower to existing opcodes. None emits `Op::Len`, a call or a native call, and none adds a
native to the module. The length is the word at offset zero and byte `i` is the byte at offset
`wb + i`, both read with a constant-offset `GetTupleField`. A byte at a position computed at run
time is read from the nested byte array with the total `GetIndex` form, whose out-of-range result
is a zero byte with an outcome suffix that is discarded, so no read can trap. No opcode writes one
byte of a flat body, so each constructed result is pushed one byte at a time and packed by one
`NewComposite` for the array and one for the tuple. The code size and the execution time are
therefore linear in the capacity, as the table shows.

- **`concat`** computes result byte `i` as `a[i] + b[i - len(a)]` in wrapping byte arithmetic.
  Exactly one term can be nonzero, because each operand is zero outside its live range and a read
  at a negative index is zero. The selection therefore needs no branch.
- **`eq`** folds `len(a) xor len(b)` and the wrapping byte difference of each position below
  `min(A, B)` into one accumulator with `BitOr`, and compares it with zero. Positions past the
  smaller capacity are zero in both operands whenever the lengths agree.
- **`slice`** clamps with four one-sided conditional moves and masks each result byte with the
  sign of `i - rlen`, taken by an arithmetic shift, so bytes past the result length are zero.

### The invariant, and the trust assumption it carries

**Every `Text<N>` has a live length of at most `N`, and every byte at or past the length is zero.**
The literal cast and the zero value establish it, and every operation above establishes it for
its result. `concat` and `eq` rely on it. Nothing in script code can produce a value that breaks
it. A host can, by passing a malformed `Text<N>` as an argument to an entry or a function, and
then the result of an operation is defined but unspecified. Every read remains bounds-checked by
the virtual machine, so such a value cannot reach memory outside its body. Checking the invariant
at the host boundary belongs with the `shared` field work, which needs the same check.

### Feature gating and the reserved namespace

The `text` feature is off by default, as `shell` is, and it is `no_std`. `keleusma-cli` enables it,
so the shipping command line compiles the operations. The `math` and `audio` libraries named in
the ruling are not cargo features in this tree. `math` is a library bundle and `audio` is gated
on `floats`, so `shell` is the only precedent of the requested kind.

The `text` path is reserved in every build. With the feature, a call under it is an intrinsic.
Without the feature, the same call is refused at compile time with a message naming the feature,
rather than being accepted as a permissive native call, which is what any other `a::b(..)` call
without a `use` declaration becomes. A `use text::..` declaration is refused in both builds, so
one spelling never means a host native in one build and an intrinsic in the other. **That is a
host-observable change**, since a host could previously import a native under `text::`. None in
this tree did.

The continuous-integration feature sets cover both builds. The workspace run unifies the feature
on through `keleusma-cli` and runs `tests/text_ops.rs`. The single-package runs leave it off and
run `tests/text_ops_without_feature.rs`. The isolated `self-host` run has the feature off.

### What stays refused, and why

- **`==`, `<` and the other comparison operators** on `Text<N>`, by the ruling. `text::eq` is the
  comparison.
- **A pipeline target or a guarded operation under `text::`.** A pipeline would route the call
  through the permissive native path, and the operations are total and have no outcome arms.
- **A turbofish** on an operation, and an operand whose `Text<N>` capacity the compiler cannot read
  after monomorphization.
- **A result past the flat body limit.** A flat body is at most `u16::MAX` bytes including the
  length word, and on a narrow-word target a capacity must also be a non-negative word.
- **The self-hosted compiler** does not mirror the operations. A program using one necessarily
  names a `Text<N>`, because the literal cast is the only way to make an operand, so the existing
  shipping refusal reports it as unsupported. It is pinned by
  `a_bundled_text_operation_is_outside_the_self_hosted_subset` in
  `tests/selfhost_full_language.rs`. Mirroring them in the stages was not attempted, so whether it
  fits the stage caps is not measured.

### What is not built

Lexicographic ordering, rendering a number as text in the manner of the removed `to_string`,
reading one byte, and any operation into a smaller fixed capacity. The last is the first operation
that can overflow, and with it comes the truncation default and its optional arm following
`CheckedArmKind`.

## The `ScalarKind::Text` collapse, investigated 2026-10-31 and stopped

The operator authorized the collapse on 2026-10-31, with an instruction to stop and report rather
than implement if it required a `BYTECODE_VERSION` change, a new opcode, the removal of a
capability a host uses today, or a decision the brief does not settle. It requires the third or
the fourth, so it was not performed. Nothing below changes code.

### The premise does not hold yet

[The brief](./TEXT_N_IMPLEMENTATION_BRIEF.md) and the size comment in `src/value_layout.rs` both
state that the one-address form is correct only once `Text<N>` removes the dynamic case from bare
`Text`. **`Text<N>` has not removed it.** No producer of dynamic bare text has moved to
`Text<N>`, and none can today, because a native signature naming `Text<N>` is refused and a
`shared` field of that type is refused. These are the producers that remain, each found in the
tree on 2026-10-31.

- **Host natives returning text.** `KeleusmaType for String` in `src/marshall.rs` lets any native
  return an owned string. The `shell` library does so for environment values, command output and
  file contents in `src/stddsl/shell.rs`, and one of its natives returns a tuple of two strings.
- **Host inputs.** An entry argument or a resume value of type `Text` arrives owned, and when it
  sits inside a composite it is copied into the arena by `promote_field_text` in
  `src/bytecode.rs` and by the pack path in `src/vm.rs`. Its size is the host's, which is why
  `auto_arena_capacity_with_input_text` takes a host allowance.
- **The flat field read.** `read_flat_scalar` in `src/vm.rs` rebuilds the string with
  `KString::from_raw_parts(ptr, len, ref_epoch)` from the field's two words. The length lives in
  the second word and nowhere else.
- **Static text is length-delimited, not length-prefixed.** A string constant is an offset and a
  length into the wire string pool, read by `ConstTable::str_bytes` in `src/wire_schema.rs`, and a
  constant load hands out a pointer into the bytecode image with that length. No length word sits
  before the bytes.
- **Hot swap and shared data hold no text.** `replace_module` zeroes the persistent composite pool,
  and `src/marshall.rs` refuses to write a `Text` field into a flat host buffer.

### What a one-address form would have to be

A single address must recover the length from what it points at, so both kinds of text body would
need a length word in front of their bytes. That means an arena string allocated with a length
prefix, which changes the `KString` layout and adds a word per host string to the arena bound and
to the meaning of the host allowance, and a string pool that stores a length prefix beside each
constant, which changes the constant section of the wire format. **Neither choice is settled by
the brief**, which discusses only the scalar size. The alternative, dropping the dynamic case,
removes the host capabilities listed above.

No opcode would be needed, and a wire format change at version 2 is described by the brief as
free before publication. Both of those are inferences from the documents rather than measured
consequences, because nothing was implemented.

### Where the size feeds

Recorded so that the eventual change does not have to rediscover it. The size is
`ScalarKind::size_in_bytes` in `src/value_layout.rs`, where `flat_byte_size` also keeps text
boxed on a narrow-word target. It is consumed by `src/layout_pass.rs`, by the compiler's field
classification and its two text-specific sites in `src/compiler.rs`, by the text offset checks in
`src/verify_typed.rs`, by the flat read and pack paths in `src/vm.rs` and `src/bytecode.rs`, by
`src/marshall.rs`, and by the self-hosted driver's tables in `src/selfhost/mod.rs`, which derive
from `size_in_bytes` and would follow it. The baked field offsets of every composite with a text
field would move, and those offsets are on the wire. The tests pinning the current size are in
`src/value_layout.rs` and `tests/flat_text_layout_proof.rs`. The specification text is in
`docs/spec/TYPE_SYSTEM.md` and `docs/spec/NATIVE_STRING_ABI.md`. The scalar tag, `6`, would not
change.

`docs/spec/TYPE_SYSTEM.md` states that a flat `Text` field is always dynamic. The pack path in
`src/vm.rs` now packs a constant load's pointer into the bytecode image directly, so a flat field
can also hold static text. That observation is from reading the code and is not pinned by a test
written for this record.

### The decision this needs

Whether bare dynamic text keeps its place, with both bodies gaining a length prefix, or whether
the host producers move to `Text<N>` first, which needs the generic native signature question and
a host-side `Text<N>` type answered. Until one is chosen, `ScalarKind::Text` stays two words.

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
