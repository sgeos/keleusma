# Routing `DATA_SLOTS`, and the section base the walk does not retain

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: design, not implementation. Written 2026-09-10 after three increments of reading, one
of which over-claimed and one of which is corrected below.

## Where this sits

`docs/roadmap/V0_2_X_ROADMAP.md` Order 1 needs the remaining region kinds emitted. Four are still
skipped: `ENUM_VARIANTS`, `ENUM_LAYOUTS`, `DATA_SLOTS` and `PARAM_TYPES`. The criterion separating
them from the routed kinds is that **their records carry a NAME INDEX**, and a host-supplied index
could disagree with the interner that produced `NAMES`. That is a soundness requirement, not a
convenience.

## What is already established

- **The interning walk covers all three name sections.** `mi_window_prepare` calls
  `mi_chunk_names()`, which tails into `mi_enum_names()`, which tails into `mi_slot_names()`. The
  module-input blob carries chunk names, enum type and variant names, and data-slot RUN names.
- **The order matches the reference.** `NAMES` is emitted as one record per walk position, in walk
  order, and it is byte-identical to the reference on every corpus stage — asserted by
  `no_region_the_driver_routes_disagrees_with_the_reference`. A byte-identical table of name
  records in walk order IS the statement that the walk matches `SchemaBuilder`'s interning order.
- **Walk position is the name index.** `mi_pair(k, len, mode)` indexes by `k` and dedup reuses a
  POOL OFFSET rather than collapsing a record, which is why `ck_stream_step` reads
  `wire.nmap[ck.j]` with a plain chunk index.

## THE CORRECTION THIS DOCUMENT EXISTS TO CARRY

A previous increment wrote that the section bases are "recoverable from state that already
exists", naming `ecnt`, `vcnt`, `scnt` and the running `cnt`. **That is wrong.**

`nm.ecnt` and `nm.scnt` are totals, read once per section. **`nm.vcnt` is assigned INSIDE the enum
loop** — it holds the variant count of the *current* enum and is overwritten on each iteration.
There is no running total of variant names anywhere, so the slot section's base cannot be computed
from what the walk retains.

This was caught by reading the assignment site rather than the field list. The field list made the
state look sufficient, which is the same surface-reading failure that produced the two earlier
over-claims in this arc.

## The design

**Capture each section's base at the moment that section starts**, from the walk itself:

- at the top of `mi_enum_names()`, record the current `nm.cnt` as the enum section's base;
- at the top of `mi_slot_names()`, record the current `nm.cnt` as the slot section's base.

Two new fields on the `nm` private block and two assignments. The base then comes from the walk
rather than from arithmetic over counts, so it cannot drift from the sequence it indexes.

**`ds_stream_step` then takes the slot-run index and reads the interner**, rather than taking a
name index from the host:

- the host supplies the run index `k` and the three non-name fields;
- the stage reads `wire.nmap[slot_base + k]` for the name, exactly as `ck_stream_step` reads
  `wire.nmap[ck.j]`.

No host arithmetic touches a name. The host never asserts a name index it cannot check, which is
the objection that kept this kind unrouted.

## What this design does NOT need

**No `*_stream_begin` for `DATA_SLOTS`.** The chunk stream needs one because it carries three range
cursors that must persist across records. A data-slot record carries no cursor. What it does need
is that the interner has RUN, and `wire.nmap` is shared data that survives for as long as the host
hands back the same buffer — the property `ck_stream_begin` already documents and relies on.

**CHECKED 2026-09-11, AND IT HOLDS.** `window_emit_chunks` creates ONE `shared` buffer and passes
`&mut shared` to every `enter_wire` call -- the begin and every step alike. Shared data is re-seeded
per call only for the slots the driver writes; `wire.nmap` is never among them, so the interner's
result survives for as long as the driver hands back the same buffer, which that function does by
construction. A `DATA_SLOTS` driver written the same way inherits the property.

**The question it leaves behind is sharper and smaller: WHICH command runs the interner for a slot
pass.** There is no `ds_stream_begin`, and the two existing commands that call `mi_window_prepare()`
both do something else as well -- command 174 zeroes the chunk range cursors, and command 170 emits
the `NAMES` records into the window. Either would work and both are misuses: one is chunk-specific,
the other writes bytes the driver would discard.

So the slice needs a begin after all, but for a different reason than the one this section
originally guessed. It is not that the interner's result fails to survive; it is that nothing
currently runs the interner WITHOUT also doing something a slot pass does not want. A begin whose
whole body is `mi_window_prepare()` is the smallest honest answer, and it moves `highest_command`.

## Verification, in the order that makes a failure legible

1. The two base fields are recorded, and a test asserts the slot base equals the chunk count plus
   the enum section's name count for a stage with both — **a check the walk cannot satisfy by
   accident**, since the two sides come from different places.
2. `ds_stream_step` reads the interner, with the byte-identical oracle as the verdict. A wrong base
   shows as a wrong name index in the region, not as a crash.
3. The driver routes the kind, and `selfhost_region_coverage.rs` moves `DATA_SLOTS` from `Skipped`
   to `Identical` for every corpus stage.

## What routing it buys, stated in advance so it cannot be over-read

**The PRODUCED share rises and the COMPUTED share does not.** These records format fields the host
decides; only the name index becomes the stage's own.
`the_computed_share_is_smaller_than_the_produced_share` exists precisely so the headline figure
cannot be read as the compiler deriving more of its own artifact, and it should still hold
afterwards.

## TWO CONSTRAINTS FOUND ON 2026-09-11 THAT MAKE THIS A BUDGETED SLICE, NOT A CASUAL ONE

**Command 178's contract is already driven.**
`the_four_record_formatters_lay_out_a_record_the_reference_agrees_with` feeds `ds_stream_step` a
name index taken from the reference's own record, and that is legitimate for the claim it makes --
whether the stage lays a record out the way the format specifies. Changing 178 to read the interner
would break it for no gain. **The name-aware step must be an ADDITIVE command**, leaving 178 as the
formatter it is, which matches the preference this file records elsewhere for additive commands over
flags on a proven path.

So the slice adds TWO commands, not one: a begin whose body is `mi_window_prepare()`, and a
name-aware step. `highest_command` moves from 181 to 183.

**`wire.kel` is itself one of the eleven measured stages, and its margin is not large.** Two new
functions add chunks and constants to the very stage the corpus measures, and the stage must still
emit its own regions afterwards.

**MEASURED 2026-09-11: 1,194 nodes against a 1,365 cap, a margin of 171.** The figure quoted
around the tree is **1,148**, which is stale by 46 nodes, and the margin this plan first assumed was
217 rather than 171. That is a quarter of the assumed headroom gone, in the one number the slice is
sized against -- which is precisely why the plan said to measure before editing.

**Derive it from `tests/module_input_node_budget.rs` rather than reading a number here.** That test
parses the count out of the blob the stage itself reads, asserts the walk has not drifted from the
writer, and fails if the margin falls below sixty-four. A number in prose is the thing this tree has
watched go stale four times in this arc alone.

**That is the reason this has not been done casually, and it belongs in the sizing rather than being
discovered during the change.** The slice must be budgeted against that margin, and the node count
should be re-measured after the stage edit and before the driver edit, so a cap failure is
attributed to the stage growth rather than to the routing.

## Not in scope

`ENUM_VARIANTS` is the same shape with the enum base, and should follow only after `DATA_SLOTS` is
byte-identical. `ENUM_LAYOUTS` and `PARAM_TYPES` have no emitter at all and are a different
obligation.
