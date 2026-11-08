# The Region Core and Its Reuse-Soundness Theorem

> **Navigation**: [Increment B1](../region_reuse/README.md) | [Increment A](../core_bounds/README.md) | [The prose proof](../../COMPOSITE_REGION_REUSE_PROOF.md) | [Audit record](./AUDIT_2026-11-08.md)

This directory is increment B2 of the proof line, ruled by the operator on 2026-11-07 in the
order B1, then A, then B2. It extends increment A's core with composite regions, gives the
extended core a checker that decides confinement from the program text, and proves in the Rocq
Prover, version 9.3.0, that **for every module the checker accepts and every plan giving
distinct sites distinct slots, the run that reuses those slots and the baseline run produce the
same observations, for every host, every fuel and every number of stream cycles.** The plan may
reuse every site at once. Every named result is checked with no axiom.

> **What B2 is and is not, read first.** B2 was planned as discharging Part I's nine axioms
> against increment A's semantics, that is, constructing instances of B1's abstract records,
> the trace model and the flow certificate, from a concrete machine. **That instantiation is not
> done.** B2 instead proves Part I's reuse result directly for a concrete machine, by the
> comparison method as a lockstep simulation over its interpreter. It addresses the most
> important limit B1's audit found, that confinement was a property of a certificate the
> theorem's caller supplies. Here confinement is decided by a checker over the program, and the
> theorem quantifies over the program's real runs. The relation to B1 is therefore by argument,
> recorded per axiom below, and not by a checked instantiation. The full list of limits is
> under "What is not covered" below.

## How to check it

```sh
eval $(opam env --switch=rocq)
docs/proofs/rocq/region_core/check.sh
```

The script is increment A's hardened script with this directory's modules and results,
strengthened after this increment's audit. It fails unless all of the following hold.

1. No source holds an admitted or assumed proof or a command or attribute silencing warnings,
   by a scan of each source with its comments removed and its whitespace collapsed, so a
   construct split across lines or by a comment is still found.
2. `Statements.v` and `Controls.v` open no section and declare no hypothesis, variable or
   context, which could discharge a premise added to a pinned statement.
3. All six modules compile with no output.
4. `rocqchk` re-verifies every module with no axiom and no unsafe construct in its summary.
5. `Print Assumptions` reports each of the 38 named results closed.
6. `Statements.v` holds the five expected pins. They fix the statement of `reuse_sound`, the
   statement of `loop_ok_sound`, and the claims of the divergence controls `loop_bad`,
   `sep_only` and the shared-slot module, so weakening one of those claims fails to compile.
   The other controls are not pinned, and neither are the definitions the pinned claims
   mention, among them `log_of`, `P0`, `P01` and the control modules themselves.

Six planted defects were run against this copy on 2026-11-08, and the script refused each. They
were a premise added to `reuse_sound`, an `Admitted` lemma, an unlisted theorem using the
classical axiom, the audit's own attack of a premise discharged by a section hypothesis inside
the pin and control files, a divergence control weakened to a tautology, and a warning
silencer split across two lines. The delta check then found a silencer split by a comment
passing, which the comment stripping now refuses. No log is kept in the tree. Continuous integration does not
yet run the script, under the operator's ruling that a new continuous-integration job waits for
the release or for agreement with the V0.2.X line.

## The trusted base

The Rocq kernel, `rocqchk`, the standard-library modules `List`, `Arith`, `Bool` and `Lia`, and
the statements as written. The definitions of the region core, its interpreter and its checker
are part of that base, because the theorem is about them. The pins fix the statements named
above and the definitions of `injective` and `baseline`, and not the definitions the statements
mention, among them `cycles`, `run_b`, `log`, `init` and `region_check`. A control's helper
lemmas not named in the result list are covered only by the `rocqchk` summary of every
constant.

## The region core

Increment A's core gains three things, and loses the cost and arena accounting that are A's
subject.

| element | what it models |
|---|---|
| `New s` | allocation at the static site `s`, writing a fresh bump cell, or under a plan the site's slot, and pushing a handle |
| `Get` | dereference of a handle, succeeding exactly when the handle's epoch is current |
| `Dup`, `Drop` | copying the top entry and discarding it, so a handle may be read many times |
| handles `VH r e` | a reference naming its region by its allocation index, which is its provenance, and carrying its creation epoch |
| the reset | the end of a cycle, which advances the epoch and clears the operand stack |
| the log | every value yielded and every dereference outcome, which are Definition 5's observations |
| a call | a callee run on a fresh operand stack whose final stack is pushed onto its caller's |

## The checker

`Shape.region_check` interprets each block over an abstract stack whose entries record a
scalar, or a handle of a given site, and accepts a module when all of the following hold.

1. A handle is used only by `Get`, `Dup` and `Drop`, and may be carried on the stack through
   conditional arms, loop iterations, calls and the end of a cycle. Storing it to a local,
   yielding it, branching on it and computing with it are rejected, so every value is confined
   in Definition 6's sense.
2. A loop body leaves the abstract stack as it found it, and the arms of a conditional leave the
   same stack, which is M6's restoration of the entry stack.
3. `New s` is rejected whenever a handle of site `s` may still be on the stack. This is Theorem
   B1's separation hypothesis together with the confinement that makes it suffice, checked at
   the allocation itself.
4. A function may not allocate or dereference, so no handle crosses a call. Functions are
   checked against earlier functions only, so a recursive or forward call is rejected. The
   checker also requires a function's result to be all scalars, which always holds once
   allocation is forbidden, and is kept as a guard rather than as a working rule.

Rule 3 is stronger than Theorem B1's separation hypothesis, which asks only that consecutive
executions of a site be separated by the end of its scope. Rule 3 asks that no handle of the
site be live at all when it runs again, which the abstract stack can decide exactly because
handles reside only on the stack.

## The result

`Reuse.reuse_sound` is the theorem. `Sim.simulation` is its core, the comparison method as a
lockstep simulation over the interpreter. Its relation `rel` holds when the plan's state and the
baseline's agree location by location, the stack, the locals, the log, the region table and the
epoch, and differ only in where each region's bytes reside, with every handle on the stack
dereferencing to the same bytes in both. Every instruction preserves it, and both runs stop or
finish together. At `New s` the checker guarantees that no handle of `s` is live, so the plan's
write to `s`'s slot disturbs no handle that can still be dereferenced.

## Where each axiom of Part I is realised

| axiom | how the region core realises it |
|---|---|
| M1 | `New` writes a region's bytes once, at allocation, and nothing else writes a region's cell except a later allocation into the same slot. No instruction writes a region after its allocation, so a write through a handle is not modelled. Bump cells are never reused, even across cycles, and slot and bump cells are distinct constructors, so they never overlap |
| M2 | a handle carries its region's creation epoch, a dereference succeeds exactly when that epoch is current, and epochs only grow. **In an accepted module the epoch check never fires**, because every handle is on the stack and the reset clears it, so no stale handle reaches `Get`. That `reuse_sound` would hold with the check removed is inferred and not checked |
| M3 | every instruction other than `New`, `Get`, `Dup` and `Drop` takes only scalar operands by the checker's first rule, so only `Get` reads a reference, the stack carries references between instructions, and every placement that would outlive a scope, a store, a yield or a return, is rejected rather than classified. A handle reads as the number zero, so yielding, branching on or computing with a handle could cause no divergence in this semantics, and only storing one and keeping one live are real hazards here. The other rules are conservative |
| M4 | the epoch advances only at the reset, which ends every cycle |
| M5 | the reset clears the operand stack, and no handle resides anywhere else |
| M6 | the checker's second rule, a loop body restoring the abstract stack, and a call running on a fresh stack |
| M7 | the host receives only scalars, by the first rule, so it never holds a reference. Since a yielded handle would reach the host as the number zero, this rule does no work in this semantics |
| M8 | control and scalar results never depend on where a region resides, since handles name regions by index |
| M9 | region contents are numbers and cannot be read back as handles |

## The controls

`Controls.v` runs five kinds of check, and the divergence controls are pinned.
- `loop_ok` allocates a region in each iteration of a loop, dereferences it and yields what it
  read. It is accepted, its reuse run and baseline run agree, and `loop_ok_sound` applies the
  headline theorem to it for every well-formed plan. `loop_ok_one_slot` shows six regions in one
  slot and no bump cell, which follows from the definition of a cell and measures no saving
  against the baseline.
- `loop_dup` duplicates each iteration's handle and reads both copies. It is accepted and its
  two runs agree.
- `loop_bad` leaves each iteration's handle on the stack and reads both after the loop. The loop
  rule rejects it, and its reuse run really diverges, reading 2 where the baseline reads 1.
- `sep_only` allocates site 0 while a copy of an earlier handle of site 0 is live, in straight
  line code, and only the separation rule rejects it, at that allocation. Its reuse run
  diverges, reading 2 where the baseline reads 1, so the separation rule guards a real hazard.
  Consuming both copies first is accepted.
- `two_sites` keeps handles of two distinct sites live together. It is accepted, and the plan
  that puts both sites in one slot diverges, so the premise that a plan gives distinct sites
  distinct slots does work.

Eight modules are rejected, each breaking one rule. They are a yielded handle, a stored handle,
a branch on a handle, arithmetic on a handle, a dereference of a scalar, an allocation in a
function, conditional arms that leave different stacks, and an allocation while a handle of its
site is live. The audit confirmed by probe that recursive and forward calls and a dereference in
a function are rejected, for which there is no separate control.

## What is not covered

1. **The formal instantiation of B1.** As stated at the top. Nothing here constructs B1's trace
   model or flow certificate, so B1's theorems are not applied to this machine, and the relation
   is the per-axiom argument above.
2. **Keleusma itself.** The region core is a model. Its handles name regions by index, while the
   shipping virtual machine's handles carry offsets, so address opacity, M8, holds here by
   construction. Nothing relates this core to the shipping virtual machine or verifier.
3. **Arm groups, Theorem A2 and Corollary A3.** A plan must give distinct sites distinct slots,
   so sharing one slot between the arms of a conditional is excluded.
4. **Refined confinement, Theorem B1r.** A handle may not be stored to a local at all, so the
   boundary-dead slots of Definition 7 are not modelled.
5. **The escape-copy discipline, Theorem B2 of Part I.** A handle may not escape at all, so no
   copy is ever made.
6. **Sites inside called functions.** Functions may not allocate, which excludes the case of a
   site executed twice in one scope through two calls.
7. **Memory accounting, Theorem C.** The interpreter does not count memory, so the saving of
   reuse is shown by `Controls.loop_ok_one_slot` on one module and not proved in general.
8. **Composite structure.** A region holds one number, with no fields, views or nesting, and
   is never written after its allocation.
9. **Termination.** `reuse_sound` says the two runs stop or finish together. No theorem shows
   that an accepted module finishes, although its controls do at the fuel they use. Increment A
   proves termination for its own core.
10. **Underflow.** Popping an empty stack yields the scalar zero, in the semantics and in the
    checker alike, where a real verifier would reject the program.
11. **Further results of Part I.** Theorem A1 and Corollary A1s, the branch bounds, are
    increment A's subject in a different core. Lemma 4's composition of elements is subsumed,
    since one simulation covers every reused site at once. Lemmas 1 and 2 are folded into the
    simulation rather than stated. Lemma 3 and Theorem B1r, Lemma 5, Theorem B2 and Corollaries
    B2a and B2b of Part I are not covered, as items 4 and 5 say.
