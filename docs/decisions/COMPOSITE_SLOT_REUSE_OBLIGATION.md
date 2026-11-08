# The composite slot-reuse obligation, in one place

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: required for soundness, NOT discharged.** This is a synthesis of measurements made across
roughly a dozen increments, assembled so the design decision does not require reassembling it. **Every
figure below was re-derived for this document unless marked CARRIED.**

## The defect

`region::plan_chunk_region` gives each construction site **one offset for the life of the chunk**, so
a site inside a loop rewrites the same bytes every iteration. After B28 a composite is
`FlatComposite::Arena(ArenaHandle<[u8]>)` — a pointer and a length, not a copy — and the epoch it
carries advances only on `RESET`. **An overwrite in place advances nothing.** A host holding iteration
*n*'s handle calls `resolve`, succeeds, and reads iteration *n+1*'s bytes.

$$\textbf{A silently wrong value, not a } \texttt{Stale} \textbf{ error.}$$

Established against the runtime by the `v0.2.3` line; `docs/proofs/COMPOSITE_REGION_REUSE.md` §4.1.1.

## What guards it, and how well

| guard | state |
|---|---|
| `LowerError::YieldEscapingLoopComposite`, at the placement | **present**, cost **0 newly-refused chunks** |
| fireability | **proven** by removing `Stream` from compiled bytecode and observing the refusal |
| the `Stream` refusal | shadows it — the escaping shape never reaches the placement today |
| interprocedural residual | **measured 0**: crude 0-by-call / 2-by-return, both ruled out by a scalar boundary |

**Blast radius when `Stream` lands: exactly one module**, `13_telemetry_stream.kel`, which is refused
today anyway. **Coverage does not fall**; the refusal changes from *unimplemented feature* to
*soundness*, which is why writing it early was worth doing.

## What is NOT covered

- **Slot reuse itself is unchanged.** The backend still reuses unconditionally; the guard refuses the
  shape rather than fixing the placement.
- **The interprocedural case is bounded, not eliminated.** A composite built in a loop, returned, and
  yielded by a caller is invisible to a single-chunk predicate. Measured empty over this corpus; that
  is a fact about the corpus.
- **A tail yield is lowered as a RETURN**, not a suspension, so it is outside this hazard entirely —
  a tail-yielded composite is built once and no later iteration overwrites it.

## The decision, which is the operator's

**Discharging this requires the planner to consume a confinement verdict — and consuming none is
exactly why a wrong verdict cannot miscompile today.** Those two cannot both be had for free.

| option | cost |
|---|---|
| **Leave as is** — refuse the shape | a sound program of that shape is rejected; cost is **zero today**, one module when `Stream` lands |
| **Per-iteration regions** | **unbounded in the iteration count** — gives up the bounded-memory property the backend exists to provide |
| **Planner consumes a verdict** | a wrong verdict becomes able to miscompile, forfeiting the current immunity |
| **Advance the epoch on overwrite** | would turn the silent wrong value into a `Stale` error, but epoch semantics live in `src/vm.rs` and the arena, **which this line may read and must not edit** |

**No disposition is recommended here.** The costs are stated; the trade between the fourth option's
correctness and its ownership boundary is not this line's to make.

## Tripwires already in place

- A test **fails the day `Stream` lowers**, forcing whoever lands it to confirm the refusal takes over.
- A test **fails if a corpus module acquires the escaping shape**.
- A test **fails if the interprocedural residual stops being empty**.
