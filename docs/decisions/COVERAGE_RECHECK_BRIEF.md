# BRIEF — the inventory's "next increment" points at a workstream that no longer blocks anything

**Written**: 2026-08-27, eighteenth loop iteration. **For this line's own use.**

## Why I went looking

Several closed threads later, the question was whether any **capability** work remains rather than
more instrument auditing. `NATIVE_LOWERING_INVENTORY.md` answers it directly and emphatically:

> *"**The data segment is 81 percent of blocked chunks.** It is the next increment, and it was not
> previously identified as such anywhere in this document."*

| Workstream | recorded blocking instances | recorded blocked chunks |
|---|---|---|
| D, data segment | 7832 | **267** |
| C, composites | 331 | 28 |
| B, sub-coroutines | 98 | 24 |

**This session has expired four blockers and a dozen figures.** So the first move is not to start the
increment — it is to **re-derive the table.**

## Re-derived, and it is comprehensively obsolete

| | inventory | now |
|---|---|---|
| corpus | 58 programs, 496 chunks | **69 modules, 1074 chunks** |
| chunks fully lowerable | **33.9%** | **1070 of 1074, 99.6%** |
| data segment, blocking instances | 7832 | **5** |
| data segment, chunks blocked as first blocker | **267** | **ZERO — it does not appear** |

**The recommendation names the one workstream that now blocks nothing.** Current first blockers are
sub-coroutines (2 chunks) and "other" (2). End to end, **64 of 69 modules lower; 5 are refused** — 2
on `NewComposite`, 1 on `Len`, 1 on `Stream`, 1 chunk-level.

**So "the next increment" is not a workstream at all. It is five named modules.**

## What this increment is

**Correct the inventory's headline and its recommendation, in place, with the old text visible** —
the same treatment the expired safety premise got. **Do not start Workstream D**; the measurement
says there is nothing there to start.

## Prior failures this is exposed to

1. **Acting on a recorded figure.** This is the fourth time this session that a confident recorded
   number was obsolete. **The re-derivation is the increment**, not a preliminary to it.
2. **Comparing unlike populations.** The inventory measured 496 chunks; the corpus is now 1074.
   **33.9% against 99.6% is only meaningful because both are "chunks fully lowerable"** — state that
   rather than assuming the reader checks.
3. **Reading one number as the whole picture.** Chunk-level 99.6% and module-level 92.8% are both
   true and answer different questions; a chunk refused makes its whole module refused.
4. **Deleting a superseded claim.** Correct in place with the old text quoted.
5. **Reporting a figure without the command that produces it.**
6. **Running the two suites in parallel** — invalidates the perf canary. Sequential.

## Specific wrong turns to avoid

- **Do not conclude the backend is "done".** 99.6% of chunks is not 100%, five modules are refused,
  and `Len`/`Stream` refusals have their own established reasons.
- **Do not pin these figures.** The corpus grows on absorption; report them dated.
- **Do not re-plan the roadmap.** The finding is that one recommendation is obsolete, not that a
  replacement is hereby chosen — naming five modules is a fact, choosing what to do about them is a
  separate decision.
- **Do not edit `src/` or any read-only file.**
