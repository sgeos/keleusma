# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-18 (session 47, continued)

## Where things stand

| | |
|---|---|
| all twelve stages | `loop main(...)` coroutines |
| emit path | **11 of 11 stages**; every emit-side cap removed |
| `wire.kel` | seven streaming commands; the rest answer in one yield |
| `lexer` into `parse` | **FUSED**, one-token window, byte-identical on four stages |
| architecture | one binary, selectable phases — documented, unbuilt |
| open PRs | none. Tree clean at `3f3735d2` |

## THE ARCHITECTURE IS DECIDED AND ONE FORK IS YOURS

[`../decisions/PIPELINE_THEN_MONOLITH.md`](../decisions/PIPELINE_THEN_MONOLITH.md) — the filename's
"then" is superseded and the note at its top says so.

**One binary with `--start` and `--end`.** The monolith is `--start=first --end=last`; the shell
pipeline is N invocations with `start == end`. Same program, so nothing built now is discarded later.

**The open fork**: both the pre-pass and the main pipeline read the SOURCE, so a fused run from a pipe
cannot re-read it. Buffer the source, accept a file operand with standard input as the default, or
always split. I lean to the file operand and said why. `--chunk` can only be optional under the first
two.

**Required whichever way that goes**: fingerprint the sidecar against its input. A stale table
produces a byte-plausible wrong artifact, which is worse than a crash.

## FOUR WHOLE-INPUT FACTS, THREE FOUND ONLY BY CUTTING A BOUNDARY

The intern table, the chunk table, the token count, and — probably — the chunk metadata.

**The token count is the instructive one.** `parse.kel` finds end of input by comparing its cursor
against `toks.len`. Free for a collecting driver, impossible for a windowed feed to leave unspecified.
Nothing in the source marks it as a dependency; it reads as an ordinary field.

**So: enumerate by BUILDING, not by inspecting.** The enumeration was called complete twice before it
was. Treat it as incomplete until each boundary has actually been cut.

All four come from the lexer or from parse's whole output, which points at one sidecar with sections
rather than a flag each.

## THE ONE CAP LEFT, AND IT IS ON THE PARSER

`toks.chunks` is `[Word; 256]`, so **`wire.kel` cannot be PARSED** at 475 functions. Four of the five
caps this line has found were discovered by something other than looking for them; this one surfaced
while measuring residency for a different increment.

The driver now refuses with both numbers. **Raising the array is a separate increment**: `base` and
`at` were appended after it, so widening shifts them.

## WHAT I GOT WRONG, RECORDED AS CORRECTIONS

- **A four-token lookbehind justified by a false claim.** I wrote that the cursor could sit "several
  tokens" behind `at`. It cannot, and the existing measurement already disproved it. The bound is one
  and it is derived. The widening was a misdiagnosis that did not fix the fault and was kept anyway.
- **A predicted silent-corruption window that panics instead.** Overflowing the chunk table by one
  does not corrupt silently. Smaller defect than I claimed; the real one is the misdirecting message.
- **A private intra-doc link** that only the gate's doc build catches, after correcting that same
  class earlier and then treating a prose edit as not needing the check. **Rustdoc reads comments, so
  a comment edit is a gated edit.**

## Held for the operator, with rulings

- **`Op::cost()`**: 50 of 66 opcodes unmeasured. *Ruled: after Order 1.*
- **Derived operands in type rejection**: extraction still host-side. *Ruled: before publishing V0.3.0.*
- **Publication**: *held.*
- **The Japanese FAQ entry** renders as English. *Ruled: correct eventually.*
- **The input-re-readability fork** above: open.

## What these green suites do NOT establish

The composite constant case does not stream in this shape at all — a composite's record carries a
range into children numbered after every node at its depth, so the walk needs a queue and **the queue
IS the residency**. Scalars have no children, which is the only reason `CONSTS` streams.

Nothing here has been run as an actual pipeline. The fusion is in-process, and the phase-selection
architecture is documented and unbuilt.

## Next intended increment

**`parse` into `reconstruct`**, and the measurements say what it costs. `reconstruct` is bounded per
FUNCTION and reads strictly sequentially — both good, both measured. But its `main` performs the whole
reconstruction in one resume, so it is a coroutine by form rather than by granularity, and fusing at
record level means restructuring the stage.

Residency at stake: **3x to 13x**. `parse` holds 12,048 records across all functions where its largest
single function needs 931. A fusion at FUNCTION granularity captures most of that without
restructuring, at the cost of a probable fourth sidecar fact.

**Before that, consider the diagnostics increment.** `parse.kel` reported `LoopLimitExceeded` for a
full chunk table and `IndexOutOfBounds(-1, 64)` for an unprimed window — both today, both pointing
away from the cause, both diagnosed wrongly on the first attempt.
