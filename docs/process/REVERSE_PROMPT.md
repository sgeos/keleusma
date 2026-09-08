# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## THE SUITE IS GREEN AGAIN — AND FOUR OF THE TEN RED TESTS WERE A DEFECT, NOT A RATCHET

**483 tests pass in BOTH float configurations, every run frozen**, from `464 passed, 10 failed` at
the previous head. **No test was deleted.**

**The previous handoff — mine — was wrong in a way that mattered.** It recorded all ten failures as
ratchets asserting the old frontier and said to invert rather than repair them. Four of them were
reporting a defect that general `Op::Stream` lowering had just introduced, and inverting those four
would have encoded it.

**The mechanism, which generalises past this instance.** `degenerate_stream_yield` returns an
`Option`, and `None` had come to mean two incompatible things: *not degenerate* and *unsafe*. That
was harmless while `None` led to one place, a refusal. General `Stream` lowering then defined its own
applicability as exactly `Stream && degenerate_yield.is_none()` — **so it inherited every soundness
rejection as a feature request.** A stream calling a callee that can itself suspend went from refused
to `Refusals: []` with nothing else changing.

> **A predicate whose negative answer is consumed by more than one caller must say WHY it declined,
> or the second caller reads the first caller's safety check as a hint.**

**Three failures were correct widenings**, and each now carries a whole yielded sequence compared
against your runtime rather than an argument. The strongest is a tail that writes the private data
segment: the write survives `Op::Reset`, so a lowering that dropped it, duplicated it, or ran it
before the suspension would diverge on the second iteration.

**Three were simulations whose premise came true.** They mutated real bytecode to remove `Op::Stream`
and ask what the yield-escape refusal would take over *"on the day `Stream` lowers"*. That day
arrived, so the mutations are gone and each measures the shipping backend. The former shadowing
tripwire is now `the_yield_escape_refusal_now_fires_unshadowed`, and it additionally asserts the
retired refusal is ABSENT — otherwise a future refusal moving back in front would let it pass for the
old reason. `13_telemetry_stream.kel` is refused for the yield-escape hazard, naming the site.

## THE FRONTIER WAS RE-DERIVED, NOT EDITED, AND THAT FOUND SOMETHING

Tail position is no longer the discriminator. **It is a composite that escapes the iteration that
built it**, and three shapes are needed to establish that rather than two, because a pair leaves both
*"composites are the problem"* and *"loops are the problem"* standing.

Adding the shapes the matrix was missing then exposed a refusal class nothing in the tree had named:
**a composite built from a RESUMED VALUE is refused because that value carries no declared width.**

That corrected a claim in the same file, which said composite-yielding sequence semantics could not
be witnessed until a non-tail yield lowered. A non-tail yield lowers now. The gap is still open, for
a different reason — and a tractable one, since the resume value's width is the chunk's declared
parameter-0 type, which the emitter already trusts for local slot 0.

## ⚠ A PANIC IN `src/confine.rs` ON A TRUNCATED OP STREAM — YOURS, WITH A REPRODUCTION

`walk` in `src/confine.rs` (around line 750) iterates

```rust
while ip < end { let op = &ops[ip]; ... }
```

where `end` is a block's recorded extent. **On a truncated op stream that extent can exceed
`ops.len()`, and the index panics**: `index out of bounds: the len is 19 but the index is 19`.

**Reproduction**, and it is not synthetic bytecode — it is a real corpus module with its op vector
cut short:

```text
03_enum_match.kel, chunk 0, ops truncated from 57 to 19
then keleusma::confine::module_confinement(&module)
```

**Why it reaches me.** `lower_module` calls `module_confinement`, and `lower_module` is a public
entry point that does not require a verified module. A verified module cannot present this — your
own structural verifier bounds the extents — but neither of us requires verification at that
boundary, and my own tests mutate bytecode and lower it.

**Why I am reporting rather than fixing.** `src/` is yours and read-only to this line. The shape of
the fix looks like one line, `ops.get(ip)` or clamping `end`, but **which of those is right depends
on whether an out-of-range extent should be a silent stop or a `Derailed`**, and that is your
analysis's contract rather than mine to choose.

**How I found it.** A sweep asking whether `lower_module` refuses malformed bytecode or panics on it
— `native_codegen/tests/lowering_robustness.rs`, 195 structural mutations over 8 corpus modules.
**58 panicked.** 57 were mine, in two classes, and are fixed: `pop` decrementing a `usize` below
zero, and an out-of-range local index into a `Vec`. This one is the remainder.

**It is allowed in my test by MESSAGE SHAPE, not by a count**, so a different panic cannot slip
through under its allowance — and the allowance itself asserts that it still fires, so **when you fix
it my test fails and the carve-out gets deleted** rather than quietly outliving the defect.

## A SMALL DIAGNOSTIC DEFECT, AND IT COST ME A WRONG CENSUS ENTRY

Writing a refinement predicate with the return type spelled `Bool` gives:

```text
type error: refinement predicate `in_range` must return Bool, returns Bool
```

**Both halves say `Bool` and the message is unactionable.** The hard-coded half names a spelling the
language does not use — the builtin is `bool` — while the second half displays my undeclared `Bool`
type, which renders identically. The check is `!matches!(sig_return, Type::Bool)`, so it is correct;
only the message cannot be acted on.

**It cost something real.** My first census recorded that subject as REFERENCE REJECTED, which would
have gone into the tree as a claim about your compiler's limits rather than about my typo. I caught
it by finding a working example in `examples/scripts/07_refinement.kel`, not by reading the message.

Naming the builtin as `bool` in the message, or distinguishing the two types when they render the
same, would close it. **Yours to weigh — it is cosmetic against the width defect you repaired this
session, and I am reporting it rather than ranking it.**

## ⚠ FOR YOU, NOT FOR ME: A MULTI-PARAMETER STREAM FAULTS ON THE REFERENCE AFTER ITS FIRST REWIND

**This is a question about the language, and I am reporting it rather than answering it.**

I refuse a resumable stream with more than one parameter, on the ground that `resume` writes slot 0
and nothing else. Investigating whether that refusal could be lifted, I drove the shape on YOUR
runtime, and the answer was not what either of us would have guessed from the refusal's wording.

Driving `loop main(a: Word, b: Word) -> Word { let r = yield a + b; yield r + b }` with
`a = 3, b = 10`:

| leg | value |
|---|---|
| `Yielded` | `13`, which is `a + b` |
| `Yielded` | `110`, which is `reply + b` — **slot 1 SURVIVES the suspension** |
| `Reset` | then `TypeError("Op::CheckedAdd expects Word, Byte, Float, or Fixed operands, got Int and Unit")` |

**The second parameter survives a suspension and does not survive the rewind.** `Op::Reset` clears
every local to `Unit`, `resume` writes only slot 0, and the next iteration's arithmetic on slot 1 is
a type error.

**So the shape is not "refused natively but working on the reference". The reference stops.** The
program compiles, passes the verifier, and faults on its second iteration.

**The three readings I can see, and I am not choosing between them:**

1. It is a defect — a stream's non-resume parameters ought to survive the rewind.
2. It is an intended consequence of `Reset` semantics, in which case the surprise is only that
   nothing says so.
3. It is a shape the verifier should reject outright, since a program that cannot reach its second
   iteration is not a productive stream.

**Only the third would need no runtime change**, and all three are yours. It affects my refusal only
in that lifting it would be lifting a refusal on a program that does not work anyway.

**Measured in `native_codegen/tests/probe_multi_param_stream.rs`**, which asserts the fault so that
this report stops being true the moment the behaviour changes.

**It also corrected me.** I had written in my own handoff that three standing refusals were "one
question, not three". That was an inference presented as a finding; this measurement separates one of
them out. I had also reasoned that clearing slot 1 natively would AGREE with your runtime — it would
not, because your runtime faults, and producing a value where the reference faults is the
silently-wrong-answer class I exist to refuse.

## A GUARD OF YOURS CAUGHT MY OWN OMISSION, AND I WANT TO SAY SO

The pre-push hook rejected this work because `comment_citations` found that **this very file cited a
test I had renamed**. Renaming a test silently invalidates every document that names it, and I would
not have found those by reading. Four further stale citations turned up in the sweep it prompted,
including one in `docs/decisions/YIELD_ESCAPE_REFUSAL.md`. All are corrected.

## TWO DEFECTS YOUR ORACLE CAUGHT THAT MY REASONING DID NOT

**`Op::Reset` dropped the resume value.** Your rewind is a SUSPENSION — it returns `VmState::Reset`,
the host resumes, and the resume writes slot 0 before the loop top runs. I collapsed that leg into one
native call and lost the write: `[7, 11, 0, 31]` against your `[7, 11, 20, 31]`. Fixed.

**A resume point can collide with a branch target**, where the resume edge has an empty operand stack
and the fall-through carries the branch's value. **Refused rather than reconciled** — that is a spill
question and inventing a layout is how a differential returns a wrong answer instead of a refusal.

Also refused deliberately: a stream with more than one parameter, since resume defines slot 0 only;
and any yield with operands stacked beneath the yielded value.

**Correction one: the arena IS the coroutine instance.** Same lowered code, different arena, different
stream — each with its own statically allocated frame. That is what forces the frame into the arena
rather than into globals or a machine frame, and it makes multiple instances free. I had not
articulated it, and it is the constraint the whole design hangs on. A future caller may hand the same
coroutine a different arena.

**Correction two: loops may reuse memory.** I claimed fixed offsets were not equivalent to a bump
arena for a site inside an inner loop. **Wrong** — I assumed a heap discipline. Under a stack
discipline with a per-iteration pop, every iteration reuses the same addresses, which is what fixed
offsets give. Reasoning is to be done in bump-arena terms; fixed offsets are an implementation detail.

**The model, verified against the code rather than sketched**: the arena is double-ended, persistent
`[0, X)` at the bottom, one ephemeral region ascending after it and one descending from the top.
`.bss` and the resume state go persistent; locals and composites go ephemeral and are cleared at
`Reset`. **`Op::Reset` should clear BOTH ephemeral regions** — the VM clears only the top, which is
vacuous for it since it never allocates from the bottom, but not for this backend, which does.

## A NEAR-MISS YOU SHOULD KNOW ABOUT

Extending `needs_region` to every stream changed the signature of the DEGENERATE chunks too, and
`yield_sequence.rs` calls those through a hand-written `extern "C" fn(i64) -> i64`. The extra
parameters became garbage from registers the callee never reads, and **ten tests passed on the calling
convention's good manners.** It would have stayed green until a stream chunk first dereferenced one.
**A signature change is invisible to a harness that names the signature itself.** Confined to
non-degenerate streams.

## AND ONE ATTEMPT I REVERTED RATHER THAN KEPT

Lowering `Stream`/`Reset` over the existing callback `Yield` makes a divergent `loop fn` spin inside
native code with no way for you to stop it. **Lowers but does not work is worse than an honest
refusal.** It did prove one thing: with `Stream` lowered, `13_telemetry_stream.kel` falls straight
through to the **yield-escape refusal** — so that shadow lifts exactly as predicted and the soundness
refusal beneath it is live and correct.

## Yours, unchanged

1. **`f16`** — blocked on your reference `f16` arithmetic.
2. **Publication**, held.
3. **`--features self-host` alone does not build** — `src/selfhost/mod.rs:337` names `ScalarKind::Float`
   ungated. Reported, not repaired; `src/` is yours.
4. **`wire.kel` faults** `IndexOutOfBounds(1570808, 65536)` under the differential, which gates
   mutation coverage for four opcodes. Localising it needs instrumentation in `src/vm.rs` — **the
   cheapest route, and one you can take and I cannot.**

## State

| | |
|---|---|
| backend suite | ⚠ **464 passed, 10 FAILED** at `2c645747` — all ratchets on the old frontier, to be INVERTED. Corpus differential and `--narrow` not yet run against this work. |
| uncommitted | none |
| unabsorbed | **2** — absorption 55 pending and unmeasured |

## For whoever runs this suite

`native_codegen/tools/backend-gate.sh` — fmt, clippy and both halves, each frozen-checked. The suite
**must** be split: together the halves exceed the harness's background ceiling. `--narrow` selects the
second float configuration.

---
---
# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 34 conflicted here.** Neither message is discarded.
**This is a merge resolution, not a relay** — nothing below was reviewed, re-derived, or endorsed by
the V0.3.X line, and its figures describe that line's tree.

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-08 (session 64) — a wrong answer found in the flat-composite core, repaired, and guarded in a build continuous integration already runs

## READ FIRST: THE RUNTIME RETURNED A WRONG ANSWER, AND IT IS REPAIRED

A composite bearing an **opaque** field was built and read at two different widths. The worst
outcome was not a fault:

```
P { h: h, n: 1 } == P { h: h, n: 2 }   evaluated to true
```

Two structures differing in a `Word` field compared **equal**. Nothing in that program is
unrepresentable at the width it ran at. **A silent wrong answer is worse than any refusal this tree
has catalogued**, and it is why this outranks everything else here.

**The mechanism was one field with two authorities.** `ScalarKind::Opaque` is sized by the ADDRESS
width, and the compiler bakes every field offset from that layout, the typed verifier sizes operands
from it, and the marshalling layer reports field sizes from it. The runtime disagreed in four places,
each assuming a WORD: the construction path rewrote the registry index to a one-word `Int`, the arena
packer advanced by that width, the flat scalar read read it back as a word, and the host decode asked
for a word's worth of bytes from a field the layout had already sized.

**The default target makes the two widths equal**, so all four agreed with the layout by coincidence.

**The repair did not have to choose a side, and deliberately did not.** Whether the field is a
registry INDEX (arguing for a word) or a host HANDLE (arguing for an address) is a real question. It
is left open. Three of the four subsystems already treat the layout as the authority, so each runtime
site now asks the layout for the width. If that question is ever settled differently, the runtime
follows without further change. **Do not promote it into a decision on my account.**

Repaired in #376, guarded in #378, recorded in
[`NARROW_WIDTH_FAILURE_CLASSIFICATION.md`](../decisions/NARROW_WIDTH_FAILURE_CLASSIFICATION.md).

## THE FINDING WITH THE LONGEST REACH: A WIDTH SKEW NEEDS NO FEATURE

I wrote in a merged document that the construction path **could not** be guarded without a
continuous-integration job in a configuration nothing builds. **That was wrong**, and finding out why
is worth more than the repair.

`GenericVm<W, A, F>` is generic over the word and address types independently, and every `Word`
(`i8`, `i16`, `i32`, `i64`) and every `Address` (`u8`, `u16`, `u32`, `u64`) is implemented
unconditionally. **A host-defined alias reaches any pair of widths in the default build.** The
`narrow-*` features only choose which pair the bundled `Vm` alias uses. A skewed pair also already
ships as a named target: `Target::embedded_8` is an eight-bit word with a sixteen-bit address, and
the `addr_bits_log2` field's own documentation names the 6502.

**So a width-dependent defect can be guarded at no standing per-push cost.** That is cheaper than the
job [`FEATURE_COMBINATION_SWEEP.md`](../decisions/FEATURE_COMBINATION_SWEEP.md) recommends and does
not adopt, and it was available the whole time.

**It does not extend to the feature axis.** A build omitting `floats` cannot be reached by an alias;
that is why the float hole below stayed unexercised. The sweep's central finding stands for features
and is qualified for widths.

## WHAT THE CLASSIFICATION SETTLED, AND WHAT IT DID NOT

Every failure of a finished `narrow-word-16` run now carries a verdict taken from the failing
assertion's own text rather than from the test's name. The previous grouping was by name and left
eighteen in an unexamined remainder; **its "opaque and flat-composite layout" row had guessed close
to the real mechanism and still filed it as a test premise.**

| | |
|---|---|
| distinct failures, before | 41 across 16 binaries |
| distinct failures, now | **33 across 11 binaries**, of 106 |
| repaired | 5 opaque-width, 3 hard-coded eight-byte harness reads |
| newly failing | **none** |

**The last figure was established by DIFFING the failing sets, not by comparing totals.** A total
falling by eight is equally consistent with fixing nine and breaking one.

The remaining 33 are seven groups whose tests declare a premise a sixteen-bit word voids: 14 + 6 + 8
+ 2 + 1 + 1 + 1. **The narrow widths are still not verified.** A suite that cannot run at a width
cannot vouch for it, and making it run there remains a project nobody has adopted.

## THE NEGATIVES, WHICH ARE PART OF THE RESULT

**No mutation is caught by the shape corpus alone.** I extended the skew guard to four composite
shapes, then shortened a nested child's stride by one byte specifically to demonstrate unique value.
**Seventeen tests across the suite caught it** — a broken nested stride is wrong at every width. The
corpus's case is that it drives the repaired sites through four construction paths, which is a
hypothesis about defects not yet found, not coverage of anything today.

**Two of the four repaired sites hang on a single test each**, and one test in that file is caught by
no mutation at all. All three facts are written beside the tests rather than left for a reader to
discover.

**My first attempt to measure the corpus used cargo's default fail-fast**, stopped after the first
failing binary, and showed only the corpus failing — which would have supported the opposite
conclusion. It was a lower bound, not a result. The tree already records this trap for the build
phase; it applies to the test phase too.

## TWO PROCESS FINDINGS FROM THE SESSION'S SECOND HALF

**A policy change reached one document and stopped.** `GIT_STRATEGY.md` has said since 2026-08-11
that continuous integration gates feature branches and the local gate does not. Five other documents
still said the opposite, including the one that directs an autonomous session. **That is why I spent
2h30m of the contended machine on a gate CI had already superseded in 48 minutes**, and then reported
the retired rule to you as policy. All are corrected; `DESIGN_JOURNAL.md` keeps the old wording
because it is an append-only record of what was believed then.

**The census's last unexamined group was fired by this session's own defect.** Group A is the
conversion every `?` over the flat scalar codec passes through. It fired twice in the pre-repair
`narrow-word-16` run, from ordinary programs, as `InvalidBytecode("flat scalar codec: OutOfBounds")`.
The repair closes that route; the class stays live, because the site's reach is every such `?` and the
census's instrument cannot enumerate them. **Its message says the bytecode is malformed when the
artefact was fine** — the same misattribution group F records for the hot-swap site, and equally not
repaired, because changing which variant a public API returns is a breaking change.

## THE FOUR DECISIONS ARE STILL YOURS, AND NONE MOVED

Stated in full at the top of [`HANDOFF.md`](./HANDOFF.md). Nothing in this session implements any of
them.

1. **How does a value enter a `Text<N>`?** It appears in every program anyone writes with the type.
2. **Is the width bundle worth a breaking change?** 33 signatures, 14 public, in a published crate.
3. **Should `verify()` refuse float opcodes when the feature is absent?** Evidence complete: about
   ten lines, prototyped, **zero new failures**, and the one semantic worry is moot. Unlanded only
   because I said it was your call in a merged document.
4. **Does any build configuration earn a continuous-integration job?** Note that finding 2 above
   makes this cheaper than it looked for the width axis, and unchanged for the feature axis.

## THE HOLE THAT IS STILL PINNED OPEN

A float-using module verifies, loads, and traps `InvalidBytecode` on a runtime built without the
`floats` feature — the class `verify()` exists to exclude. Pinned by
`tests/float_opcode_without_floats.rs`. **Unchanged this session**, and it is decision 3 above.

## THE INTENDED NEXT STEP

Nothing large without an answer to the four. The self-directed work with the clearest value left is
the remaining narrow-width groups, which are premise repairs and therefore a decision about whether
the suite should run at a narrow width at all — **that decision is not mine and I have not taken
it.**

**Both censuses moved since the paragraph above was first written.** The discard-arm census is
**CLOSED at 19 of 19**: its last two arms are a `_ => 0` fallback that cannot fire, because the only
slots it reads are declared `Word` and a `Word` slot yields an `Int` and nothing else. They are now
the assertion their sibling closure already was.

The `InvalidBytecode` census stands at **35 of 46**, and **should not be driven to 46**: its own
methodology says probing every member of a class is not a better use of the same effort, and the
eleven that remain are siblings inside classes that already carry verdicts, plus host-contract
surfaces.
