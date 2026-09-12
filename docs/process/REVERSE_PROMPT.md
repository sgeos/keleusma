# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

> **Absorption 57 resolved a conflict here by KEEPING THIS LINE'S MESSAGE.** The `v0.2.3` line
> writes its own reverse prompt in the same path, and absorbing is one-directional, so a merge
> would have to discard one of the two. This branch's copy carries this branch's message; the
> `v0.2.3` line's is on `origin/v0.2.3` at the same path, and nothing here overwrites it there.

## TWO SILENT DEFECTS FOUND, ONE FIXED, ONE FIXED PROPERLY AFTER BEING REFUSED

Both agreed with your runtime on every subject that existed before. Neither was found by the thing
being checked.

### 1. A local live across a `yield` was wiped on the way back in — FIXED

```
loop main(t: Word) -> Word { let keep = t + 100; let r = yield 1; yield r + keep }
   yours [1, 108]        mine [1, 3]
```

The entry preamble zeroes non-parameter locals so an unwritten slot reads as your `Unit` rather than
`undef`. **Right for a function entered once; a resumable stream is entered once per suspension.** The
initialisation now sits on the first-entry edge of the dispatch. The parameter store stays
unconditional — your resume writes slot 0, and a stream reading its parameter after a suspension sees
the resume value on both sides.

**No existing stream subject had a local live across a suspension**, which is why the suite was green
over it. Found through a control subject added so a count could not pass by failing to move.

### 2. A composite in a data slot was stored as a POINTER — REFUSED, then COPIED

`14_frame_log.kel` opens by saying a slot holds *"a COPY of the composite's bytes ... not a reference
to the ephemeral body"*. My lowering stored the reference: a slot access is one word, and for a flat
composite the operand is an address into the ephemeral region.

It agreed with you anyway, because that script reads its slot in the iteration that wrote it. A
subject that writes the slot on one loop iteration and then rebuilds the SAME site twice more yields
`0` on your side and gave `2` on mine.

**Now implemented.** The write copies the body into the persistent composite pool and the read hands
back the pool address. `14_frame_log.kel` lowers again and agrees — `[81, 84, 87, 90]` across four
cycles — and it agrees BY CONSTRUCTION rather than by that coincidence.

## THREE THINGS I TOOK FROM YOUR TREE RATHER THAN ASSUMED

- **The pool is packed with one running total and no padding.** Read from `src/compiler.rs`, so a
  body's size is the gap to the next entry's offset. **Derived, therefore validated**: the table must
  partition the pool or the lowering refuses, because a derived length feeding a copy is the one place
  here where being slightly wrong is an overrun rather than a wrong answer.
- **Our private regions are different memory images.** You size yours as `private_count *
  size_of::<Value>()` plus the pool, with a 32-byte `Value`; my private slot is 8 bytes. I place my
  pool after my own slot array and REFUSE if that would reach the resume-state word, rather than
  assuming it clears.
- **An indexed composite slot is refused**, with a subject: the shape compiles on your compiler, so
  the refusal is driven rather than hypothetical. Every element carries its own pool entry and the
  stride is not proven uniform.

## THE `Op::Reset` PREMISE, CHECKED AT LAST

The comment said every site is overwritten by the next iteration. **It is not** — an iteration
skipping a site's constructor leaves the previous body in your buffer, and the test reads those bytes
back and tells them apart from a poison and from a rebuilt body. What makes the retention
unobservable is provenance plus no pointer outliving its iteration; the escape routes are tabulated
with their mechanisms, including the two closed only indirectly.

## A THIRD THING, AND IT WAS MY INSTRUMENT RATHER THAN MY LOWERING

**My corpus harness sized its private buffer by slot count, not by the contract my backend
publishes.** One word per private slot, canary immediately after; the contract is
`required_persistent_capacity_for` plus my supplement — slot array, composite pool, resume-state
word. The pool begins exactly where the canary was.

**It stayed invisible because nothing had ever used that space.** I hypothesised that the
resume-state word had been writing hundreds of kilobytes past the allocation on every corpus run —
`parse.kel` short by 519 KB — and **checked it: FALSE.** Every corpus stream with private data lowers
degenerately, so none of them has a dispatch or a state word. Twelve modules were short of the
contract and none wrote past it.

**The part I want on the record**: a canary immediately after a too-small buffer only catches writes
that land JUST past the end. The pool starting at the canary word is the only reason this surfaced as
an assertion rather than as a write into unrelated memory.

## WHAT I GOT WRONG

- **I was about to file a mechanism I had not verified**, citing a test that measures a population,
  not copy semantics. Reading the citation is the only reason the second defect surfaced.
- **My first completion condition would have passed a lowering carrying both defects.** Eight
  clauses, seven met, neither defect described by any.
- **The second condition's clause 2 was satisfiable by a weaker property** than the one it names. A
  subject that tells survival from never-being-rewritten now exists.
- **I ran a gate while still editing** and it reported NOT FROZEN. Self-inflicted; the instrument
  caught it.

## THE COUNT THAT WENT OUT AND CAME HOME

Corpus refusals went `1 -> 2` when a silent miscompilation became a loud refusal, then `2 -> 1` when
the refusal became a correct lowering. **The digit is where it started and the tree is not.** No
aggregate distinguishes those three states.

## ABSORPTION 57, AND A PREDICTION THAT CONTRADICTED ITS OWN RISK

25 commits. Three clauses hit exactly; the fourth could not have been right.

**I named the risk precisely** — the incoming set changes `wire.kel` and `verify_types.kel`, which are
CORPUS SUBJECTS of my backend — **and then predicted a clean suite two lines later.** My corpus
fingerprint guard exists to fail when corpus content moves. If the risk was real, a green run was
impossible. The guard fired, named exactly those two files, and told me which figures to re-derive.

**Cleared by evidence.** The chunk population moved +11, measured by compiling both versions of each
file: `wire.kel` 486 to 492, `verify_types.kel` 28 to 33. File population unchanged at 74. Every
named figure re-run and none moved: refusal set 1, ISA census 63 of 66, module coverage 98.6%, 1084 of
1085 chunks lowerable, 90800 of 90845 opcode instances.

**One arithmetic does not close and I left it open.** The `1070 of 1074` figure in my tree is a
calibration dated 2026-08-29, not a measurement of the pre-absorption tree; adding the measured +11
does not reach 1084. Forcing it to balance would have been worse than naming it.

**Conflict resolutions, both in process channels**: `TASKLOG.md` took yours with my currency note
re-applied; `REVERSE_PROMPT.md` kept mine, and says so above.

## THE CENSUS FOR VALUE MOVEMENT

The data-slot defect had an unexamined class behind it: **every place my emitter moves an operand as a
word is a place a body operand would be moved as its address.** I had a deliberate census for ADDRESS
arithmetic and none for VALUE movement, which is where that defect lived.

All 17 move sites are now enumerated and classified **by what the destination outlives** — the
question — rather than by whether the move is word-sized, which is not. Operand slots, locals, the
spill slice and composite bodies outlive nothing; shared slots, private slots and the pool all
survive, and each refuses a body or copies it. `return` and `yield` are not stores, so they are named
as outside the count rather than silently skipped.

**A shared composite slot compiles on your compiler and is refused here**, so that row is driven
rather than hypothetical.

## ⚠ A THIRD QUESTION FOR YOU: IS THE WRITE-BEFORE-READ CHECK MEANT TO BE FLOW-INSENSITIVE?

**Measured, not inferred.** Your compiler REJECTS an unconditional read-before-write of a composite
private slot. It ACCEPTS this:

```
private data log { latest: F, count: Word }
fn main(t: Word) -> Word { if t < 0 { log.latest = F { a: 11, b: 22 }; } log.latest.a }
```

and at run time your virtual machine faults with `TypeError("cannot access field on Unit")` when the
branch is not taken. So the contract is enforced, but at execution rather than at compile time for
this shape.

**My backend returned 0.** The pool is bytes and zeros are indistinguishable from a written body of
zeros — the same shape as the array index that returned buffer filler. I now keep an initialisation
word per composite slot, outside the body, and **fault where you fault**: the unwritten path dies with
`SIGTRAP`, the written path agrees, and `14_frame_log.kel` still runs across four cycles.

**I did not refuse the shape**, and the reason is one of your programs: `14_frame_log.kel` writes its
slot inside `for i in 0..3` and reads after the loop. Nothing available to me proves that range
non-empty, so a sound definite-assignment analysis would refuse a program you accept and run
correctly.

**The question is yours**: is the flow-insensitive check the intended design, with the runtime fault
as the backstop, or should the compiler reject the conditional shape too? Either answer is
implementable here; I have assumed the first.

## A DECLARED INITIALIZER I NEVER APPLIED, AND THE AXIS THAT FOUND IT

```
private data log { count: Word = 7 }
fn main(t: Word, u: Word) -> Word { if t < 0 { log.count = 99; } log.count + u }

  write skipped : yours 8, mine 1
  write taken   : yours 100, mine 100
```

`private_init` carries the literal and you apply it at load. **I have no load step** — a host hands me
the buffer — so I never applied it, and every subject I had agreed because they all write before
reading.

**This one I looked for.** Four defects in a day shared a property: data that outlives something. I had
censuses for how an address is formed and where a value is moved; neither asks what is in memory
BEFORE I read it. Asking that of all sixteen read sites put four on the host boundary — and three of
those four are satisfied by a zeroed buffer, which is why the fourth hid. A plausible host is right
three times out of four.

`region::private_init_image` now states what a host must install, the same weaker guarantee as the
size figure I already publish. **Composite slots stay zero on purpose**: their initializer is `Unit`,
and my "never written" flag means what it means precisely because those bytes are zero.

## THE INDEXED COMPOSITE SLOT LOWERS, AND A SECOND HARNESS HAD THE FIRST ONE'S DEFECT

`log.items[i]` on an array-of-composite private field now lowers and agrees, at constant and runtime
indices. The stride is validated across the declared range rather than extrapolated from the direct
case, and **each element has its own initialisation word** — a lowering using the base slot's would
answer where you fault, and every test of the indexed path would still pass, since they all write the
element they read.

**The gate found a SIGSEGV and it was my own morning's defect, one harness over.** A differential
helper sized its private region as `vec![0u64; 8]` — a literal — and the indexed path was the first
subject there to write into the persistent pool. It passed alone, passed under the narrow
configuration, and died in the gate, because a literal-sized buffer fails by corrupting its neighbour
rather than by a stable observable. I had repaired exactly this in the corpus harness hours earlier.
**Fixing one instance of a class without looking for the others is the failure**, and both are now
contract-sized with canaries.

## THE DATA-SLOT STORY IS COMPLETE EXCEPT FOR ONE SHAPE

Private direct, private indexed, and shared direct composite slots all lower and agree. The indexed
SHARED form stays refused, because its layout entries are not proven contiguous and uniform and the
direct case's stride does not carry over.

**Your layout made the shared case the easy one.** `SharedSlotLayout` states the body length in a
field, so there was nothing to derive and nothing to validate — unlike the persistent pool, whose size
I have to infer from neighbouring offsets and therefore check partitions the range.

**And your two segments differ in a way I had to respect rather than unify.** A private composite
slot starts as `Unit`, so reading an unwritten one faults, and I keep an initialisation word per slot
to reproduce that. The shared segment has no such state — the host owns the buffer and you copy out
whatever is there — so I deliberately do NOT keep one, and a subject seeds your buffer and reads it
through a slot the program never wrote, which would fail if I ever invented that fault.

## STILL WITH YOU — RE-MEASURED 2026-09-11, NOT RECALLED

**All three still reproduce**, checked against your tree today rather than remembered from an
absorption scan. They are now WATCHED: `outstanding_reports.rs` fails if any of them stops
reproducing, and each message says to retract the report rather than to debug a test.

I built that after finding I had asserted, for four weeks, that your worst-case memory bound was
unsound **after you repaired it**. A report nobody re-checks becomes a standing accusation, and this
document is written to you.

| report | measured today |
|---|---|
| the `confine.rs` index panic | still fires; watched by `lowering_robustness.rs`, which allows it by origin file and asserts it has not gone |
| a multi-parameter stream after its rewind | still faults — `TypeError` on `Int and Unit`, after a RESET, so slot 1 is still left undefined |
| the flow-insensitive write-before-read check | unchanged: the unconditional shape is still rejected and the conditional one still accepted |


1. **A `confine.rs` index panic on a truncated op stream.** Three mutation kinds reach it, one guard
   closes all three. My sweep allows it BY ORIGIN FILE and asserts it still fires, so your fix will
   fail my test and delete the carve-out rather than let it outlive the defect.
2. **A multi-parameter stream faults after its first rewind.** Defect, intended consequence, or a
   shape the verifier should reject? Only the third needs no runtime change.
