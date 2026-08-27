# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-26 (session 54) — `wire.kel` compiles for the first time, and it is not
byte-identical

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

`#278`, `#279` and `#282` merged: `origin/v0.2.3` is at `f196a49c`, **152 merges**.
Publication remains held. **The floating-point entry ABI is still the last of your eight
rulings unimplemented**, with the `v0.3.0` line's `Fixed` shared-slot SCALE question attached.
**It is theirs to bring you and I have not acted on it.**

## The headline, with both halves stated together

**`wire.kel` COMPILES THROUGH THE SELF-HOSTED PIPELINE — 486 chunks, matching the reference.**
The largest stage in the corpus had never compiled at all.

**IT IS NOT BYTE-IDENTICAL.** Two chunks diverge: `emit_prologue` (40 operations against 59)
and `prologue_disagreed` (16 against 50). Both facts are pinned in one test file, because the
claim "`wire.kel` self-compiles byte-identically" was once invented on this line and reached a
doc comment, a pull-request body and all three channels before anyone checked it.

## Three causes cleared, and I first diagnosed two of them wrongly

| recorded cause | verdict |
|---|---|
| a capacity bound, read off the `1024` in an index message | **wrong** |
| the lexer having no hexadecimal or binary literal support | correct, fixed |
| a cap of 256 on the declaration count | **wrong** |
| a `Call` record whose chunk field overflows at index 256 | correct, fixed |

**Both wrong readings were a number in a message taken for a cause.** The second is the more
instructive: 256 was the right number and the wrong quantity. What refuted it was the
experiment that should have come first.

## The mechanism, because it explains every earlier confusion

A `Call` record packed `chunk + count * 256`. At index 256 the callee field carried into the
count: **the callee became chunk zero and the call popped one operand too many.** So the
symptom surfaced in the CALLER — and since chunk indices are assigned by **sorted name**, one
declaration added anywhere alphabetically earlier shifted a block of indices across the
boundary. That is how a line 1,400 lines away changed a function near the file's start.

The radix now **equals the chunk capacity**. That is the point rather than an incidental
choice: a roomier radix would leave a span no guard covers, recreating this defect one power
of two higher.

## What I deliberately did not do

The two divergent chunks compile byte-identically **when extracted verbatim**, so the gap is
context-dependent and its mechanism is unknown. I probed the construct they share four ways;
all identical. **That is guessing, and guessing failed eleven times on this file today.** The
finding is recorded with its direction — fewer operations, so a dropped construct rather than
a mistranslated one — and the increment stops there rather than expanding until it succeeds.

## Two process defects worth as much as the code

**The family was four sites and I derived three.** The missed one was a fourth implementation
of the packing in a test. The guard now walks the tree instead of naming files — **and then
flagged itself**, its pattern list being what it searches for, the third time a guard here has
done that.

**Three test runs were killed before I noticed the test was doing double work.** It compiled
four whole programs where two sufficed, at a minute each. I found it by reading a test I had
written twenty minutes earlier, after the third kill.

## What the next increment should take up

The two divergent chunks, with the context-dependence as the starting fact rather than a
surprise. Everything else in Order 1 is unchanged.

---

# Previous entry (session 54, earlier)

See [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md) for the full increment-by-increment record,
which is append-only and keeps corrected claims beside their corrections.
