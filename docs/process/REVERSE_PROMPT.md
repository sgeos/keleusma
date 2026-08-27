# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-26 (session 54) — the largest stage was blocked on hexadecimal literals,
and I published a false cause for what blocks it next

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

`#278` and `#279` merged: `origin/v0.2.3` is at `823f0894`, **151 merges**. Publication
remains held.

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with
the `v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you
and I have not acted on it.**

## The two findings, and one of them is a retraction

**1. The self-hosted lexer never supported hexadecimal or binary literals.** It consumed the
leading `0`, stopped, and interned the rest as an IDENTIFIER — `0xFF` was the number *zero*
followed by a name `xFF`. `wire.kel` uses thirty-five of them. That is why the largest stage
in the corpus could not self-compile, and it is a far smaller thing than the "capacity
bound" this line recorded for two sessions.

**2. I then published a false cause for the NEXT blocker and retracted it within the hour.**
Bisected to one line: `wire.kel` at 1,673 lines self-compiles, at 1,675 it does not, one
declaration apart. I counted declarations, got 256 and 257, and wrote *"a cap of 256 on the
chunk count"* into the brief as a finding. **A synthetic program of 300 trivial chunks
compiles**, so that is false. The measurement was true; the cause inferred from it was not.

**Third time in two increments that a number in a message was read as if it identified a
cause**, and this one happened while I was writing the document about that error. The number
was in the right place, which made it more convincing rather than less.

## Proportionality, which must be stated every time

`self_hosted_compile` cross-checks against the reference and refuses on divergence, so a
command-line user got a loud error, never a wrong artifact. Direct callers of
`self_host_compile` got a module with an undefined name where a constant belonged.

## What made the difference, in case it generalises

**Reading the reference rather than guessing.** I would have written `0B` as an
unconditional binary prefix. It is not: `0B` is binary only when a binary digit follows,
since otherwise the `B` begins the `Byte` suffix and `0Byte` is the byte literal zero.

**Taking a baseline by stashing the change.** Eight radix forms diverged before and agree
after; two numeric-suffix forms diverged before and still do, so that gap is demonstrably
**pre-existing and untouched**. Without the baseline the second clause would be an
assumption.

**The named failure modes from the previous increment.** They gave the chunk in one reading.
The equivalent trace before they existed cost seven increments.

## What is established about the remaining blocker, and nothing more

- The bisect boundary is exact and reproducible.
- The chunk count alone is **not** the trigger.
- The reported chunk name (`put_u64`, line 270) **cannot** be the location, since a
  declaration 1,400 lines later cannot affect it. It is a label from an interned id.
- The mechanism is **unknown**. Naming it is the next increment.

**A guard worth strengthening regardless.** `every_chunk_indexed_array_admits_the_chunk_cap`
exists because widening one array did not admit `wire.kel` — its own doc says a cap is a
FAMILY — yet it derives that family from a hand-written list of two index expressions in one
file. Whether or not it relates to this defect, that is the recorded meta-defect in pure
form.

---

# Previous entry (session 53)

**Trimmed 2026-08-26.** The retained text below described `wire.kel`'s blocker as a capacity
bound, `IndexOutOfBounds(-1, 1024)`. **That reading is retracted** — an index of `-1` is
below the start, not past the end. Rather than leave a superseded cause standing in a
channel a resuming session reads, the stale section is removed; the full history is in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md), which is append-only and keeps both the claim and
its correction.

What session 53 delivered, accurately: the confinement analysis in `src/confine.rs` with
callee summaries, the comment-citation guard, and bare-`for` support that self-compiles
byte-identically.
