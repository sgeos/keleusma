# The flat Text layout proof

> **Navigation** [Decisions](./README.md) | [Flat Text decode soundness](./FLAT_TEXT_DECODE_SOUNDNESS.md)

## The ruling

The operator ruled first-hand on 2026-10-28 on the first residual limit of
[the flat Text decode record](./FLAT_TEXT_DECODE_SOUNDNESS.md#residual-limits), the
VM-internal field reads.

> (a) The verifier proves every flat Text slot is written only from a Text value, and rejects
> the module otherwise.

This document is the design that ruling asked for before implementation. Increments 1 and 2
below are implemented in the reference typed pass, `src/verify_typed.rs`, and pinned by
`tests/flat_text_layout_proof.rs`. Increment 3, the self-hosted twin, is not yet built, so until
it lands the self-hosted validator admits a hand-built module whose mistyped `Text` read the
reference refuses. Increment 4 waits on the operator.

## The defect the proof closes

`read_flat_scalar` rebuilds a flat `Text` field from its two words, a pointer and a length,
during `GetField`, `GetIndex` and the related reads. `Vm::decode` checks those two words at the
host boundary since pull request 517, and the VM-internal reads do not. The compiler bakes the
correct field kinds, so compiled code does not reach a wrong layout. A hand-built module the
typed verifier admits can, because the typed pass knows only a flat body's kind and byte size.
`(Word, Text)` and `(Text, Word)` are the same abstract value to it, so a `GetField` of kind
`Text` at offset 0 of a body whose offset 0 holds a `Word` is admitted, and the read treats an
integer as an address.

## What the proof needs

The typed pass's abstract value for a flat body, `AbsVal::Flat { kind, size }`, gains the
body's field layout where it is known. The layout is a list of `(offset, kind)` for each scalar
field, with a nested body's fields at their offsets within it. It is derived where a body is
built, from the abstract values of a `NewComposite`'s elements, which are scalars of known
kinds or nested bodies of known layout. It is carried through the operand stack, locals and
the per-chunk fixpoint the pass already runs, and two layouts join to a known layout only when
they are equal.

The rule is then two checks.

1. **A read.** A `GetField`, `GetIndex` or related read of kind `Text` at an offset of a body
   whose layout is known must find a `Text` field at that offset. Any other kind there is a
   rejection.
2. **A write.** A `SetField` or indexed store of a `Text` field must store a value whose
   abstract value is `Scalar(Text)`, and a store into a known `Text` position of anything else
   is a rejection.

No opcode is added and the wire format does not change. The layout lives only in the
verifier's state.

## Where the proof cannot reach

A body whose layout the pass cannot know is one the program did not build. There are five
such sources.

1. An entry argument, built by the host and canonicalised by the VM.
2. A resume value.
3. A native's result.
4. A shared composite slot, written by the host into the borrowed buffer.
5. A private composite value seeded at a hot swap.

Each carries a kind and a size on the wire, through the chunk signatures, the native return
shapes or the shared slot table, and no field layout. This is the same limit as the host struct
question and the schema hash, and lifting it means field layouts on the wire, which needs
authorization.

Under the ruling as written, a read of kind `Text` from such a body would be a rejection,
because the pass cannot prove the field holds text. That would refuse every program that reads
a `Text` field of a struct parameter, which is ordinary code, so this line reads the ruling as
covering bodies the program builds and does not apply it to the five sources above without
the operator's confirmation. It is question 9 in the reverse prompt.

For those five, the defect is a host that passes a body of another layout. Since pull request
517 the decode path refuses a text field that does not lie in the arena or the bytecode image.
The VM-internal read does not. A constant-cost range check at that read, the same check without
the UTF-8 pass, would close the five without making a time bound depend on a string's length.

## Increments

1. The layout in `AbsVal::Flat`, derived at `NewComposite`, joined conservatively, and carried
   with no new rejection. The pinned verdicts must not move.
2. The read and write checks for bodies of known layout, with hand-built modules that the
   current pass admits and the new one refuses, and the corpus unchanged.
3. The self-hosted twin in `verify_typed.kel`, whose byte-identical verdicts the validator's
   agreement tests compare. That stage records a scalar entry by its size alone, so the twin
   needs the host to marshal each pushed scalar's kind as well, and the stage to carry each
   flat entry's `Text` offsets, which a bounded table of four per entry would hold, with a body
   holding more left unknown and deferred.
4. For the five boundary sources, whatever the operator rules.

## What this does not change

The decode-time checks of pull request 517 stay. The type checker already prevents a compiled
program from building a mistyped body, so for compiled code the proof adds a check the compiler
already satisfies, and its value is against hand-built and hostile modules, which is the class
`verify` exists to exclude.
