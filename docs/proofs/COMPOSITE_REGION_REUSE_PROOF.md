# Composite Region Reuse — Theorems and Proofs

> **Navigation**: [Documentation Root](../README.md)

> **STATUS: every premise now has measured standing, and the load-bearing distinction is this.
> Theorem A1 is unconditional. Theorems A2, A3, B1, and Corollary C rest on two invariants that
> are true of what the reference compiler emits and are NOT enforced by `verify()`. Streams never
> return, and loop bodies never pop operand-stack entries below their entry height, which is
> P6(d). The V0.2.X line has executed a bytecode shape that violates P6(d) and passes `verify()`,
> pinned in their `tests/loop_entry_floor.rs`. Consequently the conditional theorems apply to
> modules produced by the reference compiler and do NOT apply to arbitrary bytecode that merely
> verifies. Hand-written or foreign bytecode can defeat them. The commits pinning the two
> invariant measurements are to be recorded in Section 11 when they land.**

This document discharges part of the obligation stated in `docs/proofs/COMPOSITE_REGION_REUSE.md`,
which lives on the `v0.3.0` line and is cited here at commit `a49555bb`. The obligation document
states the memory model, the measurements, and the counterexample. This document supplies the
proofs. Nothing in the obligation document is assumed beyond the premises listed in Section 3, and
every premise carries its provenance.

The runtime evidence is indexed, with per-row provenance and reproduction commands, in
[`docs/decisions/COMPOSITE_REGION_EVIDENCE.md`](../decisions/COMPOSITE_REGION_EVIDENCE.md). That
index is guarded by `tests/proof_evidence_index.rs`, so its citations cannot silently go stale.
This document cites runtime facts through that index rather than restating them.

## 1. Scope

Proved here.

- **Theorem A1**, the branch bound. The worst-case memory usage of one dynamic instance of a
  conditional is bounded by the maximum of its arms. Unconditional.
- **Theorem A2**, branch overlap for a conditional executed at most once per stream cycle. A
  planner may assign the two arms' sites overlapping offsets. Conditional on P5.
- **Theorem B1**, restricted loop-body slot reuse. A construction site whose values are confined
  to their iteration, in the precise sense of Definition 8, may be given one slot reused across
  iterations, and worst-case memory accounting may count it once per stream cycle. Conditional on
  P5 and P6.
- **Corollary A3**, arm overlap inside loops, and **Corollary C**, the composed plan that is
  tighter than both current planners where the theorems' conditions hold.

Explicitly not proved here, and analyzed only as proposals in Section 9: the B2 copy-at-escape
strategy, and any instruction-set change. Section 8 states everything this document does not
establish.

Per operator ruling of 2026-08-23, this work proceeds on a top-level branch, merges into the
V0.2.X line, and reaches the V0.3.X line through the existing absorption flow. A theorem proved
here licenses but does not implement any change. The implementation surfaces and their owners are
stated in Section 10.

## 2. Model and definitions

The model is the one fixed by operator ruling and stated in the obligation document's Section 1.
It is restated here so that this document is self-contained.

**Definition 1, machine.** A virtual machine executes chunks of instructions drawn from the
66-opcode instruction set. Machine state comprises a stack of frames, each holding an operand
stack and local slots, an ephemeral arena region, a persistent arena region, a host-owned shared
buffer, and the host itself. The arena carries an epoch counter $e$.

**Definition 2, allocation.** Allocation in the ephemeral region is by bump pointer. The region is
cleared, and the epoch advanced, by `Op::Reset` and at no other time. Nothing is reclaimed between
resets.

**Definition 3, sites and paths.** Let $\mathcal{S} = \{s_1,\dots,s_m\}$ be the static composite
construction sites of a chunk, the `NewComposite` operations, and $\mathrm{sz}(s)$ the byte size
site $s$ constructs. Let $\Pi$ be the set of execution paths. For $\pi \in \Pi$,
$\mathrm{alloc}(\pi)$ is the multiset of dynamic site executions along $\pi$, a site inside a loop
executed $k$ times contributing $k$ occurrences. The memory a path consumes is
$M(\pi) = \sum_{s \in \mathrm{alloc}(\pi)} \mathrm{sz}(s)$ and the worst-case memory usage is
$\mathrm{WCMU} = \max_{\pi \in \Pi} M(\pi)$.

**Definition 4, regions, handles, and references.** Each dynamic execution of `NewComposite`
allocates a region $r$, an address interval in the ephemeral region. A handle is a triple of
address, length, and epoch. A resolve of a handle succeeds and returns the bytes at its address
exactly when the handle's epoch equals the current epoch, and otherwise fails `Stale`. A
**reference to $r$** is any value, in any location, that is a handle whose address range lies in
$r$, or a view derived from such a handle. Views obtained by projecting a nested composite field
alias the parent region and count as references to it.

**Definition 5, stream cycle and iteration.** A stream cycle is the dynamic interval between
consecutive executions of `Op::Reset`. An iteration of a loop $L$ is the dynamic interval from one
execution of $L$'s body head to the corresponding body end, whether reached at `EndLoop` or by
`Break`. For a site $s$ inside nested loops, the iteration of $s$ means the iteration of the
innermost loop enclosing $s$.

**Definition 6, plans and soundness.** A plan assigns each site a fixed offset. Under the
**no-reuse regime**, every dynamic execution of every site receives a fresh bump allocation, which
is the present virtual machine's behavior. Under a **reuse plan**, some sites' dynamic executions
share one offset. A reuse plan is **sound** when for every program and every path the machine
under the plan is observationally equivalent to the machine under no reuse, where the observations
are the outcome of every resolve, by the host or by the machine, and every scalar output.

**Definition 7, trust assumptions.** The host is trusted in the following sense. It may hold,
across resumes, any handle the machine has given it, and it may pass such a handle back into the
machine on resume. It does not fabricate handles it was never given. A native function receives
values and what it retains is unknown, which is why the native-call opcodes are classified as
escaping in P3. These are trust assumptions of the model, not theorems.

**Definition 8, confinement.** Let $v$ be the value constructed at site $s$ in iteration $n$, with
region $r_n$. The value is **confined** when, along every path, no reference to $r_n$ is ever an
operand of an opcode classified `Escapes` under P3. A site is confined when every value it
constructs on every path is confined.

Confinement is deliberately coarse. It forbids some harmless flows, for example a callee returning
the value back into the same iteration, because `Return` is classified by its worst case. The
coarseness weakens applicability and never soundness, and Section 8 records the refinement that
was not proved.

## 3. Premises

Each premise names its provenance. The mechanical guards live on this branch and are indexed in
`docs/decisions/COMPOSITE_REGION_EVIDENCE.md`.

| # | premise | provenance |
|---|---|---|
| P1 | The memory model of Definitions 1 and 2. | Operator ruling, obligation document Section 1. |
| P2 | A non-empty composite value carries an arena handle, not bytes. Resolve fails `Stale` exactly when the epoch has advanced. An overwrite in place advances nothing. | Executed. `tests/composite_escape_window.rs`, all three tests. |
| P3 | The 66 opcodes partition into `NoRegion`, `WithinIteration`, `CopiesOut`, `Escapes`, and the escaping set is exactly `Yield`, `SetLocal`, `Return`, `CallVerifiedNative`, `CallExternalNative`. `CopiesOut` opcodes write bytes rather than references. Opcodes not classified `Escapes` do not place a reference into any location that outlives the current iteration. | Totality is mechanical, `tests/composite_escape_routes.rs`. Per-row verdicts are analysis. Two `CopiesOut` rows and the `Yield` row are executed. The rest are read from dispatch. |
| P4 | `Op::Reset` executes once per stream cycle, at the end of the `loop main` body, and not per loop iteration. | Executed. `reset_is_once_per_stream_cycle_not_once_per_loop_iteration`. |
| P5 | No machine-internal location **that is ever subsequently read** survives `Op::Reset` holding a reference into the ephemeral region. After a reset, a pre-reset region is reachable only through host-held handles, which fail `Stale`, or through persistent-region copies. Every ephemeral composite-body read is epoch-checked at resolve time, so even a hypothetical stale internal handle would fault rather than return a wrong value. | **Confirmed by the V0.2.X line, 2026-08-23, and the basis is two facts, not one.** First, `Op::Reset` clears the current frame's locals and truncates its operand stack, `src/vm.rs:5304`, read from dispatch. Second, a caller frame beneath a nested stream can hold stale references and is never resumed, because stream chunks emit no `Return`. The second fact is a **code-generation invariant, not a structural one**, executed over five shapes in their `tests/stream_never_returns.rs`, commit to be recorded here when it lands. Stating the first fact alone would make the nested-stream arrangement a counterexample to the premise. The epoch-check clause is read from dispatch, `FlatComposite::resolve` and `ArenaHandle::get`, with `nested_view` bounds-checked as a real fault. |
| P6 | Within a cycle, (a) each iteration of a loop restores the exact entry operand stack at the back edge, height and per-slot shape, so no entry created in the iteration survives above the entry height, (b) `Break` edges are joined with the same discipline, (c) a callee frame's stack and locals are unreachable after its `Return`, and every call made within an iteration returns within it, which follows from totality, and (d) a loop body never pops operand-stack entries below its entry height. | (a) **Confirmed**, `TypedError::LoopNotNeutral` in the `Op::Loop` arm of `src/verify_typed.rs`, read from dispatch. Neutrality compares **shapes, not identities**, so the correct reading is that no new entry survives the back edge and the stack's shape is invariant, never that the entries are identical across iterations. (b) **Confirmed**, `join_stacks`, read from dispatch. (c) Read from dispatch. (d) **SETTLED 2026-08-23, in the unfavorable direction: a code-generation invariant only, and the verifier gap is executed, not conjectured.** `verify()` accepts a shape that pops a below-entry entry inside a loop and pushes a same-shape replacement, pinned in the V0.2.X line's `tests/loop_entry_floor.rs`. The typed pass floors pops at the frame, not at the loop entry, read from dispatch, and the frame floor does not incidentally cover the loop floor, because 122 of 245 `Loop` instances in the shipped corpus carry a non-empty operand stack at entry, executed and pinned. The emission side was measured exactly, by a per-path floor check inside the typed pass's own abstract interpretation, proven to fire on the breaching shape: **zero breaches over 588 loop instances across 23 shipped modules.** That instrumentation is reverted and the zero is **a measurement at a commit, not a standing guarantee**, and this document cites it as exactly that. A linear depth scan also reported zero and was discarded as exact for only 4 of the 245 loops, which is recorded because the flattering number came from a broken instrument first. |

The proof relies on P3 only through totality and the category semantics stated in its row. If any
single per-opcode verdict is disputed, the classification table in
`tests/composite_escape_routes.rs` is the place to argue it, and this document must be re-examined
against the outcome.

## 4. Lemmas

**Lemma 1, origination.** Every reference to a region $r$ is connected by a chain of transports
and view derivations to the single `NewComposite` execution that allocated $r$.

*Proof.* By P3 the instruction set contains no opcode that constructs a handle from scalar data.
`NewComposite` produces a reference to the region it has just allocated. When it consumes a
composite operand it copies that operand's bytes inline, by its `CopiesOut` classification, so the
produced reference is to the new region only. Projection opcodes derive views from an existing
reference. Every other opcode moves, duplicates, or consumes existing values. By Definition 7 the
host presents only handles it received, so no reference enters from outside except by returning
along a chain that began at the machine. $\blacksquare$

**Lemma 2, confinement is preserved to the iteration boundary.** Assume P3 and P6. Let $v$ be
confined, constructed at site $s$ in iteration $n$ of loop $L$, with region $r_n$. Then at the end
of iteration $n$ no reachable location holds a reference to $r_n$.

*Proof.* By induction over the instructions executed during iteration $n$, with the invariant that
every reference to $r_n$ resides either in an operand-stack entry above the iteration entry depth
of the frame executing $L$, or inside a frame created during iteration $n$.

The invariant is established at origination. `NewComposite` at $s$ executes inside the body, so
its result is pushed above the entry depth of the executing frame, and by Lemma 1 this is the only
reference at that moment.

Preservation is by cases on the category of the executed opcode, under P3.

*Case `Escapes`.* Excluded. Confinement says no reference to $r_n$ is ever an operand of such an
opcode. In particular no `SetLocal` ever stores one, so no local slot in any frame ever holds a
reference to $r_n$, and `GetLocal` can never produce one.

*Case `CopiesOut`.* `SetData` and `SetDataIndexed` write the referenced bytes into persistent or
shared storage. Bytes are not references, so no new location holding a reference arises.
`NewComposite` consuming a reference to $r_n$ as a nested operand copies its bytes inline into the
new region, by the executed evidence row, and consumes the operand. The produced value references
the new region, not $r_n$.

*Case `WithinIteration`.* `Dup` and the projections produce stack entries in the executing frame,
above the entry depth. `Call` transfers operand-stack entries into a callee frame, which is a
frame created during iteration $n$. Within the callee the same case analysis applies. By P6 the
callee returns within the iteration and its frame becomes unreachable at that point. Its result
value cannot be a reference to $r_n$, because that would make a reference to $r_n$ an operand of
`Return`, which confinement excludes.

*Case `NoRegion`.* These opcodes consume composite operands, if any, and produce scalars or
nothing. References are removed, never created. `Loop`, `If`, `Else`, `EndIf`, `Break`, `BreakIf`,
and `EndLoop` move no data, and any stack unwinding they perform removes entries.

One flow enters from outside the instruction stream. A resume after `Yield` pushes a host-supplied
reply. By Definition 7 the host presents only handles it received, and confinement ensures the
host never received a reference to $r_n$, so the reply cannot carry one.

At the end of iteration $n$, by P6(a) the operand stack of the frame executing $L$ stands at its
entry height with its entry shape, so every entry above the entry height is gone, and by P6(c)
every frame created during the iteration has returned. It remains to rule out a reference to
$r_n$ sitting at or below the entry height, which shape neutrality alone would permit if the body
popped a below-entry entry and pushed a same-shape replacement built in iteration $n$. P6(d)
excludes exactly that. Both location classes of the invariant are therefore empty. $\blacksquare$

The dependence on P6(d) is real and is why it is listed. Without it, a value could cross the back
edge through pure stack operations, touching no escaping opcode, and confinement would not see
it. The V0.2.X line has answered the question this dependence raised. P6(d) is a code-generation
invariant only, the same epistemic species as the stream-never-returns half of P5, and the
verifier's acceptance of a breaching shape is executed and pinned. The lemma therefore holds for
reference-compiled modules and can be defeated by bytecode that merely verifies.

## 5. The branch theorems

**Theorem A1, branch bound.** For every path $\pi$, replacing the contribution of each dynamic
instance of a conditional by the maximum of its two arms' contributions yields an upper bound on
$M(\pi)$. Consequently the verifier's rule
$\mathrm{plan}(\texttt{if}\ A\ \texttt{else}\ B) = \max(\mathrm{plan}(A), \mathrm{plan}(B))$,
applied per instance and multiplied by iteration counts where the conditional sits inside a loop,
over-approximates worst-case memory usage. This holds unconditionally.

*Proof.* Structured control flow executes exactly one arm per dynamic instance of `If`. The
instance's contribution to $M(\pi)$ is therefore the sum over the arm taken, which is at most the
maximum of the two arms' sums. Summing the bound over all instances of the path preserves the
inequality. $\blacksquare$

The obligation document's Section 3 states that no path contains sites of both arms. Under the
multiplicity path model of Definition 3 that statement fails whenever the conditional sits inside
a loop, because one path may take the first arm in one iteration and the second arm in another.
Theorem A1 is the form that survives, and it is the form the verifier implements at
`src/verify.rs:992`. The overlap claim needs the separate treatment below.

**Theorem A2, arm overlap once per cycle. Conditional on P5.** Let $s_i$ and $s_j$ be sites in the
two arms of a conditional that executes at most once per stream cycle. A plan assigning $s_i$ and
$s_j$ overlapping offsets is sound.

*Proof.* Within one cycle the conditional executes at most once and takes one arm, so at most one
of the two regions is allocated in that epoch, and by Lemma 1 no reference to the other exists at
all. Any reference surviving into a later cycle is, by P5, host-held, and by P2 every resolve of
it fails `Stale` under the reuse plan and under no reuse alike, because the epoch advanced at the
intervening reset in both regimes. No observation distinguishes the regimes. $\blacksquare$

**Corollary A3, arm overlap inside loops. Conditional on P5 and P6.** If the conditional sits
inside a loop and every site in both arms is confined, then the plan may overlap the arms and
additionally reuse the shared slot across iterations. *Proof.* Per iteration at most one arm
executes, so at most one write to the shared slot occurs per iteration, and by Lemma 2 no
reference to any prior iteration's value survives its iteration. The cross-cycle case is as in
Theorem A2. $\blacksquare$

## 6. Theorem B1, restricted loop-body slot reuse

**Theorem B1. Conditional on P5 and P6.** Let $s$ be a confined site inside a loop $L$. A plan
assigning $s$ one slot reused across iterations and across cycles is sound. Consequently
worst-case memory accounting may count $s$ once per cycle, contributing $\mathrm{sz}(s)$ in place
of $k \cdot \mathrm{sz}(s)$.

*Proof.* The reuse plan differs from no reuse only in that $r_n$ and $r_{n+1}$ share addresses. An
observational divergence therefore requires a resolve of a reference to that address range that
returns different bytes in the two regimes. Bytes at the shared address differ from the no-reuse
bytes of $v_n$ only from the moment the $(n{+}1)$-th execution of $s$ begins to write. So a
divergence requires a reference to $r_n$, created in iteration $n$, resolved at or after the start
of the $(n{+}1)$-th execution of $s$ within the same epoch, or resolved in a later epoch.

Within the epoch, the $(n{+}1)$-th execution of $s$ lies in iteration $n{+}1$ of $L$, which begins
after iteration $n$ ends. By Lemma 2, at the end of iteration $n$ no reachable location holds a
reference to $r_n$, and by Lemma 1 none can subsequently arise, since the originating chain is
closed and the host, by Definition 7 with confinement, never received one. So no such resolve
exists.

Across epochs, by P5 the only surviving references are host-held, and by P2 every resolve of them
fails `Stale` identically in both regimes.

All other observations are unaffected. Every resolve of a reference to $r_m$, for the current
iteration $m$, occurs while the slot holds exactly $v_m$'s bytes, which equal the no-reuse bytes,
and scalar computation never reads the region except through resolves. Hence observational
equivalence, and the accounting consequence follows by applying Definition 3 to the reuse plan's
allocation behavior, one live allocation for $s$ per cycle. $\blacksquare$

**Remark, the counterexample is bracketed, not contradicted.** The obligation document's Section
4.1 program yields the loop-body composite to the host, so a reference to $r_n$ is an operand of
`Yield` and the site is not confined. Theorem B1 does not apply to it, and the backend's current
unconditional reuse remains unsound on exactly such sites, per the obligation document's Section
4.1.1. The theorem and the counterexample partition the space rather than conflict.

**Remark, the embedder obligation.** Confinement treats `CallVerifiedNative` and
`CallExternalNative` as escaping. Under this restriction no obligation falls on the embedder. Any
future relaxation that admits a native call on the strength of a host promise not to retain the
composite is an obligation documented on the embedder, in those words, and is not licensed by this
document.

**Remark, static checkability.** Confinement is a forward dataflow property over the chunk. The
typed operand-stack verifier already reconstructs operand shapes by abstract interpretation, and
tracking whether a `NewComposite` result can reach one of the five escaping opcodes is a property
of the same kind. Any operand whose flow cannot be established must be treated as escaping. This
is implementation guidance, not part of the proof.

## 7. Corollary C, the composed plan

**Corollary C. Conditional on P5 and P6.** Define $\mathrm{alloc}^{\ast}(\pi)$ from
$\mathrm{alloc}(\pi)$ by counting each confined loop site once per cycle, counting every other
loop site with its full multiplicity, and bounding each conditional instance by the maximum of its
arms per Theorem A1. Then

$$
\mathrm{plan} \;=\; \max_{\pi \in \Pi} \sum_{s \in \mathrm{alloc}^{\ast}(\pi)} \mathrm{sz}(s)
\;\;\geq\;\; \mathrm{WCMU}
$$

and $\mathrm{plan}$ is no larger than either current planner's figure on the constructs where that
planner is loose. The overlap forms, Theorem A2 and Corollaries A3 and B1, license the
corresponding slotted layout. The soundness arguments compose because each is an observational
equivalence against the same no-reuse baseline and their location sets are disjoint by
construction.

This is the corrected form of the obligation document's Section 5, whose composition predates the
`SetLocal` finding and assumes Theorem B unconditionally. Where a site is not confined, its
contribution reverts to $k \cdot \mathrm{sz}(s)$, and the backend must also provision it per
iteration, which it does not today.

## 8. What this document does not establish

1. **The per-opcode verdicts of P3.** Totality is mechanical. Each row is analysis, with three
   rows execution-backed and the rest read from dispatch. A wrong `CopiesOut` row would make
   Lemma 2, and everything above it, unsound. Disagreements go to the table in
   `tests/composite_escape_routes.rs`.
2. **The conditional theorems hold for the compiler, not for the verifier's acceptance surface.**
   Two load-bearing facts are invariants of what the reference compiler emits rather than of what
   `verify()` refuses. Streams never return, which closes P5's nested-stream hole and re-runs as
   a pin on every build of the V0.2.X line, and loop bodies never pop below their entry height,
   which is P6(d), whose verifier-level gap is executed and pinned while its emission-side zero
   is a measurement at a commit only. A future returning stream, a code-generator change, or
   hand-written bytecode defeats Theorems A2, A3, B1, and Corollary C without failing
   verification. Theorem A1 depends on neither fact. Closing P6(d) structurally, by flooring the
   typed pass's pops at loop entry, would convert the compiler property into a verifier
   guarantee, and Section 10 records whose decision that is.
3. **A stale internal handle is an error, not a wrong value, and no route to one was found.**
   `Op::GetField` resolves through the epoch check and faults `InvalidBytecode` on staleness. The
   V0.2.X line found no live route to a stale local and does not claim unreachability. This
   document relies on the fault behavior only as defense in depth and on no claim of
   reachability either way.
4. **The refined escape conditions.** A `SetLocal` to a binding whose slot dies within the
   iteration, and a `Return` that lands in the same iteration of the same chunk, are believed
   harmless and are not proved so. The proven condition is the coarse one of Definition 8.
5. **The loop-dominated direction of the planner gap**, the obligation document's Section 6.2.
   Nothing here measures whether the backend under-provisions relative to worst-case memory usage
   on any real module. The V0.3.X line has offered to measure it on request.
6. **Anything about the native backend's lowering.** Every executed premise is against the
   virtual machine.
7. **B2.** Section 9 is a proposal, not a proof.
8. **Host behavior.** Definition 7 is a trust assumption. A host that fabricates handles, or a
   native that retains one, is outside the model.

## 9. B2 and instruction-set remedies, as proposals

**B2, copy at escape.** Reuse becomes unconditional if every escaping flow hands out a copy rather
than a reference into a reused slot. Concretely, at each of the five routes the value crossing the
route boundary must be a copy whose storage is outside every reused slot, in the persistent region
or in host memory. The escaped handle then references the copy, so every later resolve of it is
independent of the slot's overwrites, and the window question of the obligation document's Section
4.0.1 is answered by construction rather than bounded. The obligations B2 creates are the
correctness of the copy, its worst-case execution time cost at every escape site, and the
provisioning of the copy storage, which re-enters the memory bound. None of this exists today, and
this document does not license it.

**Instruction-set position.** No new opcode is required for anything proved here. Theorem B1 is a
planner and verifier change over the existing instruction set. B2, if ever adopted, has at least
two lowerings that need no new opcode, a compiler-inserted copy through existing constructs, or a
semantics change to `Yield` and its siblings. A dedicated copy opcode would require the strong
justification the operator demands for instruction-set modification, and no such justification
arises from this work. The classification test pins the opcode count at 66 and fails on any
addition, which is the intended forcing function.

## 10. Change control

| consequence of adoption | surface | owner | decision |
|---|---|---|---|
| loop accounting stops multiplying confined sites | `src/verify.rs:1079` | V0.2.X line | **operator**, it lowers a published worst-case-memory-usage figure |
| branch maximum | `src/verify.rs:992` | V0.2.X line | already implemented, Theorem A1 justifies it |
| backend stops reusing slots of unconfined sites | native backend planner | V0.3.X line | required for soundness independent of this proof, per the obligation document's Section 4.1.1 |
| backend may overlap exclusive arms | native backend planner | V0.3.X line | licensed by Theorems A2 and A3, within the compiler-emitted scope Section 8 states |
| verifier floors operand-stack pops at loop entry, converting P6(d) from an emitted invariant into an enforced one | `src/verify_typed.rs` | V0.2.X line | **operator**, it narrows the acceptance surface of `verify()`. The measured cost is zero, no loop instance among the 588 in the shipped corpus would be rejected, and the V0.2.X line has raised the item on its own channel |

This document's conclusions authorize none of these changes. Each is a request to its owning line,
and the first is an operator decision because it weakens the crate's headline guarantee in a
changelog-visible way.

## 11. Provenance

The obligation is cited at `v0.3.0` commit `a49555bb`. The evidence index and its guard are on
this branch at `docs/decisions/COMPOSITE_REGION_EVIDENCE.md` and `tests/proof_evidence_index.rs`.
The premise confirmations were requested from the V0.2.X line and answered by measurement on
2026-08-23. P5 was confirmed in the corrected two-part form its row records, P6 clauses (a)
through (c) were confirmed with the shape-not-identity reading, and P6(d) was identified in the
same exchange and settled the same day as a code-generation invariant only, with the verifier's
acceptance of a breaching shape executed, the frame-floor-is-not-the-loop-floor fact executed,
and the zero-breach emission measurement executed at a commit with its instrumentation reverted.
The commits carrying their `tests/stream_never_returns.rs` and `tests/loop_entry_floor.rs` pins
are to be recorded here when they land, and rows marked read from dispatch are not to be promoted
to executed without running them. The operator rulings relied on are the memory model, the branch
topology, and the scope, all of 2026-08-23, and the standing rule that instruction-set
modification requires strong justification.
