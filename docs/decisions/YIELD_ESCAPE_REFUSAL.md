# Refusing the composite that leaves its iteration by `yield`

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: implemented on the V0.3.X line, 2026-08-27. Costs the corpus nothing.
**Owner**: the native backend (`native_codegen/`), which is not built by continuous integration.

## The defect

`region::plan_chunk_region` gives every construction site one offset for the life of the chunk, so a
site inside a loop body rewrites the same bytes on every iteration. After B28 a composite is
`FlatComposite::Arena(ArenaHandle<[u8]>)`, a pointer and a length rather than a copy, and the epoch
the handle carries is advanced only by a `RESET`. **An overwrite in place advances nothing.** A host
holding iteration *n*'s handle therefore calls `resolve`, succeeds, and reads iteration *n+1*'s
bytes.

**The failure mode is a silently wrong value, not a `Stale` error.** That is why it cannot be left to
a runtime guard: there is no runtime guard it trips. Established against the runtime by the `v0.2.3`
line and recorded in [`../proofs/COMPOSITE_REGION_REUSE.md`](../proofs/COMPOSITE_REGION_REUSE.md)
§4.1.1.

## The recorded premise was false, and correcting it is the substantive finding

Both the V0.3.X handoff and the obligation document stated that the defect was latent because **no
corpus module has the escaping shape**. That is wrong.

`examples/scripts/13_telemetry_stream.kel` **carries the shape deliberately** and says so in its own
header: *"the value LEAVES the iteration through `yield`, so the host may still be holding it when
the next iteration builds its successor... A planner that reused a slot here would hand the host the
next reading's bytes where it asked for this one's, and the arena's epoch guard would NOT catch
it."* Measured: chunk 0, construction at op 24, yielded at op 25.

**So latency had to be explained by the backend rather than by the corpus, and it is.** Asked
directly, the backend refuses that module with:

```
main: native lowering does not yet support opcode Stream
```

**The safety is accidental.** It rests on an unimplemented opcode, not on any escape reasoning. Every
chunk that can carry the shape is a `loop` chunk, and a `loop` chunk opens with `Op::Stream`. On the
day `Stream` lowers, the defect becomes live and silent unless something else refuses it first.

## The disposition: refuse at the placement

`region::yield_escape_hazards` reports sites whose fixed offset can be overwritten while the host
still holds the value built there, and `lower_chunk_body` returns
`LowerError::YieldEscapingLoopComposite` rather than emitting the placement.

**Refusal rather than a better placement.** Not reusing the slot means one region per iteration,
which is unbounded in the iteration count and gives up the bounded-memory property the backend exists
to provide. Making `resolve` fail instead would require the epoch to advance on an in-place
overwrite, and epoch semantics live in `src/vm.rs` and the arena, which this line may read and must
not edit.

### Why this does not reopen the recorded design tension

The objection to a confinement verdict reaching the planner is that a wrong verdict would then
miscompile. **That objection does not reach this gate.** The predicate over-approximates in one
direction only and its result is used to **refuse**, never to place. A verdict wrong in the permissive
direction rejects a sound program loudly and recoverably; placement still consumes nothing. The
immunity and the guard are compatible because they act at different points.

## What it costs: nothing

Measured over 91 modules and 1117 chunks by
`native_codegen/tests/yield_escape_gate.rs`:

| | |
|---|---|
| chunks carrying the shape | **1** (`13_telemetry_stream.kel` chunk 0) |
| of those, already refused for another reason | **1** |
| **newly refused by this gate** | **0** |

## What is NOT closed, stated rather than implied

- **The interprocedural case is open.** A composite built in a loop body, returned to a caller, and
  yielded there is a hazard a single-chunk predicate cannot see. The `Call` disqualifier in
  `loop_composite_census.rs` bounds the residual.
- **The gate is shadowed today.** Every hazardous chunk is refused earlier for `Stream`, so the
  refusal cannot fire through `lower_module` on unmutated input.
  `the_yield_escape_refusal_is_shadowed_by_the_missing_stream_opcode` asserts that shadowing and is a
  **tripwire: it fails on the day `Stream` lowers**, so whoever lands `Stream` must confirm this
  refusal fires in its place.
- **Fireability is nonetheless demonstrated**, by removing the `Stream` op from compiled bytecode and
  observing `lower_module` return the yield-escape refusal. A guard whose only evidence is a
  non-empty predicate result says nothing about whether the lowering consults it, and this line has
  shipped guards that passed while unable to fail.
