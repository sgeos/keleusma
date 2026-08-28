# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## ⚠ TWO LINES SHARE THIS FILE — READ BOTH SECTIONS

The protocol says overwrite this file each session. **I did not.** The report below the V0.3.X
section is the `v0.2.3` line's, written at `b725c1f2`, and it is current for that line. Overwriting
it would have destroyed a channel this line does not own. **This choice is stated rather than made
silently**, so the next reader knows the deviation is deliberate.

The V0.3.X line's full resume prompt is [`handoffs/v0.3.0.md`](./handoffs/v0.3.0.md), which is
self-contained and carries the ancestry check. What follows is only the bounded summary.

---

## V0.3.X — native code generation, 2026-08-27, after absorption 18

**Verification.** `native_codegen` **314 passed, 0 failed, 59 binaries**, and clean under
`clippy -D warnings` and `fmt --check`. The main workspace **2461 passed, 0 failed, 87 binaries**.
Both figures read cargo's own exit status **and** the summed per-binary counts, and the two agree.
`native_codegen/` is a detached workspace **not built by CI**, so this local suite is its only gate.

> **Run the two suites SEQUENTIALLY.** In parallel they invalidate the workspace perf canary —
> 69.04s under concurrent load against a 30s tripwire, 1.20s alone. A 57x false red.

**Absorption 18 landed, and both predictions were recorded before merging and hit exactly**:
`native_codegen` unchanged, because no stage source or example script was touched; the workspace up
by exactly two, being the incoming tests. The ownership check is empty and was shown non-vacuous
against the previous absorption point.

**The finding, and it corrects something I told you before.** I reported the composite slot-reuse
defect as latent because no corpus module had the escaping shape. **That was wrong, and it was
wrong in two documents.** `examples/scripts/13_telemetry_stream.kel` carries the shape deliberately
and says so in its own header. What actually keeps the defect quiet is the backend: it refuses that
module with *"native lowering does not yet support opcode `Stream`"*, and every chunk that can carry
the shape is a `loop` chunk opening with `Stream`. **The safety is accidental — it rests on an
unimplemented opcode rather than on any escape reasoning, and it expires the day `Stream` lowers.**
I found this by measuring where I expected zero and getting one, not by re-reading.

**What I did about it.** The backend now refuses the shape at the placement itself
(`LowerError::YieldEscapingLoopComposite`). Measured cost over 91 modules and 1117 chunks: one chunk
carries the shape, that one was already refused, and **zero are newly refused** — the coverage
censuses `61 of 66` and `1070 of 1074` both held. Refusing is sound even if the underlying verdict
is wrong, because the result is only ever used to refuse and never to place, so the recorded reason
a wrong verdict cannot miscompile stays intact.

**Two things I want stated plainly rather than left implied.** The refusal is **shadowed** by the
`Stream` refusal today, so it cannot fire on unmutated input; I proved it fires by removing `Stream`
from compiled bytecode, and left a tripwire test that fails the day `Stream` lands so whoever lands
it must confirm this guard takes over. And **the obligation is narrowed, not discharged**: slot reuse
is unchanged, and a composite built in a loop, returned, and yielded by the caller is still invisible
to a single-chunk predicate.

**A gap in our own gate.** `native_codegen` had never been linted; the first `clippy -D warnings` run
found four warnings, all from this line's earlier work, one substantive — a census counted a figure
and never printed it. Neither `scripts/release-gate.sh` nor CI covers this subproject.

**Still blocked on you, all three unactionable here**: the `Fixed` shared-slot ABI, where the
recommendation splits on whether cross-language interop should be convention-based or
self-describing; the float entry ABI, ruled to settle alongside it; and the git-topology mechanism,
formally unruled but no longer contested.

---

## Last Updated (v0.2.3 line)

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
