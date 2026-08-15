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

## Open

- **The field ops' operand-stack net is UNSOUND and unfixed.**
  `GetField`/`GetTupleField`/`GetEnumField` declare net −1 where the VM's net is 0, so
  `wcmu_stream_iteration` reports 96 bytes for a Stream chunk where 128 is correct. The
  understatement scales with field reads and only surfaces when a field read is on the
  peak-determining path, which is why it survived. **`GetIndex` is NOT a fifth instance** — it
  genuinely pops two. The checked family's TRANSIENT is fine; its DECOMPOSITION is wrong and only
  `text_size.rs` notices. Scheduled as the next increment.
- **`-255` is live and has no negative test** — reaching it needs more than 16 KB of distinct name
  bytes; the corpus tops out at 7,680. Recorded in the source as a gap.
- **Two pinned coverage gaps**: no stage contributes a constant-interned name, and none nests a
  constant past depth one.
- **`bin` was raised, not fixed.** 49,152 covers `parse` at 1.47×; a stage half again as large breaks
  it. Batching the blob does NOT remove the name cap, because the deduped output table must stay
  resident for the dedup scan.
- Publication remains **HELD**.
