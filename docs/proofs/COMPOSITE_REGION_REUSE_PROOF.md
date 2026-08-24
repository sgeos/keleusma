# Region Slot Reuse Under Escape Confinement

> **Navigation**: [Documentation Root](../README.md)

> **STATUS.** Part I is a general theory over an abstract machine, proved from stated axioms and
> applicable to any system that discharges them. Part II instantiates the axioms for Keleusma
> with measured standing recorded per row. In the instantiation, Theorem A1 is unconditional,
> while Theorems A2, A3, B1, B1r, B2, and the corollaries rest on axioms discharged in part by
> reference-compiler emission invariants that `verify()` does not enforce, so their conclusions
> apply to reference-compiled modules and do NOT apply to arbitrary bytecode that merely
> verifies. Theorem B2 is additionally a **proved specification, not a description**. The
> escape-copy discipline it requires does not exist in Keleusma today, and Appendix C names the
> adoption obligations. M1's immutability clause is confirmed on four independent grounds, one
> executable and pinned, and it is scoped to the ephemeral region deliberately, because the
> persistent region is mutated in place and the unscoped statement is false in Keleusma. The
> verifier-accepted counterexample to structural enforcement is in Appendix A, and the pin
> commits are in Appendix E with their differing standings.

This document discharges part of the obligation stated in `docs/proofs/COMPOSITE_REGION_REUSE.md`
on the `v0.3.0` line, cited at commit `a49555bb`. Part I is self-contained mathematics and cites
no project artifact. Every project-specific fact, premise, measurement, ownership statement, and
proposal lives in the appendices.

---

# Part I. General theory

## 1. Setting

An abstract machine executes programs over a fixed finite instruction set with structured control
flow. Machine state comprises a stack of frames, each holding an operand stack and local slots, an
**ephemeral region** managed by a bump allocator and stamped with an **epoch** counter, optional
persistent storage, and an external **environment** that the machine can call out to and yield
values to. Conditionals execute exactly one arm per dynamic instance. Loops are structured, with a
well-defined body entry and exit. The language of programs is total, so every call terminates.

The question the theory answers is when a static planner may assign one fixed memory slot to an
allocation site whose dynamic executions would otherwise each receive a fresh bump allocation, and
what a worst-case memory bound may consequently count.

## 2. Definitions

**Definition 1, sites and paths.** Let $\mathcal{S} = \{s_1,\dots,s_m\}$ be the static allocation
sites of a program unit and $\mathrm{sz}(s)$ the byte size site $s$ allocates. Let $\Pi$ be the
set of execution paths. For $\pi \in \Pi$, $\mathrm{alloc}(\pi)$ is the multiset of dynamic site
executions along $\pi$, a site inside a loop executed $k$ times contributing $k$ occurrences. The
memory a path consumes is $M(\pi) = \sum_{s \in \mathrm{alloc}(\pi)} \mathrm{sz}(s)$ and the
worst-case memory usage is $\mathrm{WCMU} = \max_{\pi \in \Pi} M(\pi)$.

**Definition 2, regions, handles, and references.** Each dynamic execution of an allocation
instruction allocates a **region** $r$, an address interval in the ephemeral region. A **handle**
is a triple of address, length, and epoch. A **dereference** of a handle succeeds and returns the
bytes at its address exactly when the handle's epoch equals the current epoch, and otherwise
fails with a staleness error. A **reference to $r$** is any value, in any location, that is a
handle whose address range lies in $r$, or a view derived from such a handle. Views obtained by
projecting a nested component alias the parent region and count as references to it.

**Definition 3, cycles and scopes.** A **cycle** is the dynamic interval between consecutive
epoch advances. A **scope** of a loop $L$ is one iteration, the dynamic interval from one
execution of $L$'s body entry to the corresponding body end, whether reached normally or by early
exit. For a site $s$ inside nested loops, the scope of $s$ means the iteration of the innermost
loop enclosing $s$.

**Definition 4, plans and soundness.** A plan assigns each site a fixed offset. Under the
**no-reuse regime**, every dynamic site execution receives a fresh bump allocation. Under a
**reuse plan**, some sites' dynamic executions share one offset. A reuse plan is **sound** when
for every program and every path the machine under the plan is observationally equivalent to the
machine under no reuse, where the observations are the outcome of every dereference, by the
environment or by the machine, and every scalar output. Handle addresses are not observations.
An environment that compares handle addresses for value identity lies outside this observation
model, and Theorem B2 in particular does not hold for it.

**Definition 5, confinement.** Let $v$ be the value allocated at site $s$ in scope $n$, with
region $r_n$. The value is **confined** when, along every path, no reference to $r_n$ is ever an
operand of an instruction classified `Escapes` under axiom M3. A site is confined when every
value it allocates on every path is confined.

Confinement is deliberately coarse. It forbids some harmless flows, for example a callee
returning the value back into the same scope, because the classification of M3 is by worst case.
The coarseness weakens applicability and never soundness. Theorem B1r at the end of Section 6
proves the refinement that admits local stores to boundary-dead slots, which is the form every
expressible in-loop binding takes in the instantiation. Section 9 records the return refinement,
which remains unproved, and Section 8 proves the regime that removes the condition entirely at a
copy cost.

## 3. Axioms

Any system claiming the theorems must discharge these. How each is discharged, and with what
standing, is an instantiation concern. Appendix A does so for Keleusma.

| # | axiom |
|---|---|
| M1 | **Bump ephemeral allocation, immutable regions.** Allocation in the ephemeral region is by bump pointer. The region is reclaimed, and the epoch advanced, only at cycle boundaries. Nothing else reclaims within a cycle. A region's bytes are not modified after its construction completes, except by a reuse plan's reallocation of the same offsets. |
| M2 | **Epoch-guarded handles.** A handle carries the epoch current at its creation. Dereference succeeds exactly when the carried epoch equals the current epoch. An overwrite in place advances nothing. |
| M3 | **Total instruction classification.** The instruction set is partitioned, totally, into four classes with these semantics. `NoRegion` instructions neither create nor transport references. `WithinScope` instructions create or move references only into locations whose lifetime is contained in the current scope. `CopiesOut` instructions write the referenced bytes, never references, to their destinations. Only `Escapes` instructions can place a reference into a location that outlives the current scope. No instruction fabricates a handle, and references originate only at allocation instructions. Totality must hold and keep holding as the instruction set changes. |
| M4 | **Cycle cadence.** Each cycle contains exactly one epoch advance, at its end. |
| M5 | **Boundary clearance.** At a cycle boundary, no machine-internal location that is ever subsequently read holds a reference into the ephemeral region. The environment may retain handles across the boundary. |
| M6 | **Scope discipline.** (a) A loop's back edge restores the exact entry operand stack, height and per-slot shape, so no entry created within the scope survives above the entry height. (b) Early exits restore the same discipline. (c) A frame created within a scope is destroyed within it and its contents become unreachable, and every call made within a scope returns within it. (d) No instruction within a loop body consumes operand-stack entries below the loop's entry height. |
| M7 | **Environment trust.** The environment presents to the machine only handles it received from the machine. What an external callee retains is unknown, which is why external calls are classified `Escapes` in any instantiation of M3. |

## 4. Lemmas

**Lemma 1, origination.** Under M3 and M7, every reference to a region $r$ is connected by a
chain of transports and view derivations to the single allocation that created $r$.

*Proof.* By M3 no instruction fabricates a handle, allocation instructions produce a reference
only to the region just allocated, `CopiesOut` instructions produce values holding no reference
to their operands' regions, and projections derive views from an existing reference. Every other
instruction moves, duplicates, or consumes existing values. By M7 the environment presents only
handles it received, so no reference enters from outside except by returning along a chain that
began at the machine. $\blacksquare$

**Lemma 2, confinement is preserved to the scope boundary.** Under M3, M6, and M7, let $v$ be
confined, allocated at site $s$ in scope $n$ of loop $L$, with region $r_n$. Then at the end of
scope $n$ no reachable location holds a reference to $r_n$.

*Proof.* By induction over the instructions executed during scope $n$, with the invariant that
every reference to $r_n$ resides either in an operand-stack entry above the scope entry height of
the frame executing $L$, or inside a frame created during scope $n$.

The invariant is established at origination. The allocation at $s$ executes inside the body, so
its result is pushed above the entry height of the executing frame, and by Lemma 1 this is the
only reference at that moment.

Preservation is by cases on the class of the executed instruction, under M3.

*Case `Escapes`.* Excluded. Confinement says no reference to $r_n$ is ever an operand of such an
instruction. In particular no local-store instruction ever stores one, so no local slot in any
frame ever holds a reference to $r_n$, and no local-load instruction can produce one.

*Case `CopiesOut`.* The referenced bytes are written to the destination. Bytes are not
references, so no new location holding a reference arises. An allocation instruction consuming a
reference to $r_n$ as a nested operand copies its bytes inline into the new region and consumes
the operand. The produced value references the new region, not $r_n$.

*Case `WithinScope`.* Duplication and projection produce stack entries in the executing frame,
above the entry height. A call transfers operand-stack entries into a callee frame, which is a
frame created during scope $n$. Within the callee the same case analysis applies. By M6(c) the
callee returns within the scope and its frame becomes unreachable at that point. Its result value
cannot be a reference to $r_n$, because that would make a reference to $r_n$ an operand of the
return instruction, which confinement excludes.

*Case `NoRegion`.* These instructions consume reference operands, if any, and produce scalars or
nothing. References are removed, never created. Control instructions move no data, and any stack
unwinding they perform removes entries.

One flow enters from outside the instruction stream. A resume after a yield pushes an
environment-supplied reply. By M7 the environment presents only handles it received, and
confinement ensures it never received a reference to $r_n$, so the reply cannot carry one.

At the end of scope $n$, by M6(a) the operand stack of the frame executing $L$ stands at its
entry height with its entry shape, so every entry above the entry height is gone, and by M6(c)
every frame created during the scope has returned. It remains to rule out a reference to $r_n$
sitting at or below the entry height, which shape restoration alone would permit if the body
popped a below-entry entry and pushed a same-shape replacement built in scope $n$. M6(d) excludes
exactly that. Both location classes of the invariant are therefore empty. $\blacksquare$

The dependence on M6(d) is real and is why it is a separate clause. Without it, a value could
cross the back edge through pure stack operations, touching no escaping instruction, and
confinement would not see it. An instantiation that discharges M6(d) by a producer invariant
rather than a structural check inherits the scoping consequence stated in Section 9.

## 5. The branch theorems

**Theorem A1, branch bound.** For every path $\pi$, replacing the contribution of each dynamic
instance of a conditional by the maximum of its two arms' contributions yields an upper bound on
$M(\pi)$. Consequently the rule
$\mathrm{plan}(\texttt{if}\ A\ \texttt{else}\ B) = \max(\mathrm{plan}(A), \mathrm{plan}(B))$,
applied per instance and multiplied by iteration counts where the conditional sits inside a loop,
over-approximates worst-case memory usage. This holds from structured control flow alone, with no
axiom beyond the setting.

*Proof.* Structured control flow executes exactly one arm per dynamic instance. The instance's
contribution to $M(\pi)$ is therefore the sum over the arm taken, which is at most the maximum of
the two arms' sums. Summing the bound over all instances of the path preserves the inequality.
$\blacksquare$

A flat claim that no path contains sites of both arms fails under the multiplicity path model of
Definition 1 whenever the conditional sits inside a loop, because one path may take the first arm
in one iteration and the second arm in another. Theorem A1 is the form that survives as a bound.
The overlap claim needs the separate treatment below, and it does need liveness reasoning.

**Theorem A2, arm overlap once per cycle. Requires M1, M2, M4, M5.** Let $s_i$ and $s_j$ be sites
in the two arms of a conditional that executes at most once per cycle. A plan assigning $s_i$ and
$s_j$ overlapping offsets is sound.

*Proof.* Within one cycle the conditional executes at most once and takes one arm, so at most one
of the two regions is allocated in that epoch, and by Lemma 1 no reference to the other exists at
all. Any reference surviving into a later cycle is, by M5, environment-held, and by M2 with M4
every dereference of it fails stale under the reuse plan and under no reuse alike, because the
epoch advanced at the intervening boundary in both regimes. No observation distinguishes the
regimes. $\blacksquare$

**Corollary A3, arm overlap inside loops. Requires M1 through M7.** If the conditional sits
inside a loop and every site in both arms is confined, the plan may overlap the arms and
additionally reuse the shared slot across scopes. *Proof.* Per scope at most one arm executes, so
at most one write to the shared slot occurs per scope, and by Lemma 2 no reference to any prior
scope's value survives its scope. The cross-cycle case is as in Theorem A2. $\blacksquare$

## 6. Theorem B1, confined-site slot reuse

**Theorem B1. Requires M1 through M7.** Let $s$ be a confined site inside a loop $L$. A plan
assigning $s$ one slot reused across scopes and across cycles is sound. Consequently worst-case
memory accounting may count $s$ once per cycle, contributing $\mathrm{sz}(s)$ in place of
$k \cdot \mathrm{sz}(s)$.

*Proof.* The reuse plan differs from no reuse only in that $r_n$ and $r_{n+1}$ share addresses.
An observational divergence therefore requires a dereference of a reference to that address range
that returns different bytes in the two regimes. Bytes at the shared address differ from the
no-reuse bytes of $v_n$ only from the moment the $(n{+}1)$-th execution of $s$ begins to write.
So a divergence requires a reference to $r_n$, created in scope $n$, dereferenced at or after the
start of the $(n{+}1)$-th execution of $s$ within the same epoch, or dereferenced in a later
epoch.

Within the epoch, the $(n{+}1)$-th execution of $s$ lies in scope $n{+}1$ of $L$, which begins
after scope $n$ ends. By Lemma 2, at the end of scope $n$ no reachable location holds a reference
to $r_n$, and by Lemma 1 none can subsequently arise, since the originating chain is closed and
the environment, by M7 with confinement, never received one. So no such dereference exists.

Across epochs, by M5 the only surviving references are environment-held, and by M2 every
dereference of them fails stale identically in both regimes.

All other observations are unaffected. Every dereference of a reference to $r_m$, for the current
scope $m$, occurs while the slot holds exactly $v_m$'s bytes, which equal the no-reuse bytes, and
scalar computation never reads the region except through dereferences. Hence observational
equivalence, and the accounting consequence follows by applying Definition 1 to the reuse plan's
allocation behavior, one live allocation for $s$ per cycle. $\blacksquare$

### The refined form, local stores to boundary-dead slots

The coarse Definition 8 forbids every local store, and in a language whose local bindings are
immutable that forbids too much, because the only expressible in-loop store targets a binding
declared inside the body, whose slot the compiler retires with the iteration. The refinement
below admits exactly that shape. Its definitions and lemma are numbered by order of addition, so
they follow Section 8's Definition 9 and Lemma 3 numerically while preceding them in the
document.

**Definition 10, deadness at loop boundaries.** Fix a loop $L$. A local slot $\ell$ of the frame
executing $L$ is **dead at the boundaries of $L$** when, at the back edge and at every exit edge
of $L$, every path onward either never reads $\ell$ or writes $\ell$ before its first read of
$\ell$. This is the standard liveness notion, decidable by dataflow over the program unit.

**Definition 11, refined confinement.** Let $D$ be a set of slots dead at the boundaries of $L$.
The value $v$ at site $s$ in scope $n$ is **confined relative to $D$** when, along every path,
no reference to $r_n$ is ever an operand of an `Escapes`-classified instruction other than a
local store targeting a slot in $D$.

**Lemma 4. Requires M3, M6, M7.** Let $v$ be confined relative to $D$. Then no dereference of a
reference to $r_n$ occurs after scope $n$ ends.

*Proof.* Extend Lemma 2's invariant with a third location class. Every reference to $r_n$
resides in an operand-stack entry above the scope entry height of the frame executing $L$, in a
frame created during scope $n$, or in a slot belonging to $D$. The new cases preserve it. A
local store to $\ell \in D$ places the reference in $\ell$, which the invariant now admits, and
a local load of $\ell$ during scope $n$ pushes a copy onto the executing frame's stack above the
entry height. Every other case is as in Lemma 2. At the end of scope $n$, the first two classes
are empty by M6 exactly as before, so every surviving reference to $r_n$ sits in a slot in $D$.
By Definition 10, on every path after the boundary each such slot is written before it is read,
so its content is never loaded again, and by M3 a reference in a slot can reach an instruction's
operands only through a local load. A dereference requires the reference as an operand, so no
dereference of a reference to $r_n$ occurs after the boundary, and no further copies of it can
arise. $\blacksquare$

**Theorem B1r. Requires M1 through M7.** Let $s$ be a site inside a loop $L$ whose values are
confined relative to some $D$ of slots dead at the boundaries of $L$. Then the conclusion of
Theorem B1 holds for $s$ unchanged. One slot reused across scopes and cycles is sound, and
worst-case memory accounting may count $s$ once per cycle.

*Proof.* The proof of Theorem B1 used Lemma 2 only to conclude that no dereference of a
reference to $r_n$ can occur at or after the start of the $(n{+}1)$-th execution of $s$ within
the epoch. Lemma 4 yields the same conclusion under the refined hypothesis, since the
$(n{+}1)$-th execution begins after scope $n$ ends. The stale reference sitting unread in a
$D$-slot is overwritten without ever being loaded, so it is never an operand of anything. The
cross-epoch and equivalence arguments are unchanged. $\blacksquare$

**Remark, what the refinement unblocks.** A binding declared inside the loop body is written at
its declaration before any read in each scope, and it is out of scope at the boundary, so its
slot is dead at the boundaries whenever the compiler's slot assignment preserves definite
initialization. Both confinement relative to $D$ and membership of $D$ are static dataflow
properties of the compiled unit, so the gate on applying Theorem B1r is an analysis, not a
further theorem.

## 7. Corollary C, the composed plan

**Corollary C. Requires M1 through M7.** Define $\mathrm{alloc}^{\ast}(\pi)$ from
$\mathrm{alloc}(\pi)$ by counting each confined loop site once per cycle, confinement in the
sense of Definition 8 or of Definition 11, counting every other
loop site with its full multiplicity, and bounding each conditional instance by the maximum of
its arms per Theorem A1. Then

$$
\mathrm{plan} \;=\; \max_{\pi \in \Pi} \sum_{s \in \mathrm{alloc}^{\ast}(\pi)} \mathrm{sz}(s)
\;\;\geq\;\; \mathrm{WCMU}
$$

and the overlap forms, Theorem A2 and Corollaries A3 and B1, license the corresponding slotted
layout. The soundness arguments compose because each is an observational equivalence against the
same no-reuse baseline and their location sets are disjoint by construction. Where a site is not
confined, its contribution reverts to $k \cdot \mathrm{sz}(s)$, and any planner reusing its slot
anyway is unsound.

## 8. Theorem B2, universal slot reuse under an escape-copy discipline

Theorem B1 restricts the plan to confined sites. The alternative regime changes the machine
instead of restricting the plan. If every escaping flow transports a copy, no site needs
confinement. This section proves that regime sound in general. Whether any concrete system
implements the discipline is an instantiation question, and Appendix C records that Keleusma
does not today.

**Definition 9, escape-copy discipline.** A machine satisfies the escape-copy discipline when
every execution of an `Escapes`-classified instruction whose operand is a reference to a region
transports a **fresh copy** in its place, subject to all of the following.

1. **Faithfulness.** The copy's bytes equal the referenced bytes at the instant of the copy.
2. **Stability.** A copy's bytes are not modified while any reference to it is reachable within
   its epoch. In particular the copy store is disjoint from every reused slot.
3. **Epoch stamping.** A copy handle carries the epoch current at its creation, and dereference
   remains governed by M2.
4. **Recursion.** The discipline applies equally when the operand is a reference to a copy, so
   an escape of a copy produces a further copy.
5. **Depth.** A copy contains no reference into any ephemeral region or reused slot.

**Lemma 3, unconditional scope clearance.** Under M3 with the escape-copy discipline in force,
M6, and M7, for every site $s$ in a loop and every scope $n$, at the end of scope $n$ no
reachable location holds a reference to $r_n$. No confinement hypothesis is needed.

*Proof.* The induction of Lemma 2 goes through with its `Escapes` case replaced. An escaping
instruction with a reference to $r_n$ as operand consumes it and places a reference to a fresh
copy in the outliving location. By clause 5 the copy holds no reference to $r_n$, so the case
creates no location holding one. The environment-reply flow also closes without confinement.
Under the discipline the environment only ever receives copy references, by clauses 4 and 5
those reference no site region, and by M7 it presents only what it received, so a reply cannot
reintroduce a reference to $r_n$. Every other case is as in Lemma 2, and the scope-end argument
by M6, including its dependence on M6(d), is unchanged. $\blacksquare$

**Theorem B2. Requires M1, M2, M4, M5, M6, M7, and the escape-copy discipline.** The plan
assigning every site, loop or straight-line, one slot reused across all its dynamic executions
is sound, under the observation model of Definition 4, which excludes handle addresses.

*Proof.* Every observation is a dereference outcome or a scalar output, and scalar computation
reads regions only through dereferences, so it suffices that every dereference returns the same
result in the two regimes.

A dereference of a reference to a site region $r_m$ occurs, by Lemma 3, only during scope $m$
itself, before the site's next execution begins, so the slot holds exactly $v_m$'s bytes, which
equal the baseline bytes by M1's immutability. For a straight-line site, consecutive executions
are separated by an epoch boundary by M4, and the cross-epoch case below covers them.

A dereference of a copy reference within the copy's epoch returns, by clauses 1 and 2 and
induction along the copy chain of clause 4, the construction bytes of the originally escaped
value, which is what the baseline returns for the corresponding original reference, since
baseline regions are immutable after construction and unreclaimed within the epoch by M1.
Across epochs, by clause 3 and M2 the copy dereference fails stale, and the corresponding
baseline dereference fails stale by M2 and M4, identically. By M5 no internal location that is
subsequently read carries any of this across a boundary in either regime. Hence observational
equivalence. $\blacksquare$

**Corollary B2a, accounting.** Under Theorem B2's hypotheses, worst-case memory accounting
counts each site once per cycle in the arena term and must add a **copy-store term**,

$$
\max_{\pi \in \Pi} \sum_{e \in \mathrm{esc}(\pi)} \mathrm{sz}(e)
$$

with $\mathrm{esc}(\pi)$ the multiset of escaping executions per cycle along $\pi$. The copy
term re-enters the machine's bound except where the copy store is environment-owned, as it can
be for values yielded outward. Consequently, for a site that escapes on every iteration, the
regime does not tighten the bound, it relocates it. The gains lie elsewhere, in the soundness
of uniform reuse with no confinement analysis, in the removal of yielded values from the
machine's bound where the environment owns the copy store, and in the neutralization of the
escape hazard for a planner that already reuses slots.

**Corollary B2b, the hybrid plan.** The regimes compose per site. A plan that reuses confined
sites without copies, by Theorem B1, and unconfined sites with the discipline applied to their
escapes, by Theorem B2, is sound, since each argument is an observational equivalence against
the same baseline and the location sets are disjoint. The hybrid pays copy costs only where
confinement fails.

## 9. Limits of the general theory

1. **The axioms are obligations, and their standing transfers to the theorems.** An instantiation
   that discharges an axiom by a **producer invariant**, a property of what a particular compiler
   emits rather than of what a verifier refuses, holds the dependent theorems **only for that
   producer's output**. Foreign or hand-written programs that pass verification can defeat them.
   This is not a defect of the theory but a scoping consequence every instantiation must state.
2. **Confinement is sufficient, not necessary.** The theory proves nothing about deciding
   confinement, only that a site established as confined may be reused. A static analysis that
   cannot establish a flow must treat it as escaping.
3. **The return refinement is not proved.** A return that lands in the same scope of the same
   program unit is believed harmless and is not proved so here. The local-store refinement,
   formerly in the same position, is now proved as Theorem B1r over slots dead at the loop
   boundaries, so only the return case remains, and refining it requires frame-relative side
   conditions this document does not develop.
4. **Nothing is claimed about external callees.** M7 makes their retention unknowable, so any
   instantiation must classify external calls as escaping or document the exclusion as an
   obligation on the integrator, in those words.

---

# Part II. The Keleusma instantiation, appendices

Everything project-specific is below this line. The mapping is as follows. The machine is the Keleusma
virtual machine, allocation instructions are `NewComposite`, dereference is `resolve`, the epoch
advance is `Op::Reset`, a cycle is one stream cycle of `loop main`, a scope is one loop
iteration, the environment is the host, and external calls are the two native-call opcodes.

## Appendix A. Axiom instantiation, with measured standing

The runtime evidence is indexed, with per-row provenance and reproduction commands, in
[`docs/decisions/COMPOSITE_REGION_EVIDENCE.md`](../decisions/COMPOSITE_REGION_EVIDENCE.md),
guarded by `tests/proof_evidence_index.rs`. Rows marked read from dispatch are not to be promoted
to executed without running them.

| axiom | instantiation and standing |
|---|---|
| M1 | Operator ruling on the memory model, obligation document Section 1. Bump arena, ephemeral region cleared only at `RESET`, nothing reclaimed within a run. The immutability clause is **confirmed by the V0.2.X line, 2026-08-23, on four independent grounds**, and Theorems B1 and B2 both rest on it. Ground one is **executable and pinned**. The instruction set carries seven read accessors into a composite and **zero write accessors**, `the_instruction_set_has_no_write_accessor_into_a_composite` in `tests/composite_escape_routes.rs`, derived from the `Op` enum, mutation-tested two ways, and holding for any module because it is about what the instruction set contains. Grounds two through four are **read from dispatch plus a scan of the public interface**. No mutable accessor exists on a composite body, `resolve` returning a shared slice and nothing else. Every raw-pointer write in the virtual machine targets the persistent region. The native boundary is immutable by signature, arguments arriving as a shared slice with no public route to a mutable arena view. **The clause is scoped to the ephemeral region deliberately, and the scoping is load-bearing. The persistent region is mutated in place, repeatedly and across resets, by the data-slot writes, so the unscoped statement is false in Keleusma.** Definition 2 ties regions to the ephemeral region, and an abstraction pass must not widen that. Stated refutation boundary, so no more is read than was measured. A native casting a resolved slice to a mutable pointer under `unsafe` is undetectable from the safe interface and sits on the same trust boundary as the native escape routes, and an out-of-band `unsafe` arena rewind reclaims rather than mutates, refuting not this clause but the epoch discipline M5 rests on. Grounds two through four say nothing about what `verify()` enforces. |
| M2 | **Executed.** A non-empty composite value carries an arena handle, not bytes. `resolve` fails `Stale` exactly when the epoch has advanced, and an overwrite in place advances nothing. `tests/composite_escape_window.rs`, all three tests. The load-bearing assertion is that two iterations' composites resolve simultaneously to different values, which is exactly what one reused slot collapses. |
| M3 | The 66 opcodes are partitioned in `tests/composite_escape_routes.rs`, with **totality asserted against the `Op` enum at test time**, so a route can be missed only by misclassification, never by omission, and a new opcode fails the test. The escaping set is exactly `Yield`, `SetLocal`, `Return`, `CallVerifiedNative`, `CallExternalNative`. `SetLocal` is classified by its worst case because the opcode cannot distinguish an inner binding from an outer one, and it is the route that defeats a restriction phrased as no `yield`, with no host involved. One illustration history matters here. The obligation document's Section 4.3 illustrated this route with a `let mut` loop assignment, and that illustration was **retracted 2026-08-23** because local bindings are immutable in Keleusma and the program is refused at parse, a refusal the V0.3.X line verified independently. The classification stands unchanged, since it is over bytecode and hand-written bytecode can store to an outer slot with no yield. The consequence lands on the source surface instead. Every expressible in-loop local store targets a binding declared inside the body, whose slot dies with the iteration, which is exactly the shape Theorem B1r admits over boundary-dead slots, so for source programs B1r rather than the coarse B1 is the operative theorem. **That operative status rests on two claims of different standing.** The refusal of `let mut` is verified. The further claim, that no other source form writes an outer slot from inside a loop, was inferred from that one probe and has **not been enumerated from the grammar**. The enumeration is requested from the V0.2.X line, in the same style as the escape routes, and until it lands the operative-theorem statement carries this mark. Per-row verdicts are **analysis, not proof**. The `Yield` row and both `CopiesOut` rows, private-data writes and flat nesting, are executed, the latter two deliberately because a wrong `CopiesOut` makes the theory unsound rather than loose. The boxed construction path does alias and does not arise for the transitively-scalar composites this instantiation concerns, a boundary stated rather than assumed away. The rest of the rows are read from dispatch. Disagreements go to the table, where the test makes them concrete. |
| M4 | **Executed.** `Op::Reset` is emitted once per stream cycle, at the end of the `loop main` body, not per `for` iteration. `reset_is_once_per_stream_cycle_not_once_per_loop_iteration`. |
| M5 | **Confirmed by the V0.2.X line, 2026-08-23, and the basis is two facts, not one.** First, `Op::Reset` clears the current frame's locals and truncates its operand stack, `src/vm.rs:5304`, read from dispatch. Second, a caller frame beneath a nested stream can hold stale references and is never resumed, because stream chunks emit no `Return`. The second fact is a **code-generation invariant, not a structural one**, executed over five shapes in their `tests/stream_never_returns.rs`. Stating the first fact alone would make the nested-stream arrangement a counterexample to the axiom. Every ephemeral composite-body read is epoch-checked at resolve time, `FlatComposite::resolve` and `ArenaHandle::get`, with `nested_view` bounds-checked as a real fault, read from dispatch. |
| M6 | (a) **Confirmed.** `TypedError::LoopNotNeutral` in the `Op::Loop` arm of `src/verify_typed.rs` compares the entire abstract stack, height and per-slot shape, read from dispatch. Neutrality is on **shapes, not identities**, so the correct reading is that no new entry survives the back edge, never that the entries are identical across iterations. (b) **Confirmed.** `Break` edges join through `join_stacks`, read from dispatch. (c) Read from dispatch, with call termination from totality. (d) **Settled 2026-08-23 in the unfavorable direction, a code-generation invariant only, with the verifier gap executed rather than conjectured.** `verify()` accepts a shape that pops a below-entry entry inside a loop and pushes a same-shape replacement, pinned in the V0.2.X line's `tests/loop_entry_floor.rs`. The typed pass floors pops at the frame, not at the loop entry, and the frame floor does not incidentally cover the loop floor in general, because 122 of 245 `Op::Loop` scopes in the pre-extension corpus carried a non-empty operand stack at entry, an **approximate figure from a linear depth scan** whose bias runs toward over-reporting non-empty entries. **Re-measured with the iteration discriminator, zero of the eight genuinely iterating loops carries a non-empty entry stack**, so on this corpus the frame-floor gap is reachable only through dispatch scopes, the `match` lowering among them. Nothing softens, since the floor protects dispatch scopes too and the accepted counterexample is real, but the row must not imply that iterating loops carry non-empty entry stacks, because measured they do not. One sharpening follows and is **a derivation of this line, confirmation requested**. An iterating loop whose entry stack is empty has its loop floor coincide with the frame floor, so a below-entry pop there is a frame underflow the typed pass already refuses, and clause (d)'s emission-invariant caveat then binds only where the entry stack is non-empty. The emission side was measured exactly, by a per-path floor check inside the typed pass's own abstract interpretation, proven to fire on the breaching shape, with the result **zero breaches over 588 fixpoint visits of the 245 static `Op::Loop` scopes across the 23 pre-extension modules**. The 588 is a **visit count, not a population of loops**, since the locals fixpoint re-enters a body until it stabilizes, and zero breaches over the visits still implies zero over the scopes. That instrumentation is reverted and the zero is **a measurement at a commit, not a standing guarantee**, cited as exactly that. A linear depth scan also reported zero and was discarded as exact for only 4 of the 245 loops, recorded because the flattering number came from a broken instrument first. |
| M7 | Trust assumption, not a theorem. The host holds and may return handles it received and does not fabricate them. Native retention is unknowable from this side, which is why both native-call opcodes are classified escaping. |

**The scoping consequence of Section 9, instantiated.** M5's second half and M6(d) are invariants
of what the **reference** compiler emits. Therefore Theorems A2, A3, B1, B2, and the corollaries
hold for reference-compiled modules and do not hold for arbitrary bytecode that merely passes
`verify()`.
The producer matters further. The zero-breach measurement covered the eleven example scripts and
the twelve stage sources compiled by the reference compiler, and says nothing about modules the
self-hosted compiler emits, a different producer, beyond the constructs where the byte-identity
corpus makes the two artifacts identical.

## Appendix B. What the instantiation does not establish

1. **The per-opcode verdicts of M3's table.** Totality is mechanical, rows are analysis, three
   are execution-backed, the rest read from dispatch, and the table is the place to argue.
2. **A standing guarantee for M6(d).** The zero over 588 is a measurement at a commit. A
   code-generator change, a future returning stream, or hand-written bytecode defeats the reuse
   theorems without failing verification.
3. **A stale internal handle is an error, not a wrong value, and no route to one was found.**
   `Op::GetField` resolves through the epoch check and faults `InvalidBytecode` on staleness. The
   V0.2.X line found no live route to a stale local and does not claim unreachability. This
   carries no load in any proof, defense in depth only.
4. **The loop-dominated direction of the planner gap**, the obligation document's Section 6.2.
   Nothing here measures whether the backend under-provisions relative to worst-case memory usage
   on any real module. The V0.3.X line has offered to measure it on request.
5. **Anything about the native backend's lowering.** Every executed premise is against the
   virtual machine.
6. **The counterexample is bracketed, not contradicted.** The obligation document's Section 4.1
   program yields its loop-body composite, so the site is not confined, Theorem B1 does not apply
   to it, and the backend's current unconditional reuse remains unsound on exactly such sites per
   the obligation document's Section 4.1.1.
7. **The embedder obligation.** Confinement treats both native-call opcodes as escaping, so no
   obligation falls on the embedder here. Any future relaxation admitting a native call on a host
   promise not to retain the composite is an obligation documented on the embedder, in those
   words, and is not licensed by this document.
8. **Static checkability is guidance, not proof.** Confinement is a forward dataflow property of
   the kind the typed operand-stack verifier already computes. Any operand whose flow cannot be
   established must be treated as escaping.

## Appendix C. B2 in Keleusma, and instruction-set remedies

**Theorem B2 is proved in general, and the discipline it requires does not exist in Keleusma
today.** No escape route copies. A yielded composite is a handle, `SetLocal` stores the handle,
and the native calls receive it. In this codebase B2 is therefore a **proved specification for a
change**, not a description of the present system, and it answers the escape-window question of
the obligation document's Section 4.0.1 by construction rather than by bounding. Adoption
carries the following obligations, each named here so none is discovered at implementation time.

1. **The copy itself.** Every escape of a composite must copy its bytes, at `Yield`, at
   `SetLocal` where the target slot outlives the scope, at `Return` across the surviving frame,
   and at both native calls. Correctness is Definition 9's faithfulness and depth clauses. The
   flat representation makes copies deep by construction for transitively-scalar composites, and
   the **boxed path remains a boundary**. It stores operands as separate values, a deep copy
   there is additional work, and nothing here covers it.
2. **Worst-case execution time.** A copy of $\mathrm{sz}$ bytes at every escaping execution
   enters the worst-case-execution-time model, which is the crate's headline claim, so the cost
   belongs in the cost model before adoption, not after.
3. **Copy-store provisioning, and the mutability asymmetry.** Corollary B2a's copy term
   re-enters the worst-case-memory-usage bound except for copies into host-owned storage. A
   per-local copy slot reused on overwrite would bound `SetLocal` copies at one slot per local,
   but its soundness needs a further argument, that no live reference to the old copy exists at
   overwrite time, which is **not proved here**. The proof needs the copy **source** immutable,
   which M1 gives for the ephemeral region, and the copy **destination** stable, which is
   Definition 9's second clause and is an **obligation, not a given**. The persistent region is
   mutated in place by the data-slot writes, so a copy store located there must be disjoint from
   every data slot, or stability fails at the destination even though the source is immutable.
4. **Epoch stamping.** Copy handles must carry the creation epoch and stay behind `resolve`'s
   check, or behavior after `RESET` diverges from the baseline and the equivalence fails.
5. **Handle-address opacity.** Definition 4 excludes addresses from observations. A host that
   compares handle addresses for value identity breaks Theorem B2's equivalence, so adoption
   requires documenting address opacity as an embedder obligation, in those words.
6. **The accounting is honest, not free.** By Corollary B2a, a site escaping on every iteration
   relocates its bound rather than tightening it. The definitive gains are uniform reuse with no
   confinement analysis, yielded values leaving the machine's bound where the host owns the copy
   store, and soundness for a backend that already reuses slots. Corollary B2b's hybrid, copies
   only where confinement fails, is the form that dominates both pure regimes.
7. **Embedder obligations under `unsafe`.** A native that casts a resolved slice to a mutable
   pointer defeats M1's instantiation and the discipline together, undetectably from the safe
   interface, and an out-of-band `unsafe` arena rewind defeats the epoch discipline M5 rests on.
   Adoption documentation must list both beside handle-address opacity, in the same
   embedder-obligation terms as the native escape routes.

**Instruction-set position.** Unchanged by the proof. No new opcode is required. The discipline
has at least two lowerings, a compiler-inserted copy through existing constructs, or a semantics
change to the five escaping opcodes' handling of composite operands. A dedicated copy opcode
would require the strong justification the operator demands for instruction-set modification,
and no such justification arises from this work. The classification test pins the opcode count
at 66 and fails on any addition, which is the intended forcing function.

## Appendix D. Change control

| consequence of adoption | surface | owner | decision |
|---|---|---|---|
| loop accounting stops multiplying confined sites | `src/verify.rs:1079` | V0.2.X line | **operator**, it lowers a published worst-case-memory-usage figure |
| branch maximum | `src/verify.rs:992` | V0.2.X line | already implemented, Theorem A1 justifies it |
| backend stops reusing slots of unconfined sites | native backend planner | V0.3.X line | required for soundness independent of this proof, per the obligation document's Section 4.1.1 |
| backend may overlap exclusive arms | native backend planner | V0.3.X line | licensed by Theorems A2 and A3, within the reference-compiled scope Appendix A states |
| verifier floors operand-stack pops at loop entry, converting M6(d) from an emitted invariant into an enforced one | `src/verify_typed.rs` | V0.2.X line | **operator**, it narrows the acceptance surface of `verify()`. The measured cost is zero, no loop instance among the 588 in the shipped corpus would be rejected, and the V0.2.X line has raised the item on its own channel |

This document's conclusions authorize none of these changes. Each is a request to its owning
line, and the first is an operator decision because it weakens the crate's headline guarantee in
a changelog-visible way.

## Appendix E. Provenance

The obligation is cited at `v0.3.0` commit `a49555bb`. The evidence index and its guard are on
this lineage at `docs/decisions/COMPOSITE_REGION_EVIDENCE.md` and `tests/proof_evidence_index.rs`.
The premise confirmations were requested from the V0.2.X line and answered by measurement on
2026-08-23. M5 was confirmed in the corrected two-part form its row records, M6 clauses (a)
through (c) were confirmed with the shape-not-identity reading, and M6(d) was identified in the
same exchange and settled the same day as a code-generation invariant only, with the verifier's
acceptance of a breaching shape executed, the frame-floor-is-not-the-loop-floor fact executed,
and the zero-breach emission measurement executed at a commit with its instrumentation reverted.
Both pin files landed in the V0.2.X line's commit `435a8f6d` on `docs/proof-evidence-index`,
their #259, verified at origin by ref, with their gate green at 2,580 passed by cargo's own exit
status. **The two files have different standings, and reading them as five tests of one kind
would misstate what is guaranteed.** `tests/stream_never_returns.rs` pins an **invariant** and
re-runs on every build, `no_compiled_stream_chunk_emits_return` over five shapes,
mutation-tested, and `a_stream_calling_a_stream_compiles_verifies_and_runs`, which pins the
nested arrangement as constructible so the premise is not vacuous. `tests/loop_entry_floor.rs`
pins a **gap**. `a_loop_body_may_consume_from_below_its_entry_height` asserts that `verify()`
**accepts** the breaching shape, with its control and with
`compiled_loops_really_do_carry_a_non_empty_entry_stack` establishing reachability at 122 of
245. If the gap is ever closed structurally, those tests fail deliberately, with a message
saying a proof premise moved rather than reading as a routine fix. The zero-over-588 emission
measurement is in no commit and no tree, and remains a measurement at a commit. The operator rulings relied on are the memory model, the
branch topology, the generality directive that produced this document's structure, and the scope,
all of 2026-08-23, and the standing rule that instruction-set modification requires strong
justification. The scope was expanded by operator direction later on 2026-08-23 to include the
proof of Theorem B2, which entered as Section 8 with Definition 9 and Lemma 3, together with
M1's explicit immutability clause. That clause was confirmed by the V0.2.X line the same day,
sought as a refutation and not found, on four independent grounds recorded in the M1 row, with
ground one pinned as `the_instruction_set_has_no_write_accessor_into_a_composite` in
`tests/composite_escape_routes.rs`, whose failure message directs a future editor to the proof's
owner rather than to updating the test. That pin landed in the V0.2.X line's commit `a288ae26`
on `docs/proof-evidence-index`, their #259, verified at origin by ref, derived from the `Op`
enum at test time and mutation-tested two ways. **The commit pins ground one only.** Grounds two
through four remain read from dispatch and are in no test, so citing `a288ae26` against M1 as a
whole would claim more than the test does, and this record cites it against ground one exactly. The same exchange contributed the persistent-region precision, the source and
destination asymmetry now stated in Appendix C's third obligation, and the `unsafe` refutation
boundary now in its seventh.

Three later items from 2026-08-23 complete the record. First, the obligation document's `let
mut` illustration of the `SetLocal` route was retracted by its authors, struck in place on
`v0.3.0` rather than repaired silently, and the retraction is what prompted Theorem B1r, since
the only expressible in-loop store is the iteration-scoped shape B1r admits. Second, a corpus
measurement by the V0.3.X line found that across 87 compiled modules the 36 genuinely iterating
loop scopes contain **zero composite construction sites**, so the reuse theorems have no subject
in the shipped corpus today. Three examples are landing on operator direction, with
`12_sensor_window.kel` intended as the first subject, admitted through B1r once a confinement
analysis exists. Their measurement method matters and is recorded because a first attempt was
wrong. `Op::Loop` marks break scopes, including `match` dispatch, not iterations, and the
discriminator is that a genuinely iterating body carries no `Break` targeting its own exit.
Third, the V0.2.X line answered the count-precision question on 2026-08-23, and the answer
corrected this appendix twice over. Neither instrument applied the iteration discriminator, so
both figures cover all `Op::Loop` scopes, the 245 from a static walk and the 588 from a counter
inside the abstract interpretation's `Op::Loop` arm, which the locals fixpoint re-enters, making
588 a **visit count rather than a population of loops**. The soundness of the zero-breach
conclusion survives both readings, since zero over the visits implies zero over the scopes. The
second correction ran against a directional claim this appendix previously made, that the
reachability motivator would only strengthen if `match` scopes were included. **Measured, the
motivator exists only because they are included.** Zero of the eight genuinely iterating loops
carries a non-empty entry stack, from the approximate linear scan whose bias over-reports
non-empty entries, so a zero from it is the stronger direction. The earlier claim is recorded
rather than deleted because it was wrong in direction, not merely in precision. The corpus also
moved on the same day. The V0.2.X line landed three scripts on operator direction, taking the
corpus to 248 scopes across 26 modules and adding the first three iterating loops that construct
composites, the reuse theorems' first subjects. Every figure above is stamped by which side of
that extension it was measured on, and the zero-breach measurement predates it. An earlier revision of this document interleaved the instantiation with the
theory. The reorganization changed no theorem, no proof, and no recorded standing.
