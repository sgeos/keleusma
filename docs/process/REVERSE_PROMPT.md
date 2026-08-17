# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-16 (session 47)

## Where things stand

| | |
|---|---|
| Operand-stack models | **agree on every opcode**; the known-disagreement list is empty |
| `Op::Yield` | was **understating** the bound; repaired, bounds rise |
| `FixedMul` / `FixedDiv` | were overstating; repaired, bounds fall |
| `--all-features` | **has never passed**; `CLAUDE.md` said it did |
| `CONSTS` blocker | neither recorded obstacle; it is a **170-node capacity bound** |
| Emit coverage | four region kinds of twenty, ten of eleven stages |

## THE YIELD ENTRY WAS UNSOUND, AND THE READING PUT TO ME COVERED HALF OF IT

The reading was that the yielded value lives in the caller's memory and therefore does not affect
the worst-case-memory bound. That is **correct about the yielded value**, and the model already
treated it that way. What it does not cover is the **resumed** value: `resume_after_enter` pushes
the reply back onto the same operand stack, so the depth on the far side of the boundary is the
depth on the near side. The pop was modelled and the push was not.

Measured end to end rather than argued. Two sources carrying the identical peak expression,
differing only in whether three yields precede it: **192 bytes against 288**, a shortfall of exactly
one value slot per yield. The running offset reached **-4** on a three-yield body, first going
negative at the `SetLocal` binding the first resumed value. An operand stack cannot hold a negative
number of entries.

The invariant is now a test in its own right — the running offset never goes negative — rather than
a pair of numbers, because the numbers version only catches shapes a case list happens to name.

## `--all-features` HAS NEVER BEEN GREEN AND THE INSTRUCTION FILE SAID IT WAS

Found by running it as a gate. It cascades the mutually exclusive `narrow-word-*` selectors into the
narrowest word, under which the test pinning 64-bit checked-addition semantics fails, and it pulls
in `sdl3-example`. **The CI workflow already documents exactly this** in a comment on its
broad-features job.

`CLAUDE.md` claimed the opposite and pointed the everyday verification command at the same
unsupported set. Corrected to the three sets CI actually runs: default, `signatures,shell`, and
`self-host`.

Eighth stale-figure incident on this line, and the first one in the file that governs how the work
is done. Seven of the eight were in documents no test reads.

## CONSTS: THE INVESTIGATION ANSWERED A DIFFERENT QUESTION THAN THE ONE ASKED

Option B was authorised — adopt the self-hosted interning order and re-sequence the reference
flattener — subject to a discovery-order investigation first. **The investigation says there is
nothing to re-sequence.** The flattener interns only for `StaticStr`, `Struct` and `Enum` nodes, and
all **40,332 constants across the eleven stages are `Int`**. The ordering conflict is a true
statement about the general case with zero instances here.

**What actually blocks it is capacity.** The flattener already runs from real modules and already
emits a byte-identical region. `wire.fin` is 1,024 **words** at six words a node, so the walk takes
**170 nodes** against `parse`'s 17,391. My own mailbox table stated the word count as though it were
a node count, which made a hundredfold margin look marginal.

**Widening the array diverges rather than converging.** A stage's private data array is initialised
one `Int(0)` per word, so a `fin` wide enough for N nodes adds `6N` records to the walking stage's
own `CONSTS`. Holding `parse`'s forest costs 1,669,536 bytes to emit 278,256 — six times over. The
stage's capacity to describe a data segment is paid for out of a data segment described the same
way. Batching is the route, and this corpus is its easy case: scalars, no interning, no children, so
no state crosses a batch.

## A MEASUREMENT THAT COULD NOT DISCRIMINATE, CAUGHT BEFORE IT WAS RECORDED

The first probe walked `Chunk::constants` only and reported zero name-bearing nodes. Right answer,
wrong evidence — chunk pools are 2,245 of the 40,332. The second compared string pools with and
without every constant, saw a 5,264-byte difference, and **nearly recorded the opposite
conclusion**; clearing `private_init` also removes the slot names `add_data_layout` interns
directly. Only the third form separates them.

Second occurrence of this exact failure, after `analyze.kel`. Five tests now pin the figures.

## A SECOND GAP IN THE EMIT PATH, AND MY FIRST FIX FOR IT BROKE TWO TESTS

The tested node model omitted `DataLayout::private_init`, which is 38,087 of the 40,332 constants,
so the byte-identical path covered 6% of the region. Every `FLATTEN_CASES` source used `const data`,
which folds into chunk constants; only `private data` reaches the other pool. Three cases added, and
the must-fire check confirms them: without the second source, `data-scalar` reports one node against
the reference's two.

Folding both sources into the one shared helper took `parse`'s blob from about 8 KB to **530,675
bytes**, past `bin`, and broke two join tests. The blob model and the encoder model are different
things; they are now different functions.

## Held for the operator

- **The zero-record representation.** Every one of the 38,087 data-segment initialisers is `Int(0)`,
  at a 16-byte record each — roughly **85% of the corpus auxiliary body spent encoding zeros**. It is
  also what makes `CONSTS` too large to window. Collapsing it is a wire-format change that moves
  every artifact the differential compares against.
- **`Op::cost()` recalibration**, 17 of 66 opcodes ever measured.
- **Derived operands in type rejection**; the extraction is still host-side.
- **Publication**, held.

## What these green suites do NOT establish

The `CONSTS` figures are properties of **this corpus**. A stage that gained a string or struct
constant would make the interning-order question real, which is why it is a test and not a note.
Nothing here emits a single `CONSTS` byte for a stage module; the byte identity covers synthetic
sources under the 170-node cap.

## Next intended increment

Batched `CONSTS` emit: a window command that formats a seeded batch of scalar nodes at offset zero,
with the host placing batches and a refusal for any forest containing a composite or a name-bearing
node. That refusal is load-bearing — without it the path would silently emit a wrong region for the
general case it cannot handle.
