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

## V0.3.X — native code generation, 2026-08-27, `origin/v0.3.0` at `9c87f24e`

**Verification.** `native_codegen` **306 passed, 0 failed, 58 binaries**; the main workspace
**2459 passed, 0 failed, 87 binaries**. Both figures read cargo's own exit status **and** the summed
per-binary counts, and the two agree. `native_codegen/` is a detached workspace **not built by CI**,
so this local suite is its only gate. Measured at `1a228270`, whose tree hash is identical to the
stamped commit's, so the figures transfer by construction.

> **Run the two suites SEQUENTIALLY.** In parallel they invalidate the workspace perf canary —
> 69.04s under concurrent load against a 30s tripwire, 1.20s alone. A 57x false red.

**The increment.** The Order-1 differential gate now seeds **12 of 12 stage sources, 0 unseeded**
(was 3 unseeded), at **2460 comparisons**; the last three seeded without the read-only accessors
previously assumed to be the only route. `FixedDiv` lowers, taking the backend to **61 of 66
opcodes** and **1070 of 1074 corpus chunks, 99.6%**. Every remaining opcode is accounted for by
name. Static-site region non-reuse is now **enforced by a test on ranges**, not merely documented.

**One concern, stated plainly, and it is the reason to read the handoff before touching the
planner.** The backend reuses a loop site's slot across iterations **unconditionally**, with no
reference to whether the previous value escaped. For a composite that leaves its iteration by
`yield`, that is **unsound**: the value is a handle, an in-place overwrite advances no epoch,
`resolve` succeeds, and the host silently receives the wrong iteration's bytes. **This is required
for soundness and is NOT discharged.** It is latent only because no corpus module has the shape —
a fact about the corpus, not about the backend. I earlier told the proof line this obligation was
discharged, having conflated static-site disjointness (true) with cross-iteration reuse (false);
that was retracted and their record stands as written.

**The design tension worth the operator's attention**: discharging it requires the region planner to
consume a confinement verdict, and consuming no verdict is exactly why a wrong verdict cannot
miscompile anything today. Both properties cannot be had for free.

**Three items are blocked on the operator and none is actionable here**: the `Fixed` shared-slot ABI
(preference B > A > C, but the recommendation now splits on whether cross-language interop should be
convention-based or self-describing — the measured facts are that the scale `N` is absent from every
host-visible surface and the width is build-dependent); the float entry ABI, which was ruled to
settle alongside it; and the git-topology mechanism, which is formally unruled here but no longer
contested, since both operators' actual words were a merge and seventeen absorptions have used one.

**Next**: absorption 18, three commits from `v0.2.3` (#304), measured alone.

---

## Last Updated (v0.2.3 line)

**Date**: 2026-08-27 (session 55) — `wire.kel` self-compiles byte-identically. The corpus is
eleven stages.

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

Publication remains held. **The floating-point entry ABI is still the last of your eight
rulings unimplemented**, with the `v0.3.0` line's `Fixed` shared-slot SCALE question attached.
**It is theirs to bring you and I have not acted on it.**

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
