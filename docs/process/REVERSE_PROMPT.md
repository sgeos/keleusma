# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-16 (session 46)

## Where things stand

| | |
|---|---|
| The understated WCMU bound | **FIXED**, and it was much larger than reported |
| A second, opposite defect underneath it | **FIXED** in the same increment |
| The five-case model control | **replaced** by a check ranging over the opcode set |
| What that check found on its first run | **two opcodes** no case list had reached |
| The roadmap's Order 1 cell | corrected; it now states the actual gap |
| Owed to `v0.3.0` | one visibility decision, deliberately not bundled |

## THE BOUND WAS NOT OFF BY ONE. THE BODY CONTRIBUTION WAS ABSENT

The `v0.3.0` line reported `06_multiheaded::classify` and `rogue_bestiary::corpse_fill` at a bound
of **2** where both peak models and their emitter said **3**. Reproduced, and **their report
understated its own finding**: the 2 is `local_count` alone. The reported body peak was exactly **0**.

**The cause is a type, not the arm anyone was looking at.** `wcmu_region` returned
`Option<McuResult>`, in which `None` meant "no path reaches the end" and carried no resources at all.
Four sites discarded an accumulated peak and heap on that encoding: the `Trap` arm, the `If` arm when
both branches exited, the `Loop` arm when the body never fell through, and every top-level caller's
`unwrap_or(McuResult::empty())` — `module_wcmu` included, so the shipped module header carried it.
**Patching the arm named in the report would have left three.**

It is now `McuOutcome`: `peak_above_initial` and `heap_total` are always meaningful, and only
`delta: Option<i32>` carries the control-flow fact. Resources are monotone along a path; control flow
is not, and the old encoding conflated them.

**The reach is the whole multihead construct**, since each compiles to guarded heads with a trailing
no-match dispatch `Trap`. The corpus split before the repair is total rather than suggestive:

| | body peak zero | body peak non-zero |
|---|---|---|
| ends in `Trap` | **6** | 0 |
| no trailing `Trap` | 0 | **58** |

## A SECOND DEFECT, POINTING THE OTHER WAY, WAS UNDERNEATH IT

With the discard fixed, `classify` reported **7** against an emitter allocating 3. `Op::Return` fell
through the catch-all, so a multiheaded dispatch was walked as though every head ran in sequence,
each head's `Return` leaving its offset for the next. Made a path exit like `Trap`. Both chunks now
report a body peak of **3**.

**The two errors were present simultaneously and partially cancelled.** That is why the symptom
presented as a small understatement rather than as two large opposite ones, and it is the reason to
distrust a bound that looks nearly right when checked against a hand walk.

## THE CONTROL, BECAUSE THE MEASUREMENT ALONE WOULD NOT HAVE SETTLED IT

Two sources whose compiled bodies differ only by the trailing dispatch trap. Single-head reports a
body peak of 3; multihead reports 0; and the multihead body strictly contains the single-head body.
That is what makes it a defect rather than a modelling choice.

## THE RANGING CHECK FOUND TWO OPCODES IMMEDIATELY

`the_peak_model_agrees_with_the_depth_model` compared the two models over five hand-written sources.
**Its coverage was a property of its case list**, and it is now superseded by a check ranging over
the opcode set, with completeness asserted against the wire format's canonical opcode table so a new
opcode is reported **by name** rather than skipped. It is mutation-verified: mutating one shared
match arm makes it report all nine opcodes in that arm.

On its first run it reported **`FixedMul` and `FixedDiv`**, which declare a peak-model net of 0
against a virtual-machine handler that pops twice and pushes once. **Reachable by no case in the list
it replaced.**

**Pinned, not repaired**, and the reason is directional: that error OVERSTATES, so it is a precision
defect rather than a soundness one, and repairing it LOWERS bounds on shipped chunks — the opposite
direction from this increment's subject. `Op::Yield` stays pinned for the same reason and a different
cause. The check fails if either is repaired without removing its entry, so neither can be lost.

## A MISTAKE I ALMOST WROTE DOWN AS A FINDING

The first draft of the known-disagreement list predicted that the six control-flow opcodes would
disagree, on the reasonable ground that `verify_depth_region` intercepts them before their
`op_depth_effect` entry is read. **All six agree.** The staleness assertion — every known entry must
still disagree — is what said so. Six plausible entries with a plausible reason would otherwise have
gone in unchallenged. **A list of expected failures needs the same control as a list of expected
passes.**

Two instrument faults were mine and neither reached a conclusion. My first depth walk destructured
`op_depth_effect` as `(push, pop)` when it returns `(required, net)` — the same misreading that
produced a retracted report on the other line last week. My first corpus walk was straight-line over
a branching op array and reported an understatement of 1801 slots.

## `wcet_region` HAS THE IDENTICAL DEFECT, AND I HAVE NOT REPAIRED IT

**This is now the top open correctness item on this line.** `wcet_region`'s `Op::Trap` arm
accumulates `cost`, then does `let _ = cost;` and returns `Ok(None)` — the same idiom, in the sibling
analysis, so the cycles spent before a trap are discarded from the worst-case EXECUTION TIME bound.
Found by reading the structure, not from any report.

**Deliberately out of this increment.** It is a different analysis with its own corpus and tests, and
a self-hosted mirror whose cost folding would have to move with it. This increment already changes a
bound model, and mixing a WCET change into it would make the diff illegible. **It is a real
understatement and should be the next thing taken, or the one after the `ParsedFn` decision.**

## `analyze.kel` HAD THE SAME DEFECT, AND I FIRST REPORTED THAT IT DID NOT

**Correcting myself.** I read `account_op` running before the broke flag, saw the frame retain its
peak, and concluded the self-hosted stage never had the reference's top-level discard. That was
wrong, and the differential said so. `run()` ended with

    let region_peak = if an.child_broke == 1 { 0 } else { an.child_peak };

for cost, peak and heap alike — the exact analogue of `unwrap_or(McuResult::empty())`. **Every
single-head function ends in a top-level `return`, so this zeroed the body contribution of
essentially every `fn` in every stage.** I had checked the child-frame path and stopped before the
place the answer is actually produced.

**Two further things were wrong underneath it.**

`Op::Return` had no control-flow class, so `analyze.kel` walked a multiheaded dispatch as though
every head ran in sequence. It now shares class 8 with `Trap`, which is a PATH EXIT rather than a
trap class; no tenth class was needed and the pinned nine-class boundary still reports nine.

**And `tests/selfhost_codegen.rs` carried its own second copy of the class table**, which had already
drifted: it kept the `_ => (0, 0)` catch-all after the driver's was made exhaustive, and passed `0`
where the driver passes real `EndLoop`/`Break`/`BreakIf` targets. **The differential that is supposed
to be the oracle was running against the unrepaired table**, which is why my first driver-side fix
changed nothing. `analyze_class` and `analyze_opk` now live in `selfhost_host`, which is gated on
`compile + verify` rather than `self-host`, and the duplicate is deleted — one encoding, the
same reasoning as the per-item seed accessors.

**WHY THE COPY EXISTED, WHICH I ONLY LEARNED FROM A RED CI JOB.** My first fix made the driver's
copies `pub`. That failed the MSRV and broad-feature jobs with `unresolved import
keleusma::selfhost`, because `selfhost` is gated on `self-host` and **the test file builds without
it**. The duplicate was not carelessness; the consumer genuinely could not reach the original.
`selfhost_host` already existed for precisely this — its own doc says it is there "so the
parse-record transport lives in one place instead of being copied into every consumer" — so that
is where they belong. **A duplicate with a structural cause comes back unless the cause is
removed.**

**This is the strongest argument in the whole increment for the differential.** Three defects in the
self-hosted analyzer, one of them a silently drifted copy of a table repaired a day earlier, and none
of them was visible until a reference change forced the two sides apart.

## What these green suites do NOT establish

- The corpus invariant is a property of the example corpus, which is a case list. The property-level
  test that does not depend on any corpus is `a_chunk_that_only_traps_still_reports_what_it_consumed`.
- Agreement at 3 between the repaired bound, the two peak models and the emitter is agreement among
  four readers of the same instruction stream, not a measurement of the machine's actual stack use.
- The ranging check compares the two models against each other. It is not evidence that either
  matches the virtual machine; that link is made per opcode, by hand, against the handler.

## One behavioural widening, stated rather than buried

A loop whose body always returns is now accepted with an iteration count of 1 where it previously
required an extractable bound. **Unreachable from this compiler** — the dispatch `Loop` wrapper is
emitted only for Stream chunks, whose heads emit `Break` rather than `Return` — but reachable by
hand-crafted bytecode, and sound there, since a body that always returns iterates once.

## Owed to the `v0.3.0` line

**Their per-chunk WCMU numbers have moved and should be re-measured, not reused.** Bounds rise on
chunks whose paths exit without falling through and fall on multiheaded chunks. Their
emitted-slots-exceed-proven-bound count and their 8-of-958 negative walk are both computed against
numbers that changed.

**`ParsedFn` is still blocking their `seed_reconstruct_*` accessors**, and I deliberately did not
bundle a visibility decision into a bound repair. My inclination is a `pub fn` returning the records
for a source string rather than opening the fields, so the stage's input shape stays ours to change.
It is the next thing I pick up unless redirected.

## Open

- **`wcet_region` discards the cycles consumed before a trap**, the same defect in the sibling
  analysis. **Highest open correctness item.** Not repaired here, deliberately.
- **`Op::Yield`'s peak-model net**, pinned in the ranging check. Different cause from the above.
- **`FixedMul`/`FixedDiv` peak-model nets**, pinned in the same place, verified against the handler.
- **`Op::cost()` disagrees with measurement**; 17 opcodes of 66 were ever measured.
- **Order 1 remains the real gap**: the self-hosted path emits two region kinds against a schema of
  about twenty, and type rejection is 7 tests against ~15 shapes. The roadmap cell now says so.
- **The `for` trailing-semicolon asymmetry**, pinned. Widening is the operator's call.
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- Publication remains **HELD**.

## Questions for the operator

1. **`FixedMul`/`FixedDiv`.** Repair the peak-model nets, which lowers bounds on shipped chunks, or
   leave them pinned? I have not taken it, because lowering a bound in an increment about raising one
   would make the diff illegible.
2. **`ParsedFn` visibility.** A `pub fn` returning records, or `pub` fields? The other line asked us
   to choose and I would rather confirm than guess.
3. **`Op::cost()`.** Still pinned, still a judgment call I have deliberately not taken.
