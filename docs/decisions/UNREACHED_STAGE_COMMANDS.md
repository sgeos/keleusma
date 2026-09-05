# Two dispatched stage commands that nothing has ever driven

Found 2026-08-20 while scoping `CONSTS`, Order 1 item 1.

## The finding

`wire.kel` dispatches commands **176 `fl_stream_begin`** and **177 `fl_stream_step`**
— the one-node-in, one-record-out streaming path for constant nodes. They were
written, dispatched, and ANNOUNCED to the other line in the mailbox
(`handoffs/v0.2.3.md:130`, "`highest_command` MOVES 175 to 177").

**No driver calls them. No test calls them.** Verified by searching the whole
repository for the command numbers and for `fl_stream`; the only hits are the
dispatch arms themselves and the mailbox announcement.

The control is `CMD_STEP = 175`, the chunk-streaming command immediately below
them, which `window_emit_chunks` does drive.

## Why it matters, and it is not "delete them"

It changes the cost of `CONSTS`. The tree's own analysis says the flattener already
emits a byte-identical region, that the 170-node walk cap is the only blocker, and
that **batching is the route** because a scalar forest carries no state between
batches. Reading that plus "a streaming variant already exists" makes the remaining
work look like driver wiring.

**It is not.** The stage side has never executed. Taking `CONSTS` means writing the
driver AND validating stage code that has never run, which is a materially larger
piece than the analysis suggests to a reader who does not check whether the path is
reached.

## The class this belongs to

Same shape as two other findings this week, and worth naming as a class:

- The `v0.3.0` line found `Op::Reset` credited as lowered because the *chunk*
  containing it lowered, while the op sat in a region no edge reaches. A mutation
  crediting it moved their figure to 57 of 66 **with every test still green**.
- `Op::IsStruct` and `Op::Len` were emitted only on fallback paths, and one of them
  still has no witness. **`Op::Len` has had no emission site at all since
  2026-09-04**; both of its fallback paths were replaced by a folded length or a
  compile error.

**Code being present, dispatched, and even announced is not evidence it runs.** The
cheap check is to search for its callers before costing work that depends on it.

## What to do

Nothing urgent. When `CONSTS` is taken, budget for validating 176/177 as part of
it, and drive them from a test first so the stage side is proven independently of
the driver. Do not delete them — they are the intended route, merely unexercised.
