# Region Slot Reuse Under Escape Confinement

> **Navigation**: [Documentation Root](../README.md)

> **STATUS.** This is the fourth revision, following three adversarial audit rounds recorded in
> [`AUDIT_2026-08-24.md`](./AUDIT_2026-08-24.md),
> [`AUDIT_2026-08-24_ROUND2.md`](./AUDIT_2026-08-24_ROUND2.md), and
> [`AUDIT_2026-08-26_ROUND3.md`](./AUDIT_2026-08-26_ROUND3.md), with every verified finding of
> all rounds repaired here. Part I is a general theory over an abstract machine, proved from
> stated axioms. Part II instantiates the axioms for Keleusma with measured standing per row.
> Theorem A1 is unconditional. Theorem A2, Corollary A3, Theorems B1 and B1r, and the
> composition results without designated elements apply, in the instantiation, to
> reference-compiled modules whose composites are transitively scalar, under the producer
> invariants listed exhaustively in Appendix A's scoping paragraph. Theorem B2 and the
> composition of designated elements are proved for the escape-copy discipline machine, which
> does not exist in Keleusma, a proved specification only. The merge-readiness assessment under
> the operator's decision rule is recorded in the round-three audit record and the session.

This document discharges part of the obligation stated in `docs/proofs/COMPOSITE_REGION_REUSE.md`
on the `v0.3.0` line. The obligation was read at commit `a49555bb` and has since been corrected
on its own line at `d5b706e8` and `c3ff3c06`, both verified reachable, so the pin names the
state this work was drafted against. Part I is self-contained mathematics whose proofs cite no
project artifact. Everything project-specific lives in the appendices.

---

# Part I. General theory

## 1. Setting

An abstract machine executes programs over a fixed finite instruction set with structured
control flow. Machine state comprises a stack of frames, each holding an operand stack and
local slots, an **ephemeral region** stamped with an **epoch** counter, optional persistent
storage, and an external **environment** that the machine can call out to and yield values to,
receiving a reply value on each resume after a yield. The machine is **deterministic given the
environment's behavior**. Conditionals have finitely many arms, execute exactly one arm per
dynamic instance, and branch on a previously computed scalar, their scrutinee evaluation
belonging to the enclosing sequence. **Loops are iterating constructs** with a body entry, one
or more back edges, and one or more exit edges, exit tests lying within the body so that an
iteration is counted per body entry, and a dispatch construct that executes a body once and
exits is a conditional in this theory, whatever its lowering. Calls push frames, a call may or
may not return, **an execution reads and writes operand-stack entries of its own frame only**,
and **values cross frames solely through call argument passing and return results**, unwinding
transporting nothing.

The question the theory answers is when a static planner may assign one fixed memory slot to an
allocation site whose dynamic executions would otherwise each receive a fresh bump allocation,
and what a worst-case memory bound may consequently count.

## 2. Definitions

**Definition 1, sites, paths, and per-cycle memory.** Let $\mathcal{S}$ be the static
allocation sites of a program unit and $\mathrm{sz}(s)$ the byte size site $s$ allocates. Let
$\Pi$ be the set of execution paths, each partitioned into cycles by Definition 3. For a path
$\pi$ and cycle $c$, $\mathrm{alloc}_c(\pi)$ is the multiset of dynamic site executions within
$c$, callee executions included, and
$M_c(\pi) = \sum_{s \in \mathrm{alloc}_c(\pi)} \mathrm{sz}(s)$. The baseline per-cycle worst
case is $\mathrm{WCMU} = \sup_{\pi, c} M_c(\pi)$, finite when Hypothesis H holds. Because the
ephemeral region is reclaimed at every cycle boundary, $M_c(\pi)$ bounds the baseline machine's
ephemeral occupancy throughout cycle $c$.

**Definition 2, regions, handles, and references, by provenance.** Each dynamic execution of an
allocation instruction creates a **region** $r$, an identity, placed at an address interval in
the ephemeral region, in the bump range under the baseline or in the site's slot under a plan.
The handle produced by that execution **refers to $r$** and carries the epoch current at its
creation. A **transport** of a reference is any copy or move of the handle value between
locations, and a **view derivation** produces a handle addressing a sub-interval from an
existing reference, **carrying its source handle's epoch**, so derivation launders no
staleness, and these are the only reference-producing operations besides allocation. A
**reference to $r$** is exactly the handle created by $r$'s allocation or a value transitively
obtained from it by transport or view derivation. A handle whose address interval merely
coincides with $r$'s is not a reference to $r$. A **dereference** of a handle succeeds exactly
when the handle's carried epoch equals the current epoch, returning the bytes then present at
its address interval, and otherwise fails with a staleness error, and every read of a region's
bytes, by any instruction or by the environment, is a dereference.

**Definition 3, cycles, scopes, and entry heights.** A **cycle** is a maximal interval of
execution containing no epoch advance in its interior, the interval from machine start
included. The **scope** of an instruction execution is the current iteration of the innermost
loop **dynamically enclosing the execution**, in its own frame or any ancestor frame of the
cycle, and its enclosing cycle when no loop encloses it, so an execution in a loop-free callee
invoked from a loop body has that loop's current iteration as its scope. An iteration runs from
one execution of the loop's body entry to the corresponding body end, reached at a back edge,
at an exit edge, or by destruction of the loop's frame, which ends every scope anchored at that
frame. The scope of a value is the scope of its allocating execution. A loop's **entry height**
is the operand-stack depth of its frame at the body entry of the current iteration.

**Definition 4, locations and lifetimes.** A **location** is an operand-stack entry, a local
slot of a frame, a cell of persistent storage, or environment storage. An entry's lifetime runs
from its push to its pop, unwinding, or frame destruction, a slot's lifetime is its frame's,
and persistent and environment storage are unbounded. A location **outlives** a scope when its
lifetime extends past the scope's end. A location **holds** a reference when the reference is
its current content, which by M9 can occur only for handle and view values themselves.

**Definition 5, plans, observations, and soundness.** A **plan** designates a subset
$R \subseteq \mathcal{S}$ of reused sites and assigns each a fixed slot inside the ephemeral
region, of at least the site's size, and may form **arm groups**, an arm group being a set of
sites, at most one lying lexically in each distinct arm of one conditional per Definition 6,
sharing one slot of at least the largest member's size, the two-site once-per-cycle case
governed by Theorem A2 and the in-loop case by Corollary A3. A plan's **elements** are its
single sites and its arm groups. A plan is
**well-formed** when its slots are pairwise disjoint and disjoint from the bump range, the
persistent storage, and any copy store. The **baseline** is the empty plan. The
**observations** of a run are, in order, the outcome of every dereference, success with
returned bytes or staleness, by machine or environment, every scalar output, and one
address-erased **receipt event** per value the environment receives, recording the value's kind
and size but not its content. An environment **behavior** is a function from observation
history to **symbolic actions**, presenting the value of a named receipt event, computing
scalars from observed bytes and scalars, or choosing a dereference or resume, so one behavior
applies in both compared regimes with its concrete handles substituted per run. A plan is
**sound** when, for its unit, every path under every environment behavior yields the identical
observation sequence under the plan and under the baseline, sequences compared as prefixes up
to the earlier run's end, a divergence in any prefix, staleness included, refuting soundness,
and the comparison method makes the runs equal in length absent a prior divergence, so the
prefix clause serves definitional totality rather than weakening the guarantee. The abstract
ephemeral region and copy store are unbounded, finite provisioning being Section 7's subject.

**Definition 6, operands, confinement, and arm containment.** The **operands** of an
instruction execution are all values it reads, operand-stack entries and any slot or storage
contents alike. Let $v$ be the value allocated at site $s$ in scope $n$, with region $r_n$. The
value is **confined** when, along every baseline path, no reference to $r_n$ is ever an operand
of an execution of an `Escapes`-classified instruction, and a site is confined when all its
values are. A site is **in an arm** of a conditional when it occurs lexically within that arm's
text, sites of callees invoked from the arm not counting as in it. Confinement over baseline
paths applies to a plan run exactly where the comparison method has established that the run
follows a baseline instruction sequence, which the soundness proofs establish inductively.

## 3. Axioms

Any system claiming the theorems must discharge these, of the machine under every well-formed
plan. Appendix A does so for Keleusma, with standing per row.

| # | axiom |
|---|---|
| M1 | **Bump ephemeral allocation, immutable regions.** Baseline allocation in the ephemeral region is by bump pointer, and under a plan a reused site's allocation writes its slot instead, the slot lying in the ephemeral region and governed like it. The region is reclaimed only at cycle boundaries, nothing else reclaims within a cycle, and a region's bytes are not modified after its construction completes, construction being atomic within the allocation instruction per M3(vi), except by a plan's reallocation of the same slot. |
| M2 | **Epoch-guarded handles, fresh epochs.** A handle created by allocation carries the epoch current at its creation, a derived view carries its source's epoch per Definition 2, dereference is governed by Definition 2, an overwrite in place advances nothing, and epoch values never repeat within a run, so no reference whose chain began in an earlier cycle ever passes the check again. |
| M3 | **Total instruction classification.** The non-allocation instructions are partitioned, totally, into `NoRegion`, `WithinScope`, `CopiesOut`, and `Escapes`, with these semantics. (i) No instruction fabricates a handle, and references to regions originate only at allocation instructions. (ii) A `NoRegion` execution may read reference operands but neither derives nor transports references and produces only scalars or nothing. (iii) A `WithinScope` execution derives or transports references only into locations whose lifetime is contained in its scope. (iv) A `CopiesOut` execution writes referenced bytes, never references, to its destinations. (v) **Exhaustiveness.** Any execution that places a reference into a location outliving its scope belongs to an `Escapes`-classified instruction. (vi) An allocation execution creates a fresh region, copies any composite operands' bytes inline, and pushes exactly one fresh reference onto its own frame's operand stack. Totality must hold and keep holding as the instruction set changes. |
| M4 | **Cycle cadence.** Epoch advances occur only as the final action of a cycle, every cycle that ends ends at one, and advances are distinct per M2. A cycle held open forever never ends and never advances. |
| M5 | **Boundary clearance.** No machine-internal location holds, at a cycle boundary, a reference into the ephemeral region that is later read from that location before the location is overwritten. The environment may retain handles across the boundary. |
| M6 | **Iterating-loop discipline.** (a) Every back edge restores the exact entry operand stack, height and per-slot shape. (b) Every exit edge, early or normal, likewise restores the exact entry stack. (c) A frame created within a scope is destroyed before that scope ends, and a call that never returns leaves its enclosing scope forever open, making scope-end conclusions vacuous for it. (d) No instruction reads or writes operand-stack entries below the innermost enclosing loop's entry height, destruction by unwinding or frame exit counting as neither, so clauses (a), (b), and (d) jointly leave every at-or-below-entry entry bitwise untouched through an iteration. |
| M7 | **Environment trust.** The environment presents only handles it received, unaltered, performs no view derivation or handle arithmetic of its own, reads region bytes only by dereference, and behaves per Definition 5. What an external callee retains is unknown, so external calls are `Escapes`-classified in any instantiation. |
| M8 | **Address opacity.** No instruction's scalar result or control effect depends on the address component of any operand handle, and dereference outcomes depend only on epoch validity and the referenced bytes. |
| M9 | **Flat values.** No machine value other than a handle or view contains a reference, region bytes never encode references, and bytes cannot be reconstituted into a handle. |

Clause M3(v) is the engine of every confinement argument. Wherever a proof shows a destination
outlives the executing scope, the placing execution is `Escapes`-classified in any conforming
machine, so confinement applies to any reference reaching it. Environment storage outlives
every scope, so every transfer of a **reference** to the environment, through yields or
otherwise, is `Escapes`-classified by (v), and cross-frame slot stores do not exist at all, the
Setting's cross-frame channels being arguments and returns only.

## 4. The comparison method, and the lemmas

**The comparison method.** Fix an environment behavior, symbolic per Definition 5, and compare
a plan run with the baseline run of the same unit. While every observation so far agrees, the
behavior's symbolic actions agree and resolve against corresponding holdings, receipt histories
being identical, and by M8 with determinism the machine's control and scalar behavior agree, so
both runs execute the same instruction sequence, the states corresponding location by location
and differing at most in region addresses, stored handle values, and the bump pointer. Under
that correspondence, a value is a reference to the plan run's $r_n$ exactly when its
counterpart is a reference to the baseline run's, so the baseline-path properties of
Definition 6 govern the plan run's prefix. A first observational divergence can therefore only
be a dereference outcome that differs between the regimes, and each soundness proof below shows
no such first divergence exists, which also forces the runs to equal length.

**Lemma 1, provenance closure. Requires M3, M7, M9.** At every moment, the references to a
region $r$ are exactly the transitive transport-and-view closure of the handle pushed by $r$'s
allocation, and the environment holds only references it received from the machine. In the
baseline machine, additionally, no value outside that closure successfully dereferences $r$'s interval
within $r$'s epoch, and under a plan that clause is exactly what the reuse theorems manage
rather than a given.

*Proof.* By Definition 2 references to $r$ are provenance-generated from the allocation's
handle. M3(i) excludes fabrication, M9 excludes reconstitution from bytes and containment in
other values, M3(vi) makes the allocation's push the sole origin, and M7 confines the
environment to received handles with no derivation of its own. In the baseline, distinct
regions occupy disjoint bump intervals within an epoch, so only $r$'s own closure addresses its
interval while $r$'s epoch is current. $\blacksquare$

**Lemma 2, confinement clearance. Requires M3, M6, M7, M9.** Let $v$ be confined, allocated at
site $s$ in a scope $n$ **that is a loop iteration** of loop $L$, and let scope $n$ end. Then
at its end no location holds a reference to $r_n$.

*Proof.* By induction over the instructions executed during scope $n$, with the invariant that
every reference to $r_n$ resides either in an operand-stack entry of $L$'s frame above scope
$n$'s entry height, or in a location of a frame created during scope $n$.

The invariant holds at origination, where M3(vi) pushes the sole reference, per Lemma 1, onto
the allocating frame's stack, in $L$'s frame above the entry height by M6(d), or in a frame
created during the scope.

Preservation is by cases on the executed instruction's class. An `Escapes` execution with a
reference to $r_n$ among its operands, slot and storage contents counting as operands by
Definition 6, is excluded by confinement, so in particular the environment never receives a
reference to $r_n$, transfers to it being `Escapes` by M3(v), and by Lemma 1 with M7 and M9 a
resume reply cannot carry or conceal one. A `WithinScope` execution in $L$'s frame transports
references only into locations with scope-contained lifetime, which by frame locality and
M6(d) are entries above the entry height. A `WithinScope` execution in a frame created during
scope $n$ has, by Definition 3's dynamic enclosure, scope $n$ or an iteration of a loop
nested within it, either way its stack effects are
confined to its own frame by frame locality, its own locations lie in the second invariant
class, and a `Return` result it produces crosses to the calling frame's stack, which is either
$L$'s frame above the entry height, class one by M6(d), or an intermediate frame created
during scope $n$, class two, the Setting's cross-frame channels being arguments and returns
only, so no cross-frame slot store exists to consider. A `CopiesOut` execution writes bytes,
referenceless by M9. A `NoRegion` execution derives and transports nothing. An allocation
consuming a reference to $r_n$ copies bytes inline by M3(vi) and produces a reference to its
own fresh region. Control transfers are classified like every instruction, and unwinding
destroys entries and frames while transporting nothing, per the Setting, removing locations
from both classes.

At the end of scope $n$, reached at a back edge or an exit edge, M6(a) or M6(b) with M6(d)
leaves the stack bitwise at its entry state, so no entry created within the scope survives, and
every frame created during the scope has been destroyed by M6(c). Reached by destruction of
$L$'s frame, whether by a `Return` or by unwinding, the frame's entries and slots die with it,
unwinding transports nothing, and a `Return` result lands in a location outliving scope $n$,
so that `Return` is `Escapes` by M3(v) and confinement excludes a reference to $r_n$ among its
operands. In every case both invariant classes are empty at the scope's end. $\blacksquare$

## 5. Branch theorems

**Theorem A1, per-instance branch bound.** Fix any assignment of a static value
$B(\text{arm})$ to each arm of each conditional such that $B(\text{arm})$ bounds the arm's
dynamic allocation contribution, its own nested constructs included, on every execution of it.
Then, taking conditionals outermost first, replacing each dynamic instance's contribution by
$\max_{\text{arms}} B$ preserves an upper bound on the allocations of any execution interval,
for conditionals of any arity, from structured control flow alone, scrutinee evaluation lying
outside the instance per the Setting.

*Proof.* Each dynamic instance executes exactly one arm, contributing that arm's dynamic sum,
at most its $B$, at most the maximum over arms, and outermost-first replacement books each
allocation once since inner instances' contributions lie inside the outer arm's $B$. Summing
over instances preserves the inequality. $\blacksquare$

**Hypothesis H, static structure.** Every loop of the unit's call closure carries a static
iteration cap dominating the number of body entries per activation, the call graph of the
closure is acyclic with every callee's structure available, the unit is the body of the
outermost cycle construct, executed at most once per cycle with no epoch advance strictly
inside its constructs, and **every allocation and every `Escapes` execution of every cycle,
the startup interval included, occurs within the unit's traversal**. Totality yields termination, not caps, so H is a genuine
hypothesis, discharged in the instantiation as Appendix A records.

**Corollary A1s, the static bound.** Under H, define $B$ over the unit's call-closed structure
by $B(\text{sequence}) = \sum B$, $B(\text{conditional}) = \max_{\text{arms}} B$,
$B(\text{loop}) = \mathrm{cap} \times B(\text{body})$, $B(\text{call}) = B(\text{callee})$, and
$B(\text{site}) = \mathrm{sz}(s)$. Then $B(\text{unit}) \geq M_c(\pi)$ for every cycle of
every path. The definition is parametric in $\mathrm{sz}$, so instantiating some sites at zero
yields the corresponding restricted bound.

*Proof.* Structural induction over the call-closed structure, well-founded by H's acyclicity.
A sequence's contribution is the sum of its parts', a conditional instance is bounded by
Theorem A1 with the inductively obtained arm values, a loop's one activation contributes at
most cap body entries each bounded by $B(\text{body})$, a call contributes its callee's
traversal, bounded inductively, and by H a cycle's allocations all lie within at most one
traversal of the unit. $\blacksquare$

**Theorem A2, arm overlap. Requires M1 through M5, M7, M8, M9.** Let $s_i$ and $s_j$ be sites
in two distinct arms of one conditional, in the lexical sense of Definition 6, and suppose
**the conditional executes at most once per cycle and each of the two sites executes at most
once per cycle**, on every path. A well-formed plan whose sole reuse is the pair sharing one
slot of at least the larger size is sound.

*Proof.* By the comparison method a divergence must be a dereference outcome. Within one
cycle, the conditional instantiates at most once and takes one arm, and by lexical containment
a site executes only when its arm is taken, so at most one of the two sites executes, at most
once, and the slot is written at most once per cycle. Every within-epoch dereference of the
written region's references reads the slot holding exactly that region's construction bytes,
equal to baseline by M1, no second write existing, and by Lemma 1 no reference to the
unwritten site's never-created region exists at all. Across cycles, any surviving reference is
environment-held or sits in an internal location never read before being overwritten, by M5,
and dereferencing requires reading, so every cross-cycle dereference of a survivor fails stale
by M2 with M4 identically in both regimes, while any view derived after the boundary from a
survivor carries the survivor's stale epoch by Definition 2 and M2, so post-boundary
derivation launders nothing and fails identically. No first divergence exists.
$\blacksquare$

## 6. Confined-site reuse

**Theorem B1. Requires M1 through M9.** Let $s$ be a confined site, with **every two
consecutive executions of $s$ separated by a boundary of $s$'s innermost scope or by an epoch
boundary**, and let $P$ be a well-formed plan whose sole reuse is one slot for $s$. Then $P$
is sound. The accounting consequence is not asserted here, being Theorem C's under
Hypothesis H, where $s$ contributes its slot once statically and zero to the bump term.

*Proof.* By the comparison method a divergence must be a dereference outcome, and by
well-formedness with M8 the address differences between the regimes, $s$'s regions and the
shifted bump layout alike, are unobservable except through dereferenced bytes. The slot's
bytes differ from the baseline bytes of $v_n$ only from the start of $s$'s next execution,
which by the separation hypothesis lies beyond a boundary of scope $n$ or beyond an epoch
boundary. In the scope case, scope $n$ is an iteration of $s$'s innermost dynamically
enclosing loop per Definition 3, Lemma 2 empties every location of references to $r_n$ at its
end, and by Lemma 1 none arises later, the environment never having received one. When no
loop dynamically encloses $s$, the separation hypothesis leaves only the epoch case. In the
epoch case, and for every cross-epoch dereference generally, M5 leaves survivors
environment-held or unread before overwrite, dereferencing requires reading, and by M2 with
M4 every dereference through a survivor or through any post-boundary derivation from one
fails stale in both regimes, Definition 2 propagating the stale epoch through derivation.
Dereferences of the current region's references occur while the slot holds exactly its
construction bytes, equal to baseline by M1. No first divergence exists. $\blacksquare$

### The refined form, local stores to boundary-dead slots

**Definition 7, deadness at loop boundaries.** Let $L$ be the innermost loop dynamically
enclosing site $s$. A local-slot **index** $\ell$ of the frame executing $L$ is **dead at the
boundaries of $L$** when, in every activation of $L$, at every back edge and every exit edge,
every path onward either never reads $\ell$ or writes $\ell$ before its first read of it,
reads in the sense of Definition 6, with frame destruction ending the question for that
activation.

**Definition 8, refined confinement.** A **local store** is an `Escapes` execution whose
entire effect on references is to write its single reference operand into one named slot of
its own frame. Let $D$ be a set of slot indices dead at the boundaries of $L$. The site $s$ is
**confined relative to $D$** when, in every scope and along every baseline path, no reference
to that scope's region is ever an operand of an `Escapes` execution other than a local store
targeting a slot in $D$.

**Lemma 3, refined clearance. Requires M3, M6, M7, M9.** Let $s$ be confined relative to $D$,
with $L$ its innermost dynamically enclosing loop, and let scope $n$ end. Then no dereference
of a reference to $r_n$ occurs after scope $n$ ends.

*Proof.* Extend Lemma 2's invariant with a third class, the $D$-slots of $L$'s activation.
The excused store places the reference in a $D$-slot and nowhere else, by Definition 8's
atomicity, a load of a $D$-slot during scope $n$ pushes a copy above the entry height, and
every other case is as in Lemma 2. At scope $n$'s end at a back edge or exit edge of $L$, the
first two classes empty as in Lemma 2, so survivors sit in $D$-slots, and by Definition 7
every path onward writes each such slot before reading it, reads counting all operand-taking
access, so the stale content is never an operand of anything, never dereferenced, and never
copied. At a scope end by destruction of $L$'s frame, the $D$-slots die with the frame,
unwinding transports nothing, and a destroying `Return`'s reference operands are unexcused
`Escapes` exactly as in Lemma 2's `Return` case. $\blacksquare$

**Theorem B1r. Requires M1 through M9.** With $s$, the separation hypothesis, and the plan as
in Theorem B1, and $s$ confined relative to some $D$, the conclusion of Theorem B1 holds
unchanged.

*Proof.* Theorem B1's scope case needed that no dereference of a reference to $r_n$ occurs
after scope $n$'s boundary, which Lemma 3 delivers, the environment premise holding because
yields, frame-crossing returns, and external calls remain unexcused. A $D$-slot residue
present at a cycle boundary is covered by M5 as an assumed axiom, a machine and program pair
violating M5 lying outside every theorem here, and Definition 7 governs the loop boundaries
within cycles. The epoch case and byte-equality arguments are as in Theorem B1.
$\blacksquare$

**Corollary A3, arm overlap inside loops. Requires M1 through M9.** Let a conditional sit
inside a loop and instantiate at most once per scope of that loop, let each arm contain, in
the lexical sense of Definition 6, **at most one** site among those sharing the slot, each
such site confined or confined relative to a suitable $D$ and satisfying the separation
hypothesis, and let the shared slot's size be at least the maximum of the sites' sizes, the
plan otherwise well-formed. The plan sharing that one slot across the arms' sites and reusing
it across scopes is sound.

*Proof.* Consecutive writes to the shared slot by one site are separated by a boundary of
that site's innermost scope, by its separation hypothesis, which for a site whose innermost
loop is nested inside its arm means the inner iteration boundaries, and the last such write
is separated from a sibling arm's write by the inner loop's exit edge together with the
enclosing instance hypothesis, at most one arm executing per scope of the enclosing loop with
at most one lexical site per arm, while for a sharing site whose innermost dynamically
enclosing loop is the enclosing loop itself the separating boundary is that loop's own back
edge or exit edge directly. At each separating boundary Lemma 2 empties references to
the written region or Lemma 3 bars their dereference, so by the comparison method no
within-epoch dereference sees overwritten bytes, the slot's size covering every site's region
in full. The cross-cycle case is as in Theorem A2, post-boundary derivations included.
$\blacksquare$

**Remark, what the refinement admits.** A binding declared inside a loop body compiles to a
slot written at its declaration before any read in each scope, and its slot index is dead at
the boundaries whenever slot assignment preserves write-before-read for that index globally,
a per-slot discipline an analysis must check, definite initialization of source bindings
alone not sufficing. Membership of $D$ and refined confinement are then static dataflow
properties, so the gate on Theorem B1r is an analysis.

## 7. Composition and the plan bound

**Lemma 4, composition. Requires M1 through M9, and the selective discipline for any
designated elements.** Let $P$ be a well-formed plan whose elements each satisfy the
**element-level** hypotheses of Theorem B1, Theorem B1r, Theorem A2, Corollary A3, or Theorem
B2, that is, confinement or refined confinement with separation, the A2 or A3 arm-group and
instance hypotheses with sizing, or designation with separation under the selective discipline, the
sole-reuse clauses of the cited statements being plan-level and replaced here by $P$'s
well-formedness. Then $P$ is sound. This statement uses Definition 9, Lemma 5, and Theorem B2
of Section 8, forward references deliberate.

*Proof.* Order the elements and let $P_0, \dots, P_k$ chain from the baseline to $P$,
adjacent plans differing at one element, and discharge the links **in order from the
baseline**. Inductively, $P_i$ is sound, so every $P_i$ run follows a baseline instruction
sequence with identical observations, and the baseline-path hypotheses, confinement,
refined confinement, separation, and the instance counts, therefore govern $P_i$'s runs. For
the link from $P_i$ to $P_{i+1}$, run the comparison method between them. Up to a first
divergence both runs execute the same instruction sequence, every element other than the
differing one is identically used, slots disjoint by well-formedness, and the differing
element's dereference analysis is its theorem's, which uses only the clearance lemmas over
the common instruction sequence, the separation or instance hypotheses, byte equality at the
element's own slot, cross-epoch staleness with Definition 2's epoch propagation, and, for a
designated element, Theorem B2's copy-chain analysis, copy points coinciding on the common
sequence. So no link has a first divergence, and identity of observation sequences composes
transitively along the chain to relate $P$ to the baseline. $\blacksquare$

**Theorem C, the plan bound. Requires M1 through M9 and Hypothesis H.** Let $P$ be as in
Lemma 4, without designated elements, define **occupancy** at a moment as the bytes of $P$'s
slots plus the bytes of the current cycle's bump allocations, and define

$$
\mathrm{footprint}(P) \;=\; \sum_{e \in E(P)} \mathrm{slot}(e) \;+\; B_{\mathrm{bump}}
$$

over $P$'s **elements** $E(P)$, an arm group contributing its one shared slot once, with
$B_{\mathrm{bump}}$ the Corollary A1s bound instantiated with reused sites' sizes at zero.
Then the machine under $P$ never occupies more than $\mathrm{footprint}(P)$ within any cycle.

*Proof.* The slots contribute $\sum \mathrm{slot}(e)$ statically by well-formedness. By Lemma
4, discharged in order, the plan run follows a baseline instruction sequence, and by H every
allocation of the cycle lies within the unit's traversal, so the cycle's bump allocations are
exactly the non-reused executions of that traversal, bounded by $B_{\mathrm{bump}}$ through
Corollary A1s's parametric form, and M1 reclaims the bump range at the boundary, bump usage
within a cycle being monotone. $\blacksquare$

**Remark, comparing the two bounds honestly.** Both $\mathrm{footprint}(P)$ and the all-bump
$B(\text{unit})$ are valid bounds for their machines, and neither dominates. For a reused
loop site outside every conditional, $\mathrm{sz}(s) \leq \mathrm{cap} \cdot \mathrm{sz}(s)$
pulls the footprint down. For a reused site inside a conditional arm, the slot is provisioned
statically outside the arm maximum, and extracting a term from under a maximum can exceed the
maximum, so the footprint can be larger than $B(\text{unit})$ on branch-dominated shapes. A
planner should compute both figures and adopt reuse per site only where it helps, which
per-site greedy selection over Lemma 4's element set achieves. Reuse of a site outside the
theorems' hypotheses is not licensed by this document, and no necessity claim is made.

## 8. Universal reuse under an escape-copy discipline

**Definition 9, escape-copy discipline, unconditional and selective.** A machine satisfies
the **escape-copy discipline** when every `Escapes` execution whose operand is a reference to
a region transports a fresh copy in its place, the source read being a **dereference**, so a
stale source faults identically to any other stale dereference, and the copied extent being
the referenced interval of the operand, a view's extent bounded by its region's size. The
clauses are, first, faithfulness, the copy's bytes equal the dereferenced bytes at the
instant of the copy, second, stability, a copy's bytes are unmodified while any reference to
it is held anywhere, environment included, within its epoch, the copy store disjoint from
every slot and the bump range, third, epoch stamping, a copy handle carries its creation
epoch and dereference is governed by M2, fourth, recursion, an escape of a copy produces a
further copy, and fifth, depth, a copy contains no reference, which M9 makes automatic.
References to copies originate at the copying escapes, M3(i) being read as scoped to region
references, and Lemma 1 continues to govern regions. The **selective discipline for a
designated element** applies the same clauses exactly when the operand is a reference to a
designated site's region.

**Lemma 5, unconditional clearance. Requires M2 through M7, M9, and the discipline,
unconditional or selective with $s$ designated.** For every site $s$ and scope $n$ that ends,
at the end of scope $n$ no location holds a reference to $r_n$, when scope $n$ is a loop
iteration, and after the cycle's boundary no dereference of a reference to $r_n$ succeeds,
when scope $n$ is a cycle.

*Proof.* For a loop-iteration scope, Lemma 2's induction goes through with its `Escapes` case
replaced. An escape with a reference to $r_n$ among its operands transports a fresh copy,
which by depth holds no reference, so no location gains a reference to $r_n$ through it, and
the environment never receives a reference to $r_n$, every transfer of one being a copying
escape, so by M7 with M9 and Lemma 1 a reply cannot reintroduce one, whatever raw references
to non-designated regions it may hold under the selective form. The scope-end `Return` case
is carried by the same substitution, the destroying `Return` being a copying escape whose
transported result is a copy rather than the reference, and unwinding transports nothing.
All other cases and the scope-end argument are as in Lemma 2, no confinement being needed.
For a cycle scope, machine-internal survivors are unread before overwrite by M5, an
environment-held survivor's direct dereference fails stale by M2 with M4, and any view
derived after the boundary from either carries the stale epoch by Definition 2 and fails
identically, so no route to a successful post-boundary dereference exists. $\blacksquare$

**Theorem B2. Requires M1 through M9 and the discipline.** In the discipline machine, let $P$
be a well-formed plan reusing any set of sites, each satisfying the **separation
hypothesis**, consecutive executions divided by an innermost-scope boundary or an epoch
boundary. Then $P$ is sound, the baseline being the **same discipline machine** under the
empty plan.

*Proof.* By the comparison method, both runs execute the same instructions up to a first
divergence, so copies occur at the same points, and a divergence must be a dereference
outcome. A dereference of a site region's reference occurs, by the separation hypothesis
with Lemma 5's loop clause, only in the region's own scope, where the slot holds its
construction bytes, equal to baseline by M1, or beyond an epoch boundary, where Lemma 5's
cycle clause fails it stale in both regimes, derivations included. A dereference of a copy
returns, by faithfulness and stability inducted along the recursion chain within each epoch,
the originally escaped construction bytes, identical in both regimes, and a chain cannot
cross an epoch boundary, its extending source read being a dereference that fails stale
identically. Hence no first divergence. $\blacksquare$

**Remark.** B2 licenses reuse within the discipline machine. The discipline machine is not
the present Keleusma machine, and relating the two is an adoption decision with Appendix C's
obligations, not a theorem here.

**Corollary B2a, accounting. Requires M1 through M9, the discipline, and Hypothesis H.**
Under Theorem B2's hypotheses, with the copy store reclaimed at each cycle boundary,
per-cycle occupancy, the slots plus the cycle's bump allocations plus the cycle's live
copies, is bounded by $\sum_{e} \mathrm{slot}(e) + B_{\mathrm{bump}} + B_{\mathrm{copy}}$,
where $B_{\mathrm{copy}}$ is the Corollary A1s style bound over the unit in which each
`Escapes` execution of a reference contributes a **static per-occurrence bound, the maximum
site size over the sites its operand can reference**, which an analysis must supply and which
the unit's largest site size coarsely bounds, covering every transported extent by
Definition 9, re-escapes of copies included as their own occurrences.

*Proof.* As Theorem C for the slot and bump terms, Theorem B2's comparison-method argument
giving the common instruction sequence and H confining allocation and escapes to the unit's
traversal, so every copying occurrence is censused. Each copying escape of the cycle
contributes at most the site size of the region its operand references, hence at most its
occurrence's static bound, a view's extent being bounded by its region's size per
Definition 9, the occurrences are censused by the same structural recursion as
Corollary A1s with each escape occurrence contributing its static bound, and the copy store
is reclaimed at the boundary by assumption, its usage within a cycle monotone. Without
boundary reclamation the copy term accumulates and no per-cycle bound follows. $\blacksquare$

For a site escaping on every iteration under cap $k$, reuse saves $(k{-}1)\,\mathrm{sz}(s)$
in the bump term while the copies cost $k\,\mathrm{sz}(s)$, so the regime relocates that
site's bound and exceeds the saving by one slot size, the gains being uniform reuse with no
confinement analysis, copies into environment-owned storage leaving the machine's bound, and
soundness for a planner that already reuses slots.

**Corollary B2b, the hybrid. Requires M1 through M9 and the selective discipline for the
designated elements.** A well-formed plan whose confined or refined-confined elements each
satisfy the hypotheses of Theorem B1 or B1r, and whose designated elements each satisfy the
separation hypothesis under the selective discipline for them, is sound. *Proof.* This is
Lemma 4 restricted to three of its four element kinds, the A2 route unused, its ordered
induction and per-link arguments applying verbatim, Lemma 5 applying over the common
instruction sequences the induction provides.
$\blacksquare$

## 9. Limits of the general theory

1. **The axioms are obligations, and their standing transfers.** An axiom discharged by a
   producer emission invariant holds the dependent theorems only for that producer's output,
   and every instantiation must list such axioms exhaustively.
2. **Confinement is sufficient, not necessary**, and nothing here decides it, an analysis
   treating unestablished flows as escaping.
3. **The return refinement is not proved**, a return landing in the same scope of the same
   unit being believed harmless and unproved, and sites violating the separation hypothesis,
   the twice-called-in-one-scope shape, are outside every reuse theorem here, by hypothesis
   rather than by silence.
4. **External callees.** M7 makes retention unknowable, so external calls are escaping or the
   exclusion is a documented integrator obligation, in those words.
5. **The theory applies only where M8 and M9 hold**, and only to machines satisfying the
   Setting's frame locality and cross-frame channel restriction.
6. **The discipline machine's relation to any present machine is not a theorem here.**
7. **Hypothesis H carries the static structure**, caps over the call closure, acyclicity, the
   unit-per-cycle alignment, and the confinement of all allocation to the unit's traversal,
   and every accounting result assumes it.
8. **The comparison method's correspondence is argued at the level of the stated invariant**,
   location-by-location with address and handle-value differences, not as a fully formal
   bisimulation, which mechanization would supply.

---

# Part II. The Keleusma instantiation, appendices

The mapping. The machine is the Keleusma virtual machine, allocation instructions are
`NewComposite`, dereference is `resolve`, the epoch advance is `Op::Reset`, a cycle is one
stream cycle of `loop main`, whose body is the unit of Hypothesis H, the environment is the
host, and external calls are the two native-call opcodes. **Part I's loops are the genuinely
iterating `Op::Loop` scopes, and dispatch scopes, the `match` lowering among them, are Part I
conditionals.** The discriminator is that a genuinely iterating body carries no `Break`
targeting its own exit, its known imperfection recorded in the M6 row together with the
grammar fact that closes the question it threatened. Measurements over all `Op::Loop` scopes
over-cover the iterating subset, conservative wherever a universal clause is discharged.

## Appendix A. Axiom instantiation, with measured standing

The runtime evidence is indexed in
[`docs/decisions/COMPOSITE_REGION_EVIDENCE.md`](../decisions/COMPOSITE_REGION_EVIDENCE.md),
guarded by `tests/proof_evidence_index.rs`. Rows marked read from dispatch are not promoted
without execution.

| axiom | instantiation and standing |
|---|---|
| M1 | Operator ruling on the memory model. Bump arena, ephemeral region cleared only at `RESET`, nothing reclaimed within a run. The immutability clause is **confirmed by the V0.2.X line on four grounds**, ground one executable and pinned **on their line at `a288ae26`**, postdating this branch's base and so absent from this tree's copy of `tests/composite_escape_routes.rs`, the pin establishing zero write accessors mechanically while the seven-read count is read. Grounds two through four are read from dispatch plus a public-interface scan. **The clause is scoped to the ephemeral region and the scoping is load-bearing, the persistent region being mutated in place by data-slot writes.** Refutation boundary, an `unsafe` native cast, and an out-of-band `unsafe` arena rewind, which refutes this axiom's reclamation clause directly. Slot residency under a plan is a property of the hypothetical plan machine, no plan existing in the shipped runtime. |
| M2 | Handle representation and the `Stale` guard **executed**, `tests/composite_escape_window.rs`, all three tests. The overwrite-in-place clause is **inferred**, nothing in the present machine overwriting in place. Epoch freshness is **read**, the arena epoch being a monotone counter, with fixed-width wraparound an integrator boundary in Appendix B. **The view-epoch clause is read from dispatch**, `nested_view` deriving through `handle.get(arena)`, an epoch-checked access, so a stale parent faults at derivation and no fresh-epoch view of stale data arises, which discharges the clause in the faulting form Definition 2 permits. |
| M3 | The 66 opcodes are partitioned in `tests/composite_escape_routes.rs`, **totality asserted against the `Op` enum at test time**, escaping set exactly `Yield`, `SetLocal`, `Return`, `CallVerifiedNative`, `CallExternalNative`. `SetLocal` is classified by worst case, and it matches Definition 8's local-store semantics, writing one slot of its own frame and placing the reference nowhere else, read from dispatch. The source surface has **no local assignment at all**, by grammar-and-AST enumeration, exactly two assignment nodes existing, both targeting data, with the retracted `let mut` illustration recorded at the obligation line's `c3ff3c06`, and the caveat standing that this is a source-form statement, false at the bytecode level, where compiler-allocated slots are written every iteration and Definition 7's deadness condition, which Theorem B1r imports, admits that ordinary case. Per-row verdicts are **analysis, not proof**. The `Yield` row and **all three** `CopiesOut` rows are executed, `SetData`-shaped writes, `NewComposite` nesting, and `SetDataIndexed`, a composite written into an indexed data slot inside a loop reading back correctly across two resets, pinned as `a_composite_written_to_an_indexed_data_slot_is_copied_not_aliased` in their `f90fe688`. The native rows are a labeled trust boundary, the rest read from dispatch. **The `Break` and `BreakIf` rows are reclassified `WithinIteration` in their `f90fe688`**, with the reason that they end the scope, and the dispatch-break behavior is itself pinned there as `a_dispatch_break_may_carry_a_value_past_the_loop_entry_height`, so a future entry-comparison check fails with a message naming what it breaks. The boxed path aliases and is outside this instantiation per M9. |
| M4 | **The count clause is structurally enforced, measured by the V0.2.X line**, `verify()` rejecting a removed, duplicated, or extra mid-body `Reset`, corroborated in this tree's verifier. Position is dead-code-true rather than enforced. Freshness is with M2's row. |
| M5 | **Confirmed by the V0.2.X line, two-part.** `Op::Reset` clears the current frame's locals and truncates its operand stack, read from dispatch, and a caller frame beneath a nested stream is never resumed because stream chunks emit no `Return`, executed over five shapes in their `tests/stream_never_returns.rs` at `435a8f6d`, a **code-generation invariant**. The axiom's write-before-read exemption is discharged trivially by the clearing. Ephemeral composite reads are epoch-checked at resolve time, read from dispatch. |
| M6 | (a) **Confirmed**, `TypedError::LoopNotNeutral` comparing the entire abstract stack at back edges, shapes not identities, read from dispatch. (b) **NOT enforced, and true by grammar for the shape that matters.** Break edges are joined only with each other and never compared to entry, load-bearing for dispatch at eighteen of two hundred forty-two. The round-two auditor found the iteration discriminator circular, filing any self-breaking loop as dispatch, and the V0.2.X line confirmed it by construction, so the zero-of-twenty-three undercounts and could not have seen a self-break violation. **The question dissolves in the grammar**, `break` having no expression form, so a `Break` targeting an iterating loop's own exit is value-free by construction, the only value-carrying `Break`s coming from dispatch arms targeting the dispatch scope's own exit. Clause (b) is therefore discharged **by grammar and code generation**, the corrected count deliberately not offered since the line's alternative discriminator over-counted and a second broken heuristic is not a correction. (c) Read from dispatch with call termination from totality for terminating callees, never-returning nested streams reconciled by the axiom's vacuity clause. (d) **Structurally enforced on the V0.2.X line at `92e5696a`**, `TypedError::LoopFloorBreach` with per-nesting floors, **not present in this tree's copy of the verifier**, the guarantee existing after the ruled merge, with the destruction exemption matching unwinding semantics, read. Frame locality and the cross-frame channel restriction are read from the dispatch's frame model, no opcode addressing another frame's stack. |
| M7 | Trust assumption, not a theorem, with native retention unknowable. The no-derivation clause is an embedder obligation beside the others in Appendix B. |
| M8 | **Measured by the V0.2.X line, both comparison families covered, the discriminator pinned.** Nameable composite equality expands field-wise, verified at the op level, the executed three-allocation discriminator measuring content-derived equality with distinctness checked, the direct composite `CmpEq` faulting, `FlatComposite` equality comparing lengths, strings comparing by content, and `Len` with the shape tests deriving from metadata, not addresses, the axiom being phrased over the address component so metadata results are not letter-counterexamples. **The ordering family with composite operands faults at run time**, measured, a **dynamic refusal, not a type-checker rejection**, a faulting instruction yielding no observable. The discriminator is pinned, `composite_equality_is_content_derived_not_address_derived` in their `f90fe688`, so the row's standing is executed-and-pinned on their line. |
| M9 | Flat composites hold bytes only, nested children inlined, executed in the `CopiesOut` evidence, the boxed path aliasing and excluded, so the instantiation covers modules whose composites are transitively scalar, that boundary being part of every theorem's scope here. |

**The scoping paragraph, exhaustive.** The producer emission invariants are, first, streams
never return, M5's second half, pinned and re-run every build, and second, iterating loops
emit no value-carrying `Break`, M6(b), resting on the grammar's expressionless `break` plus
code generation. M4's count and, after the ruled merge carrying `92e5696a`, M6(d) are
structurally enforced, and everything else is executed, read, ruled, inferred, or trust as
labeled. Therefore Theorem A2, Corollary A3, Theorems B1 and B1r, and **the composition
results without designated elements** hold, in this instantiation, for **reference-compiled
modules whose composites are transitively scalar**, with the M3 per-row analysis standing as
the residual risk Appendix B records. **Theorem B2 and designated-element composition hold
for the discipline machine only, which does not exist in Keleusma**, a proved specification.
None of the theorems holds for arbitrary bytecode that merely passes `verify()`.

## Appendix B. What the instantiation does not establish

1. **The per-opcode verdicts of M3's table**, totality mechanical, rows analysis, a wrong
   safe row of any class defeating confinement, the soundness-critical `CopiesOut` set now
   fully executed.
2. **Standing guarantees for the emission invariants**, streams never return and no
   value-carrying `Break` from iterating loops, the second resting on the grammar's
   expressionless `break` plus code generation, either defeasible by a language or generator
   change or by hand-written bytecode without failing verification.
3. **Cross-line pin residency.** The pins of `f90fe688`, `435a8f6d`, `92e5696a`, and
   `a288ae26` live on the V0.2.X line and are absent from this tree until the ruled merges
   land, so a reader of this tree alone cannot run them.
4. **A stale internal handle faults rather than lying**, no live route found, unreachability
   not claimed, defense in depth only.
5. **The loop-dominated planner gap**, obligation Section 6.2, unmeasured, the V0.3.X offer
   standing.
6. **Anything about the native backend's lowering.** The theorems are machine-generic, so
   any application to the native backend's runtime requires that runtime to discharge M1
   through M9 itself, which nothing here does.
7. **The counterexample is bracketed, not contradicted**, the obligation's Section 4.1 site
   being unconfined, the backend's unconditional reuse remaining unsound on such sites.
8. **The embedder obligations**, native retention, host-side view derivation or handle
   arithmetic, `unsafe` mutation of resolved slices, out-of-band arena rewinds,
   handle-address comparison, and epoch-counter wraparound in a long-lived embedding, all
   stated in those words.
9. **Static checkability is guidance**, confinement, deadness, designation, separation, and
   the per-slot write-before-read discipline being dataflow properties, unestablished flows
   escaping.
10. **Hypothesis H's discharge is read, not pinned**, the capped loop forms, the analyses'
   call-graph traversal, and the stream body as the unit, with the allocation-confinement
   clause resting on the compiler emitting no allocation outside the stream body's
   traversal, a code-generation reading.

## Appendix C. B2 in Keleusma, and instruction-set remedies

**Theorem B2 is proved for the discipline machine, and the discipline does not exist in
Keleusma today.** No escape route copies. B2 is a **proved specification for a change** whose
adoption carries these obligations, seven of them. First, the copy at every escape of a
composite, unconditionally at `SetLocal` under the proved discipline, the slot-lifetime
conditional copy being the selective optimization licensed only through Corollary B2b with
its hypotheses. Second, worst-case execution time in the cost model before adoption. Third,
copy-store provisioning per Corollary B2a under Hypothesis H, including boundary reclamation
and the mutability asymmetry, a persistent-region copy store avoiding every data slot.
Fourth, epoch stamping of copy handles behind `resolve`, the copy's source read being a
dereference. Fifth, handle-address opacity as an embedder obligation. Sixth, the honest
accounting, an always-escaping site relocating its bound and exceeding the saving by one
slot size, the hybrid dominating both pure regimes. Seventh, the boxed path, outside M9,
needing its own deep-copy treatment first.

**Adoption is unruled in either direction as of 2026-08-24**, the V0.2.X operator stating
they do not yet know enough to rule, so the specification must not be read as declined.

**Instruction-set position.** Unchanged. No new opcode is required, a dedicated copy opcode
requiring the strong justification the operator demands, none arising, and the
classification test pinning the count at 66.

## Appendix D. Change control

| consequence of adoption | surface | owner | decision |
|---|---|---|---|
| loop accounting stops multiplying confined sites | `src/verify.rs:1079` | V0.2.X line | **operator, still unruled as of 2026-08-24**, it lowers a published worst-case-memory-usage figure, and commissioning the analysis does not authorize adopting its result. Theorem C's remark applies, the change helps loop-dominated shapes and can hurt branch-dominated ones, so adoption should be per-site by comparing both bounds |
| the confinement predicate, Definitions 6 through 8 with the separation hypothesis | the shared crate under `src/`, one predicate, two consumers | V0.2.X line | **commissioned 2026-08-24 and landed on their line the same day, merged at their tip `f9a7b3e4`.** `src/confine.rs` under the `verify` feature, per-site and three-valued, **deliberately not wired into `verify()`**, thirteen tests, consuming the escape classification directly with `Break` and `BreakIf` as `WithinIteration` on this document's reasoning and a green drift guard asserting the shipping and test classifications agree opcode by opcode. Useful-and-sound standard, unestablished flows reporting `CannotEstablish` and treated as escaping. Three of the four per-iteration corpus sites return Confined, the first subject included, a `Call` touching only scalars no longer disqualifying. The analysis distinguishes scalar projections, which copy a word out and alias nothing, from nested projections, which are views carrying the parent's region, matching Definition 2's view semantics exactly, each direction pinned on their line |
| branch maximum | `src/verify.rs:992` | V0.2.X line | already implemented, Theorem A1 with Corollary A1s justifies it under Hypothesis H, discharged as Appendix B item 10 records |
| backend stops reusing slots of unconfined or unseparated sites | native backend planner | V0.3.X line | required for soundness independent of this proof |
| backend may overlap exclusive arms | native backend planner | V0.3.X line | the theorems license this only for a runtime discharging M1 through M9 itself, per Appendix B item 6, so the license is conditional on that discharge plus Appendix A's scoping |
| verifier floors pops at loop entry | `src/verify_typed.rs` | V0.2.X line | **implemented on their line, `92e5696a`**, zero measured rejections, not yet in this tree |
| verifier compares break edges against loop entry | `src/verify_typed.rs` | V0.2.X line | **assessed and not proposed on present evidence**, enforcement must distinguish scope kinds, their operator's call, and the dispatch-break pin of `f90fe688` fails on it with a message naming what it breaks |

This document's conclusions authorize none of these changes.

## Appendix E. Provenance

The obligation was read at `a49555bb`, corrected on its line at `d5b706e8` and `c3ff3c06`.
The V0.2.X line's 2026-08-23 confirmations, M5 two-part, the M6 clauses with the unfavorable
then structurally closed (d), M1's four grounds, and the pins at `435a8f6d`, `92e5696a`, and
`a288ae26` with their differing standings, are recorded in the rows, with the corpus
extension to 26 modules and its inferred-empty entry stacks. Their `f90fe688`, #268, carries
the four closing pins in `tests/composite_escape_routes.rs`, now eight tests, all named in
their evidence index so the index guard fails on any rename, merged at 22 of 22 with the tip
at `71792ecc` and subsequently `f9a7b3e4`, carrying the landed confinement analysis of the
Appendix D row. The merge sequence is ruled, this line into V0.2.X with V0.3.X rebasing,
authorized on the V0.2.X side, awaiting this line's operator. B2 adoption and the accounting
change remain explicitly unruled.

**The audits and revisions.** Round one, 2026-08-24, five contexts against `de8b3f68`,
record in [`AUDIT_2026-08-24.md`](./AUDIT_2026-08-24.md), repaired at `15532455`. Round two,
same day, five fresh contexts against that revision, record in
[`AUDIT_2026-08-24_ROUND2.md`](./AUDIT_2026-08-24_ROUND2.md), including the finding that the
round-one repair of Theorem A2 was itself defective, annotated in the round-one record,
repaired in the third revision with the V0.2.X line's same-day measurements folded in. Round
three, 2026-08-26, targeted, three contexts against `0f59ffed`, record in
[`AUDIT_2026-08-26_ROUND3.md`](./AUDIT_2026-08-26_ROUND3.md), finding two soundness holes
each repairable by a single stipulation, view-epoch laundering and startup-cycle allocation,
together with consistency and formalization debt. This fourth revision repairs every
verified round-three finding, the view-epoch clause in Definition 2 and M2 with A2's and the
lemmas' cross-boundary sentences extended, the allocation-confinement clause in Hypothesis
H, dynamic-enclosure scoping chosen explicitly in Definition 3 with entry height defined,
lexical arm containment in Definition 6, the symbolic environment action model and receipt
content in Definition 5, Lemma 1's third clause scoped to the baseline, Lemma 2's
callee-return and unwinding cases argued correctly, Lemma 4 rewritten as an ordered
induction with element-level hypotheses and the copy-chain analysis in its enumeration,
Theorem B1's box asserting soundness only, Theorem C and Corollary B2a carrying full axiom
lists and proofs over plan elements, Corollary B2b restricted to three of Lemma 4's four
routes, the scrutinee and cap-counting conventions in the Setting and H, and the Part II
corrections, Corollary A3's label, the composition qualifier, the B1r attribution, and
Appendix B's new items on M7's no-derivation clause and H's read discharge. Rows keep their
labels under the standing rule that nothing is promoted without execution.
