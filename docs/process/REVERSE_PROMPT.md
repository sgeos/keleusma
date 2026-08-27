# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-27 (session 55) — `wire.kel` self-compiles byte-identically. The corpus is
eleven stages.

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

Publication remains held. **The floating-point entry ABI is still the last of your eight
rulings unimplemented**, with the `v0.3.0` line's `Fixed` shared-slot SCALE question attached.
**It is theirs to bring you and I have not acted on it.**

## The milestone, measured

**`wire.kel` SELF-COMPILES BYTE-IDENTICALLY.** 486 chunks, **125,540 bytes on both sides,
zero chunks differing.** The largest stage in the corpus, and the last one outside the
byte-identity oracle, is now in it — ten stages become eleven.

**That sentence was once invented on this line** and reached a doc comment, a pull-request
body and all three channels while the compile still panicked. It is now the output of
`self_host_compiles_wire_kel_byte_identically`, not a recollection.

## The cause was one line

`forin_count` — the bare `for` form's program-order counter — was never added to the
per-function reset that already cleared its own documented analogue, `forlimit_count`. It
indexes a record as `7 * forin_count`, so the **second and every later function** containing a
bare `for` emitted a record pointing past its own parts. **That is why the stage emitted FEWER
operations rather than different ones.**

## Four causes over three sessions, and I first diagnosed two wrongly

| recorded cause | verdict |
|---|---|
| a capacity bound, read off the `1024` in an index message | **wrong** |
| the lexer having no hexadecimal or binary literal support | correct |
| a cap of 256 on the declaration count | **wrong** |
| a `Call` record whose chunk field overflowed at index 256 | correct |
| `forin_count` not reset between functions | correct |

Both wrong readings took **a number in a message for a cause**. The nearer miss was the third:
256 was the right number attached to the wrong quantity.

## What actually worked, since the tally is stark

**Guessing failed seventeen times across these four causes. Bisection succeeded three times
out of three.** The method that closed this one: prefix bisection with the predicate *do these
chunks match* (not *does it compile*, which passes everywhere); rebuilding the function with
its REAL callees rather than simplified stand-ins, which is why an earlier extract came back
clean; delta-debugging to the loop alone; then a five-line synthetic isolating one bare loop
from two.

**Then the rule predicted the file before I looked at it.** `wire.kel` has three bare-`for`
functions; the rule says every one after the first diverges; those are exactly the two that
did.

## The near-miss worth your attention

**My detector matched a COMMENT** reading `for k in 0..3`, predicted four diverging functions
against an observed two, and I was a step from concluding the rule was too strong. The
instrument was broken, not the finding. **Check the instrument before doubting the result.**

## What is next

Order 1's remaining items, unchanged: the region kinds at 93% produced / 56% computed, and the
type checker's input, whose extraction is still Rust walking the reference AST. The structural
blocker that stood behind both is gone.
