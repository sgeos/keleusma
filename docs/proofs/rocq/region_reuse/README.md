# Mechanisation of Part I of the Composite-Region-Reuse Proof

> **Navigation**: [The prose proof](../../COMPOSITE_REGION_REUSE_PROOF.md) | [Audit record](./AUDIT_2026-11-07.md)

This directory mechanises Part I of
[the prose proof](../../COMPOSITE_REGION_REUSE_PROOF.md) in the Rocq Prover, version 9.3.0. Every
lemma, theorem and corollary of Part I has a counterpart here except the cycle clause of Lemma 5,
which the trace model's epoch check carries with no separate statement, and every counterpart
is checked with no axiom. It is increment B1 of the proof line's mechanisation, ruled by the operator on 2026-11-07
in the order B1, then A, then B2.

> **What B1 does not establish, read first.** B1 checks the theory over an abstract machine,
> and it does not check that Keleusma is that machine. More sharply, the reuse theorems take a
> **flow model as a certificate** that their caller supplies, tied to the run only through where
> allocations and dereferences occur. Which executions are `Escapes`, where scopes end, and
> which locations hold which references are the certificate's choices. A certificate with no
> `Escapes` execution meets every clearance route at once, so the theorems' conclusions rest on
> the separation and read order the certificate exhibits. Confinement in the prose is a
> property of the program, and here it is a property of the certificate. Increment B2 must
> therefore build the certificate from the machine's own semantics, with its real
> classification and real scope ends, and showing that some certificate exists is not enough.
> The full list of limits is under "What is not mechanised" below.

## How to check it

```sh
eval $(opam env --switch=rocq)
docs/proofs/rocq/region_reuse/check.sh
```

The script fails unless all of the following hold.

1. No source contains `Admitted`, `admit`, `Admit`, `Abort`, `Axiom`, `Parameter`,
   `Conjecture`, their plurals, `bypass_check`, a command unsetting guard, positivity or
   universe checking, a `Set Warnings` command, or a `warnings` attribute, scanned with the
   comments removed and the whitespace collapsed so a construct split across lines or by a
   comment is still found, and `Controls.v` opens no section and declares no hypothesis,
   variable or context. These were added on 2026-11-08 after the audits of increments A and
   B2.
2. All five modules compile with no output at all, so any warning fails.
3. The standalone checker `rocqchk` re-verifies every compiled module, and its context summary
   reports no axiom, no unsafe fixpoint, no type-in-type and no assumed positivity. The summary
   covers every constant of the loaded environment, listed or not, so importing a module that
   declares an axiom fails the check even when nothing uses it.
4. `Print Assumptions` reports each of the 32 named results closed under the global context,
   and the count reported closed equals the count named.

Build output goes under `target/rocq/`, and the script deletes an output directory only when its
own marker file is in it. Nine planted defects were run against the script on 2026-11-07, and
the script refused every one. They were an `Admitted` lemma, the plural `Axioms`, an unguarded
fixpoint under `Unset Guard Checking`, an `Admit Obligations`, an unlisted theorem using the
classical axiom, a hypothesis outside any section, a bare import of the classical library, a
listed theorem using the classical axiom, and a foreign output directory, which it refused to
delete. The delta check of the audit planted three more that evade the word scan, a guard
unset split by a comment, a top-level `Context` declaring a proof of falsehood, and a
positivity unset split across lines, and the summary check refused each. No log of these runs
is kept in the tree, so they are evidence of this session, reproducible by planting the same
defects in a copy under the ignored `tmp/`. The marker file guards against accident, not
against an adversary, and the script cannot detect a named result redefined to a trivial
statement, which is why the statements are in the trusted base. Increment A's audit of
2026-11-08 confirmed this by execution, and increment A pins its headline statements for that
reason. **This directory's statements are not yet pinned**, which is an open item of the proof
line. The script is not yet run by continuous integration, because the operator ruled on
2026-11-07 that a new continuous-integration job waits for the release or for agreement with the
V0.2.X line.

## The trusted base

The Rocq kernel, `rocqchk`, the standard-library modules `List`, `Arith` and `Lia`, and the
statements as written. Section hypotheses are not axioms. They become explicit premises of each
closed theorem, so a reader checks a result by reading its statement. **The statements are
part of the trusted base, so they deserve the closest reading.** The rest of this file exists
to make that reading possible.

## The modules

| module | contents |
|---|---|
| `Bound.v` | structure trees, their executions, the static bound, Theorem A1, Corollary A1s, the lexical containment step, the bound arithmetic of Theorem C and Corollary B2a, and the remark that neither bound dominates |
| `Trace.v` | the abstract machine, its two memory regimes, the comparison method, Lemma 4's composition, the slot theorem, and Theorem A2 at the trace level |
| `Clearance.v` | the flow model of where references reside, its hypotheses, and Lemmas 1, 2 and 3 and the loop clause of Lemma 5 |
| `Theorems.v` | the join of the two models, the element theorem, and Theorems A2, B1, B1r, B2, Corollaries A3 and B2b, and Lemma 4 for every element kind |
| `Controls.v` | the controls against vacuity, one negative and one positive |

## The abstract machine

**The machine is a function from observation history to the next event.** That one choice
carries the Setting's determinism given the environment, folds the environment's symbolic
behavior of Definition 5 into the machine, and makes M8 structural, since no event can depend on
an address the machine never observes. An event names a region by its allocation index, which is
its provenance per Definition 2, never by an address. Soundness, Definition 5, is equality of the
observation sequences of every prefix of the plan run and the baseline run, for every such
function, so every theorem holds for every path under every environment behavior. Receipt events
of Definition 5 are not modelled separately, an environment's receipt being an event the machine
function already accounts for.

**The memory is region-granular.** A cell is one bump interval per region or one plan slot, and a
cell presents the bytes of the last region allocated into it. A dereference of a region whose
cell a later allocation has taken therefore returns the later region's bytes, which differs from
the baseline whenever the two regions' bytes differ, so the model observes every divergence the
theorems must exclude. An allocation's bytes are chosen by the machine function from the
observations so far, and an allocation reads no region. The inline copy of composite operands in
M3(vi) is therefore not expressed, and neither are the sizes of slots and regions.

**The flow model** records, for one baseline run, which location holds a reference to which
region before each execution, the operands each execution reads, the locations it overwrites,
whether it is `Escapes`-classified, the lifetime of each location, and the end of each
execution's scope. Its hypotheses are the record `flow_ok` in `Clearance.v`. As the note at the
top says, it is a certificate, and `Theorems.describes` ties it to the run only through
allocation and dereference positions.

## Where each axiom lives

| axiom | where it lives in the mechanisation |
|---|---|
| M1 | built into the trace model, since a region's construction bytes are fixed at allocation and bump cells are never reused |
| M2 | built into the trace model, since a handle carries its region's creation epoch, a derived view its source's, and epochs never repeat |
| M3(i) | the flow hypothesis `F_prov`, no reference except from an operand or a fresh allocation |
| M3(ii) to M3(iv) | **not used**, see the findings below |
| M3(v) | the flow hypothesis `F_exh` |
| M3(vi) | the flow hypotheses `F_alloc`, `F_fresh` and `F_inj`, the last proved of every run by `Theorems.alloc_at_inj`, and the inline copy not expressed |
| M4 | built into the trace model, since the epoch advances only at the event `EAdvance` |
| M5 | **not used**, see the findings below |
| M6 | the flow hypotheses `F_alloc` and `F_nest`, together with the lifetimes `death`. `F_alloc` is required of every allocation, including one whose scope is a cycle, which goes beyond M6's loop iterations |
| M7 | the flow hypotheses `F_prov` and `F_read`, environment storage being a location with no death |
| M8 | built into the trace model, since the machine is a function of observations alone |
| M9 | built into both models, since region bytes and copies are plain bytes that hold no reference |

## Where each result lives

| prose | Rocq | how the Rocq statement differs |
|---|---|---|
| the comparison method | `Trace.memory_agrees` and `Trace.plan_follows_baseline` | proved for an encoding in which events name regions by index and the machine function is shared, so the location-by-location correspondence of Section 9 item 8, with its differing addresses and handle values, is assumed by the encoding rather than proved |
| Lemma 1 | `Clearance.lemma1_provenance` | states only that every holding arises after its region's allocation. The closure itself, the environment clause and the baseline interval clause are assumed as `F_prov` and the trace model. No other result uses this lemma |
| Lemma 2 | `Clearance.lemma2_confinement_clearance` | holds for any scope, because `F_alloc` is required of every scope |
| Lemma 3 | `Clearance.lemma3_refined_clearance` | Definition 8 generalised to any set of excused locations, and `dead_at` bars a read of an excused location after the scope end across cycles too |
| Lemma 5, loop clause | `Clearance.lemma5_unconditional_clearance` | the discipline stated per region as no outliving placement by an `Escapes` execution, the proof being Lemma 2's with that premise |
| Lemma 5, cycle clause | the epoch check of `Trace.read` | no separate statement |
| Theorem A1 | `Bound.A1_branch_bound` | one conditional instance over complete executions. Outermost-first replacement over nested conditionals is covered only through Corollary A1s |
| Corollary A1s | `Bound.A1s_static_bound` | calls are represented by inlining, which H's acyclic call graph licenses |
| Theorem A2 | `Theorems.theorem_A2`, via `Trace.A2_arm_overlap` | the premise is at most one allocation into the slot per epoch, see the gaps below |
| Theorem B1 | `Theorems.theorem_B1` | confinement is a property of the certificate |
| Theorem B1r | `Theorems.theorem_B1r` | each region may carry its own set of dead slots, and refined confinement is a property of the certificate |
| Corollary A3 | `Theorems.corollary_A3` | the separation premise is stated for the shared slot, see the gaps below |
| Lemma 4 | `Theorems.lemma4_plan_sound`, via `Trace.lemma4_composition` | proved by one simulation rather than an ordered induction. Its premise is each slot's dereference obligation on the baseline run, so the theorem lifts that obligation to the whole run |
| Theorem C | `Bound.C_plan_bound` | the bound arithmetic only, Corollary A1s with monotonicity over prefixes. No statement relates it to the trace model or to a plan, see the gaps below |
| Theorem B2 | `Theorems.theorem_B2` | the discipline is a premise on the certificate. The copy store's observation equality is proved by `Trace.memory_agrees`, but the theorem's discipline premise never refers to it |
| Corollary B2a | `Bound.B2a_accounting` | the bound arithmetic only, copies being weighted leaves, see the gaps below |
| Corollary B2b | `Theorems.corollary_B2b_hybrid` | the same statement as `lemma4_plan_sound`, which admits all three clearance routes and the A2 route |
| the remark after Theorem C | `Bound.footprint_below_bump` and `Bound.footprint_above_bump` | two concrete units, one each way. The greedy-selection sentence has no counterpart |

The element theorem `Theorems.element_safe` is the common core. A reused slot is safe when
consecutive allocations into it are separated by an epoch boundary or by the end of the earlier
allocation's scope, and every region allocated into it is cleared by confinement, by refined
confinement, or by the discipline. Theorems B1, B1r and B2 and Corollaries A3 and B2b are that
core with one or more clearance routes.

## The controls

`Controls.reuse_without_separation_is_unsound` proves a concrete plan unsound, one that
allocates twice into one slot within one epoch and then reads the first region.
`Controls.unsound_plan_fails_the_premises` proves that this plan cannot meet the premises of
Lemma 4, so the premises do work. `Controls.premises_of_B1_are_satisfiable` meets every premise
of Theorem B1 for a run that reuses one slot for two regions and reads each, and derives that
run's soundness from the theorem, so the premises are satisfiable by a run that is not
degenerate. The positive control was built by the adversarial audit and adopted here. Its
certificate has no `Escapes` execution, so it exercises the separation and the read order, not
a nontrivial confinement argument.

## Findings of the mechanisation

These are facts about the abstract model, established by the checked proofs. Whether each
carries over to Keleusma is a question for increment B2.

1. **M5 is not used.** The prose cites M5 for the cross-cycle case of Theorems A2 and B1. In the
   model, every dereference across an epoch boundary fails stale by M2 with M4 alone, since a
   read checks the named region's own creation epoch. Two qualifications apply. For Theorem B1r,
   `dead_at` bars a read of an excused location after the scope end across cycles, carrying
   the residue the prose leaves to M5. And `F_alloc` with `F_exh`, required of every
   allocation, may assume part of what M5 supplies for an allocation whose scope is a cycle.
2. **M3(ii) to M3(iv) are not used.** Exhaustiveness, M3(v), with provenance gives every
   placement by a non-escaping execution a lifetime contained in its scope, which is all Lemma
   2's case analysis needs.
3. **Lemma 4 needs no ordered induction.** The prose discharges the plan's elements one at a
   time because each element's hypotheses are stated over baseline paths. Induction over the
   events of the run relates the whole plan to the baseline at once, given each slot's
   dereference obligation on the baseline run.
4. **Lemma 2 does not need the scope to be a loop iteration** in the model, because `F_alloc`
   is required of every scope, and Definition 8's single named slot generalises to any set of
   excused locations.

## What is not mechanised

1. **The certificate's correspondence to the machine.** Nothing ties a flow model's
   classification, scope ends, lifetimes or holdings to a run beyond allocation and dereference
   positions. Increment B2 must construct the certificate from the machine's semantics.
2. **The correspondence to Keleusma.** Nothing here shows that a Keleusma run is a function of
   its observations, that its memory behaves as the trace model's, or that the location
   correspondence of Section 9 item 8 holds. Those are increment B2's obligations against
   increment A's semantics, and until then Part II's measured standing is the only evidence.
3. **The structural counting steps of Theorem A2 and Corollary A3.** The prose derives at most
   one allocation into the shared slot per cycle from the conditional's instance count, lexical
   containment and each site's execution count, and derives slot-level separation in Corollary
   A3 from the inner loops' exits and the instance hypothesis. `Bound.cond_one_arm` mechanises
   lexical containment, and nothing composes it with the trace-level statements, whose
   counting is a premise.
4. **The link from Theorem C and Corollary B2a to the machine.** `Bound.C_plan_bound` and
   `Bound.B2a_accounting` relate no trace object. That a plan's slots total `slots`, that its
   reused leaves are the plan's sites, that occupancy measures machine state, that each cycle's
   allocations are a prefix of one execution of the unit, and that each copy matches an
   `Escapes` occurrence and its static bound are all unproved. `Theorems.plan_runs_baseline_events`
   supplies only the equality of the plan's and the baseline's event sequences.
5. **The inline copy of composite operands in M3(vi)**, and the sizes of slots and regions.
6. **The cost paragraph after Corollary B2a**, which states arithmetic about one shape and has
   no counterpart.
