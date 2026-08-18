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
| branch | `feat/parse-diagnostics`, cut from `v0.2.3` |

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
rest still trap raw: 47 arrays of 8 entries (nesting stacks), 22 of 32, 4 of 64 (struct-definition
tables), 19 of 256, 17 of 512. **None has been probed**, so none is known reachable or unreachable.

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

## Next intended increment

**Raising `toks.chunks` above 256**, the one cap still standing, which excludes `wire.kel` from
being PARSED at 475 functions. Deliberately separate work: `base` and `at` were appended after that
array, so widening it shifts them, and a mid-block insertion has silently broken four tests once.

**Then `parse` into `reconstruct`**, worth 3x to 13x residency, needing `reconstruct.kel`
restructured or fused at function granularity with a probable fourth sidecar fact.
