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

**Date**: 2026-08-27 (session 55) — `wire.kel` self-compiles byte-identically. The corpus is
eleven stages.

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

Publication remains held. **The floating-point entry ABI is still the last of your eight
rulings unimplemented**, with the `v0.3.0` line's `Fixed` shared-slot SCALE question attached.
**It is theirs to bring you and I have not acted on it.**

## Order 1 item 3 moved: two of five extractions

`decl_call_rows` has a pipeline analogue, the second of five to leave the reference parser's
abstract syntax tree. **The count is derived by a test rather than restated**, because a
hand-written count is a second definition that goes stale — which is how the handoff came to
assert a closed gap was still open.

**Both moved slices hit the same trap**: the reference numbers functions in declaration order,
the pipeline numbers chunks by sorted name. Comparing indices compares two unrelated
numberings. The escape both times was to carry a string.

**And a vacuity the obvious test would have missed.** If every corpus source declared its
functions in sorted order, comparing by name would be indistinguishable from comparing by
index — the property under test would go untested while the test passed. The corpus is now
asserted to separate the two orders.

**Not moved, and the test says so**: the per-argument ACTUAL-argument tag needs an expression
classifier, which is new work rather than a re-projection.

## The smaller finding, which I would not want lost under the milestone

**The citation guard never scanned the handoff**, and the handoff had carried a false claim:
its open correctness item 4 asserted a gap that commit `63574d1f` had closed, citing a pin
that **does not exist**. Three comments under `src/` and `tests/` repeated it, and the
`UNRESOLVED` register excused all three. **A citation in a debt register is not a citation
that is right — it is one excused from being checked.**

The guard now covers the two documents that are OVERWRITTEN each session and therefore hold
only current claims. It does **not** cover `TASKLOG.md` or `DESIGN_JOURNAL.md`, and that is
measured rather than assumed: those are append-only, and guarding them would have required a
sixty-entry excuse list on the first run — answering a guard by widening the excuse, which is
the failure being corrected.

It manufactured its own findings on the first run, flagging four corpus script filenames as
dangling citations. That lesson was already in this file once, from a wrapped identifier. It
is now recorded twice, because it has happened twice.

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
