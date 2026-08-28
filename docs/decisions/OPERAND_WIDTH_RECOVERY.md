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

## The seeding was reverted, then re-landed with evidence

**Reverted first, and that was right at the time.** It changed no corpus chunk, and nothing in the
tree could execute it: the only source-string differential lowers a single chunk and therefore
refuses `Op::Call` outright, while the whole-module differentials are file-driven. A
behaviour-widening change to a compiler with no execution-backed check is how a silent mispack ships.

**Re-landed once the missing capability existed.**
`native_codegen/tests/module_source_differential.rs` runs a multi-function program written inline
through both the native lowering and the reference implementation. The case it was built for —

```keleusma
struct P { a: Word, b: Word }
fn f(x: Word) -> Word { x * 3 }
fn main(v: Word) -> Word { let p = P { a: f(v), b: v }; p.a + p.b }
```

— was **refused for an operand of unknown packed width before the seeding**, and now lowers and
agrees with the reference across four inputs. **The test fails if the seeding is removed**, which is
what distinguishes it from a test that passes for unrelated reasons.

**It still does not lift the two sites this document is about.** Coverage remains 1070 of 1074, and
the cause there is the multi-write local, not the call result.

## What would actually lift the refusal

A **fixpoint over local widths** rather than a linear scan. The increment's width depends on the very
local being analysed, so one pass cannot settle it; a monotone dataflow analysis can, because each
local moves at most from undefined, to a concrete width, to unknown. Termination is bounded by twice
the local count.

**Not attempted here.** It is a real analysis with real mispack risk, and it deserves its own
increment with its own differential evidence.
