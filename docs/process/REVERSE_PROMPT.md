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

**Date**: 2026-09-05 (session 63) — two load-time holes found, one repaired, and three of my own instruments corrected

## READ FIRST: A MODULE CAN VERIFY, LOAD, AND THEN TRAP, AND IT IS NOT REPAIRED

**A module using floats verifies, loads, and traps `InvalidBytecode` on a runtime built without the
`floats` feature.** Measured with `--no-default-features --features verify`:

| step | result |
|---|---|
| `Module::from_bytes` | accepted |
| `verify()` | **accepted** |
| `Vm::new` | **loaded** |
| call | **`InvalidBytecode`** |

`InvalidBytecode` asserts the artefact should never have been produced. It is the class `verify()`
exists to exclude, so this is a hole in the load-time guarantee rather than a bad program.

**Two independent reasons nothing catches it earlier**, either of which would suffice. `verify.rs`
has **no `floats` gating at all** — not one conditional mentions the feature. And the header check
cannot reject it: loading admits when `got <= max_supported`, and `RUNTIME_FLOAT_BITS_LOG2` is not
gated on the feature either, so a build without floats still advertises the full width.

**Nothing here is corrupt.** The fixture is ordinary reference-compiler output, and omitting floats
is the POINT of the feature — an embedded target is exactly where it is used, and producing bytecode
on one build to run on another is the normal shape for a language that ships precompiled modules.

**Proportionality.** The trap is loud: a clean error at call time, not a wrong answer, a crash, or
memory unsafety. What is wrong is the LAYER.

**Why I pinned it and did not repair it — and what has changed since.** The repair belongs in
`verify()`, is about ten lines, and I prototyped it to validate the pin. My stated objection was that
continuous integration builds no configuration in which the pin compiles, so it would be exercised
only by the release gate. **Precisely** — the loose form of this claim was wrong: CI does run
`--no-default-features`, but bare, so `verify` is absent and the pin is configured out; every other
job adds to the default features and therefore has floats.

**I have since supplied that verification rather than handing you the objection.** The full
`--no-default-features --features compile,verify` suite was run with the repair and compared against
the same suite without it: **the repair introduces zero new failures.** The only test whose result
changes is the pin itself, which is built to fire when the hole closes.

**And the one semantic worry is moot for locally-compiled code.** The repair refuses a module
CONTAINING a float opcode rather than one that executes it, so a module with unreachable float code
would be refused. Measured: **the LEXER refuses a float literal without the feature**, so no float
program can be compiled on such a build at all. Only imported bytecode is affected — exactly the case
where refusing at load is unambiguously right.

**It is still not landed, and that is deliberate.** I told you in a merged document that this was
your call, and reversing that within the hour would make the record untrustworthy; a deferral is
worth something only if it is honoured. What is left is a one-line semantic judgement, with the
engineering risk removed.

**Getting that evidence required repairing the configuration itself, and that is a finding.**
`--no-default-features --features compile,verify` **did not compile**, and two more tests failed once
it did — five defects, all float-dependent code with no `floats` gate. None was tolerated; every one
was invisible, because the release gate's no-default step does not add `compile,verify` and CI never
omits floats, so **nothing built this combination.** The configuration in which the hole lives was
the configuration nothing exercised. Now green at 105 binaries and 1863 tests. Same family as the
verify-without-floats failure V0.2.2 repaired, which suggests a feature-combination sweep would be
worth more than any single fix in it.

Pinned by `tests/float_opcode_without_floats.rs`; recorded in
[`INVALID_BYTECODE_CENSUS.md`](../decisions/INVALID_BYTECODE_CENSUS.md).

## A SECOND THING FOR YOU: NINE OF ELEVEN BUILD CONFIGURATIONS ARE BUILT BY NOTHING

The float hole survived in a configuration nothing built, so I swept eleven configurations that a
host might plausibly ship. **Three are covered; the other eight are verified by nothing**, and one
of them did not compile.

| covered by CI and the release gate | built by nothing |
|---|---|
| default; bare no-default; the broad docs.rs surface | `verify` alone; `compile` alone; `compile,verify`; `verify,floats`; `signatures` alone; `compile,verify,signatures`; `encryption` alone; `compile,verify,encryption` |

**A job named for a feature does not necessarily cover that feature.** Continuous integration's
`--features signatures` job is ADDITIVE to the default features, so signatures-alone — the shape its
name suggests — is unbuilt. That is a standing property of the matrix rather than an incident.

**`--features compile` did not build.** A compiler without a verifier is presented in the feature
documentation as an independent choice; three test files imported `keleusma::verify` while gated on
`compile` alone. Repaired. No source file was involved.

**A recommendation, deliberately not adopted, because the cost is yours to weigh.** Adding
`--no-default-features --features compile,verify` to continuous integration costs one job on every
push and covers the configuration already shown to hide a defect of consequence. A **build-only**
`cargo check --tests` matrix over all eleven would have caught every compile defect found tonight at
a fraction of a test job's cost — but would NOT have caught the two lex-time test failures, which
need the tests to run.

**What the sweep does not say.** It ran `cargo check --tests`, so an "ok" verdict means a
configuration COMPILES and nothing more. It is not a claim that eleven configurations are supported,
and the unswept space — the narrow word and address selectors, `self-host` — is far larger than the
swept one. Recorded in [`FEATURE_COMBINATION_SWEEP.md`](../decisions/FEATURE_COMBINATION_SWEEP.md).

## WHAT ELSE THE SWEEPS TURNED UP, IN ONE PLACE

**The narrow widths compile and nothing verifies them.** All ten selectors build; no continuous-
integration job or release-gate step selects any of them. Running the suite at `narrow-word-16`
gives **89 binaries passing, 15 failing, 37 distinct failures** — but the failures share a PREMISE,
not a cause in the virtual machine. The clearest case: the performance canary's own program uses
`1234567`, which cannot be represented at 16 bits. **The suite assumes a 64-bit host**, so running
it at 16 bits mostly measures that assumption.

That licenses neither conclusion on its own. It does not show the narrow widths are broken, and it
does not show they work. They are **unverified, and now measurably so rather than presumptively**.
Making the suite run at 16 bits is a project rather than an increment, and I am not recommending it
be started.

**The performance canary could not fail.** Its ceiling was asserted after the timed call returned,
so it could only fire once the thing it guarded had finished — it spun 57 minutes at 99% of a core
instead of failing. Now bounded on a channel, mutation-tested in three directions, with the existing
ceiling and result assertions shown to still fire.

**Census: every operand INSIDE a chunk is checked against tables inside the module; four things are not.** A reserved
`PushImmediate` and an unrecognised `Trap` kind code are each admitted, load, and trap. Eight
indices beside them are rejected at load with precise messages.

**One unchecked operand is an oversight; two against eight checked indices is a boundary in what the
pass was built to cover.** Which it is remains yours to say — the observation is recorded, not the
intent.

**Defence in depth, not a guarantee hole** — reaching either needs a corrupt artefact, and both
outcomes are safe because the runtime refuses regardless. Kept deliberately distinct from the float
finding above, which is a module the compiler itself produced.

The census is at **34 of 46 sites examined**; the twelve that are not are named group by group in the
document. **FOUR admissions in total**, not two: the reserved immediate and the trap kind above, plus
the module-level `entry_point` past the chunk count and a native index past its table — each admitted
at load, loading, and trapping at the call. The entry point is plainly checkable, since the chunk
count sits in the same structure; whether the native index is checkable at load is **not** established,
because natives are registered by the host after loading.

**This paragraph has been wrong three times, always the same way.** It said "one operand RANGE" and
22 of 46, then two and 30 of 46, then thirty-one — each figure produced by adjusting the previous
number rather than re-summing the per-group column. Re-summing is what finally exposed that **group G
was missing from every list of what remained**: three arena-staleness sites nobody had counted. One
of them is now probed and found to have **no witness** -- a transient composite cannot be NAMED after
the reset that ends its iteration, so the language's shape prevents it rather than a check catching
it.

**Read every count here as "message classes examined", not lines of source visited.** Probes map to
sites by message shape, and sibling sites emitting the same message are credited together. That is a
weaker claim than the bare number suggests. The earlier text said 22 of 46 until finishing the
probing found the second instance —
corrected here rather than left standing, because a stale figure in this file is the defect this
session spent itself on.

## THE THING I WOULD MOST WANT A REVIEWER TO CHECK

**Three of my own instruments were wrong tonight, and all three failed in the same direction.**

| instrument | how it lied |
|---|---|
| the feature sweep's file column | listed WARNING locations under a PASSING verdict |
| a mutation test of the boundary canonicalization | aimed at the wrong call site and PASSED |
| the operand-range probe | mutated an op the program did not contain, then reported "admitted" |

Each reported a clean or confident result about something it never touched. The third survived only
because the follow-through ran the module and it returned the correct answer; a verdict-only probe
would have shipped a fabricated finding.

**None was caught by review. All three were caught by making the instrument fail on purpose.** If
any conclusion in this session's documents deserves suspicion, it is one whose instrument was never
made to misbehave.

## THE TWO QUESTIONS THAT BLOCK EVERYTHING LARGE ARE STILL YOURS

Neither moved this session, and neither is mine to decide. They are stated in full at the top of
[`HANDOFF.md`](./HANDOFF.md).

1. **How does a value enter a `Text<N>`?** A literal is static text, `Text<8>` is dynamic text, and
   the two deliberately do not unify. `GRAMMAR.md` states that no implicit coercion exists, so
   emission needs a surface form — a cast, a constructor, or a method. It appears in every program
   anyone writes with the type.
2. **Is the width bundle worth a breaking change?** `addr_bytes` is taken by 33 signatures across
   five files, 14 of them public, in a crate published at 0.2.2.

## WHAT LANDED: THE COMPILER NO LONGER EMITS AN OPCODE THE MACHINE REFUSES

Both emission sites for `Op::Len` are gone. Each folds the length from the operand's type, or fails
with a compile error naming the unfoldable length. The virtual machine keeps its refusals, which now
defend against a corrupt or hand-built module rather than against the compiler.

**Every iterable form that can carry an array type folds and runs** — eleven tried, all pass, each
asserted on its ITERATION COUNT rather than on compilation, so a wrong bound fails rather than
passing quietly. A wrong bound would have been worse than the trap: a trap is loud and a wrong
iteration count is silent, and the worst-case execution time analysis consumes it.

**Four mutations, four caught**, each by the guard that should catch it: removing the multi-word
length arm, removing the delegation fallback, folding a bound one short, and reinstating an emission.

## THE FINDING I DID NOT GO LOOKING FOR, AND IT IS THE MORE SERIOUS ONE

The recorded hazard was latent: held shut by a loop-bound refusal that the taxonomy calls liftable,
so it needed someone else's future improvement to open.

**The second site needed nothing.** The checked-index construct over a `Multiword` folded its length
through a helper that answers only for array types, fell back to the opcode, and a multi-word body
is flat. Measured against the pre-change baseline by stashing: the program **compiled, passed
`verify()`, took a memory bound, loaded, and trapped `InvalidBytecode`** — the class `verify()`
exists to exclude, reachable on the day it was measured.

It is repaired by folding the multi-word width, so the construct now works rather than being
refused. **Found by enumerating every emission of the opcode**, not by following the witness already
in hand.

## AN ISA OBSERVATION FOR YOU, DELIBERATELY NOT ACTED ON

**No producer for `Op::Len` was found in the reference compiler**: zero emissions in `src/`, and none
in the self-hosted `codegen.kel`. On a project whose opcode count is a design constraint that reads
like a removal candidate.

**I am recording it, not proposing it.** Removing an opcode is a wire change and your call. And the
tree records `Op::IsStruct` being declared producerless and having four producers found by another
line within the hour, so the claim here is "no producer FOUND", never "unreachable". The machine
keeps both refusals regardless.

## THE CONCERN I RAISED AND THEN MEASURED, WHICH SHRANK IT

I flagged the native-array length as a possible soundness gap and then measured it rather than
leaving it as a worry. **It is not a soundness gap**, and saying so is as much the job as raising it
was. `tests/native_array_length_contract.rs` pins all four rows.

| the native declares | it returns | outcome |
|---|---|---|
| `[Word; 3]` | 3 | iterates 3 times |
| `[Word; 3]` | 5 | iterates 3 times; **the excess is silently dropped** |
| `[Word; 3]` | 1 | **traps `IndexOutOfBounds`**, loudly |
| no signature | anything | **refused at type checking** |

**The iteration bound is not wrong in any row.** The loop runs exactly the declared count or it
traps. What would be unsound is running MORE times than the analysis predicted, and no row does
that. **The memory bound does not come from this type either**: a native's worst-case memory is
host-attested per native, not derived from its declared return type, so an over-allocating native
has broken its own attestation rather than found a compiler defect.

What is left is narrow and real: **an over-long return is silently truncated with no diagnostic.**
Recorded rather than repaired, because validating a native's return against a declared shape at the
call boundary is a design change and your call, not a fix.

**And the fourth row is why the floor cost no capability.** The only native whose array length is
unknown is one the type checker will not admit in iterable position at all. Every row was also
measured against the pre-change compiler by restoring `src/compiler.rs` from the branch point; all
four are identical, so the `Op::Len` removal changed nothing here.

## THE ONE CONCERN THAT REMAINS OPEN

**The floor has no witness.** No source form reaches the compile error behind it, so it is pinned by
a source scan for the emission form. Its reach is stated in the test: one written shape, in one file.
It would not see an emission written through a differently named binding or built by pushing to the
op vector directly. Recorded as *not found*, never as *unreachable*.

## WHAT THE PROCESS COST THIS TIME

**I corrected every stale claim in `docs/` and `src/` and did not grep `tests/`.** The corpus run
found two tests asserting the old behaviour. The suite caught what my scan did not — the fourth
instance this week of a scan scoped to where its author was looking, and the first where the thing
missed was a test rather than a source file.

Both tests carried their own instruction for this moment and were right on both counts. Neither was
deleted.

## THE CLASS THOSE TWO HOLES BELONG TO IS NOW ENUMERATED

Both were found the same way and neither was predicted, so I enumerated the class rather than
waiting for a third accident. `docs/decisions/INVALID_BYTECODE_CENSUS.md` covers all **46**
construction sites: **17 carry an examined verdict, 29 are explicitly marked not examined.** No site
is claimed unreachable — this tree already carries one retraction on exactly that distinction.

Two defences are worth your attention because neither is visible where it matters:

- **The `Fixed` group is held by two checks that only work together.** `verify()` compares fraction
  bits against the MODULE's declared word width; loading rejects a module whose declared width
  exceeds the RUNTIME's. Loosening the load-time comparison reopens five sites at once, and nothing
  said so anywhere.
- **The composite-form group is held by a canonicalization at the host boundary**, sitting between a
  compiler that bakes a flat access and a marshalling layer that produces a boxed body. Each side
  looks locally correct. If it regresses, seven refusals open at once on legitimate programs. Now
  pinned, after a mutation test in which **my first mutation was aimed at the wrong call site and
  passed**, which would have put a wrong mechanism into the record.

## THE INTENDED NEXT STEP

Nothing large without your answer to the two questions above. **If you want one thing decided, make
it the float repair**: it is small, prototyped, and the only open hole in the load-time guarantee I
know of.

Absent that, the remaining self-directed work is census groups E and I — the structural-index and
operand-range sites, where `verify()` plausibly has a corresponding check and plausibly does not. A
first reading suggests both are mostly corrupt-artefact territory rather than holes reachable from
compiler output, which would make them a defence-in-depth question rather than a guarantee question;
that reading is NOT yet confirmed by a program. After that, the discard-arm census pass six, which
[`DISCARD_ARM_REACHABILITY_BRIEF.md`](../decisions/DISCARD_ARM_REACHABILITY_BRIEF.md) establishes is
a **fixture** problem rather than a harness problem: the pass-five harness is reusable as-is, and
what is missing is source constructs.
