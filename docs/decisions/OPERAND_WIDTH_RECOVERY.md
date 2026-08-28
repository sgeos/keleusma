# The last two composite sites: the cause, and why it was not fixed here

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: cause established 2026-08-28. **Not fixed.** Corpus coverage unchanged at 1070 of 1074.

## The condition, and why that was not yet a finding

`12_sensor_window.kel::main` op 23 and `14_frame_log.kel::main` op 24 were refused for an **unknown
operand width**. That names the check that fired, not the reason it fired. Three candidate causes
were on the record and two of them were wrong.

## The cause, derived rather than guessed

The refusal now names **which** operand is unknown: the **first of three**. Simulating the operand
stack from the instruction set's own published stack effects identifies the instruction that pushed
it:

| module | site | operand 1 produced at | by | writes to that local |
|---|---|---|---|---|
| `12_sensor_window.kel` | op 23 | op 15 | `GetLocal(1)` | **2** |
| `14_frame_log.kel` | op 24 | op 15 | `GetLocal(2)` | **2** |

In both cases the operand is the **`for` loop's induction variable**, and in both cases it is written
twice: once at initialisation and once by the loop's increment.

**A local's width is trusted only when the chunk writes it at most once.** That rule is deliberate
and sound: the width pass is a linear scan and cannot see a back edge, so a local rewritten in a loop
body would be read at the width of whichever write appears earlier in the text and packed wrongly on
every iteration after the first. The rule costs coverage and cannot mispack, which is the correct
direction for a decision that is otherwise silent.

## Two hypotheses refuted, one of them by building it

**The `Boxed` composite form.** Refuted earlier by measurement: the corpus contains zero non-`Flat`
composites.

**The `Call` result.** The instruction immediately before *both* refused sites is a `Call`, and both
callees declare `ret = Scalar`. The backend seeds *native* results from `native_return_shapes` but
never consulted `Module::signatures` for chunk calls — a real and unexplained asymmetry. **Seeding
was implemented, and the refusal did not move**: coverage stayed at 1070 of 1074. The adjacent
instruction was not the producer of the offending operand.

> **Adjacency is not provenance.** The `Call` was one instruction before the site and had nothing to
> do with it. The stack simulation was needed, and a first attempt that used "the nearest preceding
> `GetLocal`" instead picked the loop CONDITION's read and reported a write count for the wrong
> local. **The heuristic produced a confident wrong answer; the published stack effects produced the
> right one.**

## Why the seeding was reverted rather than kept

It is sound and it is an obvious asymmetry to close. It was still reverted, and the reasoning is
recorded at the instruction arm itself:

- **It changed no corpus chunk**, so nothing in the tree executes it.
- **Nothing can execute it.** The only source-string differential lowers a single chunk and therefore
  refuses `Op::Call` outright; the whole-module differentials are driven from files through per-file
  sizing helpers.
- **A behaviour-widening change to a compiler with no execution-backed check is how a silent mispack
  ships.**

**The prerequisite is a source-string whole-module differential harness.** With that in hand the
seeding is a one-line change with a test that proves it.

## What would actually lift the refusal

A **fixpoint over local widths** rather than a linear scan. The increment's width depends on the very
local being analysed, so one pass cannot settle it; a monotone dataflow analysis can, because each
local moves at most from undefined, to a concrete width, to unknown. Termination is bounded by twice
the local count.

**Not attempted here.** It is a real analysis with real mispack risk, and it deserves its own
increment with its own differential evidence.
