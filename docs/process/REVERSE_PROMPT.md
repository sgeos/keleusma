## The op-tag residue is four, not sixteen

Earlier in the session I reported sixteen op tags the byte-identity corpus cannot check, and said
the per-construct tests were a different population I had not measured. I measured a second one —
the fifteen shipped examples — and **it covers twelve of the sixteen**, the whole composite family.

Four remain unreached by either corpus: the unchecked arithmetic that `Byte` operands take, plus
unary negation. The description is checked by probes inside the test rather than asserted, because
this project has called an unwitnessed opcode unreachable before and been wrong.

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

**I closed a gap I had described wrongly, and the correction is the interesting part.** Last time I
told you a tail-yielded composite lowers with nothing executing it, and called the untested code a
composite crossing the yield boundary. **There is no yield boundary there.** The lowered module
declares no host yield hook at all and the entry returns a pointer into the region the caller
provides, so a single yield in tail position is compiled as a return. The probe I wrote to examine the
boundary failed on its own message, which had said in advance that if no hook were declared then the
probe was aimed wrongly.

The shape is now witnessed properly: the native body and the reference's body, resolved through the
arena, are identical byte for byte. My first attempt compared the reference's debug text, which prints
the handle rather than the contents, and failed — comparing an address to an address would have proved
nothing about marshalling.

One fact worth your attention: **the reference suspends where the native side returns**, and they
agree on the value. That is what the degenerate-yield path means, and it is a thing to know before
reasoning about suspension. What is genuinely still uncovered is sequence semantics for a
composite-yielding stream, and that is blocked rather than merely unwritten, because it needs a
non-tail yield and those are refused.

**Two things this iteration, and one of them is a red I am not going to fix.**

**The stream frontier is tail position, not composites.** A single `yield` in tail position lowers —
**including a yielded composite** — and everything else is refused: a yield followed by code, two
yields, a yield inside an `if`, a yield inside a `for`. The pair that settles it is that a composite
in tail position lowers while a plain `Word` with code after it does not, which refutes the natural
guess that composites are what the telemetry stream cannot get past. Consequence for the open
soundness item: **the yield-escape refusal is still shadowed**, because the escaping shape is still
refused — now asserted by a test rather than inferred. One gap named and not fixed: a tail-yielded
composite lowers and nothing in the tree executes it.

**The blockage is over and the branch is published again.** PR #314 landed, absorption 25 carried it
in, and I then **ran** the workspace suite rather than inferring it from the fix: 2479 passed, 0
failed, 89 binaries. The prediction's arithmetic was written before merging and held. Nine commits
waited four iterations rather than go through `--no-verify`, which cost nothing but patience and kept
the gate meaning what it says.

**Superseded, quoted so it is not mistaken for current:**

> *"I cannot push, and I want you to know that rather than discover it."* The pre-push gate runs the
workspace suite, the workspace suite is red, so `v0.3.0` is **5 commits ahead of origin with
everything committed and nothing published**. I did not use `--no-verify`: the gate is correct that
the suite is red, and bypassing it would publish a branch its own gate rejects and set the precedent
that a red attributed to someone else is one worth skipping. If `keleusma-02` has not acted by the
next resume, this is yours to rule on.

**The workspace suite is red, and it is not a defect in either line.** Absorption 24 brought a test of
yours whose pinned set of unexercised op tags is branch-dependent. On this branch the residue is
smaller than the pin, which its own message calls a coverage gain, and the cause is one of this line's
own witness scripts doing Byte arithmetic. **I did not touch it** — this line keeps `src/` and
`tests/` byte-identical to `v0.2.3`, and the ownership check I run at every absorption asserts exactly
that, so editing it would destroy the property those checks rest on. I reported it to `keleusma-02`
with the cause. **I am not reporting the workspace suite as green.**

**The opcodes whose lowering had never run are now each resolved to a status.** An arm that exists is
not an arm that works, so I asked the four one at a time. Two float conversions are simply **refused**
— one by name, the other unreached behind it. `IsStruct` has no producer I could find, and the
reference's own arm only accepts a boxed struct body, of which the B28 work left none, so even a
mutation witness would compare against a fault rather than a value.

**`Reset` was the interesting one, and my own brief was wrong about it.** I predicted it was gated
behind the `Stream` refusal. A minimal `loop main` emits it and the backend refuses nothing — so
**`Stream` is lowered for that shape**, and an earlier statement of mine that `Stream` is unsupported
was true of one module rather than of the opcode. `Reset` has in fact had an execution witness all
along, in the suspension differential's fifteen subjects, which the census cannot see because it
surveys only the shipped corpus. **No census figure moved, and none should have.**

**Nothing was widened to make a test possible.** The float guard blocks the conversion witness, and
that is the finding rather than an obstacle to work around; the float ABI is yours to rule on in any
case.

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

**Date**: 2026-08-28 (session 56 CLOSE) — six merges, Order 1 item 3 at FOUR of five, and the
twelfth stage's silence explained

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their own record now says the recommendation *splits* on a question you have not
answered — whether the fixed-point format must interoperate across object files from different
languages. Publication remains held.

## Five increments merged, each at 22 of 22

`origin/v0.2.3` at `93e66b24`, **162 merges**, **no open pull request**. Publication remains held.

| | |
|---|---|
| #308 | the op-tag tables agree, and something now checks that they do |
| #309 | `field_sets` reaches the type channel — Order 1 item 3 at **three of five** |
| #310 | the declared names reach it too, and the wildcard-import gap is located |
| #311 | the twelfth stage does not self-compile, and the tree now says why |
| #312 | a second corpus narrows the unexercised op tags from sixteen to four |
| #313 | the name occurrences move — Order 1 item 3 at **four of five** |

## Nothing is waiting on you except the ruling you already have

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I have
not acted on it.** Their record says the recommendation now splits on a question you have not
answered: whether the fixed-point format must interoperate across object files from different
languages.

## The one mistake I made three times

**I reasoned from a component's internals about what crosses its boundary.** Twice I read the
parser's data structures, concluded the host could not see something, and sized a large increment —
and the record stream already carried it, so both slices needed no stage change at all. Once I
inspected a function's constructs to explain a refusal and named three plausible culprits;
declaration order was the cause and none of the three mentioned it.

I measured before acting on the second and third occasions, which is the only reason they cost
nothing. Both handoffs now carry the rule and name the two instruments.

## The decision I want visible rather than taken quietly

**`verify_types.kel`, the twelfth stage, does not self-compile.** A function reads a `data` block
declared later in the file, and the parser builds its field table as it meets each block, so the
reference resolves to nothing. Four-line witness, with a control differing only in declaration
order.

**I did not attempt the repair.** It means collecting data declarations before parsing bodies — a
two-pass restructuring of a single-pass streaming parser, not a defect fix. What landed converts an
unexplained absence into a documented, reproducible gap whose pins fire when it closes. If you want
the corpus at twelve, that is the next large item and it is your call whether it is worth the
restructuring.

## Two things I corrected in my own work

A guard I wrote earlier in the session compared arm **spellings** where its own message described
which **codes** were handled; splitting a range made that visible and it now compares coverage.

And a mutation harness reported "zero compile errors" for three mutants while running **nothing** —
a shell variable escaped inside a quoted heredoc. Zero errors from a command that never ran looks
exactly like a clean mutant. Re-run properly, two of three fired.

## On "four of five", because the number would flatter the state

**Moved means an analogue exists, not that nothing is left.** The count pin's own documentation now
carries a table of the residual in each of the four, so the figure cannot be read as completeness:
`decl_call_rows` left its actual-argument tag, `field_sets` its field accesses, and
`occurrence_rows` two shapes.

Those two are different in kind and the difference is the useful part. A `data` block identifier is
**representational** — the pipeline has no ident node there at all, so nothing is missing from the
wire. A `for` loop variable is **a wire gap**: the read reaches the forest but nothing binds its
slot to its name, because only `let` bindings emit a name record. The occurrence is dropped rather
than reported under a wrong name, which is the better of the two failures. Closing it is the same
shape of change as the record that already exists, and that one needed your ruling.

## What I would take up next

The last extraction, `expression_nodes_resolvable`; or the two-pass data resolution, which would
take the byte-identity corpus to twelve and is the largest single item I can see — **that one I
flagged as yours to call rather than start unilaterally.**
