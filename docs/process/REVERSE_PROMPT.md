# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-15 (session 44)

## Where things stand

| | |
|---|---|
| `v0.2.3` | PR #101 merged green (22 of 22); the join-corpus increment is in flight |
| The name ceiling | RAISED. `parse` joins byte-identically: 627 names, 33,395-byte blob |
| The join corpus | all ten stages byte-identical through `mi_join` |
| `selfhost_wire` | 161 tests |

## The ceiling, and four premises that were wrong

**"The hard limit is 512" was a guard naming the wrong buffer.**
`emit_name_records_from_nout` reads `wire.nout` and was bounded by `fin_capacity()`, copied from a
sibling that reads `wire.fin`. At two fields per record that yields 512 — a number with no
relationship to the buffer the function touches, which reached the plan, the roadmap and a goal
statement. **This is the same failure as the 395,804: twice in two sessions, in one document.**

**The binding ceiling was `bin`, and three stages breached it, not one**: `parse` 33,395,
`codegen` 21,225, `reconstruct` 8,849 against 8,192, with `lexer` at 97% of it.

**"`parse`'s artifact does not fit the window" was true and not load-bearing.** The join writes two
regions and the DIRECTORY places them, so a two-region directory is 12,840 bytes. The fork the plan
posed — windowed variant or second harness — did not have to be taken.

**Right, and the one that mattered**: "it is not two constants". A loop left at `limit 8192` behind a
guard admitting 49,152 killed three tests with `LoopLimitExceeded`, past a guard that had said yes.

## Two defects the control found that the raise did not cause

- **`mi_chunk_names` ignored `nm.mode`**, overwriting the directory from the seventh chunk onward.
  The join corpus topped out at three chunks; `parse` has 94.
- **`mi_join` SUMMED its three emitter results.** `-202` plus 7,680 reported 7,478 — positive,
  therefore success — with `NAMES` entirely zero. A sum is not a conjunction.

## The dedup branch has no real-module coverage, and it prices the cap

Making `nm_find` report "not found" unconditionally leaves all ten stages byte-identical. `nm_find`
is the quadratic scan whose cost is the stated reason the name count is capped at all, and the raise
multiplied its static bound by sixteen. **The cap is priced on a path no stage reaches.** Established
by MUTATION; the counts only suggested it.

## What the gate caught that my bench could not

`the_driver_refuses_more_names_than_one_call_can_intern` pinned the literal `257` against a cap of
256; the raise made that input admissible, so the driver accepted where the test demanded a refusal.
**It sits behind `self-host`, which neither `cargo test --workspace` nor `cargo test --features
compile` enables.** Both were run and both were green. The gate is `cargo nextest run --profile ci`
across a five-entry feature matrix; a default-feature run is an approximation of it. Every cap-pinned
test now derives from a named `NAME_CAP`.

## Cost, measured

`shared_data_bytes` 155,704 → 237,624 (+52.6%). The WCET bound moves further: the interning phase is
quadratic in the cap, so 256 → 1024 multiplies its static bound by sixteen. Real input is unaffected;
the BOUND moves, and the bound is the product.

## The unsound worst-case-memory bound, FIXED

`GetField`/`GetTupleField`/`GetEnumField` declared an operand-stack net of −1 where the virtual
machine's is 0. The net propagates, so every later operation's peak was computed from a base one slot
too low per field read: `wcmu_stream_iteration` reported **96 bytes for a Stream chunk where 128 is
correct**. An UNDERSTATED bound, not a loose one — the opposite direction from the
conservative-verification stance.

**The root was one model with two readers.** `stack_growth`/`stack_shrink` served both the peak walk
(which wants a transient reach and a NET) and `text_size`'s shadow stack (which wants literal POP and
PUSH counts). Those coincide only for an operation that does not both pop and push. The repair splits
the ROLES: those two are now exclusively the peak model, and `verify::op_depth_effect` —
`(required, delta)`, true semantics all along — is the pop/push model that `text_size` reads. The
field reads become `(0, 0)`: exact, not merely conservative.

**Two corrections to the report this came from.** `GetIndex` is NOT another instance — it genuinely
pops the container AND the index, so its −1 is right and only the match arm was misleading. And the
checked family's TRANSIENT is not understated: the virtual machine pops both operands before pushing,
so `growth = 1` is the true reach. Its DECOMPOSITION was wrong, which only the shadow stack noticed.

**Why nothing caught it, and it generalises.** `analyze.kel` consumes these numbers as host-seeded
arrays, so the self-hosted differential agrees by construction. **A differential against the model
under test cannot detect that the model is wrong.** The control compares against an independent model
instead, and fails before the repair.

## Open

- **`-255` is live and has no negative test** — reaching it needs more than 16 KB of distinct name
  bytes; the corpus tops out at 7,680. Recorded in the source as a gap.
- **Two pinned coverage gaps**: no stage contributes a constant-interned name, and none nests a
  constant past depth one.
- **`bin` was raised, not fixed.** 49,152 covers `parse` at 1.47×; a stage half again as large breaks
  it. Batching the blob does NOT remove the name cap, because the deduped output table must stay
  resident for the dedup scan.
- Publication remains **HELD**.
