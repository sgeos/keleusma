# What can the corpus differential actually detect?

**Status**: measured, the harness repaired twice, and **no hole open**. `Trap`
was closed in Part D by changing the OBSERVABLE, not the inputs. The only
undetected mutation left is `PushImmediate`, established in round two as vacuous
rather than a hole.
**Date**: 2026-08-14, extended 2026-08-15 (Parts B, C and D).

## The question, and why the obvious instrument was wrong

`probe_stage_vacuity` found nine self-hosted stages agreeing while producing
nothing. That search looked only at the stages. The follow-up question is whether
the same thin agreement sits in the rest of the corpus, and the first instrument
reached for was a count of trivial observables: one repeated result, no host
calls, an untouched shared segment.

That instrument said **32 of 40 modules are trivial in all three**. It is wrong,
and `10_multbyte.kel` is the standing refutation. That module scores three of
three — one result, no calls, no segment — and **that single integer is what
caught the composite-return aliasing defect**, `vm 7` against `native 8`. One
result is not the same as no information.

The refined instrument asks instead whether the output **responds to its input**,
which moved the count to 20 of 40 and correctly separated the rogue scripts from
the numbered examples. But the ten numbered examples take **zero arguments**, so
even that cannot classify them, and they are reported as unknown rather than
guessed at.

## The instrument that answers the question directly

The real question is not how much a module emits. It is **whether a defect in the
emitter would change what it emits**. That is measured by mutating the emitter and
seeing which modules notice.

| mutation | sites / modules | outcome |
|---|---|---|
| `CheckedAdd` computes a subtraction | 1835 / 43 | **SIGBUS.** Detected, fatally |
| `CmpGe` lowered as `SGT` (boundary) | 499 / 22 | **SIGTRAP.** Detected, fatally |
| `CmpLt` lowered as `SGT` (inverted) | 126 / 25 | detected by **2 of 25** modules |
| `CmpLt` lowered as `SLE` (boundary) | 126 / 25 | **NOT DETECTED. The whole differential passed.** |

The opcode occurrence counts come from `probe_agreement_depth.rs`, and they matter:
a mutation to an opcode with no sites proves nothing, and checking that first is
what separates a coverage finding from a vacuous experiment.

## The hole, stated precisely

`SLT` and `SLE` differ **only when the two operands are equal**. The harness drove
every module with a single argument vector of pairwise-distinct ascending values,
so no comparison ever reached its boundary. A whole opcode could be lowered with an
off-by-one and 34 modules would agree.

**This is a coverage hole about INPUT DIVERSITY, not about the modules being
vacuous.** That distinction matters, because the fix for a vacuous module is a
different input and the fix for a thin corpus is more inputs. The first instrument
would have pointed at the wrong repair.

That two of the four mutations were caught *fatally* rather than as a reported
disagreement is also worth recording: a signal kills the whole harness and yields
no per-module census, which is why the table above has two rows with no counts.

## The repair

`corpus_differential` now drives every non-stream module with **four** argument
vectors rather than one, and compares every seed:

| seed | vector | what it reaches |
|---|---|---|
| 0 | ascending, pairwise distinct | the original, so reported figures are unchanged |
| 1 | every argument equal | comparisons between equal arguments |
| 2 | all zeros | the identity and boundary cases |
| 3 | descending | an ordering assumption that holds under seed 0 |

A stream keeps seed 0 alone: its single parameter is the tick, which the driver
already varies across sixty iterations, so seeding it would change what the run
means rather than broaden it.

A disagreement now reports **which seed** found it, because "module X disagrees" is
much less useful than "module X disagrees on the all-equal vector".

## Verified, and this is the load-bearing evidence

With `Op::CmpLt` lowered as `SLE`:

- the **old** harness reported 34 executed and agreeing, and **passed**;
- the **new** harness **fails**, on `rogue_bestiary.kel` and `rogue_gear.kel`, both
  at **seed 2**, the all-zeros vector.

`native_codegen/src/lib.rs` was restored byte-identical under `cmp` after every
mutation, and each mutation was confirmed present in the file before its result was
read.

## What this does NOT establish

- **Four vectors are not a proof of adequacy.** They close the equal-operand case
  because that case was measured to be open. Another opcode may have a boundary
  these four never reach, and nothing here rules that out.
- **No per-module detection census exists**, because two of the four mutations
  killed the harness with a signal. Obtaining one needs process isolation per
  module, which is not built.
- **The classification in `is_vacuous` still reads seed 0 only.** The reported
  figures therefore describe the same run they always did, which is deliberate, but
  it means a module vacuous at seed 0 and substantive at seed 2 is still counted as
  vacuous.

## Reproduce

```sh
cd native_codegen
cargo test --test probe_agreement_depth -- --nocapture   # depth and opcode census
cargo test --test corpus_differential -- --nocapture     # the seeded differential
```

---

## THE SWEEP, 2026-08-15: the census is no longer four samples

`tools/mutation_sweep.py` carries a **pre-registered** mutation set, committed in
`e157e271` before any of it was run. 24 opcodes get a semantic perturbation; 25
more with sites are listed as not perturbed, each with its reason.

### Two instrument defects had to be fixed before any result was trustworthy

**It hung.** The first attempt stalled twelve minutes on one module, because
`CheckedAdd → sub` stops a loop counter ever reaching its bound and the driver had
no per-invocation timeout. `HANG` is now an outcome, and it counts as detection: a
language whose value proposition is a definitive worst-case execution time does
not get to loop forever.

**It misclassified, and a known answer caught it.** The fixed driver reported
`CmpLt` as *undetected across 25*, contradicting the hand-verified result that
seed 2 catches it on two modules. The cause was `"EXEMPT" in txt`, true of every
run because the summary always prints an `EXEMPT` line, so every disagreement was
filed as `NOLOWER`. It now classifies on exit status first and reproduces the
hand-verified `CmpLt DETECTED by 2/25` exactly. **Without a known answer to
calibrate against, "CmpLt is undetectable" would have been published as a
finding.**

### Round one: 24 opcodes

Detected, with the fraction of owning modules that noticed: `CmpEq` 20/42, `If`
21/44, `Const` 27/52, `SetLocal` 18/47, `GetLocal` 15/51, `Return` 15/50,
`CheckedAdd` 15/43, `CheckedSub` 9/24, `CmpGt` 7/24, `BreakIf` 6/15, `CheckedMul`
5/20, `CmpGe` 4/22, `CmpLt` 2/25, `Not` 2/14, `CheckedNeg` 1/18, `CmpLe` 1/11,
`BitXor` 1/1.

Undetected: `BitAnd`, `BitOr`, `ByteToWord`, `CmpNe`, `Dup`, `PushImmediate`,
`Shl`, `Shr`.

### Round two: which of those are HOLES and which are EQUIVALENT MUTANTS

An undetected result has two readings and round one cannot separate them. Round
two replaces each opcode's result with a constant — the most observable change
available — so a survivor is a real hole.

- **`Dup` became DETECTED (1/10).** Round one's mutation was too weak, not the
  corpus blind.
- **`ByteToWord` is an equivalent mutant by construction.** Its arm sets an
  operand WIDTH and emits no value; the width only matters when the value is
  packed into a composite body, and these two modules never pack it.
- **`PushImmediate` was a VACUOUS MUTATION, and this is the important one.** All
  **1337** sites in the corpus carry immediate index **0**. The mutation changed
  the arm for index **1**, which has **zero** sites. It could not have been
  detected because it changed nothing. Reporting it as a coverage hole would have
  been exactly the error this whole arc is about — a vacuous experiment read as
  evidence.

### The four real holes, and they have ONE cause

> **SUPERSEDED 2026-08-15 — all four are CLOSED.** See PART C. The diagnosis
> below is correct and the proposed repair worked, but it was not sufficient on
> its own: seeding the shared segment closed `BitAnd` and `Shr`, and closing
> `BitOr` and `Shl` additionally needed the harness to drive a range of COMMAND
> SELECTOR values. Retained because the diagnosis is what made the repair
> findable.

| opcode | sites | owning modules |
|---|---|---|
| `BitAnd` | 54 | `wire.kel` |
| `BitOr` | 9 | `wire.kel` |
| `Shl` | 48 | `wire.kel` |
| `Shr` | 20 | `wire.kel` |
| `CmpNe` | 26 | `analyze`, `lexer`, `parse`, `piano_roll_3/4/8/9`, `wire` |

**Every module owning an undetected opcode is one this line already knows is not
really running.** `wire.kel` finishes after **0 ticks**. `analyze`, `lexer` and
`parse` are the shared-segment stages driven on zeros. The four `piano_roll`
modules are exempt on the string ABI.

**The sweep independently rediscovers the vacuity finding from the opcode side.**
A different instrument, the same root cause: 131 sites of bitwise and shift
lowering plus 26 comparisons are unobserved because the modules that contain them
do not execute.

That makes the repair concrete rather than open-ended. Driving `wire.kel` with
real input would close four of the five, and it is the same blocked seam Part B of
the previous increment named: `wire.kel` reads its input from a shared segment
whose layout belongs to `src/selfhost/mod.rs`.

### What this still does NOT establish

- **25 opcodes were not perturbed**, each with a stated reason, so the sweep is
  not exhaustive over the instruction set.
- **One perturbation per opcode.** `CmpLt` needed a boundary mutation to expose
  its hole and an inversion to characterise it; other opcodes may have boundaries
  a single perturbation misses.
- **Static sites, not dynamic execution.** The map counts where an opcode is
  emitted, not where it runs. A detected opcode may still be observed by only a
  fraction of its sites.

---

## PART B, 2026-08-15: the corpus under the O2 middle end

### A correction first

This gap was stated as *"no differential and no object file has ever been
optimised"*. **The second half was wrong.** `aot_linkage.rs` runs `default<O2>`
and links the result into a running C program, and its own header says that is
why it exists. The real gap was narrower: one hand-written module through the
middle end, against thirty-seven in the corpus.

### The result

`corpus_differential` runs the whole corpus through `default<O2>` when
`KEL_OPTIMIZE` is set. **37 executed and agreeing, 6 vacuous, zero
disagreements — identical to the unoptimised run.**

### What that does and does NOT show

**It shows** the optimiser did not exploit anything on these inputs, for these
modules, and that the emitted IR still passes `Module::verify` after the middle
end — checked on six modules of different shapes in `optimised_lowering.rs`.

**It does NOT show the IR is free of undefined behaviour.** Undefined behaviour is
a licence the optimiser may or may not take, and not taking it on one input set is
not evidence of its absence. A future pass, a different target, or an input that
reaches a different path can all change the answer. **This must not be recorded as
the stronger claim.**

### EXTENDED 2026-08-15: every differential, not just the corpus one

The first pass covered `corpus_differential` only. The goal said *"run the
differentials"*, plural, and eleven test files create a JIT — including
`composite_return_aliasing.rs`, which pins the composite-return aliasing defect,
**the only genuine codegen defect this line has ever found**. Region aliasing is
exactly what an optimiser reasons about, so that was the worst one to leave at
`-O0`.

`tests/common/mod.rs::maybe_optimize` now runs `default<O2>` on demand and is
called from all eleven, so the two paths cannot drift apart. It verifies the
module after the pipeline, since IR that verified before and not after is the
finding this exercise is looking for.

**Result: 195 tests pass under `KEL_OPTIMIZE`, identical to the unoptimised
baseline. No module diverges only under optimisation.**

Checked for vacuity two ways: every one of the eleven files was confirmed to CALL
the helper rather than merely declare the module, and the instruction-count guard
below shows the pipeline transforms real code.

### The vacuity guard, because this run could have been a no-op

A green optimised differential proves nothing if the pipeline never ran.
`the_o2_pipeline_measurably_transforms_a_real_module` asserts a measured change:
`09_big_numbers.kel` goes from **408 instructions to 61**, a 6.7x reduction. The
increment this belongs to opened with nine modules agreeing while doing nothing,
and an unguarded "it passes under O2" would be the same mistake in a new place.

---

## PART C, 2026-08-15: round three, a false-positive instrument, and one hole left

> **THE HOLE IS CLOSED. See PART D**, which changed the observable rather than the inputs.
> Part C's diagnosis — that no seed could close it — is correct and is what made Part D
> findable; only its status is superseded.

### Read this first: an intermediate result in this section's history was WRONG

Round three ran, then the harness was widened, and the widened harness reported
**every opcode detected**. That was false. The sequence is recorded rather than
tidied away, because the error was silent, it favoured the flattering answer, and
the thing that caught it was a deliberately planted known answer.

### Round three: the memory and composite surface

Seventeen mutations, pre-registered in `f8f7dcc4` and committed **before** being
run, covering opcodes round one had skipped. Most had been skipped because
several opcodes share an emitter arm and one swap could not be attributed; round
three guards each mutation on the opcode, so `GetData` and `SetData` attribute
separately through `is_read`, and `Div` and `Mod` through `matches!`.

Every variant was confirmed reachable first. `GetField(FlatNested)` has **zero**
sites and is deliberately absent, because mutating it would have repeated the
`PushImmediate` error, where the largest apparent hole was a mutation of an
operand the corpus never emits.

**Result under the corrected instrument, 16 of 17 detected:**

`NewComposite` 18/21, `Div` 11/12, `SetData` 8/23, `GetData` 6/22, `Yield` 4/20,
`Call` 2/31, `GetIndex` 2/2, `GetTupleField` 2/5, `GetDataIndexed` 2/13,
`SetDataIndexed` 2/12, `CallVerifiedNative` 1/15, `Mod` 1/12, `GetField` 1/1,
`GetEnumField` 1/1, `IsEnum` 1/3, `WordToByte` 1/1.

**Undetected: `Trap`, across all 28 modules that emit it.**

### THE INSTRUMENT DEFECT: a fixed timeout manufactured four closures

Round two's four holes were all owned by `wire.kel`, and the recorded repair was
to drive that module with real input. Two changes did that: a seeded shared
segment, and widening the differential from 4 argument vectors to 24 so that a
module dispatching on a command selector reaches more than four of its commands.

**The widened run then reported every opcode detected, and that was an
artefact.** `PER_MODULE_TIMEOUT` was a flat 20 seconds, chosen when the slowest
module took about 4. At 24 seeds `wire.kel` takes **30.7 seconds unmutated**, so
it exceeded the budget under *every* mutation, and the driver counts a timeout as
detection. Every `DETECTED by 1/1` was `wire.kel` failing to finish.

**`PushImmediate` is what exposed it.** Round two had established that its
mutation edits an emitter arm with **zero** sites, so it cannot be detected by
anything. When it reported DETECTED, the contradiction with a known answer was
the signal — exactly as the `CmpLt` misclassification was caught earlier in this
same document. **Without a case whose answer is fixed in advance, four false
closures would have been published as findings.**

The repair is that the budget is **measured, not fixed**. `calibrate()` times
each module unmutated and allows `HANG_MULTIPLIER` times that, with the old
constant kept only as a floor; `wire.kel` gets 168 seconds. A real infinite loop
does not terminate at any budget, so nothing is lost. The verdict line now also
prints **how** each detection was reached — `[DISAGREE 1]` against `[HANG 1]` —
because the three outcomes are not interchangeable evidence and the summary
previously hid the difference.

A flat constant that must be re-tuned whenever the harness changes is a standing
maintenance obligation, and this document already records the same species of
error twice.

**HOW FAR BACK THE DEFECT REACHES, and the answer is bounded.** The failure is
one-directional: an under-sized budget can turn a healthy run into a false
DETECTED, but it can never turn a genuine detection into an UNDETECTED. So
**every undetected finding in this document survives** — rounds one and two, and
`Trap` — and only positive results measured under the flat budget are suspect.
Those are re-measured above. The earlier rounds are additionally unaffected in
fact, since they predate the seed widening that made any module approach 20
seconds.

### The round-two holes ARE closed, on corrected evidence

| | `BitAnd` 54 | `Shr` 20 | `BitOr` 9 | `Shl` 48 | `CmpNe` 26 |
|---|---|---|---|---|---|
| round two | -- | -- | -- | -- | -- |
| seeded shared segment, 4 seeds | **YES** | **YES** | -- | -- | -- |
| 24 seeds, calibrated budget | YES | YES | **YES** | **YES** | **YES** |

Every one is a genuine `DISAGREE`, except `CmpNe` which is a `SIGNAL`. None rests
on a timeout.

**Seeding alone was not sufficient, and the reason generalises.** A payload in
the shared segment reaches only the EMIT direction, which extracts bytes with a
mask and a right shift — hence `BitAnd` and `Shr`. `BitOr` and `Shl` live in the
PARSE direction, which reassembles a multi-byte integer, and no argument SHAPE
reaches it: only a selector VALUE does. `wire.kel` branches twenty-odd ways on
its first argument, so four argument shapes reached four of its commands and left
the rest unexecuted while the harness reported the module as running.

Seeds 4 and up therefore sweep a consecutive small constant. This is
**deliberately generic**: the values are not chosen by reading `wire.kel`'s
dispatch table, because picking `cmd == 9` for being where the undetected sites
are would make the exercise a demonstration rather than a measurement.

`WordToByte` closed the same way. At 4 seeds its single site was measured
**unreachable** — a probe that made the site branch to the trap block still went
undetected, which no value perturbation could have shown. At 24 seeds it is
reached and detected. The 4-seed reading was correct about the harness at the
time and wrong as a statement about the opcode.

**`PushImmediate` remains undetected and is not a hole**, for the reason round
two established.

### `Trap`: the one real hole, and it is undetectable BY CONSTRUCTION

Undetected across all **28** modules that emit it, under 24 seeds and a
calibrated budget, with a maximally destructive mutation: branch-to-trap replaced
by return-zero, so a program that must abort instead returns a value. Nothing
noticed.

The cause is in the harness and it is deliberate. `corpus_differential` runs the
virtual machine **first**, precisely so a trapping module becomes a named
exemption rather than a `SIGTRAP` that kills the entire run. The consequence had
never been written down:

- a module that **reaches** a trap is exempted, and never compared;
- a module that **is** compared is one whose virtual-machine run did not fault,
  so it reached no trap either.

**Every compared run has an unexecuted trap block.** No amount of input diversity
changes this, because the exemption rule removes exactly the evidence needed.
This is a different kind of hole from every other one in this document: the
others were inputs the harness never supplied, this one is an observation the
harness is structured to discard.

Closing it needs a different OBSERVABLE, not better inputs: for a module whose
virtual-machine run faults, run the native side in a subprocess and require it to
die with `SIGTRAP` — agreement on the FACT of the fault rather than on a returned
value. **This is the named next increment.** It matters more than one row
suggests, because the trap path is the safety path of a language whose
proposition is that unbounded programs are rejected rather than silently wrong.

### Cost, and what this still does not establish

24 seeds instead of 4 takes `corpus_differential` from **35s to 58s**, sublinear
because a `Stream` entry and a zero-parameter entry each keep a single seed. The
sweep additionally pays one unmutated calibration run per module it will drive.

- ~~**The `Trap` hole is open**, and it is the largest this census has found: 28
  modules, the safety path, undetectable by construction.~~ **CLOSED in Part D**,
  by a subprocess `SIGTRAP` differential. Struck rather than deleted: the
  reasoning that made it look permanent is why the fix took the shape it did.
- **Eight opcodes have never been perturbed in any round** — `Break`, `Else`,
  `EndIf`, `EndLoop`, `Loop`, `PopN`, `Reset`, `Stream` — each with a stated
  reason. The summary now computes that residue by subtracting every mutation
  table, because the previous hand-maintained count printed "25" underneath a
  round that had just perturbed seventeen of them.
- **Static sites, not dynamic execution.** The map counts where an opcode is
  emitted, not where it runs; a detected opcode may be observed by only a
  fraction of its sites.
- **One perturbation per opcode per round**, so an opcode may have a boundary a
  single perturbation misses.
- **A calibrated budget is not a proof of termination.** It rules out the false
  positive measured here; it does not establish that every remaining `HANG` is
  genuine non-termination rather than a slower machine.

---

## PART D, 2026-08-15: the `Trap` hole is CLOSED, and the first attempt closed nothing

**`Trap` is DETECTED by 30/30, all genuine `DISAGREE`.** The census now has no
undetected opcode other than `PushImmediate`, which round two established is a
vacuous mutation rather than a hole.

### The observable, as Part C specified it

Part C established that no seed could close this: `corpus_differential` runs the
virtual machine FIRST precisely so a trapping module becomes a named exemption
rather than a `SIGTRAP` that kills the run, so a module that reaches a trap is
never compared and a module that is compared reached none.

The fix is a different observable — **the fact of the fault, not a returned
value**. For a program whose virtual-machine run faults, the native side runs in
a child process and must die with `SIGTRAP`. The parent asserts three things: a
marker printed immediately before the native call (so a child that died in setup
is not mistaken for one that trapped), that the child did not SURVIVE, and that
the signal is `SIGTRAP` specifically.

### THE FIRST IMPLEMENTATION PASSED AND PROVED NOTHING

It used three corpus files that fault: `faulty.kel`, `led.kel`,
`rogue_dungen.kel`. It was green. **Mutating `Op::Trap` left it green**, which is
how the gap surfaced — the verification step, not review.

**None of those modules emits `Op::Trap` at all.** `faulty.kel` faults through the
emitter's DIVISION GUARD and `rogue_dungen.kel` through its BOUNDS CHECK. Both
are emitter-inserted branches to the trap block, not the opcode. Confirmed
against `dump_opcode_module_map`: **no module in the shipped corpus that faults on
the virtual machine emits `Op::Trap`.**

So a synthetic subject is not a convenience here, it is the only way to reach the
opcode. A multiheaded function whose guards all fail emits `Trap(NoMatchingHead)`:

```keleusma
fn pick(x: Word) -> Word when x > 100 { 1 }
fn pick(x: Word) -> Word when x < 0 { 2 }
fn main(a: Word) -> Word { pick(a) }
```

**The guard that would have caught it is now in the test.** Every subject is
labelled `Op` or `Guard`, and the labels are ASSERTED: an `Op` subject must
contain `Op::Trap`, a `Guard` subject must not, and at least one `Op` subject must
run or the test fails as vacuous. The two kinds are both worth covering and are
not interchangeable evidence.

### `led.kel` is EXCLUDED, and the exclusion is asserted rather than commented

Its virtual-machine run faults with `NoMatchingArm`, so it passes the entry
criterion. **Its native side dies with SIGSEGV, not SIGTRAP.** `host::gpio_set`
records a return shape of `Flat { kind: 3, size: 16 }` and the generic stub
returns a plain integer, which the native side dereferences as a body address.

**The two sides fault for different reasons, so admitting it would be a false
agreement** — and a check that accepted "died by some signal" would have counted
it. The exclusion is a live assertion: if the composite return path lands and the
stub returns a body address, the test fails and says to MOVE the module into the
subject list.

### What this does NOT establish

- **`DETECTED by 30/30` is one test detecting, not 30 modules independently.**
  The trap check runs in every invocation of this binary, so under the mutation
  every invocation fails. That is honest detection and it is not thirty
  witnesses.
- **One `Op::Trap` subject, and it is synthetic.** The corpus supplies no
  reachable one. If a real module ever reaches `Op::Trap`, it is a better subject
  than this.
- **`SIGTRAP` is agreement on the FACT of a fault, not on WHICH fault.** The
  virtual machine distinguishes `NoMatchingHead` from `DivisionByZero`; the
  native side raises the same signal for both. A lowering that trapped for the
  wrong reason would pass.

---

## PART E, 2026-08-16: the corpus this census measures EXCLUDES five modules

**Every figure above is over a corpus that silently omits every `examples/rtos/scripts/` source.**
Measured, not inferred: `event_listener`, `faulty`, `heartbeat`, `led` and `sensor` appear **zero**
times in `dump_opcode_module_map`, for *every* opcode — checked against `Const`, the most ubiquitous
one, not merely against `Trap`.

**The cause is a compile failure the map does not report.** The map compiles each source standalone.
These five need `prelude.kel` prepended, which the real host does at
`examples/rtos/src/setup.rs:429`. Standalone they do not compile, so they never enter the map.

**`tools/mutation_sweep.py` drives only the modules the map lists per opcode.** So a
`DETECTED by n/m` denominator is over a corpus smaller than a reader would assume, and these five
modules have never participated in any mutation round.

**This is the same species as the vacuity findings** — a coverage claim that is really a claim about
the instrument's input list, which is the error this whole document exists to catch. It cost nothing
here because no reported conclusion depends on those five; it is recorded so that the next figure
quoted from this census carries its true denominator.

**Recorded rather than repaired, deliberately.** Prepending the prelude would change what the map
MEANS: a module compiled with a prelude is not the module `corpus_differential` drives standalone,
and the two would no longer be the same corpus. That is a decision about the instrument, not a
defect to patch quietly. `the_opcode_map_excludes_every_rtos_script` pins the current state and says
what to do if it ever changes.

### A related closure, for the record

**`led.kel` is no longer exempt.** `host::gpio_set` records a sixteen-byte enum body and the generic
stub returned a plain integer, which the native side dereferenced as an address — SIGSEGV against
the virtual machine's `NoMatchingArm`. The stub now builds a real body on both sides from one shared
byte builder.

**It cost a `Trap` subject rather than gaining one**, which is the opposite of what was expected.
`led.kel` does emit `Op::Trap`, but it matches both `Status::Ok` and `Status::Err(code)`, so a
faithful stub returns a valid variant, an arm matches, and the trap is never reached. Reaching it
would require a discriminant matching no variant — an unfaithful stub and a false agreement. **The
`Op::Trap` subject remains synthetic.**
