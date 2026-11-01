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

## The internal driver panics where the shipped path refuses, and only one of those reaches a user

Measured 2026-09-06, following an incidental observation from the discard-arm census.

A **`Byte` literal** -- `fn f() -> Byte { 7Byte }` -- panics through
`self_host_compile_full`. Through `self_hosted_compile`, the function behind
`keleusma compile --compiler self-hosted`, the same source **refuses cleanly** with
`Unsupported { detail: "reconstruct.kel refused on chunk ..." }`.

**So the user-facing behaviour is correct and this is an internal note, not a defect.** Saying so
plainly matters more than the observation: a session that reports every internal wrinkle at the
severity of a real hole spends the credibility its real holes need.

**Three corrections to the way the observation was first written**, each found by running rather
than reasoning:

| the first framing | what a run showed |
|---|---|
| "a function returning a `Byte` constant panics" | the culprit is the **literal**, not the return type -- `fn f(a: Byte) -> Byte { a lsr 1 }` compiles through both paths |
| "whether the construct is inside the supported subset is not determined" | the boundary table classifies the byte-shift case **SOk**, and that classification is accurate |
| the panic as the headline | the panic is confined to the lower-level entry; the shipped path never exposes it |

**The distinction it illustrates is worth more than the case.** The driver and the shipping compiler
are two implementations of the same idea, and this repository has already recorded five defects that
existed in one and not the other. **Testing a lower-level entry point and generalising to the
shipped one is how those five were missed**; the same error one level up would have reported a
user-facing panic that does not exist.

**Not repaired.** Making the internal entry refuse instead of panicking is a change to a compiler
error path, and the case for it is convenience for a future census rather than user-facing
correctness.
