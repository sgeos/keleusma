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
| **`parse` into `reconstruct`** | **FUSED at function granularity, 3.4x to 41.1x** |
| shared-slot layouts | **nine copies collapsed to two definitions** |
| `parse.kel` failure modes named | **THIRTEEN**; eleven counters guarded |
| branch | `feat/typecheck-input-from-pipeline`; #164-#172 merged, at `52cbb6c4` |

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

## `parse` INTO `reconstruct` IS FUSED, AND THE PREDICTED COST DID NOT EXIST

Cut at FUNCTION granularity. `self_host_compile_fused` holds one GROUP -- consecutive same-named
heads, which are one chunk -- where `self_host_compile` holds every function's records for the whole
program. Byte-identical modules, mutation-verified: flushing per function instead of per group fails
the equivalence test by naming the multihead chunk.

**Measured 3.4x to 41.1x**, against a recorded estimate of 3x to 13x. `wire` is the 41x case, so the
largest stage benefits most.

**THE FOURTH SIDECAR FACT DID NOT MATERIALISE.** A group ends when the next function's NAME differs,
so a completed function waits for the following HEADER -- a bounded one-function lookahead, not a
whole-input dependency. The name table is available before the drive. That predicted cost was the
reason this increment ranked below the diagnostics work; it was not real.

## I SHIPPED A DEFECT MY OWN GUARD WAS WRITTEN TO CATCH

Raising the chunk table moved the parser's shared block, and a FIFTH copy of the layout in
`compiler/src/main.rs` actively seeds the parser. That binary was reading the keyword and type ids
from inside the chunk array. Nothing caught it: `run_parse_pipeline` is reachable only from `main`,
so its constants are compiled by continuous integration and never executed.

**The guard I wrote to prevent this walked `src/` and `tests/`.** A guard with a scope narrower than
the class it guards is the same defect it was written to prevent. It now walks the repository and
asserts that `compiler/` was actually reached.

**The lexer's block was restated in four places too** and had failed nothing, because it has not
moved -- exactly the state the parser's five copies were in the day before. Both layouts are now
published and chained, all nine copies alias them, and both derivation tests are mutation-verified.

**Two corrections on my own reporting**: I said `compiler/` has zero tests; it has 86, and my check
was scoped to `compiler/src/`. And root `cargo fmt --all` does not reach `compiler/`, which declares
its own workspace -- a local gate touching it needs a `cd compiler` pass.

## FIVE MORE CAPS, FOUND BY SWEEPING RATHER THAN BY TRIPPING OVER THEM

Parameters (32), `if` nesting (32), `for` nesting (8), array-literal nesting (8), and enum variants
(256, a WHOLE-PROGRAM total). **Two more pairs shared a message**, one array-size down from the pair
fixed the morning before -- fixing the instances I had measured left the class, and sweeping found
the rest.

**The enum bound's size does not say what it counts**: 128 enums of two variants refuse at the same
point as one enum of 257. No message naming an array size could convey that.

**The family lesson was applied rather than relearned.** `ps.pcount` alone indexes twelve arrays;
the widening derived thirty-one arrays across five counters from the stage. Fourth consecutive
increment where a hand-written list would have been wrong, and the first where I did not find out by
failing.

**Corrected from my own probe**: call arguments are NOT a separate cap. A call cannot exceed its
callee's arity, so the parameter cap fires first. A probe that varies two quantities measures neither.

**Naming a cause has a measured price**: 645 to 660 names, 34,148 to 34,785 blob bytes. The
diagnostics programme has spent 33 of the 1,024-name budget across two increments, leaving 64%
margin.

## THE LAST TWO UNNAMED FAILURE MODES ARE NAMED

**The token array had TWO failures**, and which one a caller got depended on how far over they were:
`IndexOutOfBounds(40960, 40960)` from the stage, or a shared-slot range error from the driver's own
seeding loop. One refusal now fires before any seeding. **This is the bound the corpus is closest
to** -- `parse.kel` is 32,907 tokens, 80% of it.

**Six bare `unwrap()`s became one diagnostic.** A top-level `struct` declaration was the measured
cause; `parse.kel` has no struct handling at all. **It does not decide whether `struct` should be
supported** -- that is yours, and the test says so.

**Both of my own mistakes here were the session's recurring one.** My test generated against the
REFERENCE tokenizer while the cap governs the STAGE's lexer -- measuring the wrong quantity, so
`lex_token_count` is now public and documented as the count the cap uses. And an insertion detached
`#[allow(clippy::type_complexity)]` from its function, because I anchored on the signature rather
than the item. I restored from `HEAD` and reapplied rather than stack a third correction.

## THE SWEEP CONVERGES, AND THE PROGRAMME NOW HAS A UNIT PRICE

Two more caps: call nesting (8) and data-block fields (512, a WHOLE-PROGRAM total like the enum
bound). **`IndexOutOfBounds(8, 8)` had THREE sharers**, not two -- call nesting sat behind a construct
I had not generated. All three are now held distinct by test.

**A distinction that is this session's trap in miniature**: array-literal ELEMENTS have no wall
through 1,025; array-literal NESTING caps at 8.

**The sweep is converging**: two caps this round against five last, and four constructs came back
clear (data blocks and `use` through 64, tuple elements through 32, array-literal elements through
1,025).

**The margin pin has moved SIX times and now yields a rate**: roughly three names per cause named --
an error code, a capacity, a guard. 39 of the 1,024-name budget spent, 65% margin left. It has not
once moved for a reason its author was thinking about, which is why it is pinned rather than computed.

## THE SWEEP IS DONE, AND IT CAUGHT A STALE DIAGNOSTIC OF MINE

A final round found **no new reachable caps**. It found something better: the chunk-table guard's
message and comment were **stale in four ways and I made them so** when I raised the cap -- it told a
caller with 1,025 functions about a *257th* entry, cited a 256-entry array that is now 1,024, and said
raising the array "is NOT done here" after it had been done. Both copies now derive from
`PARSE_CHUNK_CAP`.

**Five of my probes this session measured something other than what I intended.** The rule that came
out of it: when a generated program fails, confirm the REFERENCE accepts it before concluding anything
about the stage. It caught three of the five.

**`HANDOFF.md` is rewritten** against `3ffd5a4c` with every value re-measured, and its own check block
was run as a resuming session would.

## DERIVED OPERANDS IN TYPE REJECTION ARE CLOSED

Your ruling was "before publishing V0.3.0", so this needed no new decision. `let a = 1 + 2` left `a`
UNKNOWN and `a + b` was accepted; the stage now proves `a` from its operands and rejects.

**It needed a fixpoint, as recorded.** A binding may take form 2 -- "takes whatever node N yields" --
and the stage proves a tag only for an operator node whose operands agree. **The host supplies only
WHICH NODE**, which is verified by mutation: neutering the stage's join fails the test.

**The cap I almost documented was not the bound.** I nearly wrote "reaches a chain of four" from
`tyb_rounds() = 4`. Setting it to 1 rejects every depth through six: scoping forces `let` bindings
into dependency order, so one pass proves the chain. The cap is insurance for out-of-order rows.

**The new edge is pinned**: a `let` bound to a FIELD READ or an INDEX is still unreached, and the
test says so as a measurement rather than an aspiration.

## IDENTITY NOW TRAVELS WITH THE STRUCTURE (your fork, option 1)

Order 1 said the type checker's input should come from `parse.kel` plus `reconstruct.kel` because
"structure is available". **Measured, that was half true**: a `Local` record carries a SLOT and no
body record mentioned a name, while the type channel is keyed by interned NAMES. You ruled that a
`let` record should carry its name id.

Built. The statement table emits in the PACKED form (kinds capped at 63), so the name goes out on the
migrated path with tag 90 -- a full word, no packing, no radix. The driver pairs it with the
following `LetIn` and diverts it, leaving the node stream unchanged.

**I claimed the blast radius before measuring it and was wrong.** I said nothing else was touched,
having run one suite; eight tests then failed because a THIRD decoder -- the Rust reconstruction that
checks `reconstruct.kel` -- panicked on kind 90. Three decoders now consume the record stream, and
only the TAG is shared, which is correct: their skip sets legitimately differ.

**The margin pin moved a seventh time, and this is the first move predicted in advance**: 669 names,
35,154 blob bytes.

## Next intended increment

**Nothing is queued that does not need a decision from you**, and the sweep that needed no ruling is
now exhausted. The three live decisions are in the handoff's operator-held list and repeated below.



**`parse.kel` is 32,907 tokens against its own 40,960-token array, at 80%** -- newly measured,
unowned, and nothing reports it when it binds. I would NOT widen it unilaterally: raising a capacity
widens what is admitted, and the chunk-table raise was widened only because you had named it. A
NAMED REFUSAL costs nothing and widens nothing, which is what I would do absent direction.

Beyond that, the remaining structural work is the phase-selection architecture, which is blocked on
the input-re-readability fork below.
