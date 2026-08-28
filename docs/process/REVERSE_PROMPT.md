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

**Absorptions 18, 19 and 20 are complete**, every prediction recorded before merging and every one
hitting exactly.

**Still blocked on you, all three unactionable here**: the `Fixed` shared-slot ABI, where the
recommendation splits on whether cross-language interop should be convention-based or
self-describing; the float entry ABI, ruled to settle alongside it; and the git-topology mechanism,
formally unruled but no longer contested.

---

## Last Updated (v0.2.3 line)

**Date**: 2026-08-28 (session 56) — an inbound finding closed by measurement, and sixteen op tags
the self-hosting oracle cannot see

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their own record now says the recommendation *splits* on a question you have not
answered — whether the fixed-point format must interoperate across object files from different
languages. Publication remains held.

## What this increment did

The `v0.3.0` line handed this line a finding they could not close: the self-hosted codegen stage's
63 op tags and the driver's decoder are two hand-maintained tables of the same numbers, and their
only guard asserts that decoding does not panic. **A transposition passed it.** It was unrecorded
here.

**The tables agree.** That is now a measurement rather than an inference from a comment claiming
they are kept in lockstep.

## Three things worth your attention

**ONE. THERE WERE THREE TABLES, NOT TWO.** The third is the copy of the decoder inside
`tests/selfhost_codegen.rs` — the one the differential oracle actually runs — which the shipping
decoder's own comment names as its source and claims to be in lockstep with. Nothing checked that.
It is the same pairing that produced five defects from one cause in August, and a drift there
would corrupt the oracle rather than the product.

**TWO. SIXTEEN OF THE SIXTY-THREE TAGS ARE INVISIBLE TO THE BYTE-IDENTITY ORACLE.** No stage source
emits them — the whole composite family, the unchecked arithmetic, and `checkedneg` — so a
transposition among them produces no byte difference to detect. They are named in the test rather
than counted, because the names are where such a defect would hide.

**I have scoped that claim deliberately and want the scope read.** It is the eleven-stage corpus.
The per-construct tests do compile struct constructions, array indexing, enum payloads and tuple
fields through the self-hosted compiler, so these are **not** "unchecked" — they are "invisible to
the self-hosting oracle", which is a narrower and true statement.

**THREE. ONLY ONE OF THE FOUR NEW GUARDS CATCHES THE DEFECT THE FINDING NAMED**, and mutation
testing is what established that rather than reasoning. A one-sided swap leaves the table a
bijection and leaves the two decoders agreeing with each other. The guard that sees it compares
each tag's NAME to the operation its number decodes to — a fourth hand-written table, which is a
hazard, and which earns its place only because it derives names from names where the others derive
numbers from numbers.

## What went wrong, since that is the more useful half

**The citation guard caught me inside ten minutes.** I renamed the census test for scope precision
and left the module header naming the old one. Fourth occurrence of that class here, and the
shortest interval yet between creating a stale citation and having it reported. The guard added
last session is now paying for itself against its own author.

**My first extractor would have compared two different populations.** A naive line pattern reports
63 decoder arms on one side and 111 on the other, the excess being arms of nested matches that look
identical by line shape. I checked the instrument before trusting the reading, which this line has
now had to do three times.

## One observation, attributed rather than assumed

`cargo clippy --tests --no-default-features -- -D warnings` fails with seven diagnostics. It fails
**identically on a clean `v0.2.3`** — established by stashing and re-running, not inferred — so it
is pre-existing and no part of this work. That combination is not one continuous integration runs.
Recorded as a fact about the tree, not as a claim that something is broken, and not fixed here
because it is outside what this increment was about.

## What I would take up next

The third type-channel extraction, which is Order 1 item 3 and the roadmap-advancing work.
`field_sets` at 80 lines or `occurrence_rows` at 100; leave `expression_nodes_and_derived` at 142
for last despite the capability argument favouring it. The pattern is established by the two slices
that already moved and is written into the handoff so it is not rediscovered.
