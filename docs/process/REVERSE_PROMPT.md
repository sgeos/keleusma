# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-27 (session 55 CLOSE) — `wire.kel` is byte-identical, the corpus is eleven
stages, and nothing is open

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

`origin/v0.2.3` is at `51d512c8`, **157 merges**, **no open pull request**. Eight merged today,
each at 22 of 22 green. Publication remains held.

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I
have not acted on it.**

## The milestone

**`wire.kel` self-compiles byte-identically** — 486 chunks, **125,540 bytes on both sides, zero
chunks differing.** The largest stage in the corpus, and the last one outside the byte-identity
oracle, is in it. Ten stages become eleven.

That sentence was **invented on this line once** and reached a doc comment, a pull-request body
and all three channels while the compile still panicked. It is now a test's output.

## Four causes, and I first diagnosed two of them wrongly

| recorded cause | verdict |
|---|---|
| a capacity bound, read off the `1024` in an index message | **wrong** |
| the lexer having no hexadecimal or binary literal support | correct |
| a cap of 256 on the declaration count | **wrong** |
| a `Call` record whose chunk field overflowed at index 256 | correct |
| `forin_count` not reset between functions | correct |

Both wrong readings took **a number in a message for a cause**. The nearer miss had the right
number attached to the wrong quantity.

**The tally is stark and it is now guidance rather than history: guessing failed seventeen times
across those four causes; prefix bisection succeeded three out of three.**

## What else landed

**Order 1 item 3 moved from one of five extractions to two of five.** The count is derived by a
test, never restated — because a hand-written count is a second definition that goes stale, which
is exactly how this handoff came to assert an already-closed gap was open.

**The citation guard now scans the documents that make current claims.** It had never scanned the
handoff, which had carried a false claim for at least a session. It does **not** scan the
append-only documents, and that scope was measured rather than assumed: guarding them would have
needed a sixty-entry excuse list on the first run.

**The proof line's branch merged**, `#303`, merge commit `8414a1a1`, documentation only.

## The one thing I want to flag about that merge

**The peer stated that you had authorized acceptance. I did not act on that**, and could not — a
peer cannot supply your approval. It merged on my own standing authorization for a green pull
request, plus this line's own recorded arrangement, plus my own verification that the merged proof
is **byte-unchanged** from the audited commit. The peer accepted the correction without
qualification.

They also said an earlier message from this line had told them acceptance was authorized. **I
cannot verify that.** The closest thing in our mailbox is the opposite — this line telling them a
relayed ruling is not authorization it can act on. If such a message existed it was never
persisted, and I am not treating it as fact.

## What almost every defect this session had in common

**They were in my verification, not in the code.** I derived a family of three that was four and
one of seven that was 26. I wrote guards that could not fire, one that flagged itself, and one
that reported four filenames as dangling citations. Two mutation attempts silently failed to
COMPILE, which looks exactly like a guard not firing. Twice a local gate covered the feature the
work was about and missed the feature sets that lack it.

**Every one surfaced by running something. None by reading.**

## What I would take up next

The third type-channel extraction. `field_sets` at 80 lines or `occurrence_rows` at 100; leave
`expression_nodes_and_derived` at 142 for last despite the capability argument favouring it. The
pattern is established and written into the handoff so it is not rediscovered.
