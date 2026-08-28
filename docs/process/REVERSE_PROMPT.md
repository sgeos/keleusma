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

**A gap in our own practice, and I described it wrongly the first time.** I reported that neither
`scripts/release-gate.sh` nor CI covers `native_codegen`. **⚠ CORRECTED. THE GATE DOES COVER IT; IT WAS NEVER RUN.** The superseded claim was that
`scripts/release-gate.sh` and CI do not cover `native_codegen`. **False.** That script runs
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, AND
`RUSTDOCFLAGS="-D warnings" cargo doc` over the subproject, in a step whose own label reads *"gated
nowhere else"*. The step is conditional on an LLVM 22.1 install and prints a loud SKIPPED banner
otherwise, and **LLVM 22.1 IS installed here**, so it would have run. The warnings accumulated
because **the gate was never run on this line** — the everyday loop substituted `cargo test` for it.
**That is a worse finding than the one it replaces**: the coverage existed and was bypassed. Running
the missing step found a real failure it had been hiding — a public item linking to a private one,
which fails `cargo doc -D warnings` and is invisible to both test and clippy. Now fixed and the doc
build is clean.

**The interprocedural residual is now measured rather than named.** I said last time that a composite
built in a loop, returned, and yielded by the caller was a hazard I could not see. Following the call
graph: of 14 chunks that construct inside a loop, the crude figures are zero by call and two by
return, and **both return candidates are ruled out because the yielding caller returns `Word`** — a
`loop` chunk's return type is what it yields, so no composite can reach the host through it. Refined
residual **zero**. I deliberately did not refuse it: that refusal would rest on three stacked
over-approximations with no data flow and no instance to justify the cost, and the whole class sits
behind the `Stream` refusal anyway. The census asserts zero, so an instance would fail loudly.

**The last two composite refusals are now explained to the cause.** The unknown operand is the first
of three, produced by a read of the `for` loop's induction variable, which each chunk writes twice —
and a local's width is trusted only when written at most once, because the width pass is a linear
scan and cannot see a back edge. I derived that by simulating the stack from the instruction set's
own published effects, after a heuristic walk gave me a confident wrong answer.

**The fix I reverted last time is back, with the thing that can judge it.** I built a differential
that runs a multi-function program written inline through both the native lowering and the reference,
which the tree could not do before. Its ABI check earned its place on the first run: a pure-`Word`
program emits a one-parameter entry, not the four-pointer one I had assumed, and calling it the wrong
way would have been a SIGSEGV inside JIT code with nothing to attribute it to. With that in place,
the target case was seen to **fail** first — refused for an unknown operand width — and then to pass
and agree once the width seeding returned. **Coverage is unchanged at 1070 of 1074**, because the
seeding widens the accepted set only for programs the corpus does not contain, which is precisely why
it needed a harness rather than a coverage number.

**Superseded, quoted so it is not mistaken for current:**

> *"Closing that asymmetry was correct and it changed nothing... Since no harness here can execute a
> source-string program containing a call, keeping it would have meant widening a compiler's accepted
> set with no execution-backed check, so it is out... The named prerequisite is a source-string
> whole-module differential harness."*

That prerequisite is what this increment built, so the change is back rather than out. **What still
stands from it**: lifting the two composite refusals needs a fixpoint over local widths, which is its
own increment, and coverage is still 1070 of 1074.

**A coverage figure I gave you last increment was wrong, and I found it by naming things.** I set out
to name the last chunks the backend will not lower, because "other" is a bucket and not a cause. There
are three refusals — `Stream` in the telemetry stream, a float constant in the float witness, and
`Len` in the refused witness — where the coverage figure implied two. That gap was the finding: the
census marks a chunk unlowerable by matching a refusal's symbol to a chunk name, and **a module
refused as a whole names no chunk**, so both chunks of the float witness were counted as lowerable
while the backend emitted nothing for them. Corrected, **1072 of 1074 becomes 1070 of 1074**.

**What survives of the previous claim, precisely**: the delta was right and the level was not. The
width certification did lift exactly two chunks, so the honest movement is **1068 → 1070**, and the
execution evidence — 59 to 61 modules running and agreeing with the reference — never depended on the
census at all.

**One process note.** A gate run was killed by a signal partway through and reported 51 tests passed,
which is a plausible-looking number and not a result. I re-ran it rather than record it; the clean run
is 332 passed, 0 failed, 63 binaries.

**The last two composite refusals are closed.** A local written more than once is now trusted when
every write's producer fixes its width by the instruction itself. **Coverage 1070 → 1072 of 1074**,
and — the part that actually matters — the corpus differential goes from 59 to **61 modules executed
and agreeing** with the reference. A wrong width would have raised coverage identically and mispacked
in silence, so the execution figure is the evidence and the coverage figure is not.

**The fixpoint I told you this would need was not needed.** The arithmetic result slot carries a
literal width whatever its operands, so the loop counter's two writes depend on nothing. Reading that
one line removed an increment of planned work.

**One thing I got wrong and want on the record.** I built a stack walk on `stack_growth`/`stack_shrink`
as though they were pop and push counts. **They are the peak model, their own documentation says so
and names the right function, and this repository had already recorded another place making the same
mistake.** The wrong walk mis-attributed the loop increment's value — precisely the classification the
fix depends on. I re-derived the earlier published conclusion rather than assume it survived; it did,
but that was only knowable by checking.

**Absorptions 18 through 21 are complete**, every prediction recorded before merging and every one
hitting exactly.

**Still blocked on you, all three unactionable here**: the `Fixed` shared-slot ABI, where the
recommendation splits on whether cross-language interop should be convention-based or
self-describing; the float entry ABI, ruled to settle alongside it; and the git-topology mechanism,
formally unruled but no longer contested.

---

## Last Updated (v0.2.3 line)

**Date**: 2026-08-28 (session 56) — an inbound finding closed, and Order 1 item 3 moved to three of
five

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their own record now says the recommendation *splits* on a question you have not
answered — whether the fixed-point format must interoperate across object files from different
languages. Publication remains held.

## What this increment did

Two things landed. The first closed a finding the `v0.3.0` line handed over and could not close
themselves; the second is the roadmap item.

**ORDER 1 ITEM 3 IS AT THREE OF FIVE.** `field_sets` joins `binding_rows` and `decl_call_rows`.
Two remain, and only the DECLARED half of `field_sets` moved — its field accesses still walk the
reference syntax tree, which the function and the test both say in their own words rather than
letting the headline imply more.

## The part worth your attention: my brief was wrong, cheaply

I wrote a brief saying the work meant surfacing a table held inside `parse.kel`, which would have
required new emission from a stage that is itself in the byte-identity corpus — a much larger and
riskier increment.

**`parse.kel` was already emitting all of it.** The struct's name and every field name were on the
record stream, in declaration order, and the driver mapped the whole run to skip state and threw it
away. The increment touched no stage source.

**The lesson is not "read more".** I did read — I read the producer's internal data structures and
reasoned about what the host could not see. The record stream is the interface, and it already
carried the answer. Reading the producer's internals told me about the producer, not about what
crosses the boundary. The correction is recorded beside the original claim rather than edited away.

## What mutation testing caught that reasoning did not

The driver had one skip state covering struct, trait and impl declarations, and that state exists
because those three once faulted the driver on 29 boundary cases. Collecting structs meant
splitting it.

**Re-admitting trait and impl into the collect leaves the agreement test PASSING**, because its
probes contain neither. A guard whose corpus lacks the construct is a guard for a different
question. A second test now carries that case, with its spelling taken from a shipped example
rather than invented, because five of this line's probes have measured a malformed input and
reported the result as a finding about the stage.

## The earlier increment, briefly

The `v0.3.0` line observed that the self-hosted codegen's 63 op tags and the driver's decoder are
two hand-maintained tables whose only guard asserts that decoding does not panic — **a
transposition passed it**. There were **three** tables, not two; the third is the decoder copy the
differential oracle actually runs, which the shipping decoder claims lockstep with and nothing
checked. **They agree**, now measured rather than inferred.

And a measurement that bears on coverage generally: **sixteen of the sixty-three tags are exercised
by no stage source**, so the self-hosting oracle cannot see a transposition among them. Scoped
deliberately — the per-construct tests do cover composites, so these are invisible to that oracle,
not unchecked.

## Nothing is waiting on you except the ruling you already have

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Publication remains held.

## One observation, attributed rather than assumed

`cargo clippy --tests --no-default-features -- -D warnings` fails with seven diagnostics, and fails
**identically on a clean tree** — established by stashing, not inferred. Pre-existing, not a
combination continuous integration runs, and not fixed here because it is outside what these
increments were about.

## What I would take up next

The occurrences half of `occurrence_rows`, then `expression_nodes_and_derived`. Node kind 2 is
`Local` and carries a slot, and the driver holds parameter and `let` names, so a slot-to-name map
is available. What I have NOT established is which record carries a bare identifier that is neither
a call nor a binding site, and I am not going to predict it again.

## A postscript on that, because it is the session's most repeated mistake

I predicted twice that a slice would be harder than it was, both times by reading the stage's
internal data structures and reasoning about what the host could not see. Both times the record
stream already carried the answer. The second time I measured before acting, which is the only
reason it cost nothing.

The instrument for this is `parse_record_trace`, and it is public precisely so the stream can be
read from outside the driver. Both handoffs now say to use it.
