# Composite region reuse — PROOF OBLIGATION

> **STATUS: the obligation is stated. THE PROOF IS NOT WRITTEN.**
> Everything below is setup, measurement, and what must be discharged. Nothing below is a proof, and
> no claim here should be cited as one.

## 0. Why this document exists

Two independent memory planners disagree, and each is tighter than the other on a different
construct. Adopting either one's number — or a combined strategy tighter than both — changes a
published WCMU bound. **That is the crate's headline guarantee, so the change needs a proof rather
than a measurement.**

## 1. The memory model

Assumed, per operator ruling:

- The native runtime uses a **bump arena** for heap and stack.
- Allocation is **ephemeral**: the region is cleared on `RESET` and at no other time.
- **Nothing is reclaimed within a run.** There is no free, no refcount, no collector.
- Consequently **memory locations are nominally unfixed**: a branch that allocates a different
  amount shifts the concrete address of every subsequent allocation on that path.

Let

$$
\mathcal{S} = \{s_1,\dots,s_n\}
$$

be the **static composite-construction sites** of a chunk — the `NewComposite` operations — and let
$\mathrm{sz}(s)$ be the byte size site $s$ constructs.

Let $\Pi$ be the set of **execution paths** through the chunk. For $\pi \in \Pi$ let
$\mathrm{alloc}(\pi)$ be the multiset of sites executed along $\pi$, **with multiplicity** — a site
inside a loop body executed $k$ times contributes $k$ occurrences.

Under the model above, the memory a path consumes is

$$
M(\pi) \;=\; \sum_{s \in \mathrm{alloc}(\pi)} \mathrm{sz}(s)
$$

and the worst-case memory usage of the chunk is

$$
\mathrm{WCMU} \;=\; \max_{\pi \in \Pi} M(\pi).
$$

**This is the quantity to be bounded.** Every strategy below is an attempt to compute or
over-approximate it.

## 2. The two planners, as measured

Neither is $\mathrm{WCMU}$. Each is tighter on one construct and looser on the other.

| | loop body, $k$ iterations | mutually exclusive `if` arms |
|---|---|---|
| backend `plan_chunk_region` | **one slot**, reused | **both arms**, disjoint offsets |
| verifier `wcmu_region` | $k \times \mathrm{sz}$ | $\max$ of the two arms |

Measured on minimal programs:

| program | backend | verifier |
|---|---|---|
| one 24-byte site in each arm of an `if` | $48$ | $24$ |
| one 24-byte site in a 4-iteration loop (plus a 32-byte array) | $56$ | $128$ |

So

$$
\text{backend} \not\geq \mathrm{WCMU} \quad\text{and}\quad \text{verifier} \not\leq \mathrm{WCMU}
$$

are **both** live possibilities, and the direction differs per module. The eleven corpus modules
recorded as "backend exceeds verified" are the branch-dominated ones; **the loop-dominated direction
has never been measured.**

## 3. Theorem A — branch overlap

> Let $s_i, s_j$ be sites in the *then* and *else* arms of one `if`. Then no
> $\pi \in \Pi$ contains both. Hence a planner may assign them overlapping offsets without any
> $\pi$ observing aliasing, and

$$
\text{plan}(\text{if } A \text{ else } B) \;=\; \max\bigl(\text{plan}(A),\, \text{plan}(B)\bigr).
$$

**Expected to be short.** It follows from control-flow exclusivity alone and needs no liveness
argument. The verifier already implements it; the backend does not.

## 4. Theorem B — loop-body slot reuse

**This is the real theorem, and it does not hold unconditionally.**

> *Proposed:* let $s$ be a site in a loop body executed $k$ times. A planner may assign $s$ one slot
> reused across iterations, contributing $\mathrm{sz}(s)$ rather than $k \cdot \mathrm{sz}(s)$.

Reuse overwrites iteration $n$'s bytes when iteration $n{+}1$ executes. **It is sound only if no
value constructed at $s$ in iteration $n$ is readable after iteration $n$ ends.** Formally, writing
$v_n$ for the value constructed at $s$ in iteration $n$, reuse requires

$$
\forall n:\quad \mathrm{live}(v_n) \subseteq \mathrm{iteration}_n
$$

where $\mathrm{live}(v)$ is the set of program points at which $v$ may be read, **including reads by
the host after control leaves the virtual machine.**

### 4.0.1 THE ESCAPE SET IS BOUNDED BY `RESET`, NOT BY THE YIELD

Established by the `v0.2.3` line, 2026-08-22, and **it widens the obligation materially**:

> The host may hold a yielded handle, **resume**, and read it afterwards. It resolves fine until
> `RESET`.

So the naive reading — *"values live at the moment of the yield"* — is **too narrow**. The correct set
is

$$
\mathrm{live}(v) \;=\; \{\text{points at which the host could still resolve } v\text{'s handle}\}
$$

which extends **to the next `RESET`**, across arbitrarily many intervening resumes and iterations.

**MEASURED 2026-08-23, and the real bound is TIGHTER AND MORE USEFUL than "until `RESET`".** Pinned
by the `v0.2.3` line in `tests/composite_escape_window.rs`:

| step | state | epoch | handle held from iteration 1 |
|---|---|---|---|
| 0 | `Yielded` (iteration 1) | 0 | resolves → 1 |
| 1 | `Yielded` (iteration 2) | 0 | resolves → 1 |
| 2 | `Yielded` (straight-line site) | 0 | resolves → 1 |
| 3 | `RESET` | 1 | **`Stale`** |

**`Op::Reset` is emitted ONCE PER STREAM CYCLE**, at the end of the `loop main` body — *not* once per
`for` iteration. The op stream confirms exactly one `Reset`, with the `for` a `Loop`/`EndLoop` pair
wholly inside the cycle, containing the `Yield`.

$$
\text{escape window} \;=\; \textbf{one stream cycle} \;\supseteq\; \text{arbitrarily many loop iterations}
$$

**"Until `RESET`" without saying when `RESET` fires is not actionable; this is.** B1's author needs a
bound, and the bound is neither the yield nor the iteration.

**THE ASSERTION THAT CARRIES THE THEOREM IS NOT THE ONE THAT LOOKS LIKE IT.** *"The held handle still
reads 1"* passes on a runtime that merely yields the same value twice. The load-bearing property is
that **at the instant iteration 2 yields, the held handle and the fresh one resolve to DIFFERENT
values — 1 and 2, both live, both resolving.** That is exactly what one reused slot collapses: same
address, same epoch, both `resolve` calls succeed, both return 2.

**The window CLOSING is asserted too** — without it the suite would pass on a runtime that never
invalidates anything, which is a different and worse property than the one being proved over.

**A proof of B2 over the narrower reading would be unsound.** Copying "at the yield instant" does not
discharge the obligation if the host can resolve the handle three resumes later; what must be copied
is anything whose handle remains resolvable, and that window closes only at `RESET`.

### 4.1 THE CONDITION FAILS AS STATED — counterexample

**This compiles today:**

```keleusma
struct P { a: Word, b: Word, c: Word }
loop main(t: Word) -> P {
  let xs = [1, 2];
  for x in xs { let _ = yield P { a: x, b: x, c: x }; }
  let _ = yield P { a: 0, b: 0, c: 0 };
  P { a: 9, b: 9, c: 9 }
}
```

A composite constructed in the loop body is **yielded to the host**. The host receives it, and
control returns to the loop, which constructs the next iteration's value at the same offset if the
slot is reused. So $\mathrm{live}(v_n)$ **escapes** $\mathrm{iteration}_n$.

**The backend reuses that slot today**, and the question of whether that is a live mis-compilation is
**no longer open.**

### 4.1.1 ANSWERED — a yielded composite is a HANDLE, and the epoch guard does not cover this

Established by the `v0.2.3` line against the runtime, 2026-08-22:

- After B28 the only non-empty composite representation is
  $\texttt{FlatComposite::Arena(ArenaHandle}\langle[\texttt{u8}]\rangle\texttt{)}$ — **a pointer and
  length into the arena**, read through `resolve`. **A yielded composite is not a copy.**
- The handle carries the arena **epoch**, and `resolve` fails `Stale` when a `RESET` advances it.
- **An overwrite in place advances nothing.** Same address, same epoch ⟹ `resolve` **succeeds** and
  returns iteration $n{+}1$'s bytes to a host that asked for iteration $n$'s.

$$
\textbf{Consequence: a silent wrong value, not a }\texttt{Stale}\textbf{ error.}
$$

**So slot reuse across iterations is UNSOUND TODAY for any composite that leaves its iteration by
`yield`.** This is the live-defect branch, not the benign one. It is not caught by the epoch guard,
and it is not caught by the differential — no corpus module has the shape.

Their independent measurement of the §4.1 program: stack $320$, heap $112$, where
$112 = 2\times24$ (loop-body site, two iterations) $+\;24 + 24$ (the straight-line sites)
$+\;16$ (the array) — **exactly $k \times \mathrm{sz}$.** The backend's $\mathrm{sites} \times
\mathrm{sz}$ would give $88$.

### 4.2 What Theorem B must therefore become

Not "reuse is sound", but one of:

- **B1 (restriction).** Reuse is sound for a loop body containing no `yield` and no other construct
  by which a constructed value escapes the iteration. *The escaping constructs must be enumerated and
  the enumeration justified as exhaustive* — a survivor makes the restriction unsound rather than
  incomplete. **And "escapes" must be read per §4.0.1** — resolvable until `RESET`, not live at the
  yield. **THE ENUMERATION IS NOW DONE — see §4.3 — AND IT IS FIVE ROUTES, NOT ONE.** A B1 restricted
  to "no `yield`" is **unsound**: `SetLocal` to a binding declared outside the loop escapes with no
  yield and no host involved.
- **B2 (copy).** Reuse is sound unconditionally if every escape copies out of the region. **No such
  copy exists today** (§4.1.1), so B2 is a proposal for a change rather than a description of the
  present system. It moves the obligation to the copy's correctness and its WCET cost, which is then
  no longer free. **B2 must define "escape" over §4.0.1's wider set** — a copy at the yield instant
  does not discharge it, because the host can resolve the handle after resuming.

**A proof of B1 must not assume the language prevents escape.** The counterexample above shows it
does not.

## 4.3 THE ESCAPE SET, ENUMERATED BY OPCODE — and B1's naive form is UNSOUND

Discharged by the `v0.2.3` line, 2026-08-23, in `tests/composite_escape_routes.rs`.

### The method, which is why this is not a list

**A list of routes someone could think of cannot answer the question**, whatever it contains — it has
the shape of *coverage that is a property of the case list, mistaken for a property of the thing under
test.* Their line has recorded that meta-defect six times.

**So the classification starts from all 66 opcodes and classifies every one**, with **totality
asserted against the `Op` enum read out of `src/bytecode.rs` at test time.**

$$
\text{a route can be missed only by MISCLASSIFICATION, never by OMISSION}
$$

and a **new opcode fails the test** rather than slipping through unclassified. Weaker than a proof,
much stronger than a list.

### The five escaping routes

| opcode | why it escapes |
|---|---|
| `Yield` | the host receives the **handle, not a copy** — verified by execution |
| **`SetLocal`** | **a binding declared OUTSIDE the loop keeps the handle after the iteration ends.** The opcode cannot distinguish an inner slot from an outer one — that is a property of the slot the compiler assigned — so it is classified by its **worst case** |
| `Return` | a callee **invoked from** the loop body returning a composite it built. A `return` in the loop itself exits the loop, so that direction is not the case |
| `CallVerifiedNative` | **host trust boundary** |
| `CallExternalNative` | **host trust boundary** |

### ⚠ `SetLocal` DEFEATS "a loop body containing no `yield`"

$$
\texttt{let mut last = ...; for x in xs \{ last = P \{ .. \} \}}
$$

**No `yield`. No host. No native call.** Under one reused slot, `last` aliases whatever the final
iteration wrote. **B1 restricted to "no `yield`" is NOT SUFFICIENT** — the restriction must be stated
over all five routes, and the `SetLocal` case is the ordinary one, not the exotic one.

### The two native calls are an EMBEDDER obligation, not a language property

A native receives the composite and **what it retains is the host's affair.** They are classified as
escaping because they **must be assumed to be.** If B1 excludes them, that is a **documented
obligation on the embedder**, and B1 must **say so in those words** rather than silently count them
safe.

### The two "safe" answers are backed by EXECUTION, and the asymmetry is deliberate

> A wrong `Escapes` makes the restriction **loose**. A wrong `CopiesOut` makes it **UNSOUND.**

So the safe ones were **run, not read**:

- **A composite written to a `private data` slot COPIES.** It survives two resets that reclaim the
  region it was built in, reading back correctly at a later epoch — **which a stored handle could not
  do.** `write_data_slot` packs the body into the persistent pool at its baked offset. *(`private`
  was used rather than `shared` deliberately: a host `&mut [u8]` buffer must copy by construction, so
  proving that would have proved the easy half.)* **This confirms the guess §6.3 previously withdrew,
  in a stronger form than the guess.**
- **Nesting into a flat composite COPIES.** `Outer { i: Inner { x: 11, y: 22 }, z: 33 }` resolves to
  `[11, 22, 33]` in 24 contiguous bytes — the child's words are **inline in the parent's own
  allocation**, not a handle to the child's.

**STATED WITH ITS LIMIT: the BOXED path DOES alias.** It stores operands as separate values. It does
not arise for the transitively-scalar composites this proof concerns, **but the boundary is given
rather than a claim that reads as universal.**

### What is NOT claimed

**Not that each individual opcode's classification is correct.** What is mechanically guaranteed is
**totality, and that it stays total.** Three entries are backed by execution; the rest by reading the
VM dispatch — and the test says which where a reader meets it.

**If the proof's author disagrees with any single classification, the table is the place to argue**,
and the test makes the disagreement concrete rather than rhetorical.

Mutation-tested three ways — a dropped opcode, a stale entry naming a non-opcode, and an escaping
route reclassified as safe. All three fire. **The middle one matters most: it catches a table
maintained against memory rather than against the enum.**

## 5. What "best" means, and what it costs

The strategy tighter than both planners is **Theorem A and Theorem B together**, giving

$$
\mathrm{plan} \;=\; \max_{\pi} \sum_{s \in \mathrm{alloc}^\ast(\pi)} \mathrm{sz}(s)
$$

with $\mathrm{alloc}^\ast$ counting a loop-body site once rather than per iteration — **valid only
where Theorem B's condition holds.** Where it does not, that site reverts to $k \cdot \mathrm{sz}(s)$.

**A proof licenses the change on BOTH sides.** The verifier currently computes $k \cdot \mathrm{sz}$
for loop bodies; adopting B lowers published WCMU. That is a change to `src/verify.rs`, owned by the
`v0.2.3` line.

## 6. Open questions the proof does not have to answer, but a reader should not assume away

1. ~~**Does the native lowering hand the host a pointer into the region, or a copy?**~~
   **ANSWERED 2026-08-22 — a POINTER.** See §4.1.1. B2 does *not* already hold, and §4.1 **is** the
   live-defect branch. This question is closed and the answer is the unfavourable one.
2. **Is the loop-dominated direction of the planner gap ever unsafe today?** The backend under-counts
   a loop relative to $\mathrm{WCMU}$; a host provisioning from the backend's supplement alone could
   under-provision. No corpus module has been checked for this shape.
3. ~~**Are there escape routes besides `yield`?**~~ **DISCHARGED 2026-08-23 — see §4.3. THE ANSWER
   IS FIVE, AND ONE OF THEM BREAKS B1 AS ORIGINALLY WRITTEN.** The text below is the state of this
   question before that work and is kept for the reasoning it records.

   ~~**STILL OPEN, AND SILENCE IS NOT CLEARANCE.**~~ The
   measurement in §4.0.1 settles `yield` as ONE member of the escape set and enumerates nothing else.
   Two named candidates, **neither verified**: a composite written into a `shared data` slot, and a
   composite reaching the host as an `fn`'s RETURN VALUE rather than through a yield. My earlier note
   that shared-data writes "cop[y] bytes and [are] likely safe" was a guess and is withdrawn as
   support for anything. **B1 requires the enumeration justified as exhaustive, and one survivor makes
   the restriction UNSOUND rather than incomplete.** The obligation sits on the `v0.2.3` line's
   surface, not this one.

## 7. Provenance

Every figure in §2 and the counterexample in §4.1 were produced by running the stated programs
through the reference compiler and both planners on 2026-08-22. **The characterisation of the
verifier as computing "peak concurrent liveness" appeared in an earlier revision of the resume
document and is WRONG** — it accumulates cumulatively and takes a maximum only across mutually
exclusive branches. That error is recorded because it is the reason this obligation was mis-scoped
once already.
