# Brief: The Mechanism Behind `wire.kel`'s Empty-Stack Refusal

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Drafted**: 2026-08-26 (session 54)
**Status**: working brief

## The goal

`wire.kel` is the largest stage at 486 chunks and the only one outside the byte-identity
corpus. With radix literals landed it now refuses with a named cause: **a pop from an empty
work stack**, meaning the record stream consumed more operands than it produced.

**The deliverable is the MECHANISM, named and demonstrated.** Not a repair, and explicitly
not another inference from a coincidence.

## What is established, and it is a short list

| fact | how it was established |
|---|---|
| `wire.kel` at 1,673 lines self-compiles; at 1,675 it does not | prefix bisect over the real file |
| line 1674 is blank, so **one declaration** flips it | direct reading |
| the chunk count alone is **not** the trigger | a synthetic program of 300 trivial chunks compiles |
| the intern cap is not involved | the whole file has 667 distinct identifiers against a cap of 1,280 |
| the token cap is not involved | the whole file is ~25,700 tokens against a cap of 40,960 |
| `put_u64` in isolation compiles, as do the three real writers verbatim | eleven guessed reproductions, all passing |

## What is NOT established, stated because I twice claimed otherwise

**THE MECHANISM IS UNKNOWN.** Two causes have been published for this file's failure and both
were wrong:

1. *"A capacity bound, `IndexOutOfBounds(-1, 1024)`"* — false. An index of `-1` is below the
   start. The real first cause was that the lexer had no radix-literal support at all.
2. *"A cap of 256 on the chunk count"* — false, and disproved within the hour by the
   experiment that should have come first.

**Both were a number in a message read as if it identified a cause.** The second was worse
than the first: the number was in the right place, which made it more convincing. **Three
occurrences in two increments. Assume a fourth is available.**

## The wrong turns to avoid, in priority order

1. **DO NOT TREAT THE REPORTED CHUNK NAME AS A LOCATION.** The failure names `put_u64` at
   line 270. A declaration 1,400 lines later cannot affect a function 1,400 lines earlier, so
   the name is a **label** derived from `names[fns[i].name]`, an interned id. Either the name
   is misattributed, or the records assigned to it are not its own. **Establish which before
   reading any code at line 270.**
2. **DO NOT WIDEN ANY ARRAY.** Nothing has been shown too small. The recorded lesson from
   `every_chunk_indexed_array_admits_the_chunk_cap` is that widening one member of a family
   moves the wall rather than removing it: an index trap became a loop-limit trap became a
   different index trap, "each naming a size and none naming the cap".
3. **DO NOT GUESS CONSTRUCTS.** Eleven guessed reproductions passed here; fourteen passed on a
   previous defect of this kind. The prefix bisect found the boundary in one run. **Prefer an
   instrument or a bisect over a hypothesis, earlier than feels natural.**
4. **DO NOT INFER FROM A COINCIDENCE WITHOUT THE FALSIFYING EXPERIMENT.** If a threshold is
   suspected, construct the minimal program that crosses it and nothing else. That experiment
   costs minutes and has already overturned one published finding.
5. **DO NOT CLAIM `wire.kel` SELF-COMPILES** on any partial result. That false claim was
   invented once and reached a doc comment, a pull-request body and three channels.
6. **Expect another failure after this one.** `wire.kel` is the largest stage and has never
   been through the pipeline. Each cause named is progress, not completion.

## Promising directions, offered as directions and not as findings

- **Arity.** An empty-stack pop is what a call record produces when its operand count exceeds
  what was pushed. A pre-existing audit regression, `tests/zz_call_underflow_repro.rs`, exists
  precisely because a `Call` whose argument count exceeds the callee's local-slot count
  underflows. If a call record resolves to the wrong callee, its arity is wrong by
  construction. **This is a direction to test, not a conclusion.**
- **Function-boundary assignment.** If `parse.kel` mis-places a function-end marker, records
  shift across chunk boundaries and a later construct changes an earlier chunk's stream. This
  would explain the 1,400-line distance without any global cap.
- **The guard worth strengthening regardless of this defect.**
  `every_chunk_indexed_array_admits_the_chunk_cap` derives its family from a hand-written list
  of **two** index expressions in **one** file, while its own doc says a cap is a family. That
  is the recorded meta-defect — coverage as a property of the case list — and it is true
  independent of whether it relates to this failure.

## Instruments that already exist

`parse_record_trace` (what parse emits, carrying its own cursor), `parse_cursor_trace` (where
it reads), `lex_token_trace` (what it reads), and the five named reconstruct causes. **Do not
zip the cursor and record traces**: they sample at different rates and pairing them by index
produces a table that looks like data and is not.
