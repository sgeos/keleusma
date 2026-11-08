# A Rocq Core of Keleusma With Its Bound Theorem

> **Navigation**: [Increment B1](../region_reuse/README.md) | [Structural ISA](../../../spec/STRUCTURAL_ISA.md) | [Audit record](./AUDIT_2026-11-08.md)

This directory is increment A of the proof line, ruled by the operator on 2026-11-07 in the
order B1, then A, then B2. It defines a small core of Keleusma's structural instruction set in
the Rocq Prover, version 9.3.0, gives it an executable semantics, gives it an executable bound
checker, and proves that **every module the checker accepts terminates, keeps every interval
between consecutive yields within the checker's time figure, and keeps every stream cycle's
arena within the checker's memory figure, for every host behaviour.** Every named result is
checked with no axiom.

> **What A does not establish, read first.** The core and its checker are models written for
> this proof. Neither is Keleusma's Rust verifier, and nothing here relates the two. The
> operator's ruling of 2026-11-07 links them by differential testing of the Rust verifier
> against this model on generated modules, which is future work and not part of this
> increment. Until then, the theorem says that **this checker** is sound for **this
> semantics**, and it says nothing about the shipping verifier. The full list of limits is
> under "What is not modelled" below.

## How to check it

```sh
eval $(opam env --switch=rocq)
docs/proofs/rocq/core_bounds/check.sh
```

The script is B1's hardened script with this directory's modules and results, strengthened after
this increment's audit. It fails unless all of the following hold.

1. No source contains an admitted or assumed proof, a command unsetting guard, positivity or
   universe checking, a `Set Warnings` command, a `warning` or `warnings` attribute, or a
   `Redirect`, any of which could silence
   the next step, scanned with comments removed and whitespace collapsed, and `Statements.v` and `Controls.v`
   open no section and declare no hypothesis, variable or context. The last two checks were
   added after increment B2's audit.
2. All six modules compile with no output at all.
3. `rocqchk` re-verifies every module, and its summary reports no axiom, no unsafe fixpoint, no
   type-in-type and no assumed positivity for any constant.
4. `Print Assumptions` reports each of the 32 named results closed, which catches a named
   result that is not closed. It does not catch a name removed from the list, which step 3
   covers for axioms regardless.
5. `Statements.v` pins the exact type of each of the four headline theorems, and proves the
   definitions of `good`, `wcet` and `wcmu` by equations, so a statement weakened by an added
   premise or a changed conclusion fails to compile. A change to another definition a statement
   mentions does not, as the trusted base below records. The script requires exactly four type pins.

Five planted defects were run against the script on 2026-11-08, and it refused every one. They
were an `Admitted` lemma, an unlisted theorem using the classical axiom, an unguarded fixpoint
under a guard unset split by a comment, a premise added to `stream_bounded` as the audit's own
attack, which its pin refused, and a `Set Warnings` command. No log is kept in the tree.
Continuous integration does not yet run the script, under the operator's ruling that a new
continuous-integration job waits for the release or for agreement with the V0.2.X line.

## The trusted base

The Rocq kernel, `rocqchk`, the standard-library modules `List`, `Arith` and `Lia`, and the
statements as written. The definitions of the core, its semantics and its checker are part of
that base, because the theorem is about them. The pins of `Statements.v` fix the four headline
statements and the definitions of `good`, `wcet` and `wcmu`, but not the definitions those
statements mention, among them `run_b`, `cycle`, `cycles`, `reset`, `stream_segs`, `summary` and
`le_summ`. The delta check showed by execution that a change to `stream_segs` passes the pins.
Those definitions are therefore checked only by reading them and by the concrete values in
`Controls.v`.

## The core

| element | what it models |
|---|---|
| `Const`, `Load`, `Store`, `Add`, `Sub`, `Lt` | the computation core, over natural numbers |
| `If t e` | block-structured choice, taking `t` on a nonzero condition |
| `LoopN n body` | a loop whose iteration count is the constant `n`, the only backward edge |
| `Call f` | a statically resolved call of function `f` |
| `Alloc k` | taking `k` words of the arena, the quantity the memory bound counts |
| `Yield` | sending the top of the stack to the host and pushing its reply |
| a function of kind `KFn` | an atomic total `fn` |
| a function of kind `KYield` | a non-atomic total `yield` function |
| the module's main block | the productive divergent `loop main`, run once per stream cycle with the arena and operand stack reset |

**Time is measured by the semantics.** The interpreter returns the costs of the segments between
consecutive yields. Every core instruction costs one unit, each loop iteration costs one more for
its back edge, and each reset costs one. A cycle's last segment continues through the reset into
the next cycle's first segment, so the stream's segments are the intervals between consecutive
yields under that cost model. `Algebra.summary_join` proves that summarising a joined run is
sequencing the summaries, so the checker's algebra composes as the segments do. A summary keeps
the first segment, the largest middle segment and the last, and `Algebra.summary_elements`
recovers from it a bound on every segment.

**The host is any function from the program's outputs so far to its next reply.** Every theorem
quantifies over every such function. The outputs grow at every yield, so such a function can
produce any sequence of replies, which covers every environment behaviour on a single run.

## The checker

`Check.check` accepts a module when all of the following hold.

1. Each function's bound is computed only from the bounds of earlier functions, so a recursive
   or forward call is rejected.
2. Each `fn` function's bound admits no yielding run.
3. The main block's bound admits no run that does not yield, which is the productivity rule
   that every path from the stream entry to the reset yields.

On acceptance it publishes `wcet`, which bounds every interval between consecutive yields of
every stream, the junction through each reset included, which costs the last segment of one
cycle, the reset, and the first segment of the next, and `wcmu`, which bounds each cycle's
arena. It also computes the fuel the interpreter needs, and `Soundness.fuel_suffices` proves the
fuel enough, which is the totality of every accepted function and of every cycle.

## The results

| result | statement |
|---|---|
| `Soundness.function_bounded` | every function of an accepted module completes on the checker's fuel, within its bound |
| `Soundness.fn_never_yields` | every run of an accepted `fn` function is one segment, so it never yields |
| `Soundness.cycle_bounded` | every cycle completes, yields at least once, keeps every segment within `wcet`, and ends with at most `wcmu` arena words |
| `Soundness.stream_bounded` | every stream of any number of cycles completes, keeps every segment, junctions included, within `wcet`, and keeps every cycle's arena within `wcmu` |
| `Soundness.runs_within_bounds` | the induction behind them, every completed run within its block's bound |
| `Soundness.fuel_suffices` | the induction behind totality |
| `Algebra.summary_join` | summarising a joined run is sequencing the summaries |
| `Statements.v` | the four headline statements pinned by type, and the definitions of `good`, `wcet` and `wcmu` pinned by proved equations |

## The controls

`Controls.v` defines a module with one function of each category. The checker accepts it, with
a time figure of 14 and a memory figure of 7 words. Two concrete hosts drive it for two cycles,
one reaching 7 arena words per cycle and the other 3. `Controls.ex_bounds_attained` proves of
the run itself that **both figures are attained**, the time figure at the junction through the
first reset, a last segment of 0, the reset's 1 and the next first segment of 13.
`Controls.junction_alone_attains` gives a second module whose within-cycle segments are at most
2 and whose stream reaches its time figure of 4 only at the junction, so the junction term of
`wcet` is exercised. `Controls.loop_iterations_counted` runs an empty loop of twenty
iterations followed by a yield and measures 22 units, the loop instruction, twenty back edges
and the yield, and its time figure is 23, which adds the reset at the junction. `Controls.ex_stream_bounded` applies the headline theorem to the
first module for every host and every number of cycles.

The checker's three rules reject seven modules, two for the call rule, a recursive call and a
forward call, two for the `fn` rule, a yielding `fn` and an `fn` calling a yielding function,
and three for the productivity rule, a main block with a path that does not yield, a main block
that never yields, and a yield inside a loop of zero iterations. A loop of one iteration around
a yield is accepted, which shows the zero-count rejection depends on the count. The adversarial
audit also found the near misses accepted, a `yield` function with no yield, an `fn` with no
yield, a call to an earlier function, and an `If` whose arms both yield.

## What is not modelled

1. **The link to the Rust verifier.** As stated at the top, the differential testing that the
   operator's ruling names is not done here.
2. **Costs.** Every core instruction, loop iteration and reset costs one unit, and the block
   delimiters `EndIf` and `Return` and the stream entry are not charged separately. The
   shipping analysis uses calibrated per-instruction costs from `keleusma-bench`. A cost table
   would be a parameter of the semantics and the checker, and is not added.
3. **Values and types.** Values are natural numbers and an empty stack reads as zero, so the
   semantics never faults and the typed stack discipline that the shipping verifier checks is
   not modelled. The bound theorems do not depend on values beyond the branch each `If` takes,
   and every branch is bounded.
4. **The operand stack and locals in the memory bound.** Only arena words taken by `Alloc` are
   bounded. The arena only grows within a cycle, so the end-of-cycle figure bounds every moment
   of the cycle, but a statement about intermediate moments is not proved separately. The reset
   clears the operand stack, standing for the stack balance the shipping verifier enforces, so
   the stack does not grow across cycles. Its depth within a cycle is finite, since every cycle
   completes, but no theorem bounds it. Callers and callees share one set of locals, which the
   bounds do not depend on but which is unlike real call frames.
5. **Composite values, regions, handles, epochs and slot reuse.** None of B1's machinery is in
   this core. Increment B2 must extend the core with them before it can discharge B1's
   obligations, and it must build B1's flow certificate from this semantics rather than
   supply one.
6. **Language features beyond the core.** `Break` and `BreakIf`, multiheaded functions,
   arguments passed other than on the shared operand stack, natives, strings, floats, generics,
   and hot swap are not modelled.
7. **The `fn` rule is semantic.** The checker rejects an `fn` whose bound admits a yielding run.
   So an `fn` may call a `yield` function that never yields, and an `fn` may contain a `Yield`
   that can never run, such as one inside a loop of zero iterations. The structural instruction
   set forbids the instruction itself in a FUNC block, which is stricter.
8. **The `yield` category is not required to yield.** `docs/spec/STRUCTURAL_ISA.md` requires
   every REENTRANT block to contain a YIELD, while `docs/spec/GRAMMAR.md` allows a `yield`
   function that does not yield on a path and says some heads need not contain one. The two
   specifications disagree, the model follows the grammar, and the disagreement is recorded here
   rather than resolved.
9. **The checker's own cost.** The checker iterates a loop's bound once per iteration, computing
   each step once. The audit measured the first version exponential in the count, which is
   corrected. Measured on 2026-11-08, the corrected checker grows linearly with the count for a
   body that is a yield, an allocation or empty, and quadratically for a body containing a
   conditional, the latter at 97 seconds for 20,000 iterations. The quadratic growth is thought,
   not verified, to come from unary natural-number arithmetic on figures that grow with the
   count. A production checker would use closed forms and binary numbers.
