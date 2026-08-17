# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-17 (session 47)

## Where things stand

| | |
|---|---|
| Auxiliary body | **712,936 → 103,544 bytes**, a factor of 6.9 |
| Stages fitting one window | **11 of 11**, where three did not |
| Chunk region emitted | **9 of 11 stages**, up from 7 |
| Remaining emit limit | the 90-record chunk batch cap only |
| Operand-stack models | agree on every one of the 66 opcodes |
| Empty statement | landed; both parsers agree byte-identically |

## THE ALL-DEFAULT INITIALISER POOL IS ELIDED

Authorised as Option A, with no `BYTECODE_VERSION` change: no version-2 artifact has ever been
published, so refining the format costs nothing.

A private slot with no explicit initialiser is zero, materialised as one `ConstValue::Int(0)` per
slot **word** at a sixteen-byte record each. **38,087 of the corpus's 40,332 constants were exactly
that**, roughly 85% of the whole auxiliary body encoding a value the decoder can supply for nothing.

`DataInitRecord.first` now carries `ABSENT` for a wholly-default pool and stores no records. A pool
with any non-default value is stored in full — a trailing-run scheme would elide nothing for a value
written last. The sentinel is explicit and `decode_constant_pools` **rejects** it, so a reader that
has not handled the elision fails on the range rather than returning whatever `u32::MAX` addresses.

Measured at the artifact rather than the encoder, because an encoder that computed the elision and
stored the records anyway would pass a test of intent.

## REMOVING THE WASTE BROKE FIVE VACUITY CONTROLS, AND THAT IS THE FINDING

Seven byte-identity tests failed and **none was a defect**. Five were controls of the form *"this
input must exceed the buffer, or the mechanism under test is untested"*, and the elision removed
every oversize real input. **Without those controls the windowing and batching machinery would have
stopped being exercised while the whole suite stayed green.**

Two of them carried comments recording they had already been re-aimed **twice** for the same reason,
and a previous increment had built `synthetic_source_over` to end the cycle — sized against the
encoder's own output, so a win grows the input rather than disqualifying it. This was the third
round, there is no larger real stage left, and the generator is now the input.

Preconditions were **relocated, not weakened**. A real stage still proves region coverage and byte
identity; the synthetic case carries the oversize and batching guarantees. Two assertions came out of
the shared `assemble_whole_artifact` helper for the same reason.

## Where `CONSTS` emission now stands

The artifact-size ceiling is gone. What remains:

| bound | value | who it excludes |
|---|---|---|
| chunk batch | 90 records | `parse` (94), `wire` (475) |
| module-input node walk | 1,024 nodes | `wire` (1,148 chunk constants) |
| flattener out of `wire.fin` | 170 nodes | every stage, for the full region |

`parse` now has 817 chunk constants, under the 1,024-node walk cap, so `CONSTS` emission needs about
five flattener batches rather than a hundred and three. **The two 1,024-figures are different caps**
and I conflated them once, retracted in `50d949ab`.

## Held for the operator, with their rulings

- **`Op::cost()`**: 50 of 66 opcodes unmeasured. *Ruled: close sometime after Order 1.*
- **Derived operands in type rejection**: extraction still host-side, reaching them is a fixpoint.
  *Ruled: before publishing V0.3.0.*
- **Publication**: *held.*
- **The Japanese FAQ entry** is stale and renders as English. *Ruled: correct eventually.*

## What these green suites do NOT establish

Nothing here emits a single `CONSTS` byte for a stage module. The byte identity covers synthetic
sources under the 170-node flattener cap. The elision figures are properties of **this corpus**: a
stage gaining a non-default initialiser would store its pool in full, which
`a_pool_with_any_non_default_value_is_stored_in_full` pins because the corpus cannot supply it.

## ALL TWELVE STAGES ARE COROUTINES NOW

`verify_types.kel` and `wire.kel` were the last two entered as `fn main(cmd)`. Merged at `eec49eae`.

**`verify_types` genuinely streams** — one row per resume, cursors in a private block so they survive
the loop's `RESET`. **The eleven verdict tests could not establish that**: a stage folding everything
in its first step and yielding the answer immediately satisfies all of them while streaming nothing.
`the_fold_advances_one_row_per_resume` measures the resume count instead, and it failed first at two
yields per row — my counter was counting the loop's `RESET` as a step. The stage was right and the
instrument was wrong.

**`wire.kel` has a coroutine ENTRY, not streaming commands.** Each still answers in one yield. That is
a shell that would pass all 169 tests while streaming nothing, and the file says so.

### A COROUTINE MUST BE RESUMED, NOT RE-CALLED

Three parity tests failed with `OutOfArena` at 67,424 bytes while 166 passed. The three were the only
ones issuing hundreds of commands against ONE machine: each `call` on a suspended coroutine stacks
another activation instead of replacing it. Resuming reclaims the iteration's arena, so a thousand
commands cost what one costs — **which is the bounded-memory property the windowed goal is about**.
It surfaced as an arena error naming the operand stack, not the call pattern.

## Next intended increment

**Convert `wire.kel`'s emit commands to yield per record.** The entry is done and could not have been
done second. The concrete prize is the LAST remaining emit exclusion: the 90-record chunk batch cap
that keeps `parse` (94 chunks) and `wire` (475) from emitting their chunk regions. A per-record yield
removes the cap rather than routing around it, which is the difference between the windowed
architecture and a workaround — and it subsumes the batched-`CONSTS` increment that was planned
before the goal was stated.

Whatever is built, a refusal for any constant forest containing a composite or a name-bearing node
stays load-bearing: without it the path would silently emit a wrong region for the general case it
cannot handle.
