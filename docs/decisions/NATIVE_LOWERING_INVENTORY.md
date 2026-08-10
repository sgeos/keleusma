# Native Lowering Inventory (V0.3.x Workstream A)

> **Navigation**: [Decisions](./README.md) | [V0.3.x Roadmap](../roadmap/V0_3_X_ROADMAP.md) | [V0.4.0 Architecture](../roadmap/V0_4_0_NATIVE_CODEGEN.md)

What `native_codegen/` lowers today, what remains, and what each remaining group
actually costs. Written 2026-08-08 at the point 22 of 66 opcodes lower.

This is a scoping document, not a design. Where a group needs a real design it
says so and stops, rather than sketching one that has not been probed.

## The target set is the whole instruction set

Measured rather than assumed: **all 66 `Op` variants are emitted by the
reference compiler.** There is no legacy or dead region to skip. The enumeration
compared the variants declared in `src/bytecode.rs` against `.emit(Op::…)` call
sites in `src/compiler.rs`; the only two variants absent from that pattern,
`If` and `Else`, are emitted through `emit_jump` because they carry back-patched
targets.

That check took three attempts, which is worth recording because each wrong
answer was confident and plausible. Matching `Op::[A-Z]` against the file caught
doc-comment references such as "Replaces single-slot `Op::Pop`", producing an
emitted set containing opcodes that do not exist. Adding a word boundary was
still wrong because `Op::` is a substring of `BinOp::`, `UnOp::` and
`ShiftOp::`. And a `head -12` on an intermediate grep produced the claim that
`Op::Add` is never emitted, which is false. **A grep is a measurement and
deserves the same scepticism as any other.**

## Status

**Lowered (46).** `GetLocal`, `SetLocal`, `PopN`, `Dup`, `Const` (scalars),
`PushImmediate`, `CheckedAdd`, `CheckedSub`, `CheckedNeg`, `CheckedMul(0)`,
`Div`, `Mod`, `CheckedDiv(0)`, `CheckedMod`, `CmpEq`, `CmpNe`, `CmpLt`, `CmpGt`,
`CmpLe`, `CmpGe`, `Not`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `If`, `Else`,
`EndIf`, `Loop`, `EndLoop`, `Break`, `BreakIf`, `Return`, `Trap`, `Call`, `WordToByte`, `ByteToWord`, `BoundsCheck`, `GetData`, `SetData`, `GetDataIndexed` and `SetDataIndexed` (shared scalar slots, and private slots on a flat native layout), `Yield` (reentrant chunks, callback ABI).

**Remaining (20),** grouped below by what they actually cost.

Three entries in that list are **partial**, and the count treats them as lowered
because the unsupported case is refused rather than mislowered: `Const` handles
scalar constants only, and `CheckedMul` and `CheckedDiv` handle a zero
fraction-bit count only. A reader taking the count as a completeness measure
would overstate coverage by three.

## CORRECTED: `Add`, `Sub`, `Mul` and `Neg` are not the integer opcodes

Group 1 below previously described `Add`, `Sub`, `Mul` and `Neg` as "direct LLVM
integer operations" needing only a check of the overflow behaviour. **That
premise is false**, and acting on it would have produced four lowerings that no
compiler output ever reaches.

Consolidation B narrowed all four away from `Int` operands. The reference
compiler emits, at `src/compiler.rs` around line 8900:

| Source | Opcodes |
|---|---|
| `a + b` on `Word` | `CheckedAdd; PopN(2)` |
| `a - b` on `Word` | `CheckedSub; PopN(2)` |
| `a * b` on `Word` | `CheckedMul(0); PopN(2)` |
| `-a` on `Word` | `CheckedNeg; PopN(2)` |

`Op::Add`, `Op::Sub`, `Op::Mul` and `Op::Neg` are emitted **only** when the
operand type is explicitly `Byte`, `Fixed` or `Float`, and the VM raises a type
error if an `Int` reaches any of them. So the whole `Word` arithmetic surface is
the checked family, and it is now complete. The four unchecked opcodes move to a
blocked status: they need the `Byte` representation settled, and `Fixed` and
`Float` belong to Group 4.

This also answers a Group 2 question recorded as unprobed. **`CheckedMul`'s `u8`
operand is the Q-format fraction-bit count.** Zero is exactly integer multiply,
which is what the compiler emits for `Word`; a non-zero count is fixed-point and
is refused rather than lowered as if the operand were absent.

Verified by dumping opcode streams from the reference compiler, not by reading
opcode names. The names are actively misleading here, which is the whole reason
this section exists.

## CORRECTED: `i64::MIN / -1` does not trap, and the four division opcodes differ

Group 2 previously said of the division family that "the same trap applies to
`i64::MIN / -1`". **It does not.** Confirmed by executing the VM rather than by
reading the arm:

| | zero divisor | `i64::MIN` by `-1` |
|---|---|---|
| `Div` | `VmError::DivisionByZero` | `i64::MIN`, **no fault** |
| `Mod` | `VmError::DivisionByZero` | `0`, **no fault** |
| `CheckedDiv(0)` | flag `3`, numerator in low | flag `1`, low `i64::MIN` |
| `CheckedMod` | flag `3`, numerator in low | flag `0`, low `0` |

Both inputs are undefined behaviour in LLVM and neither is undefined in the VM,
so both need excluding — but only one of them by a trap. The lowering forces the
divisor to `1` for the `i64::MIN / -1` case, which is exact rather than
approximate for both opcodes: `sdiv(i64::MIN, 1)` is `i64::MIN` and
`srem(i64::MIN, 1)` is `0`, which are the two answers wanted. The checked forms
need no such substitution because the division is performed in 128 bits, where
`2^63` is representable, and that is precisely how the VM arrives at flag `1`.

Division and modulo also truncate toward zero in both, not floor: `-7 / 2` is
`-3` and `-7 % 2` is `-1`. Had either language floored, a lowering that assumed
agreement would be wrong on every negative dividend, which is a large fraction of
real inputs rather than a corner.

## An opcode being emitted does not mean its operands are

The Status section above rested on a measured claim: all 66 opcodes are emitted
by the reference compiler. True, and it hid something.

`PushImmediate` is emitted. Its **operand space is not**. Probed across 16 source
shapes covering literals, tuples, arrays, structs, enums, matches, calls, shifts,
bounded loops and handled arithmetic, the only operands the compiler emits are
`0` (`Unit`) and `1`/`2` (the boolean literals). **Every integer literal,
including `0` through `15`, routes through `Const` and the constant pool
instead**, so the documented `4..=19 = Int(operand - 4)` encoding has no
reachable caller.

The consequence is that the lowering's decode of that range carried an
arithmetic offset that nothing exercised. An off-by-one would have been
invisible, and the test named `small_integer_literals_agree_with_the_vm` looks
from its title as though it covered exactly this. It does not; it covers the
constant pool.

It is now tested by **rewriting real bytecode** — substituting `PushImmediate`
for a `Const` load in a compiled module — which the VM accepts through its
ordinary verified path rather than through `new_unchecked`, so the oracle is
genuine. The same technique reaches any opcode or operand the compiler declines
to emit, and it is the general answer to this class of gap.

**Generalisation worth carrying**: an enumeration over opcodes answers "is this
instruction reachable", not "is this instruction's behaviour covered". For any
opcode carrying an operand, ask the second question separately.

## The high word and the flag were unobserved until 2026-08-09

Worth recording as a class of gap rather than as one fixed bug. Every arithmetic
case in the suite went through `Checked*; PopN(2)`, which discards the flag and
the high word. A lowering that computed either incorrectly — or pushed the
triple in the wrong order — passed the entire suite. The opcode was "supported"
and two thirds of its output were untested.

The handled form `a * b { ok(v) => v, overflow(h, l) => h, ... }` makes both
observable, and probing showed it needs **no opcode outside the existing
subset**: it lowers as a dispatch on the flag built from `Loop`, `CmpEq`, `If`
and `Break`. It had been available the entire time.

The general form of the lesson: **an opcode that pushes more than it is usually
asked for has untested outputs by default.** Ask what a passing suite would
still permit to be wrong.

## DONE: the structural increment (loops)

`Loop`, `EndLoop`, `Break` and `BreakIf` introduce **backward jumps**, which the
original lowering could not express. **This is now implemented and passing.**
The design below is retained because it proved correct in every particular. Its merge-depth algorithm walks the opcode
stream linearly and requires every incoming edge to a block to have been
recorded before that block is entered. That holds for `If`/`Else`/`EndIf`, whose
targets are always forward. It does not hold for a loop back edge.

This is the gate on almost everything else. Programs without iteration are a
small corner of the language, so widening the arithmetic or composite surface
before loops work buys little. **Loops before more opcodes.**

The fix is not large. The depth at a loop header is known when the header is
first entered, so the back edge can assert agreement rather than establish it.

### The semantics, read from the VM rather than the opcode comments

| Opcode | Runtime behaviour |
|---|---|
| `Loop(exit)` | **No-op.** The operand is the index past the matching `EndLoop`, carried for `Break` and `BreakIf`. |
| `EndLoop(head)` | **Unconditional** back edge to the instruction after the matching `Loop`. |
| `Break(exit)` | Unconditional forward jump past the enclosing `EndLoop`. |
| `BreakIf(exit)` | Pop a bool; jump to `exit` if true, fall through if false. |

The consequence worth stating: because `EndLoop` is unconditional, **a loop exits
only through `Break` or `BreakIf`.** That is the productive-divergence construct
behaving exactly as designed, and it means the exit block is never reached by
fall-through.

### The lowering, worked out

Collect block targets from `EndLoop`, `Break` and `BreakIf` in addition to `If`
and `Else`. Then:

- `Loop(_)` emits nothing. The following index is a block because it is
  `EndLoop`'s operand, so the ordinary fall-through path opens it and records
  its depth.
- `EndLoop(head)` asserts depth agreement against the already-recorded entry for
  `head` and branches. **This is the whole back-edge problem**: the header's
  depth was established on first entry, so the back edge only has to agree with
  it, which the existing assertion already does.
- `Break(exit)` records and branches. `BreakIf(exit)` pops, records, and emits a
  conditional branch into a fresh fall-through block.

**Two edge cases that must be handled rather than discovered.** A loop with no
`break` leaves its exit block with no incoming edge, so restoring depth from the
recorded map would panic on a legitimate program. And any code following such a
loop is unreachable, so the current block already carries a terminator when the
next opcode arrives. Both want the same answer: when a block has no recorded
incoming edge, it is unreachable — emit `unreachable` into it and set depth to
zero, rather than treating it as a lowering bug.

Do **not** collect `Loop`'s own exit operand as a block target. It duplicates
the `Break` targets when a break exists and manufactures an unreachable block
when one does not.

### CORRECTED: a range `for` IS the test vehicle, and it lowers today

An earlier revision of this document claimed a `for` statement drags in
`Stream`, `Reset`, `Yield`, `NewComposite`, `Len`, `GetIndex`, `IsEnum`,
`IsStruct` and `GetField`, and therefore could not exercise loops in isolation.
**That was wrong.** It came from counting `Op::` occurrences across the whole of
`compile_for`, which includes the paths for iterating over composites. Measured
against real output instead, `for i in 0..3 { }` emits only:

```
Const SetLocal Loop GetLocal CmpGe BreakIf PushImmediate PopN CheckedAdd EndLoop Return
```

No coroutines, no composites. It is the canonical counted loop, and it now
lowers and passes differentially.

The keyword is still overloaded three ways, which is worth keeping:

| Form | What it is |
|---|---|
| `loop { block }` | The divergent loop block. |
| `loop name(..) -> T` | A **coroutine definition**, not a loop at all. |
| `for x in it [limit n] on { .. }` | Bounded iteration. |

### What the verifier admits, which is narrower than what parses

A data-dependent `break` is **rejected**, not by the lowering but by
`verify_resource_bounds`:

> `main: loop at instruction 2 has no statically extractable iteration bound;
> strict mode requires loops with fall-through bodies to match the canonical
> for-range pattern`

So `loop { if x > b { break; } }` compiles and then fails verification. The
admitted forms are the range `for`, and a `loop` whose break is unconditional
and therefore trivially bounded. This is the conservative-verification stance
working exactly as designed, and it means loop test programs must be chosen from
what verifies rather than from what parses.

### Locals are immutable, which bounds what a loop test can observe

`s = s + b` inside a loop is rejected with "assignment is only supported for data
block fields". Accumulating across iterations therefore needs a data block,
which is a later increment. **Consequence: with the current subset a loop's
iteration count is not observable**, so a differential test alone would pass
against a lowering that dropped the loop entirely. That is why
`the_range_for_lowering_actually_emits_a_back_edge` asserts the cycle directly.

## Group 1 — mechanical, no new design

Same shape as what already lowers. Each needs differential cases that
distinguish it, not merely exercise it.

| Opcode | Note |
|---|---|
| ~~`CheckedSub` `CheckedNeg`~~ | **DONE.** The three-slot `low, high, flag` pattern, shared with `CheckedAdd` and `CheckedMul(0)` through one helper. |
| `Add` `Sub` `Mul` `Neg` | **BLOCKED, and not what they look like.** See the correction above: these carry no `Int` operands and are reachable only for `Byte`, `Fixed` and `Float`. They wait on the `Byte` representation and on Group 4. |
| ~~`PushImmediate(u8)`~~ | **DONE, partially, and this row contradicted the Status list above for two increments.** `Unit`, both booleans and the inline integers lower; `None` and the reserved operands are refused, which was the "legitimate first answer" this row proposed. See the note below on what the compiler actually emits. |
| `WordToByte` `ByteToWord` | Truncate and extend. Needs the `Byte` representation settled, including whether the extension is signed. |

## Group 2 — one design decision each

| Opcode | The decision |
|---|---|
| ~~`Div` `Mod` `CheckedDiv(0)` `CheckedMod`~~ | **DONE.** See the correction below; the entry that stood here was wrong about `i64::MIN / -1`. |
| ~~`CheckedMul(u8)`~~ | **DONE for `0`.** The operand is the Q-format fraction-bit count; zero is integer multiply. A non-zero count is fixed-point and is refused. |
| `Const(u16)` | Scalar constants are easy. Composite constants are not, and route into Group 4. |
| `BoundsCheck(u16)` | A compare and a branch to trap. Cheap once the trap path carries a reason code. |
| ~~`Loop` `EndLoop` `Break` `BreakIf`~~ | **DONE.** The structural work above. |

## Group 3 — the host and call boundary

| Opcode | Depends on |
|---|---|
| ~~`Call(u16, u8)`~~ | **DONE** via `lower_module`, which declares every chunk before lowering any body so a call can reach a chunk declared later. Symbols are `kel_chunk_<index>`, deliberately NOT the R4.2 scheme: that encodes purity, category, module path and type arguments for EXTERNAL linkage and needs metadata a `Chunk` does not carry. Nothing is externally linked yet, so a provisional name is more honest than a half-implemented mangling that looks authoritative. A short call relying on the VM's Unit-fill convention is refused rather than approximated. |
| `CallVerifiedNative` `CallExternalNative` | The native application binary interface, Workstream D. Not a Workstream A item. |

## Group 4 — the workstreams that own them

These are not deferred out of convenience; each belongs to a workstream with its
own design.

- **`Stream`, `Yield`, `Reset`** — Workstream B, sub-coroutine lowering. The
  roadmap identifies this as where the risk concentrates, and it is the
  load-bearing primitive for V0.5.0. **The mechanism is now probed and works at
  both the LLVM and the binding layer**; see the Workstream B section below,
  including the correction that BOTH intrinsic families are available and the
  switched-resume form is the one demonstrated.
- **`NewComposite`, `GetField`, `GetIndex`, `GetTupleField`, `GetEnumField`,
  `Len`, `IsEnum`, `IsStruct`** — the flat byte composite representation and
  arena residency, Workstream C.
- **`GetData`, `SetData`, `GetDataIndexed`, `SetDataIndexed`** — the data
  segment, including the shared region's C-representable layout, Workstream D.
- **`IntToFloat`, `FloatToInt`, `WordToFixed`, `FixedToWord`, `FixedMul`,
  `FixedDiv`** — float and fixed-point. Float support interacts with the target
  descriptor and is not obviously a Workstream A item.

## Two analyses that are open and not started

### The native stack bound does not follow from the arena

Arenas bound dynamic allocation. They do not bound the machine stack, because
LLVM's register allocator spills to it regardless of the heap model, and LLVM
chooses frame sizes. The worst-case-memory-usage bound proven on bytecode
describes the arena, not the frame layout of code that does not exist yet.

The tractable route exists and rests on a property the language already has.
**The verifier forbids recursion**, so the call graph is acyclic, and maximum
stack depth is the longest weighted path through it with LLVM's per-function
frame sizes as weights. That is statically computable, but it is a separate
analysis from the arena bound and it needs frame sizes extracted from LLVM.

#### Probed 2026-08-08: the mechanism exists, with two constraints

LLVM can emit a `.stack_sizes` section pairing each function with its frame
size. Measured on this machine against LLVM 22.1.8, with a two-function module
whose first function carries a 2048-byte `alloca`:

| Target | `.stack_sizes` | Decoded |
|---|---|---|
| `arm64-apple-darwin` (Mach-O) | **absent** | — |
| `aarch64-unknown-linux-gnu` | present, 19 bytes | 2064 and 0 |
| `thumbv7em-none-eabihf` | present, 11 bytes | — |

The 2064 is 2048 plus 16 for saved registers and alignment, and the leaf
function reports 0, so the values are real rather than merely present.

**Constraint one: it is not reachable through the bindings.** Neither `inkwell`
0.9 nor `llvm-sys` 221 exposes it, and it is absent from the `llvm-c` headers,
so it cannot be enabled in process. The route is to emit intermediate
representation and drive `llc --stack-size-section` as a subprocess. That forces
an out-of-process step into any toolchain that must produce a stack bound, which
is worth knowing before Workstream D designs the bounds-as-linkable-contract
deliverable.

**Constraint two: it is ELF-only.** The macOS development host cannot produce
it at all. Fortunately the constraint falls the right way: it works on the
Cortex-M class target, which is exactly where the interrupt-handler deliverable
needs it and where a stack overflow is unrecoverable.

Function identity needs the accompanying `.rela.stack_sizes` relocations, since
addresses in an unlinked object all read zero.

This matters most for the interrupt-handler deliverable in Workstream D, where
the roadmap names stack size as one of the hard problems Keleusma claims as a
strength.

#### Measured 2026-08-09 on real lowered programs: the pipeline decides the number

The 2026-08-08 probe used a synthetic module with a hand-written `alloca`. This
one runs the actual lowering over four Keleusma programs, and it changes the
conclusion in a way the synthetic probe could not have shown.

Frame size in bytes for `kel_entry`, `thumbv7em-none-eabihf`:

| Program | `llc -O2` alone | `opt -O1` then `llc -O2` |
|---|---|---|
| `a + b` | 536 | **0** |
| `if a > b { a * b } else { b - a }` | 552 | **16** |
| `for i in 0..10 { } a / b` | 552 | **8** |
| handled multiply with both arms | 616 | **20** |

`opt -passes=mem2reg` alone accounts for the entire difference; after it, zero
allocas remain. The optimisation level passed to `llc` is irrelevant, measured at
`-O0`, `-O1`, `-O2` and `-Os`, which all give the same 536.

**The finding: the frame is decided by which tool runs, not by the optimisation
level.** `mem2reg` is a middle-end pass. `llc -O2` is an optimisation level above
none and does not run it. 512 of those 536 bytes are `MAX_STACK` operand slots
the program never touches.

Three consequences, in descending order of how badly they bite:

1. **A native worst-case-memory bound computed on an `llc`-only pipeline is
   wrong by up to 30x**, and wrong in the unsafe direction only if you trust the
   small number. It is a half-kilobyte per function on a part that may have four
   kilobytes of stack in total.
2. The lowering's own documentation asserted that `mem2reg` runs "at any
   optimisation level above none". That was false and is corrected. It is the
   kind of claim that reads as a fact about LLVM and is actually a fact about a
   particular tool invocation.
3. The 64-slot provisioning is what dominates the unoptimised frame, and the
   verifier already computes the true figure as
   `RuntimeFootprint::max_operand_slots`. Sizing the slot array from it, rather
   than from a constant, is the obvious follow-up and is not done yet.

**Also fixed while measuring this**: exceeding `MAX_STACK` panicked on a `Vec`
index inside a library. Its doc comment asserted that exceeding it "is a lowering
bug, not a program error", which was an assumption with nothing enforcing it. It
is now `LowerError::OperandStackTooDeep`, a refusal like any other, which is what
lets a caller raise the provisioning deliberately.

**Still open**: whether the frame sizes are stable enough across LLVM versions to
be a contract, and how `.rela.stack_sizes` is read for function identity in a
multi-function module. Neither is answered here, and nothing above should be read
as establishing that a native worst-case-memory bound is achievable end to end —
only that the per-function number is obtainable and that obtaining it wrongly is
easy.

### Whether the worst-case execution time bound transfers

Recorded here because it bounds what the artefact may claim, and it is listed as
open decision 1 in the V0.3.x roadmap.

The domination argument is sound: if native execution is faster than
interpretation, the proven bytecode bound is a conservative upper bound for
native, with no new proof needed. Three conditions attach. Domination must hold
per operation rather than on aggregate. The comparison is valid only on the
target the cost model was calibrated for. And instruction-cache behaviour is
where it can genuinely fail, on small embedded targets where a compact
interpreter loop can outperform an expanded native binary — which is the Tier 2
set, not Tier 1.

Bounded productivity is a stronger and separate claim. It is a semantic property
and transfers under semantics-preserving lowering. The precision worth keeping:
productivity is a **liveness** property, and liveness is not preserved by
refinement in the trace-inclusion sense, which preserves safety. Carrying it
requires the lowering to preserve observable behaviour over infinite and
reactive traces, not merely terminating ones. **The practical consequence is for
the oracle**: a differential test that runs programs to completion never
exercises the property productivity depends on, so the coroutine cases must
compare traces prefix-wise across suspensions.

## What governs the whole of this

The differential oracle is the correctness signal, and it is not a formality.
The first version of the lowering carried a defect that one of two test inputs
passed straight through: `maxi(2, 3)` takes the else path and was correct, while
`maxi(9, 4)` takes the then path and was not. When adding an opcode, add inputs
that distinguish its paths and satisfy yourself that each case can actually
fail.

### Vocabulary, because "negative control" collided with itself

This document used "negative control" in two opposite senses, and so did the
tests. Both are replaced by an unambiguous pair, and the same pair is now used on
the `v0.2.3` line:

| Name | Input | Catches a check that is |
|---|---|---|
| **must-fire case** | defect known PRESENT | too STRICT (never fires) |
| **must-not-fire case** | defect known ABSENT | too LOOSE (fires spuriously) |

The collision was not harmless. It concealed a coverage gap: every structural
check here had a must-fire case and only one had a must-not-fire case, while the
term "control" made all of them look equally covered.

### Must-fire and must-not-fire cases, run 2026-08-08, and what they found

The whole suite passed on its first execution, having been written without a
compiler. That is exactly when a control is skipped and exactly when it pays.
Three were run by breaking the lowering deliberately. **Two of the tests turned
out to be vacuous.**

**The comparison test could not distinguish its own subject.** Its sources were
`if a OP b { a } else { b }`, and swapping `SLT` for `SLE` left it passing. The
two predicates differ only at `a == b`, and at `a == b` both branches return the
same value. The test comment claimed the equal case was what discriminated. It
was not. Fixed by making the branches asymmetric, `{ a } else { b + b }`, after
which the swap fails in both directions.

**The shift-mask test cannot work behaviourally on this target at all.**
Removing the mask entirely left every case passing, because AArch64's
shift-by-register instruction masks the count to its low six bits in hardware.
The mask is still required — poison is a compile-time licence to the optimiser,
not a promise about the instruction — but no runtime comparison can demonstrate
its presence here. Replaced with a structural assertion on the emitted
intermediate representation.

**And the first structural assertion was vacuous in turn.** It checked that the
intermediate representation contained `and i64`, and passed its own control,
because removing the mask from the shift *operand* leaves the `and` computed and
merely unused. Asserting that a value exists says nothing about whether anything
reads it. The assertion now locates the shift instruction and requires it to
name the mask.

That is three layers: a defect, a test blind to it, and a fix blind in the same
way. Each was found by the same cheap act of breaking the code on purpose.
**Run the control even when — especially when — you are confident.**

### And a fourth, from the loop increment: four predicates, all wrong

`has_back_edge`, the assertion that the counted-loop lowering emits a cycle, took
**four attempts**, and the first three passed their control while being wrong.

1. `strip_suffix(':')` to find block labels. LLVM prints `op5:  ; preds = ...`,
   so it recorded only `entry` and reported no back edge for a real loop.
2. "branches to an earlier-defined block". Reported true for any branch to the
   `trap` block, which is emitted near the top.
3. "a predecessor defined later in the text". Same block, same cause, opposite
   direction.
4. Build the graph from the `preds` annotations and look for a cycle. Sound.

**The lesson is not "write better heuristics".** It is that loop structure is a
graph property and is not recoverable from the order blocks happen to be
printed in. Three attempts were spent approximating a graph with text position.

A second lesson, and it now has a mechanism rather than being an observation.
A **passing test rests on an unexecuted precondition** — that the check measures
what you believe it measures — and a must-fire case is the act of executing that
precondition. So "run the control" and "check your preconditions" were never two
rules.

The corollary follows from what a must-fire case *does*. It executes the
precondition in **one direction only**: it shows the check can fire when it
should. It cannot show the check fires only when it should. That is why attempt
(1), which never fired at all, walked straight through. **Both halves belong in
the test**, and both are now encoded rather than run once by hand in a shell.

## A self-check on the OPCODE is not a self-check on the REASON

The subset-boundary test had rotted twice, passing for the wrong reason each
time an opcode it named entered the subset. It was made self-checking: it now
asserts that its chosen source really emits the opcode it claims to be testing.

**That was not enough, and `Op::Call` proved it within one increment.** Calls
became supported through `lower_module` while the single-chunk `lower_chunk`
entry point still refused them, for a reason that had nothing to do with the
subset — it cannot resolve a chunk index it was never given. The test went on
passing. Its self-check confirmed the opcode was present and said nothing about
why the refusal happened.

The fix is a must-not-fire case on the OTHER entry point: the whole-module path
must refuse it too. A refusal that only one entry point makes is not evidence
that an opcode is unsupported. Generalised: when a check asserts that something
fails, pin down what it fails *because of*, or the check survives the condition
it was written to detect.

## Workstream B probed 2026-08-09: the coroutine mechanism works, with three findings

`V0_3_X_ROADMAP.md` calls sub-coroutine lowering "the load-bearing primitive"
and "where the risk concentrates", and it had never been probed. It is now, at
both layers, because the `.stack_sizes` probe showed that the LLVM layer working
says nothing about the binding layer.

**At the LLVM layer it works end to end.** A switched-resume coroutine verifies,
splits into ramp, `.resume`, `.destroy` and `.cleanup`, consumes every coroutine
intrinsic, and **executes**: driven under `lli` it yielded three successive
values across two resumes and destroyed cleanly. All five coroutine passes are
registered in LLVM 22.1.8.

**At the binding layer it also works, and the obvious inference was wrong.**
Every intrinsic that opens or closes a coroutine traffics in LLVM's `token`
type, and inkwell has no token type — it panics with "FIXME: Unsupported type:
Token" if one reaches its type enum. The natural conclusion is that coroutines
are unreachable from this backend. **That conclusion is false.**
`Intrinsic::find` plus `get_declaration` asks LLVM to construct the signature
from the intrinsic's own definition, so no Rust code ever names the token type.
All nine intrinsics declare successfully, both returned-continuation forms
(`llvm.coro.id.retcon` and `.retcon.once`) are present, and the pass pipeline
runs through `Module::run_passes`. Recorded because I nearly wrote the opposite
down as a finding before testing it.

Three results that bear on other workstreams:

1. **The frame allocator survives `coro-split`.** The probe deliberately used a
   named external allocator rather than `malloc`, and the split output still
   calls it. **Workstream C's arena-resident coroutine frames are mechanically
   supported**, not merely desirable.
2. **The frame size folds to a compile-time constant.** After splitting, the
   allocation reads `call ptr @kel_arena_alloc(i32 32)` — a literal. A
   coroutine's memory contribution is therefore statically recoverable from the
   emitted IR, which is what **Workstream E** needs for a native worst-case
   memory bound. This does not by itself establish the bound; it establishes
   that the input to one is obtainable.
3. **`llvm.coro.end` returns `void` in LLVM 22, not `i1`.** Widely published
   examples use `i1` and fail verification with "Intrinsic has incorrect return
   type". Cheap to hit and confusing, because the diagnostic names the function
   rather than the signature.

**A correction to this document.** The Group 4 entry said Workstream B goes
"through the returned-continuation intrinsic family". Both families are
available, and the form demonstrated end to end here is the **switched-resume**
one, which maps more directly onto a host that calls resume repeatedly. The
returned-continuation form is an optimisation for one-shot coroutines, which is
how `V0_4_0_NATIVE_CODEGEN.md` open question 3 frames it. The choice is a design
decision, not an availability constraint.

**Not established**: that Keleusma's `Stream`/`Yield`/`Reset` map onto this
shape, that yielded values cross the boundary correctly, or that RESET semantics
survive. Those are Workstream B proper. What is established is that the
mechanism is reachable, behaves, and does not force `malloc`.

## Ahead-of-time linkage works, which bears on roadmap open decision 2

`V0_3_X_ROADMAP.md` success criterion 2 requires native artefacts to "link as
static libraries against a host", and open decision 2 asks whether V0.3.x is
ahead-of-time only or admits a just-in-time path. Every test in this package
went through the JIT, which answers neither question: the JIT never writes an
object file, never invokes a linker, and never crosses a platform calling
convention.

Both paths are now demonstrated. A Keleusma program compiles, lowers, optimises
through `default<O2>`, writes a genuine object file, links against a C `main`
with the system linker, executes as a separate process, and **agrees with the
VM** across five argument pairs including the wrapping corner. The program spans
a branch, a counted loop and a cross-function call, so the internal call resolves
within the object rather than through JIT symbol lookup.

**The contribution to open decision 2 is that it is not a feasibility question.**
Both shapes work today on this target. The decision is about what to support and
maintain, not about what is achievable, and the roadmap can be narrowed on those
grounds.

Three things only this path exercises, which is why it is not redundant with the
JIT tests: the platform calling convention at a real boundary, external symbol
emission and linkage as distinct from JIT symbol lookup, and the optimisation
pipeline that actually ships. The JIT tests run at `OptimizationLevel::None`, so
they never run the middle end whose absence costs 30x of stack frame.

**Two claims of mine that this falsified**, both written confidently and neither
checked before a mutation run:

1. I asserted in a comment that a non-position-independent object "fails at `ld`
   with a relocation error". **It does not**, on arm64 macOS: `RelocMode::Static`
   links and runs. PIC is retained because it is right for the committed target
   set, not because the alternative was observed to break. Untested on the Linux
   and embedded targets.
2. The test's callee was `fn scale(x, k) -> x * k`, called as `scale(a, 3)`. A
   must-fire case that dropped the argument reversal in the lowering **left the
   test passing, because multiplication is commutative**. The test read as though
   it covered argument order across a linked boundary and could not have caught a
   swap. The callee now subtracts and the same mutation fails it.

The second is the **third vacuous test of this arc**, after the comparison test
whose branches returned the same value and the checked-division test whose two
arms bound the same slot. All three were found by mutation and none by reading.
The pattern is identical every time: a symmetry in the test data conceals an
asymmetry in the code. **Choosing test data that is asymmetric under every
operation the code could confuse is not a refinement, it is the difference
between a test and a decoration.**

## Workstream B: the opcode shapes, measured, and the one decision that blocks it

The mechanism probe above established that LLVM coroutines work and are
reachable. This is the other half: what the compiler actually emits, which had
not been written down anywhere.

**A `yield` function is `BlockType::Reentrant`** and must contain a `yield`
*expression*; the keyword is both a function category and an expression, and a
category without an expression is rejected by structural verification.

```
yield step(a: Word) -> Word { yield a }
  0 GetLocal(0)   1 Yield   2 Return
```

**A `loop` function is `BlockType::Stream`.**

```
loop main(a: Word) -> Word { let x = yield a; x }
  0 Stream   1 GetLocal(0)   2 Yield   3 SetLocal(1)
  4 GetLocal(1)   5 PopN(1)   6 Reset
```

Three things this settles, none of which is obvious from the opcode names:

1. **`Yield` is pop-one, push-one.** It pops the yielded value and pushes the
   resume value. `Return` at index 2 above pops exactly one operand, and
   `SetLocal(1)` at index 3 stores the resume value. A lowering that treated it
   as pop-only would underflow at the next instruction.
2. **`Stream` is a label, not an operation.** Its VM arm is literally a no-op;
   its whole purpose is to be the instruction `Reset` branches back to. It maps
   to a basic block boundary and nothing else.
3. **`Reset` is a backward branch plus state clearing**, and it is *observable*:
   it clears locals to `Unit`, truncates the operand stack, resets both arena
   bump pointers, and returns `VmState::Reset` to the host. A native lowering
   that treated it as a plain jump would preserve state the VM discards.

### The mapping, and the decision that blocks writing it

`Stream` becomes the resume target block. `Yield` becomes `llvm.coro.suspend`,
with the popped value delivered to the host and the resume value pushed on the
continuation edge. `Reset` clears the locals and branches to the `Stream` block.
The frame carries the locals, which is exactly what `coro-split` already does,
and its size folds to a constant as measured above.

**What blocks implementation is not the mechanism, it is the host ABI**: how a
yielded value reaches the host and how a resume value is supplied. The VM's
answer is `VmState::Yielded(v)` from `call`, and `resume(v)` back. Native has at
least three candidate shapes — a host callback per yield, an out-parameter the
ramp writes, or a returned continuation carrying the value — and they are not
interchangeable, because the differential oracle has to observe the same yield
SEQUENCE either way.

That is a **Workstream D** decision (host ABI), and Workstream B should not be
written against a guess at it. The mechanism is proven, the opcode shapes are
measured, and the remaining blocker is a decision rather than a risk.

## The `Byte` representation is settled; the unchecked opcodes are blocked on something else

`WordToByte`, `ByteToWord` and `BoundsCheck` now lower. Two corrections follow.

**The `Byte` representation is decided, and not by me.** The `v0.2.3` session
measured that `Byte as Word` **zero-extends**, so `0xFF` reads as `255` rather
than `-1`. A `Byte` therefore occupies a full `i64` slot holding `0..=255`, which
makes `WordToByte` a mask and **`ByteToWord` a genuine no-op**. It is written as
a no-op rather than as a redundant mask deliberately: if it ever needs to do
work, the invariant has been broken somewhere else and masking here would hide
that rather than fix it. This is the inventory item Group 1 recorded as
"needs the `Byte` representation settled, including whether the extension is
signed", and it was closed by a measurement from the other branch.

**But that does NOT unblock `Add`, `Sub`, `Mul` and `Neg`, and the reason has
changed.** Those four are reachable only for `Byte`, `Fixed` and `Float`. With
`Byte` now settled, the remaining obstacle is that **the opcode does not say
which type its operands are**, and the lowerings differ: a `Byte` add promotes,
adds and masks to `0xFF`, while a `Fixed` add is a plain integer add of the raw
bits with no mask. Choosing wrongly is silently wrong for every input above 255.

So the blocker moved from "decide a representation" to "recover operand types",
which is a different and larger problem — the auxiliary body's per-chunk
signatures carry `WireShape` data, and the typed verifier already reconstructs
operand shapes for exactly this purpose. Reusing that is the route. The inventory
previously implied these were mechanical once `Byte` was decided; they are not.

**`BoundsCheck` is unreachable from any compilable source.** It is emitted only
for MULTI-LEVEL data-segment indexing, and only when the access is not
single-level, because a single-level access lets the trailing
`GetDataIndexed`/`SetDataIndexed` do the same check against a wider operand.
Array indexing does not emit it either — `GetIndex` carries its own check. So it
is tested by rewriting real bytecode, the same technique used for
`PushImmediate`.

Two properties of it that a plausible lowering gets wrong:

- **It PEEKS, it does not pop.** The VM reads `stack.last()` and leaves the
  operand for the indexing opcode that follows. A lowering that consumed it
  leaves the next instruction reading the wrong slot, and the failure shows up as
  a wrong VALUE rather than as an obvious stack error.
- **The guard must be an UNSIGNED compare.** The VM rejects `value < 0 ||
  value >= bound`; one `icmp uge` covers both, because a negative `i64`
  reinterpreted as unsigned is enormous. A signed compare accepts every negative
  index, and no in-range differential case would ever notice.

Both are pinned by must-fire cases that were run and fail.

## Measured 2026-08-09: the remaining work is ordered wrongly, and by two orders of magnitude

This document orders the remaining instructions by what each COSTS to implement.
That says nothing about what each one BLOCKS, and the two orderings are not the
same. Measured over the shipped corpus, fifty-eight compilable programs, 496
chunks, 73,434 opcode instances:

| Measure | Value |
|---|---|
| Opcode instances lowered | **87.3 percent** |
| Chunks **fully** lowerable | **33.9 percent** |

The divergence is the whole point. A chunk with one unsupported opcode is
refused entirely, so instance-level coverage is not a capability measure. Eighty
seven percent is a number no consumer of this backend can use.

| Workstream | Blocking instances | Blocked chunks by first blocker |
|---|---|---|
| D, data segment | 7832 | **267** |
| D, native ABI | 1057 | 9 |
| C, composites | 331 | 28 |
| B, sub-coroutines | 98 | 24 |
| A, typed arithmetic | **0** | **0** |
| float / fixed-point | **0** | **0** |

**The data segment is 81 percent of blocked chunks.** It is the next increment,
and it was not previously identified as such anywhere in this document.

**The native ABI shows the instance-count trap in its clearest form**: second by
instances, fourth by blocked chunks, because its instances concentrate in large
chunks already blocked for other reasons.

### The zero that falsified a recommendation made one session earlier

`Add`, `Sub`, `Mul` and `Neg` — the class this document described as blocked on
operand type recovery — occur **zero times** in the entire corpus. One session
before this measurement I formally recommended operand type recovery as "clearly
the highest-leverage remaining work" and offered to scope a research spike.

Every step of that argument was correct: the four opcodes are reachable only for
`Byte`, `Fixed` and `Float`; the opcode does not record which; the lowerings
differ. **The conclusion was worthless.** The spike would have unblocked nothing.

The lesson is not "reason more carefully". The reasoning contained no invalid
step. It answered *what must be true before this can be implemented* and never
asked *how often does this occur*. Those are independent questions, and a
dependency argument cannot reveal the missing one from the inside. **Measure
frequency before ordering by structure.** The corpus check cost about twenty
minutes to write and two seconds to run.

Caveats, since the corpus is not a sample of the eventual target population: it
over-represents the self-hosted compiler, a text-processing workload with heavy
data-segment use, and under-represents the signal-processing and embedded
workloads the roadmap names. The zeroes are absence of evidence within THIS
corpus. Re-measure when the target population changes. The instrument is
`native_codegen/tests/spike_corpus_coverage.rs` and re-runs in two seconds.

## The data segment splits on a soundness boundary, and two thirds of it is blocked

The coverage spike identified the data segment as 81 percent of blocked chunks
and therefore the next increment. Probing it before writing any code found that
it is two workstreams wearing one name.

**Shared slots live in a host-owned byte buffer with a specified encoding.**
`read_shared_from_buffer` resolves a slot through a layout table to
`(offset, kind, len)` and decodes little-endian scalars of declared kind and
width. That encoding is part of the wire format, so native code may depend on
it.

**Private slots live in the arena as `GenericValue` records, and their layout is
not specified.** `GenericValue` is declared `#[derive(Debug, Clone)]` with **no
`#[repr]`**. Rust does not guarantee field order, discriminant size, or the
absence of niche optimisation for such a type. Native code that computed
`persistent_ptr + index * size_of::<GenericValue>()` and decoded a tag at a
fixed offset would be depending on an unspecified layout. **That is unsound
rather than merely fragile**, and no amount of differential testing would
establish otherwise, because a passing test only shows the layout happened to
match for one compiler invocation.

### Measured split, 2026-08-09

| | Instances | Share |
|---|---|---|
| Shared | 2561 | 32.7% |
| Private | 5271 | 67.3% |
| Total | 7832 | cross-checks against the coverage spike exactly |

But instance counts mislead, which is this document's own finding, so the unit
question was asked separately:

| | Chunks | Share of the 328 blocked |
|---|---|---|
| Unblocked by **shared-only** support | **53** | 16% |
| Data-only but need **private** slots too | 211 | 64% |
| Blocked by something other than data | 64 | 20% |

Shared is 33 percent of data instances and 16 percent of blocked chunks. **The
instance count overstates it by a factor of two**, inside a workstream that was
itself identified by correcting an instance-count error. The lesson recurs one
level down.

### What this means for sequencing

**Shared-only support is sound today and worth `+53` chunks**, taking unit
coverage from 33.9 percent to 44.6 percent. It needs one small decision, which
is how native code obtains the buffer base pointer, and that is a Workstream D
application-binary-interface question of the same size as the symbol-naming one
already settled provisionally.

**Private support is worth a further `+211` chunks**, reaching roughly 87
percent unit coverage.

### CORRECTED: private support is NOT blocked, and the error was mine

An earlier version of this section concluded that private slots were blocked on
giving `GenericValue` a stable `#[repr]`, called that "the single highest-value
decision available to the native workstream", and assigned it to the operator
and the runtime owner. **That was wrong.** The operator supplied the correct
model: private data is private to the code that relies on it, so it is a black
box from outside, and the only thing the outside needs to know is the size of
the box.

The reasoning error was treating the representation as an interface when it is
an implementation detail on both sides of a boundary neither crosses. Native
code has no obligation to read the Rust runtime's private storage, because
nothing observes that storage from outside. Verified rather than assumed:

- **The host API exposes `get_shared` and `set_shared` and no private
  equivalent.** Private slots are unreachable from outside the running program,
  so no external consumer can observe a layout choice.
- **The only quantity crossing the boundary is the size.**
  `required_persistent_capacity_for` computes
  `private_count * size_of::<GenericValue<W, F>>()` plus the composite pool.
- **That size over-approximates a flat native layout.** A `Value` slot is 32
  bytes; a flat native scalar slot is 8. Native code choosing its own layout
  fits inside the region the existing arithmetic already reserves, so neither
  the sizing function nor `src/bytecode.rs` nor the wire format needs to change.

The differential oracle is unaffected, because it compares returned values
rather than memory, and each side maintains its own private layout consistently
within its own run.

The worst-case-memory bound falls the right way for the same reason. The bound
proven over 32-byte tagged slots over-approximates the usage of a flat native
layout, so it stays conservative rather than becoming invalid.

### Three residual constraints on that model

1. **Mixed execution would break it.** If interpreted and native chunks ever
   touch the same persistent region within one run, the layouts must agree. The
   model requires native and bytecode to be alternative deployment shapes for a
   WHOLE PROGRAM rather than interleaved within one. The roadmap's phrasing
   supports that reading; nothing has been verified to enforce it.
2. **Hot swap inherits live data.** A swapped-in native artefact that must
   inherit a live private region requires the two versions to agree on layout.
   That is a versioning constraint within native, and the same one bytecode hot
   swap already carries.
3. **The size relationship needs a checked assertion, not an argument.** Native
   fits today because 8 is less than 32. That is a fact about two current
   choices rather than an invariant, and the lowering should assert it so a
   future wider native representation fails loudly instead of silently
   overrunning a region sized by someone else's arithmetic.

**The path to roughly 87 percent unit coverage therefore runs entirely through
`native_codegen/`** and requires no decision from the operator or the runtime
owner. The general lesson is the one this document keeps re-learning from a new
angle: a constraint that looks external is worth testing against the actual
boundary before it is escalated, because escalating it costs someone else's
attention and can be wrong.

## GROUND TRUTH, and a third level of the same conjunction

The coverage figures in this document were computed by a hand-written mirror of
the lowering. That mirror rots: it was written when 39 opcodes lowered and the
set has moved three times since, each move requiring an edit to a list the
lowering never reads.

`spike_report_modules_that_actually_lower` replaces the mirror with the real
entry point. It calls `lower_module` on every corpus program and counts what
succeeds, so it cannot drift. **Where it and the static classification disagree,
it is right.**

| Measure | Value |
|---|---|
| Opcode instances lowered | ~93% |
| Chunks fully lowerable | ~87% (projected) |
| **Whole modules that lower end to end** | **20.7%, 12 of 58** |

**The conjunction applies at three levels, not two.** An instruction is lowered
or not; a chunk lowers only if every instruction in it does; a module lowers only
if every chunk in it does. Each conjunction collapses the figure further, and
this document had been quoting the middle one while a consumer deploys the
outer one. Reporting 87 percent would have been the same category of error the
document already records for 87-against-34, one level up, in a section written
to warn against it.

### Remaining blockers, measured rather than assumed

| Blocker | Modules |
|---|---|
| `Stream` (Workstream B, sub-coroutines) | 21 |
| `NewComposite` (Workstream C, composites) | 18 |
| `CallVerifiedNative` (Workstream D, native ABI) | 3 |
| miscellaneous composite and enum access | 4 |

Coroutines lead by count and are blocked on the host application binary
interface decision recorded above. **Composites are the largest ADDRESSABLE
blocker** and are the next increment.

## Shared arrays: contiguity proven per module rather than assumed or refused

Indexed access to a shared array was previously refused, on the correct ground
that the layout table does not state a slot range is contiguous. Measuring
found **all 556,496 adjacent shared scalar pairs in the corpus contiguous, with
no exceptions**.

That is a property of today's compiler and not a wire guarantee, so three
responses were available: assume it and be silently wrong if the layout ever
changes, keep refusing and lose seven modules, or **prove it per module**. The
third is implemented. `resolve_shared_array` walks the range and requires
uniform kind and an exact stride, at a cost of one pass over `count` table
entries, converting an assumption into a checked precondition.

The guard then had no positive case, which a mutation exposed: disabling it
entirely left every test passing, because no layout the compiler emits violates
it. A test now rewrites the layout table to push one element off-stride and
requires refusal. This is the same technique used for `PushImmediate`'s
unreachable integer encoding, and the same lesson: **a defensive check with no
naturally occurring positive case is believed rather than tested until one is
manufactured.**

## CORRECTED AGAIN: composites need operand type recovery, which I called worthless

The previous section named composites the largest ADDRESSABLE blocker, at 18
modules, on the ground that coroutines were blocked on the host application
binary interface and composites were not. Probing the composite opcodes before
implementing found that conclusion too generous.

### What is baked, and what is not

`TupleField::Flat { offset, kind }` and its siblings bake **the byte offset and
the scalar kind**, so a field READ needs no type information beyond the
instruction. That half is straightforward.

`Op::NewComposite(Flat { kind, count, byte_size })` carries the COMPOSITE kind,
the number of values to pop, and the total allocation. It does **not** carry the
per-field widths. Packing is tight, in declaration order, at each field's
natural width, with nested composite bodies copied inline, so writing a body
requires knowing every field's width and the opcode does not say.

The tempting inference is that `byte_size == count * 8` implies uniform words.
**It is not sound.** A two-field tuple of a fifteen-byte nested composite plus a
`Byte` also totals sixteen bytes at a count of two. Any inference from the
aggregate to the parts admits that family of counterexamples.

### The correction

Operand type recovery was assessed earlier in this document and dismissed,
correctly, on the evidence that `Add`, `Sub`, `Mul` and `Neg` occur zero times
in the corpus. **The conclusion drawn was too broad.** The measurement showed
that type recovery is worthless FOR THAT INSTRUCTION CLASS. It does not show
that type recovery is worthless, and composites need it for a different reason:
not to choose between two lowerings of one instruction, but to compute a packing
offset that no instruction records.

So the correct statement is that operand type recovery is a prerequisite of
Workstream C, worth 18 modules, and was worth nothing for the four instructions
it was originally proposed for. Both halves of that are measurements, and the
error was in generalising the first into a dismissal.

`verify_typed::AbsVal` already distinguishes `Byte`, `Int`, `Fixed` and `Float`
at exactly the granularity packing needs, and `typed_check_chunk` computes the
per-instruction operand shapes and then discards them, returning only a verdict.
Exposing that state is the enabling change, and it is in `src/verify_typed.rs`,
which this branch does not own.

### Composites also do not escape the way the raw numbers suggest

Twenty-three of the twenty-seven chunks containing `NewComposite` declare a flat
composite return. That looks like a boundary problem and mostly is not: those are
HELPER chunks called by other chunks in the same module, and a native module
controls both sides of an internal call, so a composite may cross it as an offset
into a native-owned region. Only the module ENTRY returning a composite is a real
boundary, and that case can be refused from the signature table.

### Sequencing consequence

Neither of the two largest blockers is addressable by this branch alone.
Coroutines, at 21 modules, wait on the host application binary interface.
Composites, at 18, wait on operand shapes being exposed from the typed verifier.
The distinction from the earlier private-data case is real and worth stating:
that one LOOKED external and was not, and this one was checked against the same
boundary and is.

## CLOSED: a native stack bound, computed end to end

Workstream E's stack question is answered. `native_codegen/tests/native_stack_bound.rs`
computes a worst-case native stack bound from a Keleusma module, and two of the
three constraints recorded earlier turned out to be wrong or milder than stated.

**Function identity needs no relocation parsing.** An earlier note expected
`.rela.stack_sizes` to require hand-decoding because addresses in an unlinked
object read zero. `llvm-readobj --stack-sizes` resolves entries to symbol names
directly, so the whole concern was unfounded.

**`.stack_sizes` really is unreachable in process.** Confirmed from the other
direction this time: `TargetMachine::write_to_file` emits an object whose
`StackSizes` block is EMPTY, because inkwell cannot set `--stack-size-section`.
Driving `llc` out of process is the only route, and that subprocess is therefore
forced on any toolchain that must produce a stack bound.

**The optimisation pipeline must be `mem2reg` and NOT `default<O2>`.** This is
the finding worth carrying. The full pipeline inlines, and inlining dissolves the
call graph the longest-path traversal walks: on a three-function program
`default<O2>` reduced every reported frame to zero, because nothing survived as a
call. A bound computed from the bytecode call graph over post-inlining weights is
still conservative, since an inlined callee's needs fold into its caller's frame
and adding the callee's standalone figure only over-counts. **It is conservative
and useless: a bound of zero bounds nothing.** Promoting allocas without inlining
keeps the two graphs in correspondence.

Measured on `thumbv7em-none-eabihf`:

| Program | Frames | Bound |
|---|---|---|
| `main` calls `mid` calls `leaf` | 24, 24, 0 | **48 bytes** |
| single leaf function | 0 | 0 bytes |
| four-level chain | — | 40 bytes |

The traversal is bracketed by two envelopes rather than trusted: the bound must
be at least the largest single frame and at most the sum of all frames, so a
traversal that lost a level or double-counted a function escapes one of them. A
depth-sensitivity test pins that a deeper chain bounds higher, which a
depth-blind traversal would fail.

Acyclicity is not assumed. The type checker rejects direct and mutual recursion,
and the traversal still carries a visited set, so a cycle that slipped through
terminates and shows as an implausibly small bound rather than a hang.

**What remains open in Workstream E** is the execution-time half. Whether the
worst-case execution time bound proven on bytecode transfers to native code is
untouched, and it needs a quiet machine to measure, which has not been available.

## Workstream B may not need coroutine intrinsics for the common case

The roadmap calls sub-coroutine lowering "the load-bearing primitive" and "where
the risk concentrates", and the mechanism probe above confirmed LLVM coroutines
work and are reachable. Measuring the SHAPE of real stream chunks suggests most
of them do not need that machinery.

### The structural observation

**`Reset` clears every local to `Unit`.** Nothing carries across a reset in a
local slot, so the only state surviving an iteration lives in the data segment,
which now lowers. A suspension point that is immediately followed by a reset
therefore captures no live state, which is precisely the thing a coroutine frame
exists to hold.

### Measured over the corpus

| | Count |
|---|---|
| `Stream` chunks | 24 |
| ... with exactly **one** `Yield` | 22 |
| ... with nineteen yields | 1 |
| ... with none | 1 |
| Every `Yield` followed only by `PopN` then `Reset` | 8 |

The eight are directly a plain function: yield a value, discard the resume
value, reset. No frame is needed at all.

The remaining single-yield chunks have three to nine instructions between the
`Yield` and the `Reset`, which is the shape `let x = yield v; ...` produces:
compute `v`, suspend, bind the resume value, use it, reset. State still does not
cross the reset, so such a chunk is a **rotation** of a plain function. Running
the after-yield part with the previous resume value, then the before-yield part,
and returning the new yielded value is observationally the same sequence.

### What this would mean, and what it does not establish

If the rotation is sound, Workstream B splits into a large easy majority lowered
as ordinary functions driven by the host, and a small hard minority needing real
coroutine frames, of which this corpus contains one chunk with nineteen yields.
That would move the workstream the roadmap identifies as concentrating risk out
of the critical path for most programs.

**This is a hypothesis with a measurement behind it, not a result.** The
rotation is a program transformation and its equivalence has not been proven,
only argued from the fact that `Reset` clears locals. Three things would have to
hold and none has been checked: that no yielded value depends on a local written
after the previous yield, that the data segment is the only surviving state, and
that the trap and break edges out of the body preserve the rotation. The
nineteen-yield chunk is a counterexample to the single-yield precondition and
needs the coroutine path regardless.

Recorded because it changes what Workstream B should investigate first, and
because the cheap measurement that produced it took minutes against an
implementation that would have taken days and started from the wrong assumption.

### Two of the three rotation preconditions now checked

`native_codegen/tests/spike_stream_rotation.rs` checks the two preconditions
that are statically decidable. Both hold broadly.

| Precondition | Holds | Violated |
|---|---|---|
| P1: a `Reset` separates every pair of consecutive `Yield`s | 23 | 1 |
| P3: no `Return` or `Trap` between a `Yield` and its `Reset` | 24 | 0 |
| Both, and therefore a rotation candidate | **23** | — |

The single P1 violation is the nineteen-yield chunk already identified, which
needs a real coroutine frame regardless. P3 holds universally, so no path leaves
a stream body between suspending and resetting.

**The approximation is stated rather than buried.** The checks walk the linear
instruction stream and not the control-flow graph. For P1 that is conservative
in the useful direction: a `Reset` appearing textually between two yields might
be branched around, so a chunk reported safe COULD be unsafe, while a chunk
reported unsafe is unsafe. The figures are triage, and any implementation must
redo them over the graph.

**P2 was established by reading rather than measuring.** The runtime's `Reset`
handler clears every local to `Unit`, truncates the operand stack to the frame
base, and resets both arena bump pointers, so the data segment is the only
surviving state. That is a fact about the implementation, not a corpus
statistic.

**What remains unproven is the equivalence itself.** Rotating a loop body around
its suspension point is a program transformation, and the claim that it
preserves the observable yield sequence has been argued from the state analysis
above and not demonstrated. The natural demonstration is a differential test
over a MULTI-ITERATION sequence, comparing the yielded values from the virtual
machine against the native ones across many resumes. **No such test exists**, and
this package's oracle currently compares single returned values, so it would not
catch a rotation that produced the right values in the wrong order. Building that
harness is the precondition for attempting the transformation, and it is the next
thing Workstream B needs rather than any lowering work.

## The order-blind oracle is closed, and `Yield` lowers for reentrant chunks

The previous section named a multi-iteration differential harness as the
precondition for attempting the stream rotation, on the ground that a rotation is
a reordering and this package's oracle compared single returned values. That
harness now exists, in `native_codegen/tests/yield_sequence.rs`, and it collects
**every value a program yields, in order, together with the value it finally
returns**, comparing the sequences.

To give it something to drive, `Op::Yield` now lowers for reentrant chunks under
a **fourth provisional application binary interface decision**: `i64
kel_yield(i64)` takes the yielded value and returns the resume value. It inverts
control relative to the runtime, where `call` returns `Yielded(v)` and the host
calls `resume(r)`, and the observable SEQUENCE is identical, which is what the
oracle compares. `Op::Stream` and `Op::Reset` remain refused deliberately rather
than by omission: under an inverted ABI a divergent `loop fn` would spin inside
native code with no way for the host to stop it, and supporting it needs the
host-driven coroutine shape.

Two mutations confirm the harness sees what it was built to see. Pushing the
yielded value back instead of the reply, and eliding the suspension entirely,
each fail three of the four tests.

### The harness had a defect of its own, and it is the interesting part

The yield ABI is a plain `extern "C"` function with **no context parameter**, so
the callback has nowhere to record what it observed except process-global state.
`cargo test` runs tests in parallel, so one test's yields landed in another's
collection: the first version passed every test **in isolation** and failed two
of four when run together.

That failure mode is worth naming because it inverts the usual one. A test that
fails alone and passes in a suite is a flake. A test that passes alone and fails
in a suite is shared state, and the harness is at fault rather than the code
under test. The runs are now serialised by a lock, and **the absence of a context
pointer in the ABI is the underlying cause**, which is one concrete reason the
callback shape should not outlive the provisional label.

## Composites are TWO blockers, not one, and the ownership claim was untested

Measured 2026-08-09 by `native_codegen/tests/spike_composite_split.rs`, over 58
compiled corpus modules and 496 chunks. Three findings, one of which is a
correction to this document and one of which is a correction to the spike.

### The split, measured

Of the 27 chunks blocked **only** by the composite class:

| Population | Chunks | What it needs |
|---|---|---|
| Reads only | **5** | Nothing. No shape recovery of any kind. |
| A construction is present | **22** | Per-value widths |

The read half is genuinely free. `StructField::Flat { offset, kind }`,
`EnumField::Flat { offset, kind }`, `GetTupleField`, and `ArrayElem::Flat { kind }`
bake the displacement and the width; the nested forms bake `offset`, `size` and
`variant`. A read lowers to a load at a known offset of a known width, and needs
no abstract interpretation to get there. This document previously treated
composites as one 18-module item gated behind type recovery. **Five chunks were
never behind that gate.**

### The spike's first revision was wrong, and this document was right

The first revision classified a construction by whether
`count * word_bytes == byte_size` and reported **zero** chunks needing recovery,
with 22 "reachable by arithmetic". That contradicts the section above, which had
already rejected exactly that inference with exactly the right counterexample
family. **The recorded finding was correct and the new measurement was wrong**,
which is the reverse of this arc's usual direction and worth logging as such:
probing does not automatically beat a document, and a fresh number is not
evidence of a fresh truth.

Reading `Vm`'s handler supplies a second, sharper reason the inference fails,
beyond the counterexample already recorded:

- `pack_flat_in_arena` sizes a body as the **sum of per-value `flat_field_size`**,
  read from each runtime value's own kind. Widths are per value and never
  assumed uniform.
- For a `Tuple` or `Array` the VM passes `min_bytes = 0`, and its own comment
  calls the operand's `byte_size` **"the verifier annotation only"**. So for two
  of the four composite kinds the equality is compared against a number that
  does not describe the body at all.
- For a `Struct` or `Enum`, `byte_size` is only a **floor** that pads an enum to
  its widest variant, so equality does not pin the field breakdown either.

The equality is therefore neither necessary nor sufficient. It holds at 238 of
239 construction sites in the corpus, which is precisely what makes it dangerous:
it looks like a law and is a coincidence. `control_size_consistent_construction_still_needs_recovery`
encodes the rejection so the shortcut cannot be reintroduced silently.

### The ownership claim was an assumption, never tested

The resume prompt records the enabling change as living in `src/verify_typed.rs`,
"which this branch does not own". Measured:

```
git log <merge-base>..origin/v0.2.3 -- src/verify_typed.rs   -> empty
```

**Zero changes on the other line since the fork point**; the file was last
touched 2026-07-12. The read-only commitment this branch actually made names
`src/wire_schema.rs` and `src/bytecode.rs`, and never named this one. The
constraint was inherited by inference from a neighbouring commitment.

This is the third time on this branch that a blocker treated as external
dissolved on contact with the actual boundary — after the private-data
representation and after the `Add`/`Sub`/`Mul`/`Neg` generalisation. The pattern
is specific enough to state as a rule: **an escalation should carry the command
that establishes the constraint, not the reasoning that infers it.**

It also matters less than it first appears, which is the more useful conclusion.
`AbsVal`, `ChunkSig`, `TypedError` and `ChunkSignature` are all already `pub`,
and `WireShape` with them. A shape stack tracking per-value widths can therefore
be built **inside `native_codegen`**, seeded from `module.signatures`, without
touching the shared crate at all — the ops whose results need widths are ops this
lowering already handles, and their widths are baked (`GetField`), fixed by the
op (`CheckedAdd` yields `Int`), or seeded from the signature table. The
`verify_typed.rs` change is one option, not the prerequisite.

### RETRACTED within the hour: the read half frees nothing on its own

The first version of this section closed by calling the 5 read-only chunks "the
smallest genuinely-unblocked increment now available". **That is false at the
granularity that matters, and it is the same error this document already records
against itself** — quoting a chunk-level number when `lower_module` refuses a
whole module on the first opcode it cannot handle. I repeated it one commit after
writing the correction. Recording the repeat rather than quietly fixing it,
because a mistake that recurs immediately after being documented is evidence
about the documentation, not about the mistake.

Two probes settled it.

**Provenance** (`spike_report_read_only_chunk_provenance`). A read needs a body
to read from, and no reads-only chunk conjures one. Measured sources:

| Chunk | Body arrives via |
|---|---|
| `02_struct_field.kel :: manhattan_norm` | parameter (1 param) |
| `03_enum_match.kel :: area_estimate` | parameter (1 param) |
| `09_big_numbers.kel :: main` | `Call(2, 2)` result, then `GetTupleField` |

Both sources are **intra-module**: the callee is another chunk in the same
module, lowered by this backend, so the calling convention for a composite is
this branch's own decision and not a host-boundary question. That part is better
news than the resume prompt implies, which listed composites under a workstream
gate.

**The conjunction** (`spike_report_reads_only_conjunction`). Of the 5 modules
containing a reads-only chunk, **5 also construct a composite somewhere, and 0
are freed by the read half alone.** Which follows directly from provenance: a
body that arrives by parameter or call result was built by something, and
building is the blocked half.

### Consequence for ordering, restated

**Construction is the load-bearing half; reads are free but worthless alone.**
The increment is therefore per-value width recovery, not the read ops, and the
reads come along at nearly no additional cost once a shape stack exists. The
shape stack is a `native_codegen`-only change seeded from the already-`pub`
`module.signatures`, needs no opcode and no `BYTECODE_VERSION` bump, and is not
gated on the other line.

The general rule this keeps re-teaching, now stated where the next reader will
hit it: **a chunk-level count is not an increment.** Before calling any
population "unblocked", check whether a whole module clears, and check what the
inputs of the freed code are reachable from.

## The shape stack's seeding precondition HOLDS, measured

The restated plan is a per-value width stack inside `native_codegen`, seeded
from `module.signatures`. That plan carried an unexamined precondition worth
naming before building on it: **the signature table must actually be present and
carry real shapes.** An absent or uniformly-`Top` table seeds nothing and reduces
the plan to recovering everything from the op stream alone.

This mattered more than it sounds, because it would have failed silently. The
typed verifier is explicitly sound under an absent table — it DEFERS rather than
rejects — so an all-`Top` corpus is perfectly consistent with a green suite.
Nothing else measures it.

Measured by `spike_report_signature_seeding_quality` over 58 modules and 496
chunks:

| Quantity | Count |
|---|---|
| Chunks with a signature entry | **496 of 496** |
| Parameter shapes: `Scalar` (known width) | 613 |
| Parameter shapes: `Flat` (known body size) | 13 |
| Parameter shapes: `Top` (unknown) | **7** |
| Return shapes known / `Top` | 495 / 1 |
| Chunks needing width recovery whose every parameter is seeded | **22 of 22** |

The table is real, essentially complete at 1.1 percent `Top` parameters, and
**fully seeded across the entire population the plan has to serve.** The
precondition holds.

Two honest limits on that conclusion:

- **Seeding is necessary, not sufficient.** This establishes that the stack
  starts from solid ground. It does not establish that every INTERMEDIATE value
  is recoverable — that a width is known for each of the `count` operands at
  every `NewComposite` site. Settling that needs the full abstract
  interpretation, which is the increment itself rather than a probe of it.
- Two of 58 modules carry no table while every chunk has an entry, which means
  those two declare no chunks. Benign, and noted so the arithmetic is not read
  as an inconsistency.

The enabling primitives are all already `pub`: `verify::op_depth_effect`,
`ScalarKind::size_in_bytes`, `ChunkSignature`, `WireShape`, `AbsVal`, `ChunkSig`.
**The increment needs no change to any file outside `native_codegen/`**, no
opcode, and no `BYTECODE_VERSION` bump.

## DESIGN for the next increment: the per-value width stack

Specified by reading, while the first full gate runs on `9ac2be3`. Deliberately
no code and no compilation: a build now would contend with the gate for cores,
and I had just declined to push for exactly that reason. Everything below is
established from source, with the file and mechanism named so it can be checked.

### What the VM actually does, which is what the lowering must match

`Op::NewComposite` → `pack_flat_in_arena(&values, min_bytes, wb, fb, arena)`:

1. **Operand order.** `values` is `self.stack.drain(self.stack.len() - count..)`,
   and the packer walks it with `off` starting at zero. So **field 0 is the
   DEEPEST popped value and the last field is the top of stack.** Getting this
   backwards produces a body that is the right SIZE and wrong throughout, which
   no size check catches — it needs the differential oracle.
2. **No inter-field padding.** `off += field` exactly, per value. Alignment is
   not respected between fields; only trailing slack is zeroed, and only when
   `min_bytes` pads an enum to its widest variant.
3. **Size is the SUM of per-value widths**, never `count * word_bytes`. For a
   `Tuple` or `Array` the VM passes `min_bytes = 0` and ignores the baked
   `byte_size` entirely; for a `Struct` or `Enum` that value is a floor.
4. **Scalars are little-endian**, via `write_scalar_le`.

### The width table (`ScalarKind::size_in_bytes`, `src/value_layout.rs:109`)

| Kind | Bytes | Note |
|---|---|---|
| `Unit` | **0** | Occupies nothing. A `Unit` field shifts no offset. |
| `Bool` | 1 | |
| `Byte` | 1 | |
| `Int`, `Fixed`, `Opaque` | `word_bytes` | |
| `Float` | `float_bytes` | Behind the `floats` feature |
| `Text` | **`2 * word_bytes`** | A `(ptr, len)` `KStr` pair, not one word |

The two that will bite are `Unit` at zero and `Text` at two words. Both are
plausible to assume wrong, and both are silent when assumed wrong.

### Result shape per lowered op

Arities come from `verify::op_depth_effect`, which is `pub` and authoritative;
this table supplies only the KINDS. Two entries there are easy to get wrong from
the opcode name, and both are load-bearing:

- **The checked family is `(2, 1)`** — pops two and pushes **three**,
  `(low, high, flag)` with `low` deepest, so the following `PopN(2)` discards
  `high` and `flag`. `CheckedNeg` is `(1, 2)`. `GRAMMAR.md` states the triple in
  the wrong order; the arm bindings are right.
- **`SetLocal` is `(1, -1)`** — it POPS. It does not merely copy the top.

| Op | Result shape |
|---|---|
| `Const(i)` | Kind of `chunk.constants[i]`; a composite constant is a composite shape |
| `PushImmediate` | `Int` |
| `GetLocal(i)` / `SetLocal(i)` | The local's tracked shape; `SetLocal` writes it and pops |
| `Dup` | Duplicate of top |
| `CheckedAdd`/`Sub`/`Mul`/`Div`/`Mod`/`Neg` | `(Int, Int, flag)`, `low` deepest |
| `Div`, `Mod`, `CheckedDiv`, `CheckedMod` | `Int` |
| `CmpEq`…`CmpGe`, `Not`, `IsEnum`, `IsStruct` | `Bool` |
| `BitAnd`/`BitOr`/`BitXor`/`Shl`/`Shr` | **Propagated from the operands** — `Byte` in, `Byte` out |
| `WordToByte` / `ByteToWord` | `Byte` / `Int` |
| `GetData(slot)` | `SharedSlotLayout` kind for the slot |
| `Call(idx, _)` | `module.signatures[idx].ret` |
| `GetField`/`GetTupleField`/`GetEnumField`/`GetIndex` | The baked `kind`; the `FlatNested` forms give `size` and `variant` |
| `Len` | `Int` |
| `NewComposite` | `Flat { kind, size = Σ popped widths }` |

The bitwise and shift row is the one that needs propagation rather than a
constant: `Byte` is admitted through promote-operate-truncate masking, so the
result width follows the operands and is not always a word.

### The soundness rule

**Unknown is refused, never guessed.** An operand whose width the stack cannot
establish makes the enclosing `NewComposite` a `LowerError::UnsupportedOp`, not a
packed body at an assumed width. Imprecision then costs coverage and cannot cost
correctness, which is the same posture `check_word_width` and
`resolve_shared_scalar` already take. This matters because the corpus makes
guessing look safe: `count * word_bytes == byte_size` holds at 238 of 239 sites.

### The oracle

Not "the tests pass". The lowering's computed body size and field offsets must
agree with what the VM actually packs, so the test builds the same composite both
ways and compares **the body bytes**, not just the return value — the same shape
as `tests/shared_data.rs`, which compares the host buffer byte for byte. A
size-only check would pass on a correctly-sized body packed in reverse order,
which is precisely the failure mode point 1 above makes reachable.

Mutations that must fire, by construction: reverse the operand order; drop the
`Unit`-is-zero case; treat `Text` as one word; assume uniform word packing.

### Why this needs nothing outside `native_codegen/`

`AbsVal`, `ChunkSig`, `ChunkSignature`, `WireShape`, `verify::op_depth_effect`
and `ScalarKind::size_in_bytes` are all already `pub`. The seeding precondition is
measured and holds at 22 of 22 chunks. No opcode, no `BYTECODE_VERSION` bump, and
no file this branch does not own.
