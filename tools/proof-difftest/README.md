# Differential Test of the Rust Verifier Against the Rocq Core

> **Navigation**: [Tools](../README.md) | [Increment A](../../docs/proofs/rocq/core_bounds/README.md) | [Structural ISA](../../docs/spec/STRUCTURAL_ISA.md)

This package links increment A of the proof line to the shipping Rust verifier, as the
operator's ruling of 2026-11-07 asks. That ruling reads "an executable model of the semantics
and the bound checker in Rocq, soundness proved about the model, and the Rust verifier run
against the extracted model on generated modules." Increment A supplies the model and the
proofs. This package supplies the extraction, the generator, the translation to Keleusma
source, and the comparison.

> **What this establishes, and what it does not, read first.** It establishes that on the
> generated modules the two sides agree on every relation stated below, that every
> difference in acceptance falls under a rule stated below, and that each relation reports a
> deliberately planted defect. It does not prove anything about the shipping verifier. A
> differential test samples, and its generator reaches only the fragment described under
> "Coverage". The bound relations compare costs under a unit cost model, not the calibrated
> cycle table of `keleusma-bench`. Memory figures are not compared.

## How to reproduce

```sh
eval $(opam env --switch=rocq)
tools/proof-difftest/check.sh
```

The script extracts the model, builds it, builds the harness, runs 20,000 cases from seed
20261108, and runs the three negative controls. It exits nonzero on any unexplained
disagreement, on any stage that collapses below its floor, and on any negative control that
is not reported. On 2026-11-08 the differential run took 9 seconds and the whole script took
14 seconds once the keleusma crate was compiled. The environment variables `SEED`, `CASES`,
`MIN_BOTH` and `CONTROL_CASES` change the run. Output lands under `target/check/`, with every
explained difference in `run/explained.txt` and every unexplained one in
`run/disagreements.txt`.

A single Keleusma source can be judged and measured with the reproduction mode.

```sh
tools/proof-difftest/target/release/keleusma-proof-difftest --source FILE.kel
```

## The parts

| path | role |
|---|---|
| `extract/Extract.v` | extracts `run_b`, `cycle`, `cycles`, `stream_segs`, `check`, `wcet` and `wcmu` unchanged |
| `model/model_main.ml` | parses cases, calls the extracted code, prints verdicts, figures and runs |
| `src/core.rs` | the generator, the lowering to core instructions, the translation to Keleusma, the structural predicates |
| `src/rust_side.rs` | compile, load through `Vm::new`, the unit-cost figure, measured runs |
| `src/main.rs` | the relations, the counts and the floors |
| `check.sh` | the whole procedure |

**The extraction lives here and not beside increment A.** The extracted OCaml and the driver
are tooling for a test, not part of the proof, and `tools/` is excluded from the published
crate. Keeping `Extract.v` out of `docs/proofs/rocq/core_bounds/` also leaves that directory's
module list and its forbidden-construct scan unchanged. The script copies `Core.v` and
`Check.v` from increment A at run time, so the model tested is always the model proved.

**The extraction adds no realisation.** Natural numbers stay the unary inductive type, so the
extracted functions compute exactly what the Rocq definitions compute. The trusted base of
this test is therefore increment A's base, the Rocq extraction mechanism, the OCaml compiler,
and the driver's conversion between machine integers and unary numbers. The driver's other
work is parsing and printing. Its one piece of composition, running cycles one at a time to
read the outputs, is checked on every run against the extracted `cycles`, and a mismatch fails
relation R5.

## The translation

A module is generated as structured statements, each with one lowering to the core and one
translation to Keleusma, so that both programs take the same branches and yield the same
outputs on the same replies. The table is repeated from `src/core.rs`. In it `v` is the main
parameter `x` inside main, which is core local 0, and the function parameter `a` inside a
function, which is core local 1.

| statement | core | Keleusma |
|---|---|---|
| `Work(op, k)` | `Load v; Const k; op; Store 2` | `let w = v op k;` |
| `Emit(c)` | `Const c; Yield; Store 0` | `yield c;` |
| `YIf(c, t, e)` | `Const c; Yield; Store 0; Load 0; If t e` | `if (yield c) != 0 { t } else { e };` |
| `LIf(t, e)` | `Load v; If t e` | `if v != 0 { t } else { e };` |
| `For(n, b)` | `LoopN n b` | `for iD in 0..n { b };` |
| `Call(j)` in main | `Load 0; Store 1; Call j` | `fj(x, x);` |
| `Call(j)` in a function | `Call j` | `fj(a, a);` |

A function is `fn fj(a: Word, b: Word) -> Word { ... a }` or the same with `yield`, and main
is `loop main(x: Word) -> Word { ... x }`. The alignment rests on three facts. The runtime
writes every resume value to the parameter of `loop main`, and the core stores every reply to
local 0. Main stores `x` to local 1 before a call, and a function passes its own `a` on. Each
yield sends a constant unique to its site. The harness answers yield number `k` with the
`k`th character of a reply string, as the model's host does, and resumes after each reset
with the last reply, which is what the model's locals hold.

The unused parameter `b` is a correction found by this test. With one argument, a call from
main dispatched two operations before entering its callee against the core's three, and the
first run reported that as a lockstep disagreement. It was a translation artefact, and the
second argument removes it.

## The relations

The two sides use different instruction sets and different cost measures. The model charges
one unit per core instruction, per loop iteration and per reset, and its `wcet` bounds every
interval between consecutive yields, the junction through a reset included, with callee
bodies counted. The shipping figure, `wcet_stream_iteration_with_cost_model`, bounds one whole
iteration from `Stream` to `Reset` of the Stream chunk, and a call contributes its dispatch
only. The harness gives it a unit cost model, one cycle per operation, so that its figure and
a measured run count the same thing. The relations below are chosen to hold under those
differences, and each is a check that fails the run.

| relation | statement | justification |
|---|---|---|
| R1 acceptance | the verdicts agree, or differ only under a rule named below | the stricter rules are documented on each side |
| R1b shipping rules | the shipping verdict is what its documented rules predict, and every rejection carries the diagnostic of a rule predicted to fire | the predicates restate `docs/spec/GRAMMAR.md` and `verify.rs` independently of both verdicts |
| R2 boundedness | a module both sides accept has a finite shipping figure | the model is the oracle for which shapes are bounded |
| R3 shipping bound | every measured iteration dispatches no more operations in the Stream chunk than the shipping figure | this is the shipping figure's own documented contract |
| R4 lockstep | the outputs agree, the yields per cycle agree, and every model segment is at most the measured segment at the same position | the translation preserves control flow and never costs fewer operations than core instructions |
| R5 model bound | every model run completes on the checker's fuel, matches the extracted `cycles`, and stays within `wcet` and `wcmu` | the theorems of increment A, here checking the extraction |
| R6 figure relation | for a main block without calls, the model's first, middle and last segment bounds are at most the shipping figure, the last strictly, and its `wcet` at most twice the figure | each model path maps to a translated path at least as long, the shipping figure dominates every path through one iteration, the tail includes the reset, and the junction spans two iterations |

A difference in acceptance is explained only in these directions.

1. **The model rejects and the shipping pipeline accepts.** This is explained only when the
   call graph is acyclic, some call is forward, and the model accepts the same module with its
   functions reordered so that every call is backward. The model's call rule admits only
   calls to earlier functions, while Keleusma resolves names in any order. A model rejection
   for recursion, for a yielding `fn` or for an unproductive main is never explained.
2. **The model accepts and the shipping pipeline rejects.** This is explained only when one
   of four documented stricter rules fires and the shipping diagnostic is that rule's.
   - The category-call discipline refuses an `fn` that calls a `yield` function, which the
     model admits when that function cannot yield.
   - The structural pass refuses a yield in an `fn` syntactically, which the model admits when
     the yield sits in a loop of zero iterations.
   - The structural pass requires a `yield` function to contain a yield or a call to an
     always-yielding function, which the model and `GRAMMAR.md` do not require, as item 8 of
     increment A's limits records.
   - The productivity pass treats a loop body as skippable, because the bounded loop tests its
     exit first, so a yield inside a loop of one or more iterations does not count. The model
     counts it.

Three counts are reported and not checked, because the documented shipping contract does not
promise them. They are the iterations whose operations counted across all chunks exceed the
shipping figure, the modules with calls in main whose model figures exceed the shipping
figure, and the tightness of the shipping figure against the best of four runs.

## Coverage

The generator draws zero to three functions, each `fn` or `yield` with equal chance, and a
main block of one to four statements, with nesting up to depth three and loop counts from zero
to three. Per module it draws whether calls may target any function, which produces recursion
and forward calls, whether an `fn` may yield, and how often a loop has zero iterations. It
redraws a module whose expanded cycle exceeds 2,500 instructions or 200 yields. Each case runs
three stream cycles under four hosts, replying always zero, always one, alternately, and at
random.

It never produces `Alloc`, so memory is not compared, nor an unbalanced stack, `Break`, a
native call, a multiheaded function, a composite value or text. A loop count is a literal, so
the `limit` form and runtime ranges are not reached. The call argument is the only data a
function receives.

## Results

Seed 20261108, 20,000 cases, 2026-11-08, on the tree of this commit.

| stage | cases |
|---|---|
| generated | 20,000 |
| translated | 20,000 |
| checked by the model | 20,000 |
| compiled by the shipping compiler | 9,204 |
| accepted by `Vm::new` | 8,843 |
| compared on figures and runs | 8,843 |
| runs measured, four per compared case | 35,372 |

| acceptance category | cases |
|---|---|
| both accept | 8,782 |
| both reject | 6,772 |
| model only rejects, index-order call rule | 61 |
| shipping only rejects, `yield` function without a yield | 3,334 |
| shipping only rejects, productivity through a loop | 740 |
| shipping only rejects, `fn` calling a `yield` function | 295 |
| shipping only rejects, `fn` with an unreachable yield | 16 |
| unexplained | 0 |

| relation | checks | unexplained |
|---|---|---|
| R1 acceptance | 20,000 | 0 |
| R1b shipping rules | 31,157 | 0 |
| R2 boundedness | 8,843 | 0 |
| R3 shipping bound | 106,116 | 0 |
| R4 lockstep | 106,116 | 0 |
| R5 model bound | 70,744 | 0 |
| R6 figure relation | 5,495 | 0 |

Among the cases both accept, 1,553 have main call a `yield` function, 201 have main yield
only through such calls, 3,205 have a yield inside a loop, 1,726 have a loop of zero
iterations, and 5,207 reach nesting depth three. The lockstep relation compared 373,626
segment pairs. Some run attained the shipping figure exactly in 5,591 compared cases, the best
run reached at least half of it in 3,183 more, and in 69 every run stayed below half.

Reported and not checked, 23,253 of 106,116 iterations exceeded the shipping figure when the
operations of callees are counted, and in 503 of 3,348 compared cases with calls in main the
model's figures exceeded the shipping figure. Both follow from the shallow call cost of
finding 2 below.

## The negative controls

Each control plants one defect and must be reported under the relation it targets. The
script runs each on 2,000 cases from the same seed and fails if any goes unreported.

| control | defect | reported on 2026-11-08 |
|---|---|---|
| `accept-recursion` | the model driver replaces every call in function `i` to a function of index `i` or greater by a constant, a checker that ignores the calls it cannot resolve and so accepts recursion | 35 under R1 |
| `rust-cost` | the shipping figure is computed with `Loop`, `EndLoop` and `BreakIf` costing nothing, while runs count every operation | 1,957 under R3 |
| `swap-branches` | the translation exchanges the arms of every conditional in the Keleusma program only | 5,692 under R4 |

## Findings for the V0.2.X line

No fix is made here. Each finding has a minimal reproduction for the reproduction mode.

**Finding 1, a candidate defect. The compiler accepts recursion.** `docs/spec/GRAMMAR.md`
under "Recursion Prohibition" states that the compiler rejects any cycle in the call graph,
and `docs/architecture/LANGUAGE_DESIGN.md` states that the compiler rejects it with a
diagnostic naming the cycle. The doc comment of `module_call_depth` in `src/verify.rs` states
that the type checker rejects direct and mutual recursion. The run generated 939 modules with a
cycle in the call graph. The compiler rejected 578 of them under another rule. It accepted
every one of the other 361, ran `verify` on each successfully, and wrote a finite worst-case
execution time header. Only `Vm::new` rejected them, through the memory analysis's
topological sort. A host
that loads with `Vm::new` is protected. A host that trusts the compile step, such as one that
compiles ahead of time and loads with `Vm::new_unchecked` as precompiled bytecode, receives a
nonterminating module with a declared bound.

```text
fn f0(a: Word, b: Word) -> Word { f0(a, b) }
loop main(x: Word) -> Word { yield 1; f0(x, x) }
```

The reproduction mode reports the stage `Load` with the diagnostic
`f0: recursive call detected during WCMU topological sort`, a declared header of 18 cycles,
and a unit-cost figure of 9.

**Finding 2, a documented difference that deserves attention. The worst-case execution time
counts no callee body.** The doc comments of `wcet_stream_iteration` and
`wcet_whole_chunk` and the resolved backlog entry B29 on debug metadata state that a call contributes its dispatch only
and that there is no transitive resolver. The difference is documented, so it is classified
as deliberate, but its size is worth the V0.2.X line's review against the stated value of a
definitive bound.

```text
fn f0(a: Word, b: Word) -> Word { for i in 0..50 { let w = a + 1; }; a }
loop main(x: Word) -> Word { yield 1; f0(x, x); x }
```

The reproduction mode reports a declared header of 20 cycles and a unit-cost figure of 11,
against measured iterations of 11, 10 and 10 operations in the Stream chunk and 872, 871 and
871 operations in all chunks.

**Deliberate differences.** The four stricter shipping rules listed under the relations are
documented and are not defects. The requirement that a `yield` function contain a yield
follows `docs/spec/STRUCTURAL_ISA.md` and departs from `docs/spec/GRAMMAR.md`, a conflict
between the specifications that increment A's README already records. The shipping figure
charges the `Stream` operation on every iteration, while the first measured iteration is in
the observed runs one operation longer than the later ones. This suggests, without
verification, that `Stream` is dispatched only on entry, which makes the figure conservative
by one and is not a defect.

## Limits

1. **Sampling, not proof.** Zero unexplained disagreements on the sampled fragment does not
   show agreement outside it, and the coverage above bounds what was reached.
2. **Unit costs only.** The bound relations use one cycle per operation. They do not test the
   nominal or a calibrated cost table, and a defect that appears only under unequal costs is
   not reached.
3. **No memory comparison.** The generator produces no `Alloc`, and the shipping memory figure
   counts slots and bytes, which the model does not.
4. **The figure relation excludes calls in main.** With calls, the shallow shipping figure is
   not an upper bound on the model's figures, so R6 is checked only for call-free mains.
5. **The tightness counts are not checks.** Four hosts need not drive the worst path, so a
   figure that no run attains is not by itself a disagreement.
6. **The predicates are a third reading of the rules.** R1b compares the shipping verdict with
   the harness's own restatement of the documented rules. An error shared by the restatement
   and the shipping code would not be reported.
7. **The measurement trusts the breakpoint mechanism.** Every operation is armed as a
   breakpoint and counted at its stop. The lockstep relation would show a gross miscount, and a
   miscount by a constant per segment would not be seen.
8. **Continuous integration does not run this script.** It follows increment A's script in
   waiting for the operator's ruling on new continuous-integration jobs.
