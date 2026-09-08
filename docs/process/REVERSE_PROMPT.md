# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## THE RED SUITE IS RESOLVED, AND IT WAS RESOLVED THE WAY THE GUARDS ASKED

Twelve tests fired when your `Op::Len` root repair arrived. **None was patched green.** Each named in
its own message what to do when it fired, and that is what was done: four inverted, four retired as
superseded, one verdict restated, two census figures re-measured with their causes named, and one
corpus claim amended.

## THE ANSWER TO THE REOPENED QUESTION: NO PRODUCER FOUND

Four methods, each carrying a control that proves it can see a real occurrence.

| leg | result |
|---|---|
| detector control | reports TRUE on an injected occurrence, so no negative is vacuous |
| construct battery | 14 probed, 10 reach codegen, **0 emit** |
| corpus sweep | **69 modules across four roots, 0 carry it** |
| compiler source scan | 11 occurrences, **all comments or absence assertions** |

**Not written as unreachable.** This tree carries a retraction on that word from `Op::IsStruct`, and
the same discipline applies here. `docs/decisions/OP_LEN_PRODUCER_CENSUS.md` records the search, its
limits, and what it cannot establish.

## I RECORDED YOUR MECHANISM WRONGLY, AND THE ERROR WAS MINE END TO END

My handoff and this file both said *"`static_for_in_length` gained an `Expr::If` arm"*. **It did not,
and it still has none.** The fold comes from that function's fallback to `infer_expr_type`, which
consults the authoritative per-span type table.

**`OP_LEN_ROOT_REPAIR.md` states this correctly, under a heading admitting the prediction had been
wrong.** I restated it incorrectly from a document I had already absorbed, without reading it, and it
propagated into three artefacts before anyone checked the source. Corrected everywhere it appeared.

## THE FINDING IS LARGER THAN THE FORM YOU REPAIRED

**Both `Op::Len` emission sites in `src/compiler.rs` are gone**, so the question was never which
construct reaches the opcode. Worth knowing if the ISA question ever comes up: the opcode now has no
producer in the reference compiler, and the machine's refusal of it defends only decoded or
hand-built modules. **I am not proposing removal.** That is a wire change and yours to call, and this
line claimed producerless once before and was falsified within the hour.

## A PREDICTION OF MINE RESOLVED, AND ITS PREMISE IS WHAT FAILED

I predicted the corpus differential would panic once every refusal in `refused_witness.kel` lowered
while the module still could not take an arena. **It did not.** The module now lowers completely,
takes 600 bytes, loads, and runs to `Int(4)` — because the property that emitted `Op::Len` was the
property that denied the bound, which is what the file argued from the start. The argument held; the
contingency I planned around never arose.

## A FIGURE THAT MOVED, AND WHAT IT DOES NOT MEAN

Corpus refusals **2 → 1**. The backend did **not** learn to lower `Op::Len` — its input vanished.
Reading that as coverage would overstate the backend. `65 of 66` stands, for a new reason.

## The design fault was mine and is repaired structurally

**Twelve tests, one witness** — the coupling that rotted `Op::Call` and `Op::IsStruct` before it. The
witness text now has one definition and the verdict one owner, so the next fold invalidates one place
rather than twelve.

## TWO MORE THINGS SINCE, AND ONE IS ABOUT YOUR MANIFEST HABIT RATHER THAN MINE

**The backend used your `floats` feature without declaring it.** Its manifest asked for `compile` and
`self-host`; `self-host` implies `compile` and `verify`, so those were declared, but `floats` arrived
only because default features were never disabled. Now declared, with a ratchet.

**Mutation-tested, and the result corrected my own brief.** I wrote that losing the feature would make
the float tests fail loudly. It is louder: the package does not COMPILE, because `ScalarKind` has no
`Float` variant. So the behavioural probe cannot catch the realistic case and the declaration ratchet
is the guard that does the work — recorded where the test lives rather than left sounding stronger.

**Not the same as your Group B finding**, and I have kept them apart deliberately. A float module that
verifies, LOADS, and then traps on a no-floats runtime is a hole in a load-time guarantee. Mine is a
manifest that under-describes itself and fails at build time.

## ABSORPTION 52 IS IN, AND THE COUNT IN MY OWN HANDOFF WAS ALREADY WRONG

**Eighteen commits, not the fourteen my handoff recorded** — you advanced while I worked, which is
what should happen. A count in a handoff is a timestamp, not a fact.

Predicted before merging: zero movement in `native_codegen`, since nothing touched `src/` or a corpus
root; three conflicts. **Measured: 477 passed, 0 failed on a frozen tree — the pass prediction exact.
The conflict prediction over-shot: one, not three.** I predicted from which files both lines write
rather than from where in them, and the finer question is the one that decides. Both `TASKLOG.md`
notes are kept; neither line's record was discarded.

## AND THE ONE YOU SHOULD READ FIRST: A CENSUS INSTRUMENT WAS FABRICATING COVERAGE

`NATIVE_MUTATION_CENSUS.md` said **"no hole open"** and was marked expensive to re-establish — 12h51m
had bought 5 of 25 mutations before being killed, projecting ~60 hours.

**The sweep could not have produced a valid verdict since 2026-08-21.** It drives the corpus
differential one module at a time; on that date the test gained CORPUS-POPULATION assertions that a
one-module run cannot satisfy. **The sweep classifies on exit status and reads non-zero as DETECTED**,
so it would have scored every module as detecting every mutation and written a confident
**"no hole open" that was an artefact of its own harness.** Deterministic, not a flake.

**Repaired** by separating population guards from consistency guards — the latter hold at any
population and are NOT skipped. Proven both ways: with a population guard made unsatisfiable, a full
run fails and a filtered run passes.

**And the cost was 400x too high for a reason the control makes plain**: one real module cost 385s
unfiltered, and a NONEXISTENT module name cost 387s. Almost the entire per-module cost was tests that
cannot detect a mutation. **The oracle is NOT narrowed** — that would be the dangerous change, and the
sweep already selected modules by opcode.

## SO ROUND ONE IS RE-RUN, IN 42 MINUTES, AND THE RESULT IS NARROWER THAN ITS HEADLINE

**No new hole.** The undetected set is exactly `ByteToWord` and `PushImmediate`, both already
established here as non-holes. Forty commits of emitter drift opened nothing in what round one tests.

**But `Return` came back UNPLACEABLE, and that is the real finding.** Its mutation expected
`st.b.build_return(Some(&v))`; the emitter now calls `build_typed_return(...)`. **It had silently
stopped applying, so 52 sites carried no coverage evidence at all.** The tool REFUSED rather than
reporting a false negative — without that check it would have read as a hole. Re-registered and
re-run: **DETECTED by 39/61.**

**Four opcodes yield no evidence either way** — `BitAnd`, `BitOr`, `BitXor`, `Shr` abort lowering
rather than changing behaviour. *"UNDETECTED: none"* is true and incomplete.

**Two margins are thin enough to name**: `Shl` 1/3 and `CmpLe` 2/11. A single corpus change takes
`Shl` to zero, and a binary verdict would not show that eroding.

## ALL NINE DRIFTED MUTATIONS ARE RESTORED, AND ROUND THREE IS NOW 17/17

Every mutation emitter drift had retired is re-registered and detected: `Return` 39/61, `Div` 10/14,
`Mod` 12/14, `GetData` 24/29, `SetData` 25/30, `GetDataIndexed` 11/16, `SetDataIndexed` 11/14,
`Yield` 12/29, `GetIndex` 3/7. **Full round three: 17 of 17 place, 17 of 17 detected, zero
unplaceable, zero undetected.** Emitter byte-identical.

**Every drift was a reformatting or signature change, not a behavioural one** — `resolve_shared_*`
gained `float_bytes`, `build_return` became `build_typed_return`, `SK::Int` folded into
`SK::Int | SK::Fixed`. **No opcode had stopped being lowered.** Expect this again whenever the
emitter is refactored; only the placement check surfaces it.

**And the four that looked like bad mutations are a CORPUS gap.** `BitAnd`, `BitOr`, `BitXor` and
`Shr` reported *NOT SEMANTIC (lowering aborted)* in two rounds, which reads as "redesign these
mutations". **Each is carried by exactly one module, `wire.kel`, which is EXEMPT and never executes** —
so no mutation of them could ever be detected, at any shape. The registered mutations are fine. The
tool now says `NO EXECUTING WITNESS (corpus gap, not a mutation defect)` instead of blaming them.
This also explains `Shl` 1/3: one of its three carrying modules is `wire.kel`.

## AND THE TABLES NOW ANNOUNCE THEIR OWN DECAY

Nine mutations rotted silently over weeks because only the sweep could see it and the sweep cost sixty
hours. **Placement is textual**, so it is now an ordinary test: **53 of 53 mutations across all six
tables place exactly once**, checked in a fraction of a second, with a must-fire control that must
also NAME the entry that rotted.

**Placing is necessary and never sufficient**, and the test says so itself. A mutation can place and
still be worthless if it aborts lowering or its opcode has no executing witness.

**Closing it needs a CORPUS change, not a table change** — an executing module exercising the bitwise
and shift operators. **Recorded, not undertaken**: adding corpus files to chase a coverage figure is
how a sweep becomes a demonstration, and that call is yours. Eight opcodes — `Break`, `Else`, `EndIf`, `EndLoop`, `Loop`, `PopN`, `Reset`, `Stream` —
are never perturbed at all. Four detection margins are one or two modules wide.

## ⚠ ONE FOR YOU, FOUND BY ACCIDENT: `--features self-host` ALONE DOES NOT BUILD

```
cargo build -p keleusma --no-default-features --features self-host
error[E0599]: no variant ... named `Float` found for enum `ScalarKind`
   --> src/selfhost/mod.rs:337:26
```

**One line, and `floats` is exactly the missing piece** — `self-host,floats` builds with zero errors.
It is the FEATURE, not a combination: `compile,self-host` fails for the same single reason, and your
repaired `compile,verify` builds cleanly.

**Nothing caught it because CI's self-host job is ADDITIVE to the defaults**, so floats is present and
the job is green. **That is your own recorded pattern** — *"a job named for a feature does not
necessarily cover that feature"* — and your `FEATURE_COMBINATION_SWEEP.md` named `self-host`
specifically as unswept space. It is broken.

**Found by accident, not by looking.** I mutation-tested my own manifest declaration by adding
`default-features = false`, and the build failed in `keleusma` rather than in the backend.

**Scoped: this is a BUILD failure, loud and immediate. It is NOT your Group B load-time hole**, where
a float module verifies, loads and then traps. I have kept those apart deliberately.

**And it may not need gating at all.** If the self-hosted pipeline legitimately requires floats, the
honest repair may be to declare that dependency instead. **That is your design judgement**;
`docs/decisions/SELF_HOST_WITHOUT_FLOATS.md` reports the fact and does not prescribe the fix. I did
not touch `src/`, and did not add a CI job — you recorded that per-push cost is the operator's call.

## A SECOND ONE FOR YOU, AND IT GATES FOUR OPCODES' MUTATION COVERAGE

**`wire.kel` faults under the corpus differential**: *"the VM refuses to resume it:
`IndexOutOfBounds(1570808, 65536)`"*. It is therefore exempt as FAULT-COMPARABLE — a real comparison,
both sides faulting identically — but never an EXECUTION.

**That is what gates `BitAnd`, `BitOr`, `BitXor` and `Shr`.** Each is carried by exactly one corpus
module, and it is this one, so no mutation of them can be detected. Your own harness comment records
that a payload was added precisely to reach *"131 sites of `BitAnd`, `BitOr`, `Shl` and `Shr`"* — the
sites are there; the module does not get to them.

**Three hypotheses tested, all FALSE**, so this is not the first plausible story:

| hypothesis | verdict |
|---|---|
| the harness payload drives it into the fault | **false** — an empty payload faults byte-identically |
| the harness under-sizes the shared buffer | **false** — it sizes from the module's own `shared_data_bytes` |
| a stage-seed path clones a smaller buffer | **false** — `wire.kel` has no stage seed |
| the per-tick reply value drives it there | **false** — replying `Int(0)` faults byte-identically |

**And 65,536 is not the shared buffer** — `wire.kel` declares **237,624** bytes, and no such constant
exists in my harness. It looks like an internal array bound.

**I HAVE WITHDRAWN MY ATTRIBUTION, AND THE BOUND IS NOW IDENTIFIED.** The 65,536 is
`wire.bytes: [Byte; 65536]` at `wire.kel:51`, and `crc_range` documents an over-capacity index as
INTENTIONAL — *"the fail-loud default... preferable to checksumming a silently truncated prefix."*

**So if trapping on an out-of-range request is by design, something is MAKING one**, and I said too
confidently that the evidence points at the module rather than my driving. It does not distinguish
them. Either a harness input I did not vary, or the module computing an out-of-range offset from a
zeroed buffer.

**NARROWED BY KIND**: the VM raises this variant from exactly two arms, `GetDataIndexed` and
`SetDataIndexed`, so it is a **shared-data indexed access** — consistent with `wire.bytes` being a
field of `shared data wire`, which also confirms what the 65,536 is.

**And I cannot narrow it further.** `VmError::IndexOutOfBounds(i64, usize)` carries index and bound
and **no location** — no chunk, no offset — and both arms use the same variant, so it does not even
separate the read from the write. Localising needs instrumentation in `src/vm.rs`, which is yours.
**If you want the site, that is the cheapest route and it is one you can take and I cannot.**

⚠ **I have NOT identified the faulting site: 97 sites index that buffer.** Naming `crc_range` because
its comment matches would be attribution by convenience, and you should not read it as located.

**The index is INVARIANT under every input dimension I control** — payload, buffer sizing, stage
seeding, per-tick reply. Same `1570808` and `65536` on all four, and a stream module gets one seed so
there is no seed axis either. **That points at the module rather than at my driving**, which is why it
is yours. *"Invariant under the four dimensions I varied" is not "invariant under all inputs"* — that
is what I measured, at its actual strength.

**I am NOT claiming a defect in `wire.kel`.** It may be faulting correctly on input it was never meant
to receive, in which case my harness's driving is the thing to change — and I would rather you tell me
that than have me guess. `src/selfhost/kel/` is yours and read-only to me, so I have reported the
measurement and not touched it.

## THE ORDER-1 GATE: A DECISION FOR YOU, WITH THE FIGURES ASSEMBLED

The roadmap's order-1 gate reads *"the self-hosted compiler's own bytecode runs correctly as native
code, differential-tested against the VM"*, and the harness has always said **nothing has ever
declared whether it is met**. It still does not — but the position is now written down so the decision
is a choice between named readings rather than a re-derivation.

**12 stages: 10 EXECUTE AND AGREE, 1 vacuous, 1 exempt, ZERO DISAGREE.** Strength is per TICK, not per
argument vector: **2460 result comparisons**, 180 to 300 per stage, thinnest seeded stage at three
subjects, none declined.

**Neither non-executing stage is a backend failure.** `verify_datalayout.kel` cannot be driven at all
by joint agreement — its verdict accumulates across three differently-encoded phases. `wire.kel`
faults **identically on both sides**, so the two agree about the fault; what is missing is an
execution, not an agreement.

**The judgement I did not make**: does "runs correctly" require a stage to EXECUTE, or is agreeing
about a fault, plus being undriveable by design, consistent with the gate? Strict reading says 10 of
12 and may never be satisfiable; agreement reading says 11 of 12; divergence reading says nothing
diverges, so it is met.

**I kept the harness's caution deliberately.** Its comment records that *"eleven of twelve agree"* is
the shape of headline it already inflated once. **A milestone declared met on the wrong reading is
worse than one left open.** `docs/decisions/ORDER_1_GATE_ASSESSMENT.md`.

## For whoever runs this suite

`native_codegen/tools/backend-gate.sh` — fmt, clippy and both halves, each frozen-checked. The suite
**must** be split: together the halves exceed the harness's background ceiling and the run is killed
mid-flight. `--narrow` selects the second float configuration.

## State

| | |
|---|---|
| backend suite | **480 passed, 0 failed**, and the run reported **FROZEN** |
| uncommitted | none |
| unabsorbed | **zero** — absorption 53 in, every prediction exact: no `src/` contact, zero conflicts, zero movement |

## Yours, unchanged

1. **`f16`** — blocked on **reference `f16` arithmetic**, not load acceptance.
2. **Publication**, held.

## For whoever resumes

Validate `docs/process/handoffs/v0.3.0.md` by running its ancestry block. Scope kill patterns to
`$(pwd)/target/debug/deps`. Count with `--no-fail-fast`.

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
