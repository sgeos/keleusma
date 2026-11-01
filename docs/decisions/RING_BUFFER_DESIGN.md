# First-class ring buffers managed by the compiler

> **Navigation** [Decisions](./README.md) | [Backlog B42](./BACKLOG.md#b42-first-class-ring-buffers-managed-by-the-compiler) | [Dynamic Text capacity](./TEXT_CAPACITY_TYPE.md)

**Status.** Design, proposed 2026-10-30. Not implemented. No source file changes with this
document.

## Provenance

The operator requested first-class ring buffers managed by the compiler on 2026-10-14 and
reaffirmed the request on 2026-10-15. Backlog B42 records both. The operator ruled on
2026-10-30 that B42 be scheduled now. That ruling reached this document through the session
that commissioned it and was not received first-hand by its author. It is recorded here as
relayed, following the line's rule that a relayed ruling names its route.

At the base of this document, `origin/v0.2.3` at `c1380cee`, the reverse prompt still lists
operator questions 7 and 8 on `Text<N>` as open. The commissioning session describes item 8 as
`Text<N>` emission with the bundled text operations restored. This document does not depend on
how question 7 was answered, and it states below where it depends on question 8.

## Summary

The proposal has six parts.

1. A built-in type `Ring<T, N>` with a capacity `N` fixed at compile time, admitted in the first
   pass only as the type of a `private data` or `shared data` field with a scalar element type.
2. Seven operations on a ring-typed data place, `push`, `pop`, `peek`, `len`, `capacity`,
   `clear` and logical indexing `d.r[i]`, plus iteration `for x in d.r { }`.
3. A full ring is overwritten at its oldest element by default, and an empty ring yields the
   element type's zero by default. Each partial operation has an optional outcome arm in the
   shape of checked arithmetic. Everything that is statically wrong is refused at compile time.
4. A data ring lowers to `2 + N` ordinary data slots, a length word, a head word and `N`
   element slots, and every operation lowers to existing opcodes. **No new opcode, no wire
   format change and no `BYTECODE_VERSION` change is proposed.**
5. Every operation costs a constant number of instructions independent of `N`, allocates
   nothing in the arena, and iteration is a `for .. limit N` loop. The worst-case execution time
   (WCET) and worst-case memory usage (WCMU) bounds therefore follow from the type alone.
6. Four increments, each carrying its self-hosted twin, scheduled after items 5, 7 and 8.

## What the tree provides today

The following facts were verified in the tree at `c1380cee`.

| Fact | Where |
|---|---|
| The instruction set has 66 opcodes, and none writes one element of a flat composite in place | `pub enum Op` in `src/bytecode.rs` |
| Script values are conceptually immutable and the data segment is the sole mutable region | `docs/spec/GRAMMAR.md`, non-goals |
| A `[T; N]` data field takes one data slot per element, and `GetDataIndexed(base, len)` and `SetDataIndexed(base, len)` address those slots | `push_shared_slot_layout` and the indexed shape builder in `src/compiler.rs` |
| `GetDataIndexed` and `SetDataIndexed` are total. An out-of-range index reads the fallback or performs no write and reports a flag | doc comments on the two opcodes |
| A struct-typed data field takes one composite slot, written whole by `SetData` | doc comment on `Op::SetData` |
| A scalar private field with no initializer loads as the type's zero | `GRAMMAR.md`, data declarations |
| Private data persists across yield and RESET. Shared data is a host-owned buffer lent per call | `docs/architecture/EXECUTION_MODEL.md` |
| `Vm::replace_module` takes fresh private data from the host, and the shared buffer carries across a swap in host storage | `EXECUTION_MODEL.md` |
| The typed schema hash covers each data slot's name, visibility and declared type token | `compute_typed_schema_hash` in `src/bytecode.rs` |
| `postfix_expr` already admits `.name(args)` after a data field access | `method_call` in the `GRAMMAR.md` grammar |
| `CheckedArmKind` carries the outcome arms, including `InvalidIndex` and `StoreOk` | `src/ast.rs` |
| `Text<N>` exists at the type surface as `Type::TextN(ConstDim)` and is refused beyond it | `src/typecheck.rs` |
| A `for .. limit <const>` loop runs a runtime range under a static cap and is the canonical bounded header | `GRAMMAR.md`, bounded repetition |
| The nominal cost of `GetDataIndexed` and `SetDataIndexed` is 8 pipelined cycles, of `GetData` and `SetData` 1, of `Mod` 5 | `nominal_op_cycles` in `src/bytecode.rs` |
| No repository `.kel` source names a type or identifier `Ring` | `grep` over the tree |

## The governing analogy

The operator's analogy for `Text<N>` was that it is a `for .. limit` loop applied to space
instead of iterations. A ring buffer is the third member of that family. It holds a runtime
length and a runtime position under a static capacity, and it accepts a surface restriction to
buy a bound that the verifier reads from the type. The bound is a property of the type and not
of the operations, which is the architectural point `TEXT_CAPACITY_TYPE.md` makes for text and
which this design keeps.

## Surface syntax and type rules

### The type

```
private data hist {
  samples: Ring<Fixed, 16>,
  count: Word,
}

shared data inbox {
  events: Ring<Word, 32>,
}
```

`Ring<T, N>` is a built-in type in the manner of `Multiword<N, F>` and `Option<T>`. It parses as
`named_type` with one type argument followed by one const argument, which the grammar already
admits, and it is recognised by name in the parser as `Multiword` and `Option` are. It becomes
`Type::Ring(Box<Type>, ConstDim)` in the type checker.

The type rules are as follows.

- `N` is a const expression that resolves to a positive `Word`. A literal, a const-data field
  or a total const expression is admitted, exactly as for an array length. A zero or negative
  capacity is refused, as a zero-length array is.
- `2 + N` must fit within the twenty-four-bit indexed operand, the same bound the compiler
  applies to an indexed data array.
- `T` is a scalar in the first pass, one of `Word`, `Byte`, `bool`, `Fixed` and `Float`. `Text`,
  opaque types and composites are refused with a diagnostic naming the restriction.
- In the first pass `Ring<T, N>` is admitted only as the declared type of a field of a
  `private data` or `shared data` block. A `const data` ring is refused, since a ring that
  cannot change is an array. A ring in a local binding, a parameter, a return type, a yield type,
  a struct field, a tuple, an array element or an `Option` is refused at the type surface, in
  the way `Text<N>` is refused today.
- A ring field admits no initializer. It is empty at load because its slots are scalar slots that
  load as zero, as the next section shows.
- `Ring<T, N>` and `Ring<U, M>` unify only when `T` and `U` unify and `N` equals `M`. A ring never
  unifies with an array.

The restriction to data fields is the language's own shape and not a convenience. Values are
immutable and the data segment is the only mutable region. A ring is state, and its use in a
stream processor is the window of recent history that persists from one yield to the next. A
local ring would have to be rebuilt whole on every push, because no opcode writes one element of
a flat composite, and every rebuild would allocate a fresh body in an arena that is cleared only
at RESET. That cost is bounded but it is linear in `N` per operation and it accumulates in the
WCMU bound, and it is the subject of decision D2 below.

### The operations

Operations are written as method calls on a ring-typed data place. The type checker resolves
them as built-in operations on the place, in the same way it already gives `d.f[i] = v` a
dedicated meaning on a data place. They are not trait methods and no name becomes a keyword.
This choice is the subject of decision D1, and it should follow the operator's answer to
`Text<N>` question 8 so that the two capacity types read alike.

| Form | Type | Effect | Default on a partial outcome | Optional arms |
|---|---|---|---|---|
| `d.r.push(x)` | `()` | Appends `x` as the newest element | When full, overwrites the oldest element and advances the head | `ok`, `evicted(old)` |
| `d.r.pop()` | `T` | Removes and returns the oldest element | When empty, returns the zero of `T` and changes nothing | `ok(v)`, `empty` |
| `d.r.peek()` | `T` | Returns the oldest element without removing it | When empty, returns the zero of `T` | `ok(v)`, `empty` |
| `d.r[i]` | `T` | Returns the element at logical position `i`, where 0 is the oldest | When `i` is not below the length, returns the zero of `T` | `ok(v)`, `invalid_index(j)` |
| `d.r.len()` | `Word` | The live length, from 0 to `N` | None | None |
| `d.r.capacity()` | `Word` | The constant `N` | None | None |
| `d.r.clear()` | `()` | Sets the length to zero | None | None |
| `for x in d.r { }` | loop | Visits the live elements from oldest to newest | None | the existing `on` block |

The arms follow the discipline of every capture construct in the language. A handled form
carries an unguarded `ok` catch-all, each outcome appears at most once, and an arm may carry a
`when` guard. The form is an expression whose type is the common type of its arms. On `push` the
write happens on both outcomes and the `evicted` arm observes the element that was overwritten,
which the lowering reads only when that arm binds it, as the handled indexed write reads the old
element only when `ok(old, _)` binds it.

```
d.samples.push(s) {
  ok => (),
  evicted(old) => { d.count = d.count + 1; },
};

let oldest = d.samples.pop() {
  ok(v) => v,
  empty => 0.0,
};
```

Logical indexing reuses the existing `invalid_index` arm and its trap-free default exactly. A
logical index is not a slot index, since the compiler adds the head and wraps.

### Overflow and underflow

The rule is the one `Text<N>` was given, refuse what is static and handle what is not.

**Refused at compile time.** A capacity that is not a positive constant. A literal logical index
`d.r[k]` with `k` not below `N`, which is invalid on every execution. A ring whose slots exceed
the indexed operand bound. An element type outside the first-pass set. A ring outside a data
field. An outcome arm whose outcome cannot occur, mirroring the rule that rejects an `overflow`
arm on a `limit` loop. On a `Ring<T, 1>` every operation is still admitted, since full and empty
both occur.

**Handled at run time, with a total default.** A full ring on `push` overwrites its oldest
element. That is the behaviour that distinguishes a ring buffer from a bounded queue, and it is
the analogue of the wrapping default of checked arithmetic. An empty ring on `pop` or `peek`, and
an out-of-range logical index, yield the zero of `T`, which is the existing default of a total
indexed read. No operation traps unless a program asks it to, which keeps the language's
2026-10-20 position that only `Trap` traps. Whether an unhandled empty `pop` should instead trap
is decision D3.

The `Text<N>` precedent truncates on runtime overflow, which keeps the old content and drops the
new. A ring that did the same would be a bounded queue. Decision D4 offers that policy as an
alternative and explains why it is not recommended as the default.

## Runtime representation

### In a data segment

A field `r: Ring<T, N>` occupies `2 + N` consecutive data slots in declaration order.

| Slot | Content | Invariant maintained by the compiler |
|---|---|---|
| `base` | the live length `len`, a `Word` | `0 <= len <= N` |
| `base + 1` | the head `head`, a `Word`, the slot offset of the oldest element | `0 <= head < N` |
| `base + 2 .. base + 2 + N` | the elements, each a `T` slot | the live elements are the `len` slots starting at `head`, wrapping at `N` |

Length and head are kept rather than head and tail, so that full and empty are distinguished
without sacrificing a slot. The newest element sits at `(head + len - 1) mod N` and the next write
position is `(head + len) mod N`. The length word comes first, as it does in the flat layout of
`Text<N>`. The capacity is not stored, because monomorphization and const-data inlining erase it
to a literal the compiler bakes into every operand, as B40 erases every const parameter.

Because every slot is a scalar slot, the existing loader gives every slot the zero of its type,
so a private ring is empty at load without an initializer and without any interaction with the
definite-initialization rule of item 7. A shared ring whose host buffer is zeroed is likewise
empty.

This layout is the layout of three ordinary data fields, a `Word`, a `Word` and a `[T; N]`. The
first pass may therefore be implemented as a frontend expansion of the ring field into three
hidden data fields whose names cannot be written in source, for example `r.len`, `r.head` and
`r.buf`, followed by ordinary lowering. The exact hidden names are an implementation choice with
one hard constraint, that the reference compiler and the self-hosted compiler choose the same
bytes, since the names enter the data layout and the typed schema hash.

**Inference, not verified.** The shared-buffer layout entries for consecutive scalar slots are
expected to place the length, head and elements contiguously, so that a host can view a shared
ring as the flat composite `{ len: Word, head: Word, buf: [T; N] }`. The padding rules of the
shared layout for a `Byte` or `bool` element after two `Word` slots were not checked, and the
first increment carries a test that pins the byte offsets.

### As a value

The first pass has no ring values. If decision D2 later admits them, the representation is the
flat composite `{ len: Word, head: Word, buf: [T; N] }` with no reference field, which is the
same body as the data layout and the same choice `Text<N>` made for the same reasons. Such a value
carries no pointer and no epoch, may cross a yield, and costs the native backend no new
representation.

## Resource bounds

**WCMU.** A data ring adds `2 + N` slots to the data layout. A private ring is charged to the
arena's persistent region, which `Vm::new` and `Vm::replace_module` already check against the
persistent capacity. A shared ring is charged to `shared_data_bytes`, which the host allocates.
No operation allocates in the arena, since no operation emits `NewComposite`. The operand-stack
peak of each operation is a small constant, estimated below at no more than nine slots, and it
enters the existing stack bound unchanged.

**WCET.** Every operation is a straight-line sequence with at most one two-way branch, so its cost
is a constant independent of `N`, read off the existing per-opcode cost table. Iteration lowers to
`for k in 0..len limit N`, which the verifier already recognises as a bounded header, so the
loop's bound is `N` times its body plus the per-iteration overhead. No analysis is added. The
bound is read from the type, which is the property `TEXT_CAPACITY_TYPE.md` requires.

**Memory safety does not depend on the ring invariant.** Every element access goes through
`GetDataIndexed` or `SetDataIndexed` with the ring's element span as its operand. Those opcodes
are total and bounds-checked against that span, so even a corrupted head or length from a host
cannot read or write outside the ring's own elements. The invariant governs only the meaning of
the contents. This matters for a shared ring, whose length and head a host may write between
calls, and it is the subject of decision D5.

## Lowering to existing opcodes

The sequences below are estimates written against the opcode contracts in `src/bytecode.rs` and
the shape of `emit_data_indexed_write` in `src/compiler.rs`. They were not compiled. `L`, `H` and
`B` denote the slot indices of the length, the head and the element span of `N` slots. The wrap
is written with the remainder opcode, `Mod` followed by `PopN(2)` to keep the low result, which is
correct because its left operand is never negative for a private ring. A power-of-two capacity
admits `BitAnd` with `N - 1` instead, which is cheaper, and choosing between the two is an
implementation choice provided both compilers make the same one.

### `d.r.push(x)`

```
<x>; SetLocal v                         evaluate the element once
GetData L; Const N; CmpLt; If           not full?
  GetData H; GetData L; CheckedAdd; PopN 2
  Const N; Mod; PopN 2; SetLocal i      i = (head + len) mod N
  GetLocal v; GetLocal i
  SetDataIndexed(B, N); PopN 6          write, index in range by construction
  GetData L; Const 1; CheckedAdd; PopN 2; SetData L
Else                                    full, overwrite the oldest
  GetData H; SetLocal i
  [GetLocal i; Const 0; GetDataIndexed(B, N); PopN 6; SetLocal old]   only if evicted(old) binds
  GetLocal v; GetLocal i
  SetDataIndexed(B, N); PopN 6
  GetData H; Const 1; CheckedAdd; PopN 2
  Const N; Mod; PopN 2; SetData H
EndIf
```

About 40 instructions are emitted. The worst path executes about 30, for an estimated 45 to 60
nominal pipelined cycles plus the cost of evaluating `x`, and about 10 more cycles when the
`evicted` arm binds the old element. The arm dispatch adds the same virtual-loop shape the
handled indexed write uses.

### `d.r.pop()` and `d.r.peek()`

```
GetData L; Const 0; CmpGt; If           not empty?
  GetData H; Const 0; GetDataIndexed(B, N); PopN 6; SetLocal r
  GetData H; Const 1; CheckedAdd; PopN 2; Const N; Mod; PopN 2; SetData H     pop only
  GetData L; Const 1; CheckedSub; PopN 2; SetData L                           pop only
Else
  Const 0; SetLocal r                   the zero of T, or the empty arm
EndIf
GetLocal r
```

About 30 instructions for `pop` and about 14 for `peek`. The `Const 0` before `GetDataIndexed` is
the fallback operand, typed as the element, as the existing total read takes it.

### `d.r[i]`, `len`, `capacity`, `clear` and iteration

| Form | Estimated sequence | Estimated instructions |
|---|---|---|
| `d.r[i]` | evaluate `i`, test `0 <= i < len`, then `(head + i) mod N` and `GetDataIndexed(B, N)`, else the zero or the `invalid_index` arm | about 25 |
| `d.r.len()` | `GetData L` | 1 |
| `d.r.capacity()` | `Const N` | 1 |
| `d.r.clear()` | `Const 0; SetData L` | 2 |
| `for x in d.r { body }` | `for k in 0..len limit N`, binding `x` to the logical element `k` each iteration | about 15 per iteration plus the existing `limit` header |

For a private ring the `limit` cap is never reached, because `len <= N` holds by construction, and
the compiler supplies the `limit` outcome itself so that a bare `for x in d.r` neither traps nor
asks the programmer for an `on` block. A programmer-supplied `on` block keeps its ordinary meaning
with the `limit` outcome removed, since it cannot occur.

### The opcode-count position

**The design needs no new opcode.** It uses `GetData`, `SetData`, `GetDataIndexed`,
`SetDataIndexed`, `GetLocal`, `SetLocal`, `Const`, `CheckedAdd`, `CheckedSub`, `Mod`, `PopN`, the
comparisons, the block-structured control opcodes and, for a power-of-two capacity, `BitAnd`. The
instruction set stays at 66 opcodes, the wire format is unchanged and `BYTECODE_VERSION` stays 2.

A fused instruction that computed a wrapped ring index and accessed the element in one step would
save an estimated six to ten instructions per operation. It is not proposed. The saving is a
constant factor on a constant cost, and the radiation-hardened minimal instruction set treats the
opcode count as a design limit that a constant factor does not justify spending.

The design adds two arm kinds to `CheckedArmKind`, `Evicted` and `Empty`, which are abstract
syntax and not bytecode. If decision D3 chooses a trap for an unhandled empty `pop`, it reuses the
existing `IndexOutOfBounds` trap code, so no trap code is added either.

## Residence, yields, RESET and hot swap

| Residence | First pass | Across a yield | Across RESET | Across a hot swap |
|---|---|---|---|---|
| `private data` | admitted | persists | persists | replaced by the private data the host supplies to `replace_module` |
| `shared data` | admitted | persists, and the host may read or write it between calls | persists | carried in the host's buffer, guarded by the typed schema hash |
| local, parameter, return, yield | refused, decision D2 | not applicable | not applicable | not applicable |
| `const data` | refused | not applicable | not applicable | not applicable |

**Yields.** A ring lives in the data segment, which persists across every yield, so a ring is the
natural carrier of history from one tick to the next. Ownership returns to the host at each
yield, so a shared ring is a single-producer single-consumer channel between host and script
with no atomics, because the execution model already gives the script exclusive access while it
runs.

**Hot swap.** A private ring follows the existing private-data rule. The host supplies the new
module's private slots, and because a host cannot read a private slot, a swap cannot carry a
private ring's contents forward. That is pykeleusma question 6 in the reverse prompt, and this
design inherits its answer rather than settling it. A shared ring survives in the host's buffer.
Its slots enter the typed schema hash under their hidden names with their scalar type tokens, so
a swap that changes the capacity, the element type or the position of the ring changes the hash.
**Inference, not verified.** The hash already distinguishes a reordered struct, so it is expected to
distinguish a ring of a different capacity, since the slot count and names change. The first
increment pins this with a test rather than relying on the inference.

**Shared rings and host writes.** Between calls a host may write any value into a shared ring's
length and head. Memory safety holds regardless, as shown above. Whether the script validates the
two words on each operation is decision D5.

## The self-hosted compiler and the differential oracle

The fifteen stage sources under `src/selfhost/kel/` are affected as follows. This is an inference
from the stage names and from where the reference work lands, and each increment confirms it.

| Stage | Change |
|---|---|
| `parse.kel` | recognise `Ring` in a type position with one type argument and one const argument, and the method-call forms on a data place, which already parse as postfix method calls |
| `analyze.kel` | the type rules, the admissibility refusals and the resolution of the operations |
| `codegen.kel` | the expansion into three hidden fields and the lowering sequences above |
| `verify_datalayout.kel`, `wire_schema.kel` | the `2 + N` slot layout and the schema tokens, which must match the reference byte for byte |
| other verifier stages | no change expected, since every emitted opcode is already analysed |

**The byte-identical oracle is the correctness signal**, and it constrains the design in one way
that matters. Every choice that affects emitted bytes must be specified, not left to each
compiler. That covers the hidden field names, the slot order, the wrap instruction, the order in
which the push sequence evaluates `x` and reads the length, the names of the temporary locals and
the `limit` desugaring. Defining the first pass as a frontend expansion into ordinary data fields
helps here, because both compilers then reuse lowering paths that are already byte-identical, and
the new work is the expansion itself.

Each increment carries its self-hosted twin before it merges to the version branch, so that the
construct-support boundary pinned by `self_hosted_construct_support_boundary` in
`tests/selfhost_codegen.rs` gains new `Ok` rows rather than new `Gap` rows. The self-hosted
compiler's present refusal of multi-index data access does not bind, since every ring access is a
single-level index.

## The native backend line, V0.3.X

The V0.3.X line consumes this line's bytecode. Because every ring operation lowers to opcodes that
backend already translates, a data ring costs it nothing new, and its seeded interpreter
differential sees ordinary data traffic. If decision D2 later admits ring values, their flat
composite body with no reference field is the representation `TEXT_CAPACITY_TYPE.md` records that
backend as already packing. The line should be told of this design through the interline
mailbox once the operator accepts it, since its type and schema token appear in modules it reads.
That message has not been sent, because this task is design only.

## Alternatives considered and not recommended

- **A library ring written as a user struct.** `struct Ring<T, const n: Word> { len: Word, head:
  Word, buf: [T; n] }` with pure functions is expressible today. It is a value and so pays an
  `O(N)` rebuild and an arena allocation per push, and it cannot be mutated in a data field
  except by a whole-body `SetData`. It does not meet the request that the compiler manage the
  buffer.
- **Head and tail indices.** These need either a wasted slot or a separate full flag to tell full
  from empty. Length and head need neither.
- **A new ring opcode.** Declined above on the instruction-set constraint.
- **Inferring the bound from the operations.** Declined for the reason `TEXT_CAPACITY_TYPE.md`
  gives, that the value of a capacity type is that its bound survives loops, branches and calls.

## Schedule

Work in flight on this line is item 5, the flat Text layout proof, with its self-hosted twin
next, item 7, definite initialization, which waits on question 1, and item 8, `Text<N>` emission
with the bundled text operations restored. The proposal places B42 after all three, as items
13a to 13d, for three reasons. Increment R1 edits the data-layout code that item 7 also edits, so
running them together invites a conflicting rule. The surface of the operations should match the
surface the operator chooses for `Text<N>` in question 8. And item 5 is nearly complete.

R1 has no dependency on item 8's code and may start as soon as item 7 merges, if the operator
prefers B42 earlier. R2 and R3 should wait for the answer to decision D1.

| Increment | Scope | Test obligations |
|---|---|---|
| R1. Type surface and layout | Parse `Ring<T, N>`, `Type::Ring`, the admissibility refusals, the `2 + N` slot expansion with hidden names, the schema tokens, the spec text in `TYPE_SYSTEM.md` and `GRAMMAR.md`, and the self-hosted twin | Parse and unify tests. One negative test per refusal, zero and negative capacity, non-scalar element, ring in a local, parameter, return, struct field and `const data`, an initializer, and an over-bound capacity. Slot count and private persistent bytes. Shared byte offsets pinned. Typed schema hash differs for `Ring<Word, 8>`, `Ring<Word, 9>`, `Ring<Byte, 8>` and a plain `[Word; 10]`. Byte identity with the reference on a program that declares rings and reads nothing |
| R2. Reads | `len`, `capacity`, `peek`, logical indexing with `invalid_index`, iteration through `limit N`, and the twin | A model test against a `VecDeque` over seeded host-written states of a shared ring. Literal index refusal. Boundary indices `0`, `len - 1`, `len` and `N - 1`. Empty-ring defaults. A WCET test that the bound of `for x in d.r` scales with `N` and not with the runtime length. Byte identity for every form |
| R3. Mutation | `push` with `evicted`, `pop` and `peek` with `empty`, `clear`, and the twin | A seeded random operation sequence checked against a `VecDeque` model for `N` of 1, 2, 3, 8 and 16, covering wrap-around, full, empty and the `N = 1` case. Persistence across yield and RESET. A private ring after `replace_module`. A shared ring across a swap with an unchanged and with a changed schema. Corrupted shared length and head, negative and oversized, showing that no access leaves the ring's span, plus the behaviour chosen in D5. A test that no ring operation changes the arena WCMU. Per-operation WCET constant in `N`. Byte identity for every form and arm |
| R4. Documentation and close-out | The book chapter in English and Japanese, a changelog fragment, the completion record with receipts and residual limits | The documentation guards, the link check and the doc build |

Ring values in locals and composite elements, decisions D2 and D6, are outside these increments
and would be designed separately if the operator chooses them.

## Decisions for the operator

Each decision lists lettered options with the recommendation first. None blocks R1 except D1,
which shapes the surface R1 parses, and that dependency is light because the method-call shape
already parses.

**D1. Surface form of the operations.**
(a) Built-in operations written as method calls on a ring-typed data place, `d.r.push(x)`,
resolved by the type checker and not by a trait impl, aligned with the answer to `Text<N>`
question 8. Recommended because the call shape already parses, it names the place being changed,
and it adds no keyword.
(b) Free functions taking the place, `ring_push(d.r, x)`. This passes a place where every other
call passes a value, which is a new argument mode.
(c) Dedicated statement forms such as `push d.r <- x`, in the manner of `data_field_index_assign`.
This is the most explicit and needs new grammar.

**D2. Ring values outside the data segment.**
(a) Data fields only in the first pass, with local rings designed later if a use appears.
Recommended because the data segment is the language's only mutable region and a ring is state.
(b) Admit local ring values with functional operations that rebuild the body, at a cost linear in
`N` per operation and an arena allocation per push until RESET.
(c) Admit read-only local snapshots, `let s = d.r`, which copy the `2 + N` slots once into a flat
composite for passing to a function.

**D3. An unhandled empty `pop` or `peek` and an unhandled out-of-range logical index.**
(a) Return the zero of `T` and change nothing, with the optional arm. Recommended because it is
total, it matches the existing default of a total indexed read, and it keeps the position that
only `Trap` traps.
(b) Trap with the existing `IndexOutOfBounds` code, as an unhandled data index write does.
(c) Return the zero for `peek` and the index, and trap for `pop`.

**D4. The default when `push` meets a full ring.**
(a) Overwrite the oldest element, with an optional `evicted(old)` arm. Recommended because that
behaviour is what makes the structure a ring rather than a queue, and it is the history-window
semantics a stream processor needs.
(b) Keep the old contents and drop the new element, with an optional `full(x)` arm. This is the
direct analogue of the `Text<N>` truncation default and is a bounded queue.
(c) Two types, `Ring<T, N>` with (a) and `Queue<T, N>` with (b), sharing the layout and lowering.

**D5. Validation of a shared ring's length and head written by a host.**
(a) Validate both words on each operation and trap with `IndexOutOfBounds` when `len > N`,
`len < 0`, `head >= N` or `head < 0`, for shared rings only, at an estimated cost of ten
instructions per operation. Recommended because external input is untrusted and a secure failure
is a refusal, while memory safety already holds without it.
(b) Validate and repair a corrupted ring to empty.
(c) No validation, with the host contract documented and the meaning of a corrupted ring left
unspecified, in the manner of the documented host struct contract of question 2.

**D6. Element types.**
(a) Scalars only in the first pass, `Word`, `Byte`, `bool`, `Fixed` and `Float`. Recommended
because each element is then one data slot and the indexed opcodes address it directly.
(b) Also flat composites of scalars, which need a stride above one and the nested composite read
path, and which this document has not verified on data arrays.
(c) Also `Text<N>`, once item 8 lands, which combines (b) with a second capacity type.

**D7. Placement.**
(a) After items 5, 7 and 8, as items 13a to 13d. Recommended for the reasons in the schedule.
(b) Start R1 as soon as item 7 merges, in parallel with item 8.
(c) Before item 8, accepting that the operation surface may need revision when question 8 is
answered.

## Unverified points and residual limits

- The instruction sequences and their counts are estimates. They were not compiled, and the
  admissibility of each under the verifier's stack and block-structure rules is checked in R2 and
  R3, not here.
- The contiguity of a shared ring's bytes, and the behaviour of the typed schema hash for a
  changed capacity, are inferences pinned by R1 tests.
- Whether a `pure` function may read or write a ring was not checked. The intended rule is that a
  ring operation is admissible exactly where the corresponding data read or write is.
- The behaviour of `Mod` on the negative head a host could write into a shared ring was not
  traced. The total indexed opcodes keep it memory-safe, and D5 decides its meaning.
- The stage-by-stage impact on `src/selfhost/kel/` is inferred from stage names and is confirmed
  per increment.
- This document does not change the cross-swap rule for private data, which remains with
  pykeleusma question 6.

## Operator rulings, 2026-10-31

Received first-hand in the V0.2.X session. D2, D3, D5, D6 and D7 were relayed by the kaizen line
as option (a) and confirmed first-hand. The operator answered D1 and D4 as follows.

> D1. A. Switch `Text<N>` to match.
> D4. A. Also, we want some sort of overflow arm if it is possible to batch push something like
> 6 elements into a 4 element ring buffer.

D1 is therefore method calls on the ring's data place, `d.r.push(x)`, and the `Text<N>`
operations move to the same method-call surface. D4 is the overwrite of the oldest element, with
the optional `evicted(old)` arm on a single `push`.

### The batch push and its overflow arm, proposed 2026-10-31 for review

**Not yet ruled.** This section answers the second half of the D4 ruling and is for the
operator's review before R1 begins.

`d.r.push_all(xs)` pushes the elements of `xs: [T; K]`, oldest first, as `K` single pushes would.
`K` is a compile-time constant, since every array length is. With the D4 default, the ring then
holds the newest `min(N, len + K)` elements, and when `K > N` it holds the last `N` elements of
`xs`.

Two kinds of loss can happen, and the proposal keeps them apart because a host usually cares
about the second one.

- **Eviction.** Elements already in the ring are overwritten. Their number is
  `min(len, max(0, len + K - N))`, known only at run time.
- **Overflow.** Elements of the batch itself are overwritten by later elements of the same
  batch. Their number is `max(0, K - N)`, a compile-time constant.

| Arm | Binds | Runs when |
|---|---|---|
| `ok` | nothing | No element was lost |
| `evicted(count)` | the number of ring elements overwritten, a `Word` | At least one ring element was overwritten and no batch element was lost |
| `overflow(count)` | the number of batch elements lost, a `Word` | At least one batch element was lost |

The write happens on every outcome, as for a single `push`, and the arms only observe it.
Because overflow is decided by `K` and `N` alone, the compiler knows whether the `overflow` arm
can run.

**Decisions for the operator**, recommendation first.

- **B1. When `K > N`, is a covering arm required?**
  (a) Yes. A `push_all` whose batch can never fit is refused at compile time unless it carries an
  `overflow` arm, so the loss is always written down. When `K <= N`, an `overflow` arm is refused
  as unreachable.
  (b) No. The default keeps the newest `N` elements silently, as a single push does.
- **B2. A run-time batch length.**
  (a) Not in the first pass. A batch is a whole `[T; K]`.
  (b) Also `d.r.push_n(xs, n)`, pushing the first `n` elements with `n` at most `K` clamped,
  where overflow becomes a run-time outcome and the `overflow` arm is optional.

**Cost.** The lowering is the single-push sequence repeated `K` times with the outcome counters
accumulated, so the instruction count and the worst-case execution time are linear in `K` and
constant in `N`. No opcode is needed, and the arena use is unchanged, since a ring lives in the
data segment.
