# Brief: Name `reconstruct.kel`'s Failure Modes

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Drafted**: 2026-08-26 (session 54)
**Status**: working brief

## The present goals, as the tree states them

V0.2.x completes when the five criteria in `../roadmap/V0_2_X_ROADMAP.md` hold. None do.
Order 1 has three items:

| item | state |
|---|---|
| 1. `CONSTS` | **DONE**, byte-identical for all twelve stage sources |
| 2. the remaining region kinds | 93% produced / 56% computed |
| 3. the type checker's INPUT | rules complete, resolution in the stage, extraction still Rust |

Behind all three sits the byte-identity corpus, which covers **ten** stages and not
`wire.kel`, the largest at 486 chunks. `wire.kel` is the single largest hole in the
oracle, and the recorded lesson from the boolean-literal and `Byte`-cast miscompiles is
that *any construct the corpus does not contain is unverified by construction*.

## The recommendation, and why it is not the obvious one

**Name `reconstruct.kel`'s failure modes, then diagnose `wire.kel` with the named
message.** Not: raise a cap. Not: diagnose `wire.kel` directly.

`self_host_compile(wire.kel)` fails with `call: IndexOutOfBounds(-1, 1024)`, raised from
`reconstruct_via_kel`.

**The handoff recorded this as "a capacity bound ... the shape of a node-array bound".
That is wrong, and it is wrong in the exact way the same handoff warns about.** It read
the `1024` and inferred a limit. The `-1` says the opposite: the index is below the
start, not past the end. `pop()` in `reconstruct.kel` decrements *then* indexes, so an
empty work stack yields `rs.stack[-1]`. This is an **underflow**, and an underflow means
the record stream consumed more operands than it produced — a structural defect in what
`parse.kel` emitted for some construct in `wire.kel`.

**But that attribution is itself not yet established, and this is the whole point.**
`reconstruct.kel` declares **seven** 1024-wide arrays: `rec_kind`, `rec_arg`, `kinds`,
`args`, `lhs`, `rhs`, and `stack`. An out-of-range index on any of the seven produces a
byte-identical message. The `-1` narrows the *shape* to an underflow; it does not name
*which* array, and three plausible causes fit:

> **CORRECTION, made while writing this brief and left in rather than edited away.**
> Seven is the count of arrays that share *this* failure's message, not the size of the
> problem. Derived from the source, `reconstruct.kel` declares **26** arrays in six size
> classes -- 1024 (7), 256 (7), 64 (3), 16 (4), 8 (4), 128 (1). **25 of the 26 share a
> message with at least one sibling**; only `sqpending` is unambiguous. I wrote "seven"
> because seven is what the failure in front of me pointed at. That is the defect this
> document's own wrong-turn list warns about, committed in the paragraph above it.

1. `pop()` on an empty work stack — a record stream that pops more than it pushed;
2. a node index of `-1` (an unset field, a failed lookup) used to read `kinds`/`args`/
   `lhs`/`rhs`;
3. a read of `rec_kind`/`rec_arg` at a negative cursor.

**Guessing between them is exactly the failure this project has a programme against.**
`parse.kel` had four groups of constructs sharing one message; thirteen named modes
across eleven guarded counters removed that, and the handoff records that tracing one
such failure cost **seven increments** before the message named its cause. `reconstruct.kel`
has received none of that work. It is the same defect, one stage later.

**So the deliverable is the instrument, and the `wire.kel` diagnosis is what the
instrument is then used for.** If the diagnosis comes first it is a guess that happens to
be checkable; if the instrument comes first, every future failure in this stage names
itself, including ones not yet provoked.

## Prior failures this work must not repeat

- **A guard that cannot fire is worse than none.** One was written comparing
  `directory.len()` against the stage buffer; that length is the shared array's size for
  every module, so it was false by construction. **Construct the input that makes each
  guard fire.**
- **The input must be the one the real change would produce, not the one the checker
  expects.** A mutation test for a reach guard added the exact form the guard already
  matched, and confirmed nothing.
- **A mutation that fails to compile proves nothing, and looks like silence.** Check the
  mutant built before concluding anything about a guard.
- **`--no-fail-fast`, and read cargo's own exit status.** A composite command ending in a
  filter takes the filter's status and reports a green tree as red; a pipe through `tee`
  reports a red tree as green. Both were hit in one day. Print the captured status last.
- **Do not derive the set from the part of the system being thought about.** Six recorded
  instances, most of them a family counted as one member. Here the family is *every*
  indexed array in the stage, not the seven that are 1024 wide. Derive the list from the
  source and assert the derivation is non-vacuous.
- **A count of tests is not a count of shapes.**

## The specific wrong turns to avoid

1. **Do not raise a cap.** Nothing here has been shown to be too small. Raising 1024
   converts an honest refusal into a silent miscompile if the cause is an underflow.
2. **Do not assume the underflow is `pop()`** because it was the first candidate read.
   Three sites fit the evidence. The guard tells us which; reading does not.
3. **Do not repair `wire.kel`'s cause in this increment if the named message reveals
   one.** Naming and repairing are two claims with two evidence bars. A repair whose
   validation reuses the method that found it is the recorded `Op::IsStruct` overclaim.
4. **Do not add a shared slot in the middle of the block.** The output slot range mirrors
   `codegen.kel`'s `ast` block slot for slot and the host copies it across.
   **Append to a slot-addressed block, never insert.**
5. **Do not add a new opcode.** Rad-hard minimal ISA; the constraint is unconditional.
6. **Do not spell a test name in a comment before that test exists.**
   `tests/comment_citations.rs` makes a new dangling citation fail, and it has been
   tripped twice by explanations of itself.
7. **Do not report the byte-identity corpus as covering `wire.kel`** on any partial
   result. The false claim "`wire.kel` self-compiles byte-identically" was invented once
   already, written into a doc comment, a pull-request body and all three channels.

## Cost note

A single `self_host_compile(wire.kel)` reproduction takes **~95 seconds**. Budget for it;
do not iterate blind against it.
