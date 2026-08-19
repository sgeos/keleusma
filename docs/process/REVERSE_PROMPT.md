# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-18 (session 48)

## Where things stand

| | |
|---|---|
| all twelve stages | `loop main(...)` coroutines |
| emit path | 11 of 11 stages; every emit-side cap removed |
| `lexer` into `parse` | FUSED, one-token window, byte-identical on four stages |
| architecture | one binary, selectable phases — documented, unbuilt |
| **`parse.kel` capacity diagnostics** | **four causes now NAMED; the rest still trap raw** |
| **the last cap** | **GONE. `wire.kel` PARSES, 486 functions** |
| branch | `feat/chunk-table-cap`; `feat/parse-diagnostics` merged at `09784042` |

## WHAT THIS INCREMENT DID

`parse.kel` reported its capacity limits as raw virtual-machine traps. Measured by feeding the
stage malformed and oversized sources, not by reading it:

| input | reported | now |
|---|---|---|
| 65 local bindings | `IndexOutOfBounds(64, 64)` | names locals, the count, and the cap |
| 65 nested parentheses | `IndexOutOfBounds(64, 64)` | names expression nesting |
| 257 statements in a body | `IndexOutOfBounds(256, 256)` | names the statement table |
| an unmatched `]` | `IndexOutOfBounds(-1, 64)` | names the bracket and its token |
| an unterminated block | "did not reach DONE within its iteration budget" | names the likely cause |

**The first two are the finding.** `opstack` and `let_names` are both 64 entries, so two unrelated
limits produced a BYTE-IDENTICAL message. `the_two_sixty_four_caps_no_longer_give_the_same_message`
encodes that defect so it cannot return.

**The guard is on the pointer and each guarded array carries one spare slot.** The write precedes
the increment, so a guard on the increment alone fires one write too late; clamping at the last
usable slot would have REFUSED the exactly-full program that parses today, which is a unilateral
narrowing. Every boundary is pinned from both sides — 64 parses, 65 does not.

## WHAT I GOT WRONG, RECORDED AS CORRECTIONS

- **I widened two arrays of eight and the trap did not move.** Six more are written at the same
  local-binding counter. The test now DERIVES the array set by reading the stage, and is verified by
  mutation: reverting `let_enum` to 64 fails it by name. A hand-written list would have encoded the
  mistake I had just made.
- **A sixth constructed status, and it nearly landed.** The full suite reported `exited with code 0`
  with forty green lines. That was `grep`'s exit; `cargo test` had aborted at a failing binary and
  eighteen never ran. **The tell was the SHAPE, not the code** — `selfhost_parse` takes ninety-eight
  seconds and nothing in the list took that long. Now run with `--no-fail-fast` and the exit code
  captured outside the pipe.

## What this green suite does NOT establish

**Roughly a hundred and thirty fixed arrays remain in `parse.kel` and four causes are named.** The
rest still trap raw: the nesting stacks at 8 entries, the 32s, the struct-definition tables at 64,
and the remaining 256s and 512s. **None has been probed**, so none is known reachable or
unreachable. The chunk-table work is direct evidence that this matters: three of its walls were
unprobed arrays, and each reported a size rather than a cause.

**Separately, the probe found malformed inputs SILENTLY ACCEPTED**: a stray `)`, an unclosed `(`, a
binary operator with no right operand, and an empty index `a[]`. That is acceptance laxity rather
than a diagnostic defect, mitigated but not closed by the cross-check against the reference compiler.

**A question for you rather than a decision I took**: these refusals PANIC, matching the existing
failure mode of `parse_functions` and of the chunk-table guard. Turning them into a `Result` is
defensible and changes a signature many tests and both compile paths depend on. I did not widen the
scope to do it.

## Held for you, with rulings

- **`Op::cost()`**: 50 of 66 opcodes unmeasured. *Ruled: after Order 1.*
- **Derived operands in type rejection**: *Ruled: before publishing V0.3.0.*
- **Publication**: *held.*
- **The Japanese FAQ entry** renders as English. *Ruled: correct eventually.*
- **The input-re-readability fork** in `../decisions/PIPELINE_THEN_MONOLITH.md`: still open. It
  decides whether the monolith is one command or two.

## THE LAST CAP IS GONE, AND IT WAS NEVER ONE NUMBER

`wire.kel` parses at 486 functions. Raising `toks.chunks` from 256 to 1024 was three edits and the
first two did not work: the wall moved to `LoopLimitExceeded` (two `limit 256` loops over the chunk
count) and then to `IndexOutOfBounds(388, 256)` (the six chunk-indexed `chunkret.ret_*` arrays).

**A cap is a FAMILY, and that is the second family in two increments.** The eight local-binding
arrays were the first. Both times I widened what I could find by name and the trap did not move.

**THEN SIXTY-EIGHT TESTS FAILED AND NOT ONE NAMED A SLOT.** The shared layout was restated in FOUR
places — the driver and three harnesses — so moving the block left them seeding the type ids at the
old slots, and `parse.kel` sized every field as one byte. **My derivation test proved the DRIVER
agreed with the stage and said nothing about harnesses that never consult the driver.** Now: public
chained constants, harnesses aliased, and a guard that WALKS the tree rather than checking a list.

**Two vacuity guards fired in one run** — the family test found zero arrays (a bug in my own walk),
and the no-copies guard flagged itself. Both now verified by mutation.

## Next intended increment

**`parse` into `reconstruct`**, worth 3x to 13x residency, needing `reconstruct.kel` restructured or
fused at function granularity with a probable fourth sidecar fact.

**Also newly measured and unowned: `parse.kel` is 32,907 tokens against its own 40,960-token array,
at 80%.** That is the next array likely to bind, and nothing reports it when it does.
