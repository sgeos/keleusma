# Region Slot Reuse Under Escape Confinement

> **Navigation**: [Documentation Root](../README.md)

> **STATUS.** This is the post-audit revision. The document at `de8b3f68` was adversarially
> audited by five independent contexts on 2026-08-24, the findings are recorded in
> [`AUDIT_2026-08-24.md`](./AUDIT_2026-08-24.md), and every verified finding is repaired here.
> Part I is a general theory over an abstract machine, proved from stated axioms. Part II
> instantiates the axioms for Keleusma with measured standing per row. Theorem A1 is
> unconditional. The other theorems apply, in the instantiation, to reference-compiled modules
> whose composites are transitively scalar, and the producer invariants they rest on are listed
> exhaustively in the scoping paragraph of Appendix A. Two pins from the V0.2.X line are
> awaited, the address-opacity discriminator and the Break row correction, both marked in their
> rows. A fresh adversarial re-audit of this revision is required before any merge.

This document discharges part of the obligation stated in `docs/proofs/COMPOSITE_REGION_REUSE.md`
on the `v0.3.0` line. The obligation was read at commit `a49555bb`, and it has since been
corrected on its own line at `d5b706e8` and `c3ff3c06`, both verified reachable, so the pin names
the state this work was drafted against rather than the current text. Part I is self-contained
mathematics and cites no project artifact. Everything project-specific lives in the appendices.

---

# Part I. General theory

## 1. Setting

An abstract machine executes programs over a fixed finite instruction set with structured
control flow. Machine state comprises a stack of frames, each holding an operand stack and local
slots, an **ephemeral region** managed by a bump allocator and stamped with an **epoch**
counter, optional persistent storage, and an external **environment** that the machine can call
out to and yield values to, receiving a reply value on each resume after a yield. Conditionals
have finitely many arms and execute exactly one arm per dynamic instance. **Loops are iterating
constructs** with a well-defined body entry, one or more back edges, and one or more exit
edges. A dispatch construct that executes a body once and exits is a conditional in this
theory, whatever its lowering. Calls push frames, and a call may or may not return.

The question the theory answers is when a static planner may assign one fixed memory slot to an
allocation site whose dynamic executions would otherwise each receive a fresh bump allocation,
and what a worst-case memory bound may consequently count.

## 2. Definitions

**Definition 1, sites, paths, and per-cycle memory.** Let $\mathcal{S}$ be the static allocation
sites of a program unit and $\mathrm{sz}(s)$ the byte size site $s$ allocates. Let $\Pi$ be the
set of execution paths, each partitioned into cycles by Definition 3. For a path $\pi$ and a
cycle $c$ on it, $\mathrm{alloc}_c(\pi)$ is the multiset of dynamic site executions within $c$,
and the cycle's memory consumption is
$M_c(\pi) = \sum_{s \in \mathrm{alloc}_c(\pi)} \mathrm{sz}(s)$. The baseline per-cycle worst
case is $\mathrm{WCMU} = \sup_{\pi, c} M_c(\pi)$, a supremum that is finite exactly when static
iteration bounds exist, which is hypothesis H of Section 5. Because the ephemeral region is
reclaimed at every cycle boundary, $M_c(\pi)$ bounds the baseline machine's ephemeral occupancy
throughout cycle $c$.

**Definition 2, regions, handles, and references, by provenance.** Each dynamic execution of an
allocation instruction creates a **region** $r$, an identity, placed at an address interval in
the ephemeral region. The handle produced by that execution **refers to $r$**. A view derived
from a reference to $r$ is a reference to $r$. A **reference to $r$** is exactly the handle
created by $r$'s allocation or a value transitively derived from it by view derivation or
transport. A handle whose address interval merely coincides with $r$'s, through allocator reuse
of addresses in another epoch or under a plan, is **not** a reference to $r$. A **dereference**
of a handle succeeds exactly when the handle's carried epoch equals the current epoch, returning
the bytes then present at its address interval, and otherwise fails with a staleness error.

**Definition 3, cycles and scopes.** A **cycle** is a maximal interval of execution containing
no epoch advance in its interior, the interval from machine start to the first advance included.
The **scope** of an instruction execution is the current iteration of the innermost dynamically
enclosing loop of its frame, and its enclosing cycle when no loop encloses it. An iteration
runs from one execution of the loop's body entry to the corresponding body end, whether reached
at a back edge or at an exit edge. The scope of a value is the scope of the execution that
allocated it, so for a site inside nested loops the governing scope is always the innermost.

**Definition 4, locations and lifetimes.** A **location** is an operand-stack entry, a local
slot of a frame, a cell of persistent storage, or environment storage. An operand-stack entry's
lifetime runs from its push to its pop or unwinding. A local slot's lifetime is its frame's. A
persistent cell and environment storage have unbounded lifetime. A location **outlives** a
scope when its lifetime extends past the scope's end.

**Definition 5, plans, well-formedness, observations, and soundness.** A **plan** for a program
unit designates a subset $R \subseteq \mathcal{S}$ of reused sites and assigns each a fixed
slot, an address interval of at least its size, with sites paired under Theorem A2 permitted to
share one slot of the larger size. A plan is **well-formed** when its slots are pairwise
disjoint, except as A2 licenses, and disjoint from the bump range used for all other
allocations. The **baseline** is the empty plan, under which every execution receives a fresh
bump allocation. The **observations** of a run are the outcome of every dereference, success
with its returned bytes or staleness, by the machine or the environment, and every scalar
output. Handle addresses are not observations. The environment's behavior, its replies and its
dereference and presentation choices, is a function of its prior observations. A plan is
**sound** when, for the unit it is defined for, every path under every environment behavior
yields identical observations under the plan and under the baseline. The comparison assumes
both runs complete, provisioning being Section 7's subject.

**Definition 6, operands and confinement.** The **operands** of an instruction execution are
all values it reads, operand-stack entries and any slot or storage contents alike. Let $v$ be
the value allocated at site $s$ in scope $n$, with region $r_n$. The value is **confined** when,
along every path, no reference to $r_n$ is ever an operand of an execution of an
`Escapes`-classified instruction. A site is confined when every value it allocates on every
path is confined. Confinement is deliberately coarse, and Section 6's refined form and Section
8's discipline each relax it in a different direction.

## 3. Axioms

Any system claiming the theorems must discharge these. Appendix A does so for Keleusma, with
standing per row.

| # | axiom |
|---|---|
| M1 | **Bump ephemeral allocation, immutable regions.** Allocation in the ephemeral region is by bump pointer. The region is reclaimed, and the epoch advanced, only at cycle boundaries, and nothing else reclaims within a cycle. A region's bytes are not modified after its construction completes, except by a reuse plan's reallocation of the same slot. |
| M2 | **Epoch-guarded handles.** A handle carries the epoch current at its creation, dereference is governed by Definition 2, and an overwrite in place advances nothing. |
| M3 | **Total instruction classification.** The non-allocation instructions are partitioned, totally, into `NoRegion`, `WithinScope`, `CopiesOut`, and `Escapes`, with these semantics. (i) No instruction fabricates a handle, and references to regions originate only at allocation instructions. (ii) A `NoRegion` execution may read reference operands but neither derives nor moves references and produces only scalars or nothing. (iii) A `WithinScope` execution may derive or move references, but only into locations whose lifetime is contained in its scope. (iv) A `CopiesOut` execution writes referenced bytes, never references, to its destinations. (v) **Exhaustiveness.** Any execution that places a reference into a location outliving its scope belongs to an `Escapes`-classified instruction. (vi) An allocation execution creates a fresh region, copies any composite operands' bytes inline, and pushes exactly one fresh reference to the new region onto the executing frame's operand stack. Totality must hold and keep holding as the instruction set changes. |
| M4 | **Cycle cadence.** Every cycle ends with exactly one epoch advance, and no instruction of the cycle executes after its advance. |
| M5 | **Boundary clearance.** At a cycle boundary, no machine-internal location that is ever subsequently read holds a reference into the ephemeral region. The environment may retain handles across the boundary. |
| M6 | **Iterating-loop discipline.** (a) Every back edge restores the exact entry operand stack, height and per-slot shape. (b) Every early exit of an iterating loop likewise restores the exact entry stack, so no entry created within the exiting scope survives it. (c) A frame created within a scope is destroyed before that scope ends, and a call that never returns leaves its enclosing scope forever open, so scope-end conclusions are vacuous for it. (d) No instruction reads or writes operand-stack entries below the innermost enclosing loop's entry height. |
| M7 | **Environment trust.** The environment presents to the machine only handles it received from the machine, does not fabricate or alter handles, and behaves per Definition 5. What an external callee retains is unknown, which is why external calls are classified `Escapes` in any instantiation. |
| M8 | **Address opacity.** No instruction's scalar result or control effect depends on the address component of any operand handle. Dereference outcomes depend only on epoch validity and the referenced bytes. |
| M9 | **Flat values.** No machine value other than a handle or a view contains a reference, and region bytes never encode references, so copying bytes never copies a reference and bytes cannot be reconstituted into a handle. |

Clause (v) of M3 is the engine of every confinement argument. Wherever a proof shows a
destination outlives the executing scope, the placing execution is `Escapes`-classified in any
conforming machine, and confinement then applies to any reference reaching it. Environment
storage outlives every scope by Definition 4, so every transfer of a value to the environment,
yields included, is `Escapes`-classified by (v).

## 4. Lemmas

**Lemma 1, provenance closure. Requires M3, M7, M9.** At every moment, the set of references to
a region $r$ consists of the handle pushed by $r$'s allocation and values derived from members
of the set by view derivation or transport, and the environment holds only references it
received from the machine. No reference to $r$ exists before $r$'s allocation or arises by any
other route.

*Proof.* By Definition 2 references to $r$ are provenance-generated from the allocation's
handle. M3(i) excludes fabrication, M9 excludes reconstitution from bytes and containment in
other values, M3(vi) makes the allocation's push the sole origin, and M7 confines the
environment to received handles. $\blacksquare$

**Lemma 2, confinement clearance. Requires M3, M6, M7, M9.** Let $v$ be confined, allocated at
site $s$ in scope $n$, with region $r_n$, and suppose scope $n$ ends. Then at its end no
reachable location holds a reference to $r_n$.

*Proof.* By induction over the instructions executed during scope $n$, with the invariant that
every reference to $r_n$ resides either in an operand-stack entry of the frame executing the
innermost loop, above that scope's entry height, or inside a frame created during scope $n$.

The invariant holds at origination, where M3(vi) pushes the sole reference, per Lemma 1, onto
the executing frame's stack, above the entry height because M6(d) keeps the height from dipping
below entry mid-scope.

Preservation is by cases on the executed instruction's class, arguing from the class semantics
and never from instruction names. An `Escapes` execution with a reference to $r_n$ among its
operands is excluded by confinement outright, and Definition 6 makes slot and storage contents
operands, so no slot-reading escape evades this. Since transfers to the environment are
`Escapes` by M3(v), the environment never receives a reference to $r_n$, and by Lemma 1 a
resume reply cannot carry one, M9 excluding concealment inside the reply value. A `WithinScope`
execution moves or derives references only into locations whose lifetime is contained in its
scope, which are exactly the invariant's two classes, entries above the entry height and
locations of frames created during the scope, including callee argument slots however they are
filled. A `CopiesOut` execution writes bytes, and by M9 bytes carry no references. A `NoRegion`
execution derives and moves nothing. An allocation execution consuming a reference to $r_n$
copies bytes inline by M3(vi) and produces a reference to its own fresh region only. Control
transfers move no data, and any unwinding they perform destroys entries.

At the end of scope $n$, if reached at a back edge, M6(a) restores the entry stack exactly, and
if reached at an exit edge, M6(b) does the same, so in either case no entry created within the
scope survives, and no entry at or below the entry height was touched, by M6(d) in the strong
read-or-write form. Every frame created during the scope has been destroyed by M6(c), a call
that never returned having instead prevented the scope from ending, which the lemma's
hypothesis excludes. A `Return` that destroys the frame executing the loop ends the scope with
the frame, and its result lands in a location of the caller, which outlives the scope, so by
M3(v) that `Return` execution is `Escapes`-classified and confinement excludes a reference to
$r_n$ among its operands. Both invariant classes are therefore empty at the scope's end.
$\blacksquare$

## 5. Branch theorems

**Theorem A1, per-instance branch bound.** Fix any assignment of a static value
$B(\text{arm})$ to each arm of each conditional such that $B(\text{arm})$ bounds the arm's
dynamic allocation contribution on every execution of it. Then replacing each dynamic
instance's contribution by $\max_{\text{arms}} B$ preserves an upper bound on $M_c(\pi)$ for
every cycle of every path. This holds for conditionals of any arity, from structured control
flow alone.

*Proof.* Each dynamic instance executes exactly one arm, contributing that arm's dynamic sum,
which is at most its $B$, which is at most the maximum over arms. Summing over the instances of
the cycle preserves the inequality. $\blacksquare$

**Hypothesis H, static caps.** Every loop of the unit carries a static iteration cap dominating
every dynamic iteration count of one scope-entry. Totality yields termination, not caps, so H
is a genuine hypothesis, discharged in the instantiation by the language's capped loop forms.

**Corollary A1s, the static bound.** Under H, define $B$ over the unit's structure by
$B(\text{sequence}) = \sum B$, $B(\text{conditional}) = \max_{\text{arms}} B$, and
$B(\text{loop}) = \mathrm{cap} \times B(\text{body})$, with a site contributing
$\mathrm{sz}(s)$. Then $B(\text{unit}) \geq M_c(\pi)$ for every cycle of every path.

*Proof.* Structural induction. A sequence's dynamic contribution is the sum of its parts', each
bounded by induction. A conditional instance is bounded by Theorem A1 with the inductively
obtained arm values. A loop's contribution in one scope-entry is at most cap many iterations,
each bounded by $B(\text{body})$ inductively. $\blacksquare$

**Theorem A2, arm overlap. Requires M1 through M5, M7, M8, M9.** Let $s_i$ and $s_j$ be sites
in two distinct arms of one conditional, and suppose **each of the two sites executes at most
once per cycle** on every path. A well-formed plan whose sole reuse is one shared slot for
$s_i$ and $s_j$ is sound.

*Proof.* Within one cycle, at most one execution of each site occurs, and since one dynamic
instance of the conditional takes one arm, at most one of the two regions is created per
dynamic instance. If the conditional itself runs at most once per cycle, at most one of the two
regions exists per epoch, and by Lemma 1 no reference to the other exists at all. If the
conditional runs several times per cycle, the per-site hypothesis still permits at most one
execution of each site per cycle, and the slot's second write, if any, is the other site's
sole execution, whose region's first and only observation window begins at that write, while
every reference to the earlier region was created before it. A dereference of the earlier
region's reference after the overwrite would return changed bytes, so soundness needs such
references dead, which the per-site hypothesis alone does not give when both arms run in one
cycle. **The theorem therefore additionally requires that at most one of the two sites executes
per cycle**, which the exactly-one-arm property delivers whenever the conditional executes at
most once per cycle, the intended reading, now stated. Cross-cycle, any surviving reference is
environment-held by M5, or sits unread internally, and a dereference requires loading, which is
a read, so every dereference of it fails stale by M2 with M4 identically in both regimes. M8
ensures no scalar or control difference arises from the differing addresses. $\blacksquare$

The statement's hypothesis is accordingly, in full, that the conditional executes at most once
per cycle. The proof shows why the weaker per-site form is insufficient, which the audit
established by countermodel.

## 6. Confined-site reuse

**Theorem B1. Requires M1 through M9 and Hypothesis H not required.** Let $s$ be a confined
site, $L$ the innermost loop enclosing it, and $P$ a well-formed plan whose sole reuse is one
slot for $s$. Then $P$ is sound, and per-cycle accounting may count $s$ once, contributing
$\mathrm{sz}(s)$ in place of its full multiplicity.

*Proof.* By well-formedness, the two regimes differ only in the addresses of $s$'s regions,
successive ones sharing the slot under $P$. By M8 and Definition 5, an observational divergence
requires a dereference returning different bytes in the two regimes, and the slot's bytes
differ from the baseline bytes of $v_n$ only from the start of $s$'s next execution, which by
Definition 3 lies in a later scope of $L$, every execution of $s$ being separated from the next
by at least one boundary of its innermost scope. So a divergence requires a reference to $r_n$
dereferenced after scope $n$ ends within the epoch, or in a later epoch. Within the epoch,
Lemma 2 empties every reachable location at scope $n$'s end, and by Lemma 1 no reference can
subsequently arise, the environment never having received one. Across epochs, M5 leaves
surviving references environment-held or forever unread, and dereferencing requires reading, so
every such dereference fails stale by M2 with M4, identically in both regimes. Dereferences of
the current scope's reference occur while the slot holds exactly its bytes, equal to baseline
by M1's immutability. $\blacksquare$

### The refined form, local stores to boundary-dead slots

**Definition 7, deadness at loop boundaries.** Let $L$ be the **innermost** loop enclosing site
$s$. A local slot $\ell$ of the frame executing $L$ is **dead at the boundaries of $L$** when,
at **every** back edge and at every exit edge of $L$, every path onward either never reads
$\ell$ or writes $\ell$ before its first read of it, reads in the sense of Definition 6.

**Definition 8, refined confinement.** Let $D$ be a set of slots dead at the boundaries of the
innermost loop $L$ enclosing $s$. The value $v$ at $s$ in scope $n$ is **confined relative to
$D$** when, along every path, no reference to $r_n$ is ever an operand of an `Escapes`
execution other than a local store targeting a slot in $D$.

**Lemma 3, refined clearance. Requires M3, M6, M7, M9.** Let $v$ be confined relative to $D$,
with $L$ innermost, and suppose scope $n$ ends. Then no dereference of a reference to $r_n$
occurs after scope $n$ ends.

*Proof.* Extend Lemma 2's invariant with a third class, slots in $D$. The excused store places
a reference in a $D$-slot, and a load of a $D$-slot during scope $n$ pushes a copy above the
entry height, both preserving the invariant, and every other case is as in Lemma 2. At scope
$n$'s end, reached at a back edge or exit edge of the **same** loop $L$ whose boundaries anchor
$D$'s deadness, the first two classes are empty as in Lemma 2, so every surviving reference
sits in a $D$-slot. By Definition 7, on every path onward from that boundary each such slot is
written before it is read, and Definition 6 counts every access that takes the content as an
operand as a read, so the stale content is never an operand of anything before being
overwritten, and in particular is never dereferenced and never copied. $\blacksquare$

**Theorem B1r. Requires M1 through M9.** With $s$, $L$ innermost, and a well-formed single-slot
plan as in Theorem B1, and $v$ confined relative to some $D$ per Definitions 7 and 8, the
conclusion of Theorem B1 holds unchanged.

*Proof.* Theorem B1's within-epoch step needed exactly that no dereference of a reference to
$r_n$ occurs after scope $n$ ends, which Lemma 3 delivers, together with the environment
premise, which still holds because yields, returns crossing the loop frame, and external calls
remain unexcused `Escapes`, so the environment never receives a reference to $r_n$. The
$D$-slot residue survives internally without being read, which M5 permits, its clause covering
only locations subsequently read while so holding, and the cross-epoch and byte-equality
arguments are unchanged. $\blacksquare$

**Corollary A3, arm overlap inside loops. Requires M1 through M9.** Let a conditional sit
inside a loop, let every site in its arms be confined or confined relative to suitable $D$
sets, all anchored at their innermost loops, and let the plan share one slot across the arms'
sites and reuse it across scopes. The plan is sound. *Proof.* Per scope of the enclosing loop,
one arm executes, and within any arm a nested site's executions are governed by its own
innermost scope, so consecutive writes to the shared slot are always separated by a boundary of
the writing site's innermost scope. Lemma 2 or Lemma 3 empties references at each such
boundary, and the cross-cycle case is as in Theorem A2. $\blacksquare$

**Remark, what the refinement admits.** A binding declared inside a loop body compiles to a
slot written at its declaration before any read in each scope and dead at the boundaries
whenever slot assignment preserves definite initialization, so the refined form admits the
ordinary in-loop binding. Both membership of $D$ and refined confinement are static dataflow
properties, liveness and reference flow, so the gate on applying B1r is an analysis.

## 7. Composition and the plan bound

**Lemma 4, composition. Requires M1 through M9.** Let $P$ be a well-formed plan, and suppose
each reused element of $P$, a single site or an A2-paired pair sharing a slot, satisfies the
hypothesis of Theorem B1, Theorem B1r, or Theorem A2 respectively. Then $P$ is sound.

*Proof.* Order the reused elements and let $P_0, \dots, P_k$ be the plans reusing the first $i$
elements, $P_0$ the baseline and $P_k = P$. Adjacent plans differ at exactly one element.
Lemmas 1, 2, and 3 hold in the machine under **any** well-formed plan, because their statements
and proofs concern provenance, operands, and location lifetimes, none of which a plan changes,
a plan changing only which addresses regions occupy and hence which bytes an unsound
dereference would see. The corresponding theorem's equivalence argument between $P_i$ and
$P_{i+1}$ therefore applies verbatim, its byte-equality step using well-formedness to keep
every other element's slot and the bump range disjoint from the element under consideration.
Observational equivalence composes transitively along the chain. $\blacksquare$

**Theorem C, the plan bound.** Assume Hypothesis H. Let $P$ be as in Lemma 4, and define

$$
\mathrm{footprint}(P) \;=\; \sum_{e \in R} \mathrm{slot}(e) \;+\; B_{\mathrm{bump}}
$$

where $\mathrm{slot}(e)$ is the element's slot size, the shared maximum for an A2 pair, and
$B_{\mathrm{bump}}$ is Corollary A1s's static bound computed over the non-reused sites alone.
Then the machine under $P$ never occupies more ephemeral memory within a cycle than
$\mathrm{footprint}(P)$.

*Proof.* At any moment within a cycle, occupancy is the reused slots, statically
$\sum \mathrm{slot}(e)$ by well-formedness, plus the bump allocations of the current cycle,
which are exactly the non-reused executions of the cycle and are bounded by $B_{\mathrm{bump}}$
by Corollary A1s. The boundary reclaims the bump range and the slots are counted statically.
$\blacksquare$

**Remark, comparison and non-necessity.** Whenever every reused loop site's cap is at least one
and every A2 pair would otherwise contribute both arms, $\mathrm{footprint}(P)$ is at most the
corresponding all-bump static bound, since $\mathrm{sz}(s) \leq \mathrm{cap} \cdot
\mathrm{sz}(s)$ and $\max \leq$ sum. A site excluded from $R$ reverts to full multiplicity in
$B_{\mathrm{bump}}$. Reuse of an unconfined site's slot is **not licensed by this document**,
and no necessity claim is made, Section 9 recording flows believed harmless and unproved.

## 8. Universal reuse under an escape-copy discipline

**Definition 9, escape-copy discipline, unconditional and selective.** A machine satisfies the
**escape-copy discipline** when every `Escapes` execution whose operand is a reference to an
ephemeral region transports a fresh copy in its place, subject to, first, faithfulness, the
copy's bytes equal the referenced bytes at the instant of the copy, second, stability, a copy's
bytes are unmodified while any reference to it is reachable within its epoch, the copy store
being disjoint from every reused slot and from the bump range, third, epoch stamping, a copy
handle carries its creation epoch and dereference is governed by M2, fourth, recursion, an
escape of a copy produces a further copy, and fifth, depth, a copy contains no reference into
any ephemeral region or reused slot, which M9 makes automatic for flat machines. Under the
discipline, references to copies originate at the copying escapes, and M3(i) is read as scoped
to region references, Lemma 1 continuing to govern regions. The **selective discipline for a
designated site set** applies the same clauses exactly when the operand is a reference to a
designated site's region.

**Lemma 5, unconditional clearance. Requires M3, M6, M7, M9, and the discipline, unconditional
or selective with $s$ designated.** For every site $s$ and scope $n$ that ends, no dereference
of a reference to $r_n$ occurs after scope $n$ ends.

*Proof.* Lemma 2's induction goes through with the `Escapes` case replaced. An escape with a
reference to $r_n$ among its operands transports a fresh copy, which by depth references no
region, so the case creates no location holding a reference to $r_n$, and the environment only
ever receives copy references, so by M7 and M9 a reply cannot reintroduce one. All other cases,
and the scope-end argument through M6, are as in Lemma 2, no confinement hypothesis being
needed. $\blacksquare$

**Theorem B2. Requires M1 through M9 and the discipline.** In the discipline machine, let $P$
be a well-formed plan reusing any set of sites, each of which satisfies that **consecutive
executions are separated by a boundary of the site's innermost scope or by an epoch boundary**.
Then $P$ is sound, soundness per Definition 5, the baseline being the **same discipline
machine** under the empty plan.

*Proof.* Both regimes run the same machine, so copies occur identically, and by faithfulness a
copy taken in scope $m$ copies the slot's bytes, which equal $v_m$'s construction bytes in both
regimes, the plan not having overwritten them before the site's next execution, which by the
separation hypothesis lies beyond a scope or epoch boundary. Beyond a scope boundary, Lemma 5
empties references to $r_n$, and Lemma 1 as scoped in Definition 9 prevents re-arising. Beyond
an epoch boundary, M5, M2, and M4 make every surviving dereference fail stale in both regimes.
Copy dereferences return identical bytes in both regimes by the induction along the copy chain,
faithfulness and stability at each link. M8 removes address sensitivity. Hence identical
observations. $\blacksquare$

**Remark, what B2 is and is not.** B2 licenses reuse **within** the discipline machine. The
discipline machine is not the present Keleusma machine, and relating the two is an adoption
decision with the obligations of Appendix C, not a theorem of this document. Sites violating
the separation hypothesis, for instance a site in a function called twice within one scope, are
excluded and recorded in Section 9.

**Corollary B2a, accounting.** Under Theorem B2's hypotheses, and assuming the copy store is
reclaimed at each cycle boundary, per-cycle occupancy is bounded by the reused slots plus
$B_{\mathrm{bump}}$ plus the copy term
$\sup_{\pi, c} \sum_{e \in \mathrm{esc}_c(\pi)} \mathrm{sz}(e)$, where $\mathrm{esc}_c(\pi)$
counts every copying escape within cycle $c$, **re-escapes of copies included**. Without
boundary reclamation the copy term accumulates and no per-cycle bound follows. For a site
escaping on every iteration the copy term matches what reuse saved, so the regime relocates
rather than tightens that site's bound, the gains being uniform reuse with no confinement
analysis, copies into environment-owned storage leaving the machine's bound, and soundness for
a planner that already reuses slots.

**Corollary B2b, the hybrid. Requires M1 through M9.** A well-formed plan reusing confined or
refined-confined sites without copies and designated sites under the selective discipline is
sound. *Proof.* Lemma 4's chain, each added element discharged by Theorem B1, B1r, or B2, the
per-element arguments concerning only references to that element's regions, which the selective
discipline covers for designated elements and Lemmas 2 and 3 cover for confined ones.
$\blacksquare$

## 9. Limits of the general theory

1. **The axioms are obligations, and their standing transfers.** An axiom discharged by a
   producer emission invariant holds the dependent theorems only for that producer's output,
   and every instantiation must list such axioms exhaustively.
2. **Confinement is sufficient, not necessary**, and nothing here decides it. An analysis that
   cannot establish a flow must treat it as escaping.
3. **The return refinement is not proved.** A return landing in the same scope of the same unit
   is believed harmless and unproved. Likewise unproved, reuse for sites whose consecutive
   executions are not separated by a scope or epoch boundary, the twice-called-function shape,
   under any regime.
4. **External callees.** M7 makes retention unknowable, so external calls are escaping or the
   exclusion is a documented integrator obligation, in those words.
5. **The theory applies only where M8 and M9 hold.** A machine exposing addresses to programs,
   or embedding references in values or bytes, is outside every equivalence theorem.
6. **The discipline machine's relation to any present machine is not a theorem here.** Theorem
   B2 compares the discipline machine to itself under two plans.

---

# Part II. The Keleusma instantiation, appendices

The mapping. The machine is the Keleusma virtual machine, allocation instructions are
`NewComposite`, dereference is `resolve`, the epoch advance is `Op::Reset`, a cycle is one
stream cycle of `loop main`, the environment is the host, and external calls are the two
native-call opcodes. **Part I's loops are the genuinely iterating `Op::Loop` scopes, and
dispatch scopes, the `match` lowering among them, are Part I conditionals.** The discriminator,
recorded by the V0.3.X line, is that a genuinely iterating body carries no `Break` targeting
its own exit. Measurements over all `Op::Loop` scopes over-cover the iterating subset, which is
conservative wherever a universally quantified clause is being discharged.

## Appendix A. Axiom instantiation, with measured standing

The runtime evidence is indexed, with per-row provenance and reproduction commands, in
[`docs/decisions/COMPOSITE_REGION_EVIDENCE.md`](../decisions/COMPOSITE_REGION_EVIDENCE.md),
guarded by `tests/proof_evidence_index.rs`. Rows marked read from dispatch are not to be
promoted without execution.

| axiom | instantiation and standing |
|---|---|
| M1 | Operator ruling on the memory model. Bump arena, ephemeral region cleared only at `RESET`, nothing reclaimed within a run. The immutability clause is **confirmed by the V0.2.X line on four independent grounds**. Ground one is executable and pinned **on their line at `a288ae26`**, which postdates this branch's base, so the pin is not in this tree's copy of `tests/composite_escape_routes.rs`. The instruction set carries zero write accessors into a composite, the pin establishing the zero-store half mechanically while the seven-read count is read rather than pinned. Grounds two through four are read from dispatch plus a scan of the public interface. **The clause is scoped to the ephemeral region deliberately and the scoping is load-bearing, the persistent region being mutated in place by the data-slot writes.** Refutation boundary, an `unsafe` native cast, undetectable from the safe interface, and an out-of-band `unsafe` arena rewind, which refutes M1's own reclamation clause directly. |
| M2 | The handle representation and the `Stale` guard are **executed**, `tests/composite_escape_window.rs`, all three tests, including the load-bearing simultaneous-distinct resolution. The overwrite-in-place clause is **inferred, not executed**, nothing in the present machine being able to overwrite in place, the inference resting on the epoch advancing only at `Reset` in the executed traces. |
| M3 | The 66 opcodes are partitioned in `tests/composite_escape_routes.rs` with **totality asserted against the `Op` enum at test time**, and the escaping set is exactly `Yield`, `SetLocal`, `Return`, `CallVerifiedNative`, `CallExternalNative`. `SetLocal` is classified by its worst case, the opcode not distinguishing an inner binding from an outer one. The source surface is narrower, by grammar-and-AST enumeration there is **no local assignment in the language**, exactly two assignment nodes existing, both targeting data, and the retracted `let mut` illustration is recorded in the audit record. The caveat stands, this is a source-form statement, false at the bytecode level, where compiler-allocated slots are written every iteration and B1r's liveness clause is the instrument admitting that ordinary case. Per-row verdicts are **analysis, not proof**, the `Yield` row and both `CopiesOut` rows executed, the native rows a labeled trust boundary, and the rest read from dispatch. **The `Break` and `BreakIf` rows are corrected-pending on the V0.2.X line**, their `NoRegion` classification overstating, the opcodes transferring control with the whole operand stack, eighteen dispatch scopes demonstrably carrying arm values across a `Break`, and the correction commit is to be recorded here. The boxed construction path aliases and is outside this instantiation, per M9. |
| M4 | **The count clause is structurally enforced, measured by the V0.2.X line 2026-08-24.** `verify()` rejects a module with the `Reset` removed, a second appended, or an extra mid-body, each refusal naming the count. The position clause is **dead-code-true rather than enforced**, a single `Reset` with trailing ops being accepted while the interpreter's reset of the instruction pointer makes the trailing ops unreachable. An earlier audit concern that M4 was emission-only is refuted by this measurement. |
| M5 | **Confirmed by the V0.2.X line, two-part.** `Op::Reset` clears the current frame's locals and truncates its operand stack, read from dispatch, and a caller frame beneath a nested stream holds stale references but is never resumed, because stream chunks emit no `Return`, executed over five shapes in their `tests/stream_never_returns.rs` at `435a8f6d`, a **code-generation invariant**. Every ephemeral composite-body read is epoch-checked at resolve time, read from dispatch. |
| M6 | (a) **Confirmed.** `TypedError::LoopNotNeutral` compares the entire abstract stack at the back edge, shapes not identities, read from dispatch, with the equal-height shape witness subsumed since `92e5696a` by the loop floor. (b) **NOT enforced, and emission-true for iterating loops, measured 2026-08-24.** Break edges are joined only with each other and never compared against the entry stack, the joined state becoming the post-loop state, and the behavior is load-bearing for dispatch, eighteen of two hundred forty-two dispatch scopes carrying arm values, while **zero of twenty-three genuinely iterating scopes carry a break edge differing from entry**. Clause (b) therefore joins the producer-invariant list. A floor-style enforcement would need the iteration discriminator as a first-class verifier notion, assessed by the V0.2.X line as a real design cost and their operator's call. (c) Read from dispatch with call termination from totality **for terminating callees**, and the never-returning nested stream is reconciled by the axiom's own vacuity clause, a scope containing such a call never ends. (d) **Structurally enforced** since `92e5696a`, `TypedError::LoopFloorBreach` before every operand-consuming instruction with per-nesting floors, the gap pins inverted rather than deleted, and the empty-entry coincidence explaining the zero-rejection cost. |
| M7 | Trust assumption, not a theorem. The host holds and may return handles it received and does not fabricate or alter them. Native retention is unknowable, hence the native rows' classification. |
| M8 | **Discharged 2026-08-24 by the V0.2.X line, the risky route executed.** Nameable composite equality never reaches `CmpEq` with composite operands, the compiler expanding it field-wise, verified at the op level, and the executed discriminator, three distinct allocations with two content-equal, measured content-derived equality where an address-derived one would answer differently, allocation distinctness checked so a folded pair could not vacuate the test. The direct path is closed rather than unused, two flat composites reaching `CmpEq` fault, read from dispatch, `FlatComposite`'s own equality compares lengths only, and strings compare by content, both read from dispatch. `Len` derives from the handle's element count and the shape tests from a type name, read from dispatch, neither touching an address, and the axiom is phrased over the **address component** precisely so metadata-derived results are not letter-counterexamples, both regimes agreeing on metadata. The executed discriminator is **to be pinned on their line**, being the fact that would silently invert if composite equality were ever optimized to a handle compare, and the pin commit is to be recorded here. |
| M9 | Flat composites hold bytes only, nested children inlined, executed in the `CopiesOut` evidence, and no value embeds a handle except the boxed path, which **does** alias and is excluded. The instantiation therefore covers modules whose composites are transitively scalar, and that boundary is part of every theorem's scope here. Byte reconstitution into handles is excluded by M3's fabrication ground. |

**The scoping paragraph, exhaustive.** The producer emission invariants are, first, streams
never return, M5's second half, pinned and re-run every build, and second, iterating loops emit
no value-carrying `Break`, M6(b), measured at zero of twenty-three and not enforced. M4's count
and M6(d) are structurally enforced, M6(a) is enforced with the floor, and everything else is
executed, read, ruled, or trust as labeled. Therefore Theorems A2, A3, B1, B1r, B2, and the
corollaries hold, in this instantiation, for **reference-compiled modules whose composites are
transitively scalar**, with the per-row analysis standing of M3 as the residual risk Appendix B
records and the M8 discriminator's pin pending. They do not hold for arbitrary bytecode that
merely passes `verify()`.

## Appendix B. What the instantiation does not establish

1. **The per-opcode verdicts of M3's table.** Totality is mechanical, rows are analysis, and a
   wrong safe row of any class, not only `CopiesOut`, defeats confinement, so the executed
   subset covers two of the many soundness-critical safe rows. The table is the place to argue,
   and it has now twice narrowed a row while totality held.
2. **Standing guarantees for the two emission invariants**, streams never return and no
   value-carrying `Break` from iterating loops. A code-generator change or hand-written
   bytecode with either shape defeats the reuse theorems without failing verification.
3. **The M8 discriminator's pin**, pending on the V0.2.X line, the discharge itself recorded in
   the row with the executed route executed and the rest read from dispatch.
4. **A stale internal handle faults rather than lying**, `InvalidBytecode` at the epoch check,
   no live route found, unreachability not claimed, defense in depth only.
5. **The loop-dominated direction of the planner gap**, obligation Section 6.2, unmeasured,
   the V0.3.X line's offer standing.
6. **Anything about the native backend's lowering.**
7. **The counterexample is bracketed, not contradicted.** The obligation's Section 4.1 program
   yields its loop-body composite, the site is unconfined, and the backend's unconditional
   reuse remains unsound on such sites.
8. **The embedder obligations**, native retention, `unsafe` mutation of resolved slices,
   out-of-band arena rewinds, and handle-address comparison, all stated in those words.
9. **Static checkability is guidance.** Confinement, deadness, and designation are dataflow
   properties of the kind the typed verifier computes, and unestablished flows are escaping.

## Appendix C. B2 in Keleusma, and instruction-set remedies

**Theorem B2 is proved for the discipline machine, and the discipline does not exist in
Keleusma today.** No escape route copies. B2 is therefore a **proved specification for a
change**, and adoption carries these obligations, seven of them. First, the copy itself at
every escape of a composite, **unconditionally at `SetLocal`** under the proved discipline, a
copy conditioned on slot lifetime being the selective optimization, which requires the
deadness analysis and is licensed only through Corollary B2b's hybrid. Second, worst-case
execution time, a copy at every escaping execution entering the cost model before adoption.
Third, copy-store provisioning per Corollary B2a, including boundary reclamation, without which
no per-cycle bound follows, and the source-immutable destination-stable asymmetry, the
persistent region being mutated in place so a copy store there must avoid every data slot.
Fourth, epoch stamping of copy handles behind `resolve`. Fifth, handle-address opacity as an
embedder obligation. Sixth, the honest accounting, an always-escaping site relocates rather
than tightens, the hybrid dominating both pure regimes. Seventh, the boxed path, outside M9,
needs its own deep-copy treatment before anything here covers it.

**Adoption is unruled in either direction as of 2026-08-24**, the V0.2.X operator stating they
do not yet know enough to rule, so the specification must not be read as declined.

**Instruction-set position.** Unchanged. No new opcode is required, the discipline having
lowerings through existing constructs or through the escaping opcodes' semantics, a dedicated
copy opcode requiring the strong justification the operator demands, none arising here, and the
classification test pinning the count at 66.

## Appendix D. Change control

| consequence of adoption | surface | owner | decision |
|---|---|---|---|
| loop accounting stops multiplying confined sites | `src/verify.rs:1079` | V0.2.X line | **operator, still unruled as of 2026-08-24**, it lowers a published worst-case-memory-usage figure, and commissioning the analysis does not authorize adopting its result |
| the confinement predicate, Definitions 6 through 8 with deadness | the shared crate under `src/`, one predicate, two consumers | V0.2.X line | **commissioned 2026-08-24**, useful-and-sound standard, unestablished flows escaping |
| branch maximum | `src/verify.rs:992` | V0.2.X line | already implemented, Theorem A1 with Corollary A1s justifies it under Hypothesis H, which the capped loop forms discharge |
| backend stops reusing slots of unconfined sites | native backend planner | V0.3.X line | required for soundness independent of this proof |
| backend may overlap exclusive arms | native backend planner | V0.3.X line | licensed by Theorems A2 and A3 within Appendix A's scoping, conditional on the M8 row |
| verifier floors pops at loop entry | `src/verify_typed.rs` | V0.2.X line | **implemented, their `92e5696a`**, at zero measured rejections |
| verifier compares break edges against loop entry | `src/verify_typed.rs` | V0.2.X line | **assessed and not proposed on present evidence**, enforcement must distinguish scope kinds, eighteen dispatch scopes legitimately differing, their operator's call |

This document's conclusions authorize none of these changes.

## Appendix E. Provenance

The obligation was read at `v0.3.0` commit `a49555bb` and corrected on its line at `d5b706e8`
and `c3ff3c06`. The evidence index and its guard are on this lineage. The V0.2.X line's
confirmations and measurements of 2026-08-23, M5 two-part, M6 clauses with the unfavorable
then structurally closed (d), M1's four grounds, and the pins at `435a8f6d`, `92e5696a`, and
`a288ae26` with their differing standings, invariant pins re-running every build against gap
pins inverted on closure, are recorded across the rows above, together with the corpus
extension to 26 modules with the first three iterating-loop composite subjects, entry stacks
inferred empty rather than measured. The merge sequence is ruled, this line into V0.2.X with
V0.3.X rebasing, authorized on the V0.2.X side, awaiting this line's operator. B2 adoption and
the accounting change remain explicitly unruled.

**The audit and this revision, 2026-08-24.** Five independent contexts audited the document at
`de8b3f68` adversarially, per the operator's direction, and the findings, verdicts, failed
attacks, and post-audit measurements are recorded in
[`AUDIT_2026-08-24.md`](./AUDIT_2026-08-24.md). Every verified finding is repaired in this
revision, which renumbered the definitions cleanly, made references provenance-based, defined
operands, locations, scopes, and plan well-formedness, added the address-opacity and
flat-values axioms M8 and M9, restated M4 and M6 with their measured standings, corrected the
hypotheses of Theorems A2 and B2, anchored the refined form at the innermost loop, performed
the static-lift induction as Corollary A1s under Hypothesis H, replaced the asserted
composition sentences with Lemma 4 and its proof, replaced the false plan inequality with
Theorem C's footprint and occupancy claim, defined the selective discipline for Corollary B2b,
and rewrote the scoping paragraph to list the producer invariants exhaustively. M8 was
discharged the same day by the V0.2.X line with the risky route executed. The pending items are
the M8 discriminator's pin and the Break row correction commit, both promised, and rows marked
read or inferred keep those labels under the standing rule that nothing is promoted without
execution.
