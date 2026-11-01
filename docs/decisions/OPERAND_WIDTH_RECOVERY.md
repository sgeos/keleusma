# The last two composite sites: the cause, and why it was not fixed here

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: cause established and **FIXED**, 2026-08-28. Both modules now execute and agree with the
reference: the corpus differential reports **61 executed and agreeing, up from 59, with exempt down
to 12**.

> **⚠ THE COVERAGE LEVEL QUOTED HERE WAS CORRECTED THE SAME DAY.** The superseded text read
> *"Corpus coverage 1070 → 1072 of 1074, opcode instances 89741 → 89854 of 89940."* The census was
> overstating by two chunks, because a module refused as a WHOLE named no chunk and so marked none —
> `float_witness.kel`'s two chunks were counted as lowerable while the backend emitted nothing for
> them. **The delta was right and the level was not**: the certification lifted exactly two chunks,
> **1068 → 1070 of 1074**. The float refusal is unrelated to this change, so the two-chunk offset
> applies equally before and after. The execution figure never depended on the census.

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

## What lifted it, and why no fixpoint was needed

The recorded plan said this required a **fixpoint** over local widths, since the increment's width
appears to depend on the local being analysed. It does not. **`push_triple` pushes the arithmetic
result at a literal `Width::Scalar(8)`, independent of its operands**, so the induction variable's two
writes — a `Const` and an arithmetic result — depend on nothing, the circularity does not exist, and a
single pre-pass suffices. **Reading the arm removed a whole increment of planned work.**

### The narrowing, and the old rule was right

A local's width was trusted only when written at most once, because a linear walk cannot see a back
edge. That is sound. **"Cannot see a back edge" only matters when the writes DISAGREE**: if every
write stores the same width, that is the width whichever write reached the read.

`certified_local_widths` therefore certifies a multiply-written local when **every** write's producer
fixes its width by the instruction alone — a `Const` by its kind, or the result slot of one of the
four arithmetic instructions that push a `(low, high, flag)` triple. Two safeguards carry the
argument:

- **One unclassifiable write sinks the local**, rather than being ignored as though the rest agreed.
- **A multi-push instruction is distinguished by which push is taken.** A triple's flag is a boolean,
  not the arithmetic result; classifying on the instruction alone would label it a word.

### The method error that nearly justified this for the wrong reason

The producer walk first used `Op::stack_growth`/`stack_shrink`. **Those are the operand-stack PEAK
model, not pop and push counts, and their own documentation says so** — `CheckedAdd` reports growth 1
and shrink 0 while actually popping two and pushing three. The wrong walk attributed the increment's
stored value to a `GetLocal` rather than to the arithmetic, which is exactly the classification this
certification rests on. Corrected to `verify::op_depth_effect`, whose contract is true counts.

**The repository had already recorded this mistake**, in the same doc comment: `text_size` made it and
"desynchronised its shadow stack on every pop-and-push instruction".

### The evidence is execution, not coverage

A wrong width would raise coverage exactly as a right one does, and mispack silently. The two modules
now **run** under the corpus differential and agree with the reference.

### The refusing path is unit-tested, because no source program reaches it

Every multiply-written local in the corpus, and in every source form tried, is a loop counter written
from a constant and an arithmetic result. The obvious negative subject — a loop variable bound from an
array element — is written **once**, so the pre-existing rule already trusted it; a test asserting it
would be refused was asserting a falsehood and failed. The refusing path is tested directly against
the predicate instead.
