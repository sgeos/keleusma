# The four opcodes whose lowering arms had never run

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: all four resolved to a status with evidence, 2026-08-28. **No opcode census figure moved,
and none should have** — the census's population is the corpus, and this asked a broader question.

## Why it was worth asking

`isa_lowering_census` reports **61 of 66 opcodes lowered**, three of them "emitted, never visited,
never named" and one with no corpus witness. **"The backend lowers it" is a claim about an arm
existing, not about its behaviour**, and an arm that has never executed is where a miscompile hides.

## The four, measured

| opcode | status | evidence |
|---|---|---|
| `IntToFloat` | **REFUSED by name** | `(x as Float) as Word` compiles; the backend says *"does not yet support opcode IntToFloat"* |
| `FloatToInt` | **Unreached, behind that refusal** | the same program emits both; lowering stops at the first |
| `Reset` | **Reachable, module lowers, and already witnessed** | a minimal `loop main` is refused nothing; the suspension differential drives 15 such subjects |
| `IsStruct` | **No producer found; structurally blocked** | two further shapes refused by the type checker; the reference's arm accepts only a `Boxed` body, and B28 left zero non-`Flat` composites |

## The brief was wrong about `Reset`

It guessed `Reset` was gated behind the `Stream` refusal and therefore unreachable. **It is not.** A
minimal `loop main(t: Word) -> Word { yield t }` emits `Stream` and `Reset` and the backend refuses
nothing: `Stream` is lowered for that shape, and the refusal on `13_telemetry_stream.kel` is about
that module rather than about the opcode. An earlier increment on this line described `Stream` as
unsupported outright, which is true of one module and not of the instruction.

## The census is not wrong, and that distinction matters

Its own output says *"unproven FROM THE CORPUS, which is the only population this..."*. **"Never
visited" is a claim about the shipped corpus, not about the test suite.** `yield_sequence.rs` drives
**15** `loop main` subjects through both the native lowering and the reference, comparing whole
yielded sequences, and every such program emits `Reset`.

**The linkage is checked rather than cited**: a test reads the sibling harness for its subjects and
separately confirms such a program emits `Reset`. Citing a neighbouring file without checking it is
how a claim outlives the file it describes.

## `IsStruct`: the search is recorded, not a conclusion

The `v0.2.3` line recorded a bounded producer search in `src/compiler.rs`, with the standard it
adopted after a producerless claim there was falsified within the hour: **record the search, not the
conclusion.** Two further shapes were tried here and both are refused by the type checker — a struct
pattern against a foreign type, and a struct pattern in a `match` arm.

**That is a fact about this search, not a proof of unreachability**, and the test says so in those
terms.

## Nothing was widened to make a test possible

The float guard blocks the conversion witness, and that is the finding rather than an obstacle to it.
Relaxing a guard to reach an unexercised arm would be widening a compiler on the strength of wanting
a test, and the float entry ABI is in any case operator-held.
