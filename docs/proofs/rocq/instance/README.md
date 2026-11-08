# The Region Core as an Instance of Increment B1

> **Navigation**: [Increment B1](../region_reuse/README.md) | [Increment B2](../region_core/README.md) | [The prose proof](../../COMPOSITE_REGION_REUSE_PROOF.md)

This directory connects the concrete region core of increment B2 to the abstract theorems of
increment B1 by proofs checked in the Rocq Prover, version 9.3.0. It derives a machine in B1's
sense from the core, shows that B1's owner memory model computes the core's own observations,
discharges B1's per-slot obligation from B2's checker, and then obtains **B1's Definition 5
soundness for the core from B1's composition theorem `Trace.lemma4_composition`**. B2's
headline statement `reuse_sound` is concluded again from that result, with the identical
statement, without B2's lockstep simulation. Every named result is checked with no axiom.

> **What is instantiated and what is not, read first.** The trace level of B1 is instantiated.
> That is B1's machine and memory model in `Trace.v`, its soundness notion `sound`, its
> per-slot obligation `slot_safe`, and its composition theorem `lemma4_composition`, which
> carries the comparison method of Part I. **The flow level of B1 is not instantiated.** No
> flow certificate of `Clearance.v` is built from the core, so Lemmas 1, 2, 3 and 5,
> `element_safe`, `theorem_B1` and the other theorems of `Theorems.v` are not applied to the
> core. The per-slot obligation is instead discharged by this directory's own invariant over
> B2's checker, which plays the part that Lemma 2 and B1's slot theorem play in the prose. The
> list of what remains is under "What is not instantiated" below.

## How to check it

```sh
eval $(opam env --switch=rocq)
docs/proofs/rocq/instance/check.sh
```

The script is increment B2's script adapted to compile the dependencies too. It fails unless
all of the following hold.

1. No source of this directory, and no source of B1 or B2 that it compiles, holds an admitted
   or assumed proof or a command or attribute silencing warnings, by a scan of each source with
   its comments removed and its whitespace collapsed. `Statements.v` and `Witnesses.v` open no
   section and declare no hypothesis, variable or context.
2. B1's `Trace.v` is compiled from `../region_reuse/` under the logical name `RegionReuse`, B2's
   `Core.v`, `Shape.v`, `Sim.v`, `Reuse.v` and `Controls.v` from `../region_core/` under
   `RegionCore`, and the eight modules here under `Instance`, all fourteen with no output.
3. `rocqchk` re-verifies all fourteen modules with no axiom and no unsafe construct in its
   summary.
4. `Print Assumptions` reports each of the 42 named results closed.
5. `Statements.v` holds the ten expected pins.

Eight planted defects were run against copies of the tree under the ignored `tmp/` on
2026-11-08, after a control copy with no defect passed. The script refused every one. They were
a premise added to the pinned `core_log_is_B1_obs`, which `Statements.v` refused to compile, a
premise added to `instance_sound`, after which `Instance.v` failed to compile, an `Admitted`
lemma, an unlisted theorem using the classical axiom, which the `rocqchk` summary refused, a
witness weakened to a tautology, which its pin refused, a section hypothesis in the pin file, a
warnings attribute, and an `Admitted` lemma in a copied dependency of B2. No log of these runs is
kept in the tree. Continuous integration does not run the script, under the operator's ruling
of 2026-11-07 on new continuous-integration jobs.

## The trusted base

The Rocq kernel, `rocqchk`, the standard-library modules `List`, `Arith`, `Bool` and `Lia`, the
definitions of B1 and B2 that the statements mention, and the statements as written.

The statement of `reuse_sound_via_B1` mentions only B2's definitions, so the encoding of this
directory is outside its trusted base. The statement of `instance_sound` mentions the machine
`next`, which is defined here. The claim that `next` is the core rests on `gen_is_run` and
`core_is_oracle`, which relate it to B2's interpreter by proof, so a mistake in the oracle
interpreter would break those theorems rather than pass unnoticed. The pins fix the statements
and the definitions `sel0` and `baseline`, and not the definitions the statements mention, among
them `next`, `run_events`, `orun_all`, `ans_plan`, `strip` and `to_log`.

## The encoding

B1's parameters are instantiated with a site as a number, the bytes of a region as the scalar
it stores, a view as the unit type selecting the whole region, and a yielded value as a number.

| core execution | B1 event |
|---|---|
| `New s` storing the scalar `b` | `EAlloc s b` |
| `Get` of a handle to region `r` | `EDeref (SReg r) tt` |
| `Get` of a scalar | `EDeref (SCopy 0) tt`, a read of the copy store, which no encoded run writes, so it is stale under every plan as the core's dereference of a scalar fails |
| `Yield` of the scalar `n` | `EOut n` |
| the reset ending a cycle | `EAdvance` |
| every other instruction, including the entry of a conditional, a loop and a call | `EStep` |

B1's observation `OBytes b` is the core's `ReadE (Some b)`, `OStale` is `ReadE None`,
`OScalar n` is `OutE n`, and `ONone` is no log entry, which `to_log` computes.

## The modules

| module | contents |
|---|---|
| `Encode.v` | the oracle interpreter, which runs the core's instructions and appends one B1 event per execution, takes every dereference outcome from an oracle, and keeps the events of a run that runs out of fuel |
| `Causal.v` | two oracle runs whose oracles agree on the first reads agree on every event up to the next read |
| `Memory.v` | `core_is_oracle`, for every plan and every module, the core run and the oracle run answered by B1's own `read` under the same plan stop together, end in one state apart from the memory, and the core's log is `to_log` of B1's observations |
| `Machine.v` | the machine `next`, which replays the core from the dereference outcomes in an observation history, `gen_is_run`, which shows B1's `gen` produces the core's events under every plan, and `replay` |
| `Safety.v` | `checked_slot_safe`, which derives B1's `slot_safe` for every slot from `region_check` and an injective plan |
| `Instance.v` | `instance_sound`, `core_log_is_B1_obs` and `reuse_sound_via_B1` |
| `Witnesses.v` | evidence that the instance is not vacuous |
| `Statements.v` | the pins |

## The proof, in order

1. **The core's memory is B1's owner model.** `Memory.Inv` relates a core state to the B1 state
   reached by the events so far. The core's region table is B1's without the bytes, the epochs
   agree, the copy store is empty, and the core's memory at a cell holds the bytes of the region
   B1 names as the cell's owner. Every instruction preserves it, so a dereference has one
   outcome in both, and `core_is_oracle` follows for every plan and every module.
2. **The core is a B1 machine.** B1 requires the next event to be a function of the observation
   history alone, with no plan. `Machine.next` replays the core with an oracle that reads the
   dereference outcomes from the history in order. `Causal.causal` shows that the replay agrees
   with the real run up to the next event, because an event depends only on the outcomes of the
   reads before it. `gen_is_run` concludes that `Trace.gen` under every plan `Q` produces
   exactly the events of the core run under `Q`, and `core_log_is_B1_obs` that the core's log is
   the projection of B1's observations of them.
3. **The checker discharges B1's per-slot obligation.** `Safety.safe_sim` maintains that every
   handle on the stack was allocated at its tagged site with no allocation into its cell since.
   The checker forbids a `New s` while a handle of `s` is on the stack, an injective plan sends a
   different site to a different cell, handles never leave the stack, and called functions
   allocate nothing. At a `Get` the invariant gives `slot_safe`'s conclusion. The obligation
   holds under every oracle, so it holds of the baseline run that `lemma4_composition` asks
   about.
4. **B1's theorem applies.** `instance_sound` is `Trace.lemma4_composition` with the premise of
   step 3 and the run of step 2.
5. **B2's theorem follows from B1's.** `reuse_sound_via_B1` takes `instance_sound` at a length
   covering both runs, so the plan's and the baseline's B1 observations are equal. `replay`
   turns equal observations into equal oracle runs, and `core_is_oracle` turns those into equal
   logs and agreement on stopping. Its statement is B2's pin of `reuse_sound` verbatim, and
   `pin_same_as_B2` checks that the two theorems have one type. The proof uses no part of B2's
   `Sim.simulation` or `reuse_sound`. It does reuse B2's checker, the stack-shape definitions and
   helper lemmas of `Sim.v`, and `Reuse.region_check_wf`.

## The witnesses

- One cycle of B2's accepted `loop_ok` module generates three allocations, three dereferences
  of real regions and one epoch advance, so the machine produces the events the encoding claims.
- B2's `loop_bad` module, which the checker rejects, makes B1's `sound` false for its instance
  machine under the injective plan `P0`. So `instance_sound` does not hold of every module, and
  its checker premise does work.
- B2's accepted `two_sites` module makes `sound` false under the plan `P01`, which puts both
  sites in one slot. So the injectivity premise does work, and by `lemma4_composition` B1's
  per-slot premise fails on that module's baseline run. The premise that `checked_slot_safe`
  discharges is therefore not trivially true.

## What is not instantiated

1. **The flow certificate.** No `Clearance.flow` is constructed from the core, and so no
   `flow_ok` is proved, and the clearance lemmas, `element_safe`, `theorem_B1`, `theorem_B1r`,
   `corollary_A3`, `theorem_B2` and `corollary_B2b_hybrid` are not applied to the core.
   Building one needs the machine state at every event position, which the big-step interpreter
   does not expose. It would take an instrumented semantics recording, at each position, the
   stack and the locals as locations with their holdings, the operands each execution consumes,
   the locations it writes, the end of each location's lifetime, and the end of each
   execution's scope as a later position satisfying `F_nest` and `F_after`. In an accepted
   module no `Escapes` execution has a handle among its operands, so confinement would be the
   natural route, but that is an inference about a construction that has not been made.
2. **B1's slot theorem.** `Trace.slot_theorem` and the separation premise `separated` are not
   used. `slot_safe` is proved directly, which is stronger than the prose route and does not
   check that route.
3. **Increment B1's other modules.** `Bound.v`, with Theorems A1 and C and Corollaries A1s and
   B2a, and `Clearance.v` and `Theorems.v` are not compiled here, since nothing here uses them.
4. **The encoding's reach.** A region holds one number and a view selects all of it. No encoded
   run contains `ECopy`, so the escape-copy discipline is not exercised. The environment never
   dereferences a region. These follow from the core of B2, whose limits are listed in its
   README.
5. **Stopping is not a B1 observation.** B1's events have no halt. Agreement on whether the two
   runs stop is derived in `reuse_sound_via_B1` from equal observations and `replay`, outside
   B1's theorem. `core_log_is_B1_obs` says nothing about a run that stops for want of fuel.
6. **The machine is per configuration.** `next` is fixed by the module, the host, the fuel and
   the number of cycles, so B1's theorem is applied once per configuration. B1 folds the host
   into its machine in the same way.
7. **Keleusma itself.** As for B2, nothing here relates the region core to the shipping virtual
   machine or verifier.
