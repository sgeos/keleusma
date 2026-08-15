# What can the corpus differential actually detect?

**Status**: measured, and the harness repaired in response.
**Date**: 2026-08-14.

## The question, and why the obvious instrument was wrong

`probe_stage_vacuity` found nine self-hosted stages agreeing while producing
nothing. That search looked only at the stages. The follow-up question is whether
the same thin agreement sits in the rest of the corpus, and the first instrument
reached for was a count of trivial observables: one repeated result, no host
calls, an untouched shared segment.

That instrument said **32 of 40 modules are trivial in all three**. It is wrong,
and `10_multbyte.kel` is the standing refutation. That module scores three of
three — one result, no calls, no segment — and **that single integer is what
caught the composite-return aliasing defect**, `vm 7` against `native 8`. One
result is not the same as no information.

The refined instrument asks instead whether the output **responds to its input**,
which moved the count to 20 of 40 and correctly separated the rogue scripts from
the numbered examples. But the ten numbered examples take **zero arguments**, so
even that cannot classify them, and they are reported as unknown rather than
guessed at.

## The instrument that answers the question directly

The real question is not how much a module emits. It is **whether a defect in the
emitter would change what it emits**. That is measured by mutating the emitter and
seeing which modules notice.

| mutation | sites / modules | outcome |
|---|---|---|
| `CheckedAdd` computes a subtraction | 1835 / 43 | **SIGBUS.** Detected, fatally |
| `CmpGe` lowered as `SGT` (boundary) | 499 / 22 | **SIGTRAP.** Detected, fatally |
| `CmpLt` lowered as `SGT` (inverted) | 126 / 25 | detected by **2 of 25** modules |
| `CmpLt` lowered as `SLE` (boundary) | 126 / 25 | **NOT DETECTED. The whole differential passed.** |

The opcode occurrence counts come from `probe_agreement_depth.rs`, and they matter:
a mutation to an opcode with no sites proves nothing, and checking that first is
what separates a coverage finding from a vacuous experiment.

## The hole, stated precisely

`SLT` and `SLE` differ **only when the two operands are equal**. The harness drove
every module with a single argument vector of pairwise-distinct ascending values,
so no comparison ever reached its boundary. A whole opcode could be lowered with an
off-by-one and 34 modules would agree.

**This is a coverage hole about INPUT DIVERSITY, not about the modules being
vacuous.** That distinction matters, because the fix for a vacuous module is a
different input and the fix for a thin corpus is more inputs. The first instrument
would have pointed at the wrong repair.

That two of the four mutations were caught *fatally* rather than as a reported
disagreement is also worth recording: a signal kills the whole harness and yields
no per-module census, which is why the table above has two rows with no counts.

## The repair

`corpus_differential` now drives every non-stream module with **four** argument
vectors rather than one, and compares every seed:

| seed | vector | what it reaches |
|---|---|---|
| 0 | ascending, pairwise distinct | the original, so reported figures are unchanged |
| 1 | every argument equal | comparisons between equal arguments |
| 2 | all zeros | the identity and boundary cases |
| 3 | descending | an ordering assumption that holds under seed 0 |

A stream keeps seed 0 alone: its single parameter is the tick, which the driver
already varies across sixty iterations, so seeding it would change what the run
means rather than broaden it.

A disagreement now reports **which seed** found it, because "module X disagrees" is
much less useful than "module X disagrees on the all-equal vector".

## Verified, and this is the load-bearing evidence

With `Op::CmpLt` lowered as `SLE`:

- the **old** harness reported 34 executed and agreeing, and **passed**;
- the **new** harness **fails**, on `rogue_bestiary.kel` and `rogue_gear.kel`, both
  at **seed 2**, the all-zeros vector.

`native_codegen/src/lib.rs` was restored byte-identical under `cmp` after every
mutation, and each mutation was confirmed present in the file before its result was
read.

## What this does NOT establish

- **Four vectors are not a proof of adequacy.** They close the equal-operand case
  because that case was measured to be open. Another opcode may have a boundary
  these four never reach, and nothing here rules that out.
- **No per-module detection census exists**, because two of the four mutations
  killed the harness with a signal. Obtaining one needs process isolation per
  module, which is not built.
- **The classification in `is_vacuous` still reads seed 0 only.** The reported
  figures therefore describe the same run they always did, which is deliberate, but
  it means a module vacuous at seed 0 and substantive at seed 2 is still counted as
  vacuous.

## Reproduce

```sh
cd native_codegen
cargo test --test probe_agreement_depth -- --nocapture   # depth and opcode census
cargo test --test corpus_differential -- --nocapture     # the seeded differential
```

---

## THE SWEEP, 2026-08-15: the census is no longer four samples

`tools/mutation_sweep.py` carries a **pre-registered** mutation set, committed in
`e157e271` before any of it was run. 24 opcodes get a semantic perturbation; 25
more with sites are listed as not perturbed, each with its reason.

### Two instrument defects had to be fixed before any result was trustworthy

**It hung.** The first attempt stalled twelve minutes on one module, because
`CheckedAdd → sub` stops a loop counter ever reaching its bound and the driver had
no per-invocation timeout. `HANG` is now an outcome, and it counts as detection: a
language whose value proposition is a definitive worst-case execution time does
not get to loop forever.

**It misclassified, and a known answer caught it.** The fixed driver reported
`CmpLt` as *undetected across 25*, contradicting the hand-verified result that
seed 2 catches it on two modules. The cause was `"EXEMPT" in txt`, true of every
run because the summary always prints an `EXEMPT` line, so every disagreement was
filed as `NOLOWER`. It now classifies on exit status first and reproduces the
hand-verified `CmpLt DETECTED by 2/25` exactly. **Without a known answer to
calibrate against, "CmpLt is undetectable" would have been published as a
finding.**

### Round one: 24 opcodes

Detected, with the fraction of owning modules that noticed: `CmpEq` 20/42, `If`
21/44, `Const` 27/52, `SetLocal` 18/47, `GetLocal` 15/51, `Return` 15/50,
`CheckedAdd` 15/43, `CheckedSub` 9/24, `CmpGt` 7/24, `BreakIf` 6/15, `CheckedMul`
5/20, `CmpGe` 4/22, `CmpLt` 2/25, `Not` 2/14, `CheckedNeg` 1/18, `CmpLe` 1/11,
`BitXor` 1/1.

Undetected: `BitAnd`, `BitOr`, `ByteToWord`, `CmpNe`, `Dup`, `PushImmediate`,
`Shl`, `Shr`.

### Round two: which of those are HOLES and which are EQUIVALENT MUTANTS

An undetected result has two readings and round one cannot separate them. Round
two replaces each opcode's result with a constant — the most observable change
available — so a survivor is a real hole.

- **`Dup` became DETECTED (1/10).** Round one's mutation was too weak, not the
  corpus blind.
- **`ByteToWord` is an equivalent mutant by construction.** Its arm sets an
  operand WIDTH and emits no value; the width only matters when the value is
  packed into a composite body, and these two modules never pack it.
- **`PushImmediate` was a VACUOUS MUTATION, and this is the important one.** All
  **1337** sites in the corpus carry immediate index **0**. The mutation changed
  the arm for index **1**, which has **zero** sites. It could not have been
  detected because it changed nothing. Reporting it as a coverage hole would have
  been exactly the error this whole arc is about — a vacuous experiment read as
  evidence.

### The four real holes, and they have ONE cause

| opcode | sites | owning modules |
|---|---|---|
| `BitAnd` | 54 | `wire.kel` |
| `BitOr` | 9 | `wire.kel` |
| `Shl` | 48 | `wire.kel` |
| `Shr` | 20 | `wire.kel` |
| `CmpNe` | 26 | `analyze`, `lexer`, `parse`, `piano_roll_3/4/8/9`, `wire` |

**Every module owning an undetected opcode is one this line already knows is not
really running.** `wire.kel` finishes after **0 ticks**. `analyze`, `lexer` and
`parse` are the shared-segment stages driven on zeros. The four `piano_roll`
modules are exempt on the string ABI.

**The sweep independently rediscovers the vacuity finding from the opcode side.**
A different instrument, the same root cause: 131 sites of bitwise and shift
lowering plus 26 comparisons are unobserved because the modules that contain them
do not execute.

That makes the repair concrete rather than open-ended. Driving `wire.kel` with
real input would close four of the five, and it is the same blocked seam Part B of
the previous increment named: `wire.kel` reads its input from a shared segment
whose layout belongs to `src/selfhost/mod.rs`.

### What this still does NOT establish

- **25 opcodes were not perturbed**, each with a stated reason, so the sweep is
  not exhaustive over the instruction set.
- **One perturbation per opcode.** `CmpLt` needed a boundary mutation to expose
  its hole and an inversion to characterise it; other opcodes may have boundaries
  a single perturbation misses.
- **Static sites, not dynamic execution.** The map counts where an opcode is
  emitted, not where it runs. A detected opcode may still be observed by only a
  fraction of its sites.
