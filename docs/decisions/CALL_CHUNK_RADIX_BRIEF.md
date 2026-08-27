# Brief: Widen the Call Record's Chunk Field

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Drafted**: 2026-08-26 (session 54)
**Status**: working brief

## The goal

A `Call` record packs two fields into one word as `chunk + count * 256`. A chunk index of
256 carries into the count field: **the callee becomes chunk zero and the argument count
becomes one too many**, so the call pops an operand that was never pushed.

`wire.kel` has 486 chunks. Widening this field is the last known blocker on it.

## What is already established, and how

| fact | evidence |
|---|---|
| a callee at index 255 compiles, at 256 it does not | synthetic 300-chunk program, both directions asserted |
| the reference accepts both | checked, so the refusal is the stage's and not the language's |
| chunk indices are assigned by **sorted name** | measured, not assumed |
| the failure surfaces in the **caller** | the reported chunk is the caller in every observation |

The mechanism is **not** an inference from a coincidence: it reproduces outside `wire.kel`
entirely, and the arithmetic predicts the exact boundary.

## The family, derived rather than listed

**Three code sites and three doc comments.** A cap is a family, and this project has twice
paid for widening one member and moving the wall rather than removing it.

| site | role |
|---|---|
| `parse.kel` | packs, in the Call emission |
| `reconstruct.kel` | unpacks |
| `tests/selfhost_codegen.rs` | unpacks — **the driver COPY** |
| `mod.rs`, and two comments in the copy | describe the packing in prose |

**The copy is not optional.** Five defects with one cause came from the shipping driver and
this copy diverging, and the boundary exercised only the copy.

The other `* 256` occurrences in the tree are the **token** packing (`tok + payload * 256`)
and are a different family. Do not change them.

## The recommended radix, and why

**Make the radix equal the chunk capacity.** `PARSE_CHUNK_CAP` is 1024, so chunk indices run
0..1023 and a radix of 1024 holds exactly that range with no overlap. The existing chunk-cap
guard then becomes the single authority on the bound, and **no new silent boundary is
created** — which is the failure mode being repaired, so recreating it one power of two
higher would be the worst possible outcome.

Check the arithmetic rather than assuming it fits. The emitted word is
`7 + (chunk + count * RADIX) * 64`, both stages `require word >= 32`, and the argument count
is bounded by the parameter cap. Compute the maximum and state it.

## Prior failures this work must not repeat

- **Widening one member of a family moves the wall.** Recorded: an index trap became a
  loop-limit trap became a different index trap, "each naming a size and none naming the
  cap".
- **A guard that cannot fire is worse than none.** Two were written this session that could
  not fire as first drafted; only running them showed it.
- **Three independent signals over one feature set are still one feature set.** A local gate
  green under `--features self-host` went red on four continuous-integration jobs because a
  new test file lacked its feature attribute.
- **A number in a message is not a cause.** Three occurrences in two increments, twice
  published as findings and retracted.

## The specific wrong turns to avoid

1. **Do not pick a radix larger than the chunk cap without saying why.** A radix of 65536
   with a cap of 1024 leaves a 64-fold range that no guard covers and no test reaches — a
   silent boundary of exactly the kind being removed.
2. **Do not change the token packing.** Same literal, different family.
3. **Do not skip the driver copy**, and do not assume the two are equivalent because the
   suite passes: the boundary table exercises the copy.
4. **Do not delete the pins that now fail.** `a_call_to_chunk_255_compiles_and_to_chunk_256_does_not`
   and `the_call_record_still_packs_the_chunk_in_eight_bits` are written to fail when this
   closes. **Re-aim them at the new boundary** so the next limit is pinned rather than
   unmarked.
5. **Do not claim `wire.kel` self-compiles** until the byte-identity oracle says so. That
   false claim was invented once and reached a doc comment, a pull-request body and three
   channels. Expect a further failure: `wire.kel` is the largest stage and has never been
   through the pipeline.
6. **No new opcode. No `BYTECODE_VERSION` change.**

## What success looks like short of a self-compiling `wire.kel`

A call to a chunk at index 256 and beyond compiles correctly, the new boundary is pinned at
the chunk cap, and whatever `wire.kel` does next is named. **That is a complete increment
even if `wire.kel` still fails**, and saying so plainly is better than stretching the scope
until it does not.
