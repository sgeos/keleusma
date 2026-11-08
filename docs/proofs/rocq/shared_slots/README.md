# Shared Slots, an Excluded Case of Increment B2

> **Navigation**: [Increment B2](../region_core/README.md) | [The prose proof](../../COMPOSITE_REGION_REUSE_PROOF.md) | [Audit record](./AUDIT_2026-11-08.md)

Increment B2 proved reuse sound for plans that give distinct sites distinct slots, and listed
arm groups, the slot sharing of Part I's Theorem A2 and Corollary A3, among the cases it does not
cover. This directory covers them, in two steps. It makes B2's checker aware of the plan, and then
lets a handle's abstract tag record the set of sites it may belong to, so the arms of a
conditional can be joined. It proves in the Rocq Prover, version 9.3.0, that **for every module
and plan the join checker accepts together, the run that reuses the plan's slots and the
baseline run produce the same observations, for every host, every fuel and every number of
stream cycles**, with no premise that the plan is injective. Every named result is checked with
no axiom.

## What changed

**The plan-aware checker.** At `New s`, B2's checker rejects a live handle of `s`.
`SharedShape.shapeP_b` rejects a live handle of `s` or of any other site the plan puts in the
same slot. Every other rule is B2's, so a plan may let sites share a slot whenever their handles
are never live together. It takes the plan as an input, so it validates a planner's output
rather than the program alone. `Shared.reuse_sound_shared` proves it sound, and
`Shared.shared_generalises` proves that for a plan giving distinct sites distinct slots it
accepts exactly what B2's checker accepts.

**The join checker.** The plan-aware checker inherits B2's rule that both arms of a conditional
leave the same abstract stack. So a handle of one site produced in one arm and a handle of
another site produced in the other can never both survive the conditional. That excludes the
commonest arm group of Theorem A2 and Corollary A3, choosing between two composites and then
using the one chosen, which the audit of this directory found. `ArmShape.shapeA_b` lets a
handle's tag record a set of possible sites and joins the arms of a conditional position by
position, uniting the site sets of handles. At `New s` an entry conflicts when any site it may
belong to is `s` or shares `s`'s slot. `Arm.reuse_sound_arm` proves it sound, and
`Arm.arm_generalises` and `Arm.arm_contains_b2` prove it accepts everything the plan-aware
checker accepts, and everything B2's checker accepts under an injective plan, so nothing is lost.

## How it is proved

`SharedSim.simulation_shared` and `ArmSim.simulation_arm` are B2's simulation for the two
checkers, their proof text derived from B2's by substitution, with B2's lemmas about pops and
memory correspondence reused unchanged. For the plan-aware checker two cases differ. At `New`
the new rule supplies that no live handle names the cell the allocation writes, and at `Call` a
function's body is checked by B2's checker, which agrees with the new one when regions are
forbidden. For the join checker the stack relation says a handle belongs to one of its possible
sites, the conditional's result holds of either arm's stack by `ajoin_sound_l` and
`ajoin_sound_r`, and a callee is simulated by `simulation_shared`, whose scalar result shape
embeds into the new tags.

## How to check it

```sh
eval $(opam env --switch=rocq)
docs/proofs/rocq/shared_slots/check.sh
```

The script is B2's hardened script. It builds and checks B2's sources as its dependency, then
checks this directory's eight modules. It fails unless no source holds an admitted or assumed
proof, a warning silencer of either attribute spelling, or a `Redirect`, scanned with comments
removed and whitespace collapsed, the pin
and control files open no section and declare no hypothesis, variable or context, every
module compiles with no output, `rocqchk` reports no axiom and no unsafe construct, all 34 named
results are closed, and `Statements.v` holds the seven expected pins. Three planted defects were
run against it on 2026-11-08, a premise added to `reuse_sound_shared`, which its pin refused, an
`Admitted` lemma, and an unlisted theorem using the classical axiom, and it refused each. No log
is kept in the tree. Continuous integration does not yet run the script, under the operator's
ruling that a new continuous-integration job waits for the release or for agreement with the
V0.2.X line.

## The trusted base

The Rocq kernel, `rocqchk`, the standard-library modules `List`, `Arith`, `Bool` and `Lia`, B2's
definitions, and the statements as written. The pins fix the statements of `reuse_sound_shared`,
`shared_generalises`, `reuse_sound_arm` and `arm_contains_b2`, and the claims of the arm-group,
choose-then-use and co-live controls. They do not fix the definitions they mention, among them
`region_checkP`, `region_checkA`, `conflict`, `ajoin`, `cycles`, `log` and the control modules.

## The controls

- `choose` picks a site by a conditional, allocates it in the arm, and dereferences the chosen
  region after the conditional, under the plan `Pshare` that puts sites 0 and 1 in slot 0.
  The plan-aware checker rejects it and the join checker accepts it. Its reuse run agrees with
  the baseline, both sites allocate into the one slot, and `choose_sound` applies the headline
  theorem to it.
- `arms` allocates and consumes a handle inside each arm. Both checkers accept it, it agrees,
  and `arms_one_slot` shows both sites allocating into the slot and no bump cell written.
- `sequential` lets site 1 reuse site 0's slot after site 0's handle has died. It is accepted
  and agrees.
- B2's `two_sites`, whose two handles are live together, is rejected by both checkers under
  `Pshare` while B2's checker accepts it, and `co_live_diverges` shows that its reuse run really
  diverges under `Pshare` itself.
- `Pshare` is proved not injective, so B2's theorem cannot cover any of these.

The audit ran an exhaustive search on the plan-aware checker, every instruction word up to
length five over a fixed alphabet in five contexts under five sharing plans. It found 509,946
accepted pairs at length five and 64,643 more at lengths two to four, with no divergence, while
the same search with B2's checker under those plans found 302 divergences, which shows the
search detects one. That search is not part of this directory's checks.

## What is not covered

Every limit of increment B2 except the injectivity premise still applies, among them the absence
of refined confinement, of the escape-copy discipline, of sites in called functions, of memory
accounting and of termination, and the absence of any relation to the shipping virtual machine
or verifier. The bound side of Theorem A2, that the shared slot is provisioned once at the size
of its largest member, is not modelled, since a region here holds one number. A loop body must
still restore its abstract stack exactly, so a loop whose iterations join different sites into
one entry is rejected, which is conservative.
