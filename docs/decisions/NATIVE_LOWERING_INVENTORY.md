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

> **BOTH HALVES FALSIFIED LATER — see "Composites are TWO blockers".**
> `src/verify_typed.rs` is uncontested (zero changes on `v0.2.3` since the merge
> base), and the change is not needed anyway: the width stack can live entirely
> inside `native_codegen`. The ownership claim was inherited by inference from a
> read-only commitment that named two other files.

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

## Workstream B: precondition P2 is VERIFIED, and `Reset` does one thing this document missed

The rotation hypothesis rested on a structural claim stated as an observation and
never checked: **"`Reset` clears every local to `Unit`."** Everything else in that
section is downstream of it, so it was worth reading `Op::Reset` rather than
continuing to cite it. `src/vm.rs:5229` does five things:

1. **Clears locals `0..local_count` to `Unit`.**
2. **Truncates the operand stack** to `reset_base + local_count`, discarding
   everything above the locals.
3. **Resets the TOP arena region only** and advances the epoch, so an
   outstanding handle into the ephemeral region goes `Stale`. (This bullet said
   "both bump pointers" for about an hour, copied verbatim from an inaccurate
   comment in the VM. See the correction below — the difference decides whether
   private data survives, so it is not cosmetic.)
4. **Clears the ephemeral opaque registry**, dropping host refcounts.
5. **Sets the instruction pointer to just after the `Stream` instruction** — not
   to the top of the chunk.

### P2 is verified: the data segment really is the only survivor

Points 1 through 4 are exhaustive over the places iteration state could hide.
Locals die, the operand stack dies, arena allocations die with an epoch bump, and
opaque handles die. What remains is the data segment — the host-owned shared
buffer and the arena's persistent region — which is exactly what the hypothesis
assumed and no more. **P2 is no longer an unchecked assumption.**

A worry that arose and dissolved, recorded so it is not re-raised: `local_count`
comes from the compiler's slot high-water mark and nothing obviously enforces
`local_count >= param_count`, which would leave a parameter uncleared. It is
moot, because local-slot operands are bounds-checked against `local_count`
(`src/verify.rs:3343`), so a slot a `GetLocal` can reach is necessarily below it
and is necessarily cleared. There is no gap here to report.

### Point 5 is new, and it changes the shape of the transformation

**The pre-`Stream` prologue runs exactly once.** `Reset` jumps to the instruction
after `Stream`, so a stream chunk is already structurally

```
<prologue>          runs once, never re-entered
Stream
<body>              the loop: ... Yield ... Reset jumps back here
```

This document described the rotation as though the whole chunk were the loop. It
is not, and the distinction is useful rather than pedantic: the prologue is a
natural home for the rotation's entry special-case, and it is already outside the
repeating region, so the transformation does not have to manufacture one.

### Precondition status, restated

| | Status | Basis |
|---|---|---|
| P1: a `Reset` separates consecutive `Yield`s | 23 of 24 | Statically checked, `spike_stream_rotation.rs` |
| P2: the data segment is the only surviving state | **VERIFIED** | `Op::Reset` mechanism, above |
| P3: no `Return`/`Trap` between `Yield` and `Reset` | 24 of 24 | Statically checked |

### The one genuine obligation that remains, which reading cannot close

The rotated function runs the post-`Yield` part against the **previous** resume
value, then the pre-`Yield` part, and returns the newly yielded value. That is a
loop rotation, and loop rotations have boundary conditions:

- **The first call has no previous resume value** to feed the post-`Yield` part.
- **The final yielded value is never consumed** by a post-`Yield` part, because
  the sequence ends on a yield.

This is the classic prologue/epilogue mismatch, it is a real proof obligation,
and it is **not** closed by any amount of reading. It needs the differential
oracle — `tests/yield_sequence.rs`, which compares whole yield sequences rather
than single values, and which exists precisely because an order-blind oracle let
a wrong lowering pass. Stating it plainly so the verified P2 above is not
mistaken for the whole equivalence: **P1, P2 and P3 are the preconditions, not
the proof.**

## Two findings from reading the call boundary, one of them a latent defect in shipped code

### QUEUED FIX: non-parameter locals are uninitialised in the lowering

`Op::Call` fills the callee's frame: `extra = local_count - arg_count` slots are
pushed as `GenericValue::Unit` before the frame is entered (`src/vm.rs:5290`), so
**every non-parameter local starts defined** in the VM.

The lowering does not do this. `native_codegen/src/lib.rs:721` allocates
`local_count` allocas and stores into only the first `param_count` of them. The
rest are never initialised, and a load from an uninitialised `alloca` is `undef`
in LLVM. So a `GetLocal` of an unwritten slot is **`Unit` in the VM and `undef`
natively** — a divergence, and with `undef` feeding a branch or an offset, worse
than a wrong answer.

**Reachability, assessed rather than assumed: LATENT, not live.** Keleusma locals
are immutable and bound at declaration, so the reference compiler always emits
the write before any read, and no corpus module exercises the path. It is
nevertheless a defect by this project's own standard, which hardened
`FlatComposite::nested_view` from a `debug_assert` to a real fault precisely so a
release build could not perform undefined operations on input the verifier
admits. The verifier admits reading an unwritten local: `verify_typed` seeds it
`Top` and defers rather than rejecting.

The fix is to store zero into every non-parameter alloca at entry. `mem2reg`
folds the stores away where the slot is later overwritten, so the cost is nil.

**Deliberately not applied yet.** `tests/perf_canary.rs:131` asserts
`elapsed < CEILING_SECS` against a wall clock, so a competing build during the
running gate could inflate it into a **spurious red** and cost a 2.5-hour re-run.
That is a specific hazard rather than general caution, and it is the reason this
is queued rather than fixed in place. The must-fire control is a chunk that reads
a non-parameter local before writing it, differentially compared against the VM.

### The native call boundary needs a host trampoline, and its operand lies

`Op::CallVerifiedNative(idx, arg_count)` and `Op::CallExternalNative` share one
arm (`src/vm.rs:7072`). Three things a lowering has to know:

1. **The argument-count byte is not an argument count.** Its high bit is the
   B35 error-reify flag: `reify = arg_count & 0x80`, `n = arg_count & 0x7F`. A
   lowering that reads the byte whole would treat a three-argument reifying call
   as a request for 131 arguments. This is the same defect class as
   `CheckedMul`'s `u8` being a fraction-bit count rather than a multiplier —
   an operand whose name does not describe its bits.
2. **Arguments cross as `GenericValue`, not as machine words.** Each is decoded
   through `from_value_ctx` with a `RefContext`, and opaque arguments are
   materialised from the operand stack's POD `OpaqueRef` back to an `Arc`,
   touching a host refcount. None of that is expressible in lowered code; it
   needs a host-side trampoline, which is what makes this Workstream D rather
   than a lowering exercise.
3. **A reifying call pushes `(code, flag)`** on a soft host failure instead of
   propagating, and the surrounding construct dispatches `ok`/`error`. So the
   stack effect is operand-dependent, not fixed.

### Checked and CLEAN: `Op::Call`'s count carries no flag

Having found a hidden flag in one count byte, the same question was put to the
one already lowered. `Op::Call(u16, u8)` uses `arg_count as usize` directly with
no masking (`src/vm.rs:5273`). **The existing `Call` lowering is unaffected.**
Recorded because a check that comes back clean is still a result, and the next
reader should not have to redo it to find that out.

## Self-audit of the shipped lowering against VM semantics: 6 clean, 1 defect

The uninitialised-locals defect above was found by accident while reading the
call boundary for an unrelated reason. That is a poor way to find defects, so the
same comparison was run deliberately against the ops most likely to diverge —
chosen by risk, not alphabetically. **This is a sample of about seven of the
forty-six lowered opcodes, not an exhaustive audit**, and it should not be read
as one.

The selection criterion was: where does LLVM have undefined or poison behaviour
where the VM has defined behaviour, and where does the VM normalise a value in a
way a naive load would not?

| Check | Result | Basis |
|---|---|---|
| Shift count ≥ word width | **CLEAN** | Both mask |
| `Op::Not` truthiness | **CLEAN** | Both canonical |
| `Op::If` truthiness | **CLEAN** | Both nonzero-is-true |
| `Bool` read from a host slot | **CLEAN** | Both normalise |
| `Byte` widening sign | **CLEAN** | Both zero-extend |
| `Op::Call` count operand | **CLEAN** | No hidden flag |
| Non-parameter local init | **DEFECT** | VM defines, lowering leaves `undef` |

### The two that could have been live, and were not

**Shift counts.** `shl i64 x, 64` is *poison* in LLVM, and a shift count is a
runtime `Word`, so an unmasked lowering would be undefined on ordinary input
rather than on malformed input. The VM masks with `(c as u32) & (word_bits - 1)`,
so a shift by 64 is a shift by 0 and a shift by −1 is a shift by 63. The lowering
masks too, under a comment reading "THE MASK IS NOT OPTIONAL"
(`native_codegen/src/lib.rs:1042`). The two masks agree exactly: `& 63` reads
only the low six bits, so the VM's intermediate `as u32` truncation is invisible.

**`Bool` normalisation.** A `Bool` shared slot is one host-written byte and the
host may write any value. `read_scalar_le` normalises with `!= 0`, so `0xFF` is
`true`. A lowering that merely loaded and extended the byte would agree with the
VM under `If`, which only tests nonzero — and disagree under `CmpEq`, where the
VM compares canonical `true` and the native side would hold `255`. The lowering
normalises at `lib.rs:1375` with `NE 0` then `zext`, so the two agree everywhere,
not just on the branch path. Worth recording because the failure mode was
**conditional on which operator consumed the value**, which is the kind of
divergence a small differential corpus misses.

### What this says about where to look next

The clean results share a shape: each is a place where someone previously
noticed that LLVM and the VM disagree on undefined-versus-defined and wrote the
reconciliation down at the site. The one defect is a place where the VM does
something *positive* that is easy not to notice at all — filling a frame with
`Unit` — rather than a place where it constrains an operation.

**So the productive question is not "where is LLVM undefined" but "what does the
VM do that the lowering never had a reason to think about".** Frame
initialisation was one. Candidates not yet audited under that lens: what `Reset`
does to the operand stack, what `Return` truncates, and what the arena epoch
bump invalidates.

## `Op::Reset` resets ONE arena region, not two — and I propagated the error

Auditing under the "what does the VM do that the lowering never thought about"
lens, the next candidate was the arena epoch bump. It produced a correction to
this document, a clean result that looks alarming, and one defect on the other
branch's surface.

### The correction, which is mine

`Op::Reset` carries the comment *"Reset both arena bump pointers (R32)"*
(`src/vm.rs:5244`). It calls `reset_arena_internal`, which calls
**`reset_top_unchecked()`** — the top region only. That function's own SAFETY
note says so in terms: *"The bottom-region operand stack and frames are
unaffected."* Resetting both ends is `full_reset_arena_internal`, used by error
recovery and hot swap, and `Op::Reset` does not call it.

**I copied the wrong comment into the P2 section above and it stood for about an
hour.** The mechanism verification was still sound, because P2 asks what
SURVIVES and resetting fewer regions can only preserve more — but the reasoning
was wrong in a way that mattered, since "both bump pointers" would have implied
the persistent region is reclaimed, and **private composite data lives there and
must survive `RESET`**. Had the lowering been built to that belief it would have
re-initialised private data every iteration, and the differential oracle would
have caught it only on a test that resets and then reads private data back.

The lesson is narrower than "read the code": I *did* read the code. I read the
comment attached to the call rather than the function the call reaches. **A
comment is not a citation.**

### Confirmed for the lowering

Private data survives `Op::Reset`, which is what `native_codegen` already
assumes: the private region is a host-supplied pointer that native code writes
through directly and nothing in the lowering clears between iterations. The
behaviours agree. This was assumed rather than verified until now.

### Clean, though it looks alarming

`Op::Reset` discards the reset's result with `let _ =`. That is safe, and the
reason is worth recording so the next reader does not re-open it:

- `reset_top_unchecked` is **fail-closed and atomic**. It computes
  `epoch.checked_add(1)` and returns `Err(EpochSaturated)` **before** it touches
  `top_top`. A saturated epoch therefore leaves the arena entirely unchanged. It
  cannot reclaim storage while stale handles still believe they are live, which
  is the failure that would actually matter.
- The epoch is `u64`. Saturation requires 2^64 resets and is not physically
  reachable; the arena also exposes `epoch_remaining()` for hosts that want to
  schedule a graceful restart.

So the discarded error is unreachable, and if it were reachable it would degrade
to "the top region is never reclaimed", surfacing as an out-of-arena error rather
than as memory corruption. **No finding.**

### FOR THE `v0.2.3` SESSION: a stale comment on your surface

`src/vm.rs:5244` says `Op::Reset` resets "both arena bump pointers". It resets
the top only. This is the same defect class as the `GRAMMAR.md` line 747 push
order already reported and the `Op::CheckedAdd` doc comment already fixed: the
code is right and its description is not. It is worth fixing rather than
ignoring, because this comment cost a reader on another branch an hour and a
wrong conclusion about whether private data survives.

## Falling off the end of a chunk: a VM asymmetry, and two more queued fixes

The last of the three "what does the VM do that the lowering never thought
about" candidates was `Op::Return`. `Return` itself is clean, but reading it
surfaced the case next to it.

### `Op::Return` is clean

It pops the result, pops the frame, **truncates the stack to `old_frame.base`**,
and pushes the result — so from the caller's view the arguments are replaced by
one value. The lowering's `Call` pops `arg_count` and pushes one result, with
callee locals as its own allocas. The behaviours agree.

### Falling off the end does NOT truncate, and that asymmetry is the interesting part

`src/vm.rs:4801` handles `ip >= op_count` — "End of chunk without explicit
return: return Unit". It pops one value (or `Unit` if empty), pops the frame, and
pushes the result. **It does not truncate to `frame.base`.** So the callee's
locals and any leftover operands stay on the shared stack, and the result is
pushed on top of them.

Compared with `Op::Return`, each such call leaks `local_count` slots. If a chunk
like that were called in a loop, the operand stack would grow without bound,
which is the thing the worst-case-memory-usage analysis exists to preclude.

**Reachability, stated carefully because I have not established it.** The
reference compiler emits a trailing `Op::Return` (`src/compiler.rs:5342`,
`5414`), so this is not reachable from reference output, and no corpus chunk
exercises it. Whether `verify()` ADMITS a chunk without a trailing `Return` — and
therefore whether this is a live hole at the trust boundary that governs hot swap
and precompiled bytecode — **I did not determine.** The typed pass enforces
loop back-edge neutrality and exact height joins at merges, and I did not trace
whether it constrains a chunk's terminal depth. This is a question for the
`v0.2.3` session with the evidence attached, not a defect report.

### QUEUED FIX 2: the lowering emits invalid IR for the same input

The lowering has no fallback terminator. `Op::Return` builds the LLVM `ret`
(`lib.rs:1477`), and when the op loop ends without one the final block has **no
terminator at all**, which is malformed IR rather than a wrong answer. The VM
defines this case as returning `Unit`; the lowering should emit the matching
`ret` when the final block is unterminated.

### QUEUED FIX 3: `lower_module` never verifies the IR it produces

`lm.verify()` appears in `tests/differential.rs`, `tests/shared_data.rs` and
`tests/aot_linkage.rs`. **It appears nowhere in `src/lib.rs`.** The self-check
lives in the harness rather than in the API, so a consumer calling `lower_module`
directly receives IR that nothing verified, and Queued Fix 2's malformed output
would reach them silently while every test stayed green.

This is the more valuable of the two, because it is general rather than specific:
it closes the whole class instead of one member, and it is the project's own
stated principle — validate contracts at function boundaries — applied to a
boundary that was missed. It also explains why Fix 2 went unnoticed: the only
place that would have caught it is the place that is never run in production.

### The queue, all `native_codegen`-only, all awaiting the gate

| # | Fix | Class |
|---|---|---|
| 1 | Zero-init non-parameter locals | Latent UB (`undef` load) |
| 2 | Implicit `ret` when the final block is unterminated | Latent malformed IR |
| 3 | `lower_module` verifies its own module | Missing API postcondition |

None needs an opcode, a `BYTECODE_VERSION` bump, or a file outside
`native_codegen/`. All three need compilation, which is why they are queued
behind the running gate and its wall-clock perf canary.

## ANSWERED: `verify()` does admit a chunk with no trailing `Return`

The previous section left this open and offered it to the `v0.2.3` session as a
question. Handing over a question that can be answered by reading is worse than
answering it, so it was traced. **`verify()` admits it.**

### The trace

`verify()` runs three things per chunk (`src/verify.rs:2182`):

| Pass | Terminal state |
|---|---|
| `verify_chunk` | Structural; does not inspect the chunk's last op |
| `verify_stack_depth` | `verify_depth_region(chunk, 0, ops.len(), 0, ..).map(\|_\| ())` — **result discarded** |
| `typed_check_module` → `check_chunk_seeded` | `interp_region(..).map(\|_\| ())` — **result discarded** |

Both depth passes compute exactly the quantity that would settle it — the
region's terminal depth, `Ok(Some(d))` on fall-through versus `Ok(None)` when
every path exits via `Return`, `Trap` or `Break` — and both throw it away. No
check anywhere in `verify.rs` or `verify_typed.rs` reads a chunk's last op.

### What that costs

`Op::Return` truncates to `old_frame.base` and pushes one result, leaving depth
`base + 1`. Falling off the end leaves `base + local_count + k`, where `k` is any
leftover operands. **Each such call leaks `local_count + k - 1` slots**, and
called in a loop the operand stack grows without bound.

**This is not a memory-safety defect.** The operand stack is an arena-backed
`ArenaVec` in the bottom region, so unbounded growth exhausts the reserved
footprint and fails closed with an out-of-arena error rather than corrupting
anything.

**It is a worst-case-memory-usage under-count**, which is the more serious thing
here given the ecosystem's stated value proposition. The bound is computed as
`chunk.local_count + body_peak` per chunk (`verify.rs:1149`), which models
`Return` semantics; a callee that falls off the end leaves residue the caller's
peak never accounted for. A module can therefore be admitted by `verify()`,
attested with a WCMU bound, and exceed it at run time.

### Scope, stated precisely

> **FALSIFIED — see "FALSE PREMISE, caught by the `v0.2.3` session" near the end
> of this document.** The paragraph below is wrong: a `Stream` chunk ends in
> `Op::Reset`, not `Return`. The conclusion survives with different reasoning
> (every compiler-emitted chunk ends in a *path exit*), but the scope statement
> does not. Left in place rather than edited, because the reasoning error —
> grepping for the expected answer and stopping — is the part worth seeing.

The reference compiler always emits a trailing `Op::Return`
(`src/compiler.rs:5342`, `5414`), so **no program produced by the normal
pipeline is affected**, and no corpus chunk exercises it. The exposure is exactly
the surface `verify()` exists to protect: **hot-swapped modules and precompiled
bytecode**, which reach the VM as bytes rather than as compiler output. That is
the boundary at which "treat all external inputs as untrusted" applies.

I have **not** built a proof of concept, so this is a read-derived finding rather
than a demonstrated one. The recipe is small and stated so it can be checked
rather than taken on trust: a chunk whose ops end without `Return` and whose
`local_count` exceeds its `param_count`, called in a counted loop, with the
operand depth observed across iterations.

### For the lowering

The native side does not inherit the bug — each chunk is an LLVM function with
its own frame, so nothing leaks into the caller. It inherits the DIVERGENCE: on
such a chunk the VM's subsequent depths differ from the native ones. Queued Fix 2
(an implicit `ret` on an unterminated final block) matches the VM's *value*
semantics — "End of chunk without explicit return: return `Unit`" — and
deliberately does not reproduce the leak, since reproducing a defect for
bug-compatibility would be the wrong call. That asymmetry is recorded here so it
is a decision rather than an oversight, and it should be revisited if the VM side
is fixed.

## DESIGN REFINEMENT: `NewComposite::Flat` does not mean the body is flat

Checking how a composite CONSTANT materialises, because the width stack's table
lists `Const(i)` as yielding a composite shape, turned up a constraint that would
have produced a silent divergence had the stack been built to the design as
written.

### The operand names a kind, not a representation

In the `Flat` arm the VM calls `pack_flat_in_arena` and then branches on whether
packing SUCCEEDED (`src/vm.rs:5478`):

| Kind | Packing fails | Meaning |
|---|---|---|
| `Tuple`, `Array` | **Silently falls back to a BOXED body** | Representation is value-driven at run time |
| `Struct`, `Enum` | `VmError::InvalidBytecode` | Statically flat by the compiler's decision |

So for two of the four kinds, **the same opcode with the same operand yields
either a flat or a boxed body depending on the values on the stack.** A lowering
that always packs flat would diverge exactly when the VM boxes, and the
divergence is invisible to any test whose tuples happen to be small and scalar —
which is every obvious test case.

Packing fails on either of two conditions, both decidable by the width stack:

1. **An element is not flat-eligible** — `flat_field_size` returns `None`. A
   boxed composite constant is one such element, which is how this was found.
2. **The body exceeds `u16::MAX` bytes** — `pack_flat_in_arena` returns
   `Ok(None)` when `size > 65535`, because the access offset is sixteen bits.

### The rule this imposes on the width stack

- **`Struct` / `Enum`**: if every operand shape is known, pack flat. The compiler
  has already decided flatness and a failure is malformed bytecode, so the
  lowering may rely on it.
- **`Tuple` / `Array`**: pack flat only if every operand shape is known **and**
  every element is flat-eligible **and** the summed size is `<= u16::MAX`.
  Otherwise **refuse**. Do not attempt to emulate the boxed representation; that
  is a second representation with its own access path, and it is scheduled for
  removal at B28 P3 anyway.

Note that the size condition is a genuine third refusal reason, independent of
whether shapes are known: a fully-known tuple of 70,000 bytes is boxed by the VM
and must be refused rather than packed.

### And a coverage limit worth knowing before implementing

A composite constant is materialised through `value_from_archived` as a **boxed**
body, and `AbsVal::Top`'s own documentation lists "composite constant" among the
shapes it cannot reconstruct. So a `NewComposite` that packs a composite constant
is refused under the design's unknown-is-refused rule — correctly, but it is a
coverage cost rather than a free win.

**How much it costs is unmeasured.** Whether the reference compiler folds a
literal aggregate into a composite constant or emits `NewComposite` over scalar
constants decides it, and that needs a corpus count, which needs compilation.
Queued with the rest. Recording the question rather than guessing the answer,
since the last time this document guessed at a distribution it was wrong by the
margin that mattered.

## The queued fixes are WRITTEN, not merely decided

All three are prepared as an anchored patch and the anchors are dry-verified
against the live source — four replacements, each matching exactly once. The
post-gate work is therefore apply, compile, test, rather than think, write,
compile, test.

The patch lives in the session scratchpad, which is **not durable**. It does not
need to be: each change is specified below tightly enough to rewrite from this
document alone, which is the form that survives.

| Fix | Site | Change |
|---|---|---|
| 1 | after the parameter stores in `lower_chunk_body` | store `i64t.const_zero()` into `locals[param_count..]` |
| 2 | after the op loop, before the `stack_overflow` check | if `!dead` and the insert block has no terminator, `build_return` of `st.pop()` when `st.depth > 0` else zero |
| 3a | `LowerError` | new `InvalidIr(String)` variant plus its `Display` arm |
| 3b | end of `lower_module` | `module.verify().map_err(LowerError::InvalidIr)?` before `Ok(declared)` |

Two details that are easy to get wrong and are therefore pinned here:

- **Fix 2 must guard `st.depth > 0`.** `Lower::pop` decrements a `usize`
  unconditionally, so popping an empty stack underflows. The guard also matches
  the VM, which does `stack.pop().unwrap_or(Unit)`.
- **Fix 2 is deliberately not bug-compatible.** The VM additionally leaves the
  callee's whole frame on the shared operand stack in this path. That leak is
  the WCMU under-count reported to the runtime owner; the lowering returns the
  right value without reproducing the leak.

The two must-fire controls both still need writing, and neither is in the patch,
because a control is worth more when written against the fixed code than
alongside it: a chunk that reads a non-parameter local before writing it, and a
chunk whose ops end without `Op::Return` — each compared differentially against
the VM rather than asserted against my own expectation.

## Audit, second batch: the indexing surface is clean

The first audit batch deliberately skipped the array-indexing path, which is the
remaining shipped surface where an error would be **live rather than latent**.
Checked now. Running total: **nine opcodes audited, one defect**.

| Check | Result | Why it could have failed |
|---|---|---|
| `BoundsCheck` predicate | **CLEAN** | Off-by-one, or a signed/unsigned mismatch |
| `BoundsCheck` peek-not-pop | **CLEAN** | The VM does not modify the stack |
| Indexed data bounds | **CLEAN** | Same predicate, separately written |
| `SetDataIndexed` pop order | **CLEAN** | Index and value could be swapped |

### The predicate is right, and by a non-obvious route

The VM traps unless `0 <= value < bound`, written as two signed tests: `< 0` and
`>= bound`. The lowering emits **one unsigned compare**, `UGE v, bound`. These
are equivalent, not merely similar: `-1` becomes `0xFFFF_FFFF_FFFF_FFFF` under
unsigned interpretation and therefore exceeds any non-negative `bound`, so the
single test catches the negative case the VM tests separately. `bound` arrives
through `u64::from`, so it is non-negative by construction and the equivalence
cannot be broken by a wide bound.

Worth recording because "one unsigned compare replaces two signed ones" reads
like a shortcut and is in fact exact. A reviewer who did not know the identity
might "fix" it into something slower and no more correct.

### The pop order is right, and it is the class that has bitten twice

`Op::SetDataIndexed` pops the **index first** (`src/vm.rs:4922`) and the value
second (`4940`), so the index is on top and the value beneath it. The lowering
pops in that order and says so in a comment, and the comment is now verified
rather than asserted.

This is the third time this arc has turned on operand ORDER: the checked triple
is `(low, high, flag)` and `GRAMMAR.md` says otherwise; `Op::Call` arguments sit
in declaration order so popping yields them reversed; and now the indexed write.
**Order errors are invisible whenever the operands happen to be equal**, which
describes most hand-written test cases, so they are not caught by the obvious
test — only by a differential oracle over asymmetric values.

### What the two audit batches say together

Nine checks, one defect, and the defect is the only one that is not about
LLVM-versus-VM disagreement on an operation. Every clean result sits where
somebody previously noticed a discrepancy and wrote the reconciliation at the
site. The defect sits where the VM does something positive — filling a frame —
that the lowering had no reason to consider.

That asymmetry is now twice-confirmed and is the most useful heuristic this
document has produced for where to look next: **not "where is LLVM undefined",
but "what does the VM do that the lowering never had a reason to think about".**

## The two must-fire controls are written, and one of them is the missing proof

Prepared alongside the fixes. **I had deferred these on a reason that was
weaker than I stated**: that writing a control beside its fix risks encoding the
same assumption twice. That holds for an ASSERTION-based test. These are
DIFFERENTIAL — the expected value comes from the VM — so the objection barely
applies, and the correct move was to write them.

Both mutate real compiled bytecode rather than hand-building a `Module`, the
technique the typed-verifier conformance corpus already uses. Hand construction
would need every `Chunk` and `Module` field right, and a field I got wrong would
make the test measure my construction rather than the lowering.

### The second control is the proof of concept I said I had not built

`Vm::new` runs `verify()`. So a mutated module that reaches execution has been
**admitted by the verifier**, which is exactly the claim recorded above as
read-derived from both depth passes discarding their terminal result. The control
turns that into a demonstration.

**And it is falsifiable in the useful direction.** If `Vm::new` REJECTS a chunk
with no trailing `Return`, then `verify()` does not admit it, and the inventory
section claiming otherwise — plus the item reported to the `v0.2.3` session — is
wrong. That would be a result worth having, not a broken test, and the `expect`
message is worded so the failure reads that way.

### One decision the first control makes explicit

**Zero is this backend's `Unit`.** The operand stack is uniformly `i64` and
`Unit` occupies zero bytes, so it has no natural width — choosing zero is a
decision, not a consequence. Until now it was implicit in Fix 1's store. The
control is where it is stated, so a future change to the `Unit` encoding fails a
test rather than silently altering what an unwritten local reads as.

### What is deliberately NOT asserted

The operand-stack leak. The VM does not truncate to `frame.base` when a chunk
falls off the end, and the lowering does not reproduce that. Pinning the leak as
expected behaviour would entrench a defect in a test, which is how a bug becomes
a specification. The asymmetry stays recorded as a decision.

### Known compile risks, written down rather than discovered

1. The mutation closure is passed to both helpers, each taking `impl FnOnce` by
   value. This compiles only because both closures capture nothing and
   non-capturing closures are `Copy`.
2. The mutated module keeps its pre-mutation `signatures[0]`, so the recorded
   return shape no longer matches what the chunk returns. The typed pass
   validates offsets rather than return-type agreement, so this should be
   accepted — an assumption, not a certainty.

## SCOPED: the fixed-point family, and why it is cheaper than its opcode count

Never examined until now. It is **core language, not float-gated** — `Fixed` is a
Q-format value at the runtime's word width, and the `floats` feature does not
guard it. The class is `WordToFixed`, `FixedToWord`, `FixedMul`, `FixedDiv`,
plus `CheckedMul(fb)` and `CheckedDiv(fb)` for `fb > 0`, which are refused today
because only the `fb == 0` integer forms lower.

### The decisive structural fact: every fraction-bit check is STATIC

`frac_bits` is an **opcode operand**, not a runtime value. Every range check the
VM performs on it is therefore decidable at lowering time, and none needs an
emitted branch. The VM's own handling splits two ways, and the split is
deliberate and documented in place:

| Op | Out-of-range `fb` | VM behaviour |
|---|---|---|
| `WordToFixed` | `fb >= 2 * word_bits` | **Saturates** by sign; zero stays zero |
| `FixedToWord`, `FixedMul`, `FixedDiv` | `fb >= word_bits` | **Fails closed**, `InvalidBytecode` |

The VM explains the asymmetry rather than leaving it to be guessed: `WordToFixed`
converts an in-range integer whose *result* merely overflows the `Fixed` range,
whereas an out-of-range fraction count is *corrupt input*, for which failing
closed is the honest response.

**The lowering should refuse at lower time wherever the VM fails closed.** That
is the same relationship the rest of this backend already has with the VM — a
load-time refusal in place of a runtime error is stricter, never looser.

### The arithmetic, and it reuses machinery that already exists

All four compute in 128 bits and **saturate**, never wrap:

- `WordToFixed(fb)`: widen, `shl fb`, clamp to `[i64::MIN, i64::MAX]`, truncate.
- `FixedToWord(fb)`: arithmetic `shr fb` on the word directly. No saturation —
  the result can only shrink.
- `FixedMul(fb)`: widen both, multiply, **arithmetic** `shr fb`, clamp, truncate.
- `FixedDiv(fb)`: trap if the divisor is zero, widen both, `shl fb` the dividend,
  `sdiv`, clamp, truncate.

The lowering already has every piece: `Lower::widen` sign-extends into `i128`,
the checked-arithmetic ops already compute in that domain, and `trap_bb` already
exists. **No new infrastructure, and no new `LowerError` variant** beyond
reusing `UnsupportedOp` for an out-of-range `fb`.

One overflow question settled rather than assumed: `FixedDiv` cannot hit the
`i128::MIN / -1` hazard that makes the *integer* division lowering delicate. The
dividend is at most `i64::MIN << 63 = -2^126`, so the quotient is at most `2^126`,
comfortably inside `i128`. The 128-bit domain removes the hazard rather than
relocating it.

### A semantic point worth stating loudly

**Fixed-point arithmetic saturates silently; it does not trap and it produces no
overflow flag.** That is the opposite of the checked integer family, which pushes
`(low, high, flag)` so the program can observe overflow. A `FixedMul` that
overflows returns `MAX` and says nothing. This is the VM's behaviour and the
lowering must reproduce it, but it is a genuine difference in the language's
error model between two arithmetic families, and it belongs in the record rather
than only in the code.

### Unmeasured, and deliberately not guessed

**Whether the corpus uses fixed-point at all is unknown.** The coverage spike
buckets these under "float / fixed-point" without separating them, so the payoff
of this class is unquantified. It needs a corpus count — compilation — and joins
the composite-constant count in the queue. Recording the question rather than
assuming the class is worth implementing, on the same discipline that caught the
chunk-versus-module error earlier in this document.

## ROADMAP RECONCILIATION: "Workstream C" means two different things, and one is mine

Checking today's findings against `docs/roadmap/V0_3_X_ROADMAP.md` — the actual
planning document — turned up a labelling collision, a gate that may be
mis-scoped, and a risk assessment that today's work arguably overturns.

### The collision, which is my error

The roadmap defines **`C. Arena-resident coroutine frames and the native arena
model`**. Composites are not a roadmap workstream at all; they fall under
**`A. Bytecode-to-LLVM-IR lowering`**, whose full pass "lowers every opcode of
the full-language ISA".

This document and the lowering use "Workstream C" for **composites** in seven
places, including a string that ships inside a `LowerError`:

| Site | Text |
|---|---|
| `src/lib.rs:451` | `"shared composite body; Workstream C"` |
| `src/lib.rs:469` | `"Text slot; string representation is Workstream C"` |
| `src/lib.rs:509` | `"shared array of composite bodies; Workstream C"` |
| `src/lib.rs:1065` | `"Composite and string constants are Workstream C"` |
| `tests/spike_corpus_coverage.rs:79` | `"C (composites)"` |
| `tests/differential.rs:160` | `"Workstream C, the flat byte composite representation"` |
| this document, twice | `"(Workstream C, composites)"` |

And this document ALSO uses the label correctly, twice, for arena residency and
coroutine frames. **So the same document uses one identifier in two incompatible
senses**, which is worse than using the wrong one consistently: a reader
cross-referencing to the roadmap lands in the wrong workstream, and a consumer
reading a `LowerError` is told to consult a workstream about coroutine frames
when the refusal is about composite bodies.

**Correct label: `A (full pass)`.** Queued with the other fixes rather than
applied, because four of the seven sites are in `src/lib.rs` and changing shipped
strings wants a compile. The two in this document are prose and are corrected in
place below by this section standing as the authority.

### Order 1's gate may be mis-scoped, and I have not measured it

The dependency table's Order 1 is `A (first pass)`, gated on **"the self-hosted
compiler's own bytecode runs correctly as native code, differential-tested
against the VM."**

That gate is about a *specific ten-module subset* — the stages under
`src/selfhost/kel/`. **Every coverage figure this document reports is
corpus-wide, never restricted to that subset.** 20.7% of whole programs is not an
answer to the Order-1 question.

The prior is unfavourable: those stages are a compiler, so they manipulate
tokens, syntax trees and symbol tables, which are composite-heavy by nature. If
that holds, **Order 1 is gated on composites and therefore on the width stack**,
which the table does not indicate. But it is a prior, not a measurement, and this
document has already been wrong once today by reasoning from a plausible
distribution instead of counting. Queued: coverage restricted to
`src/selfhost/kel/`.

### Workstream B's risk assessment is arguably overturned

The roadmap says of B: *"This is the piece the V0.4.0 strategy identifies as
where the risk concentrates."* Today's work weakens that for the common case:
P2 is verified from the `Op::Reset` mechanism, P1 and P3 hold at 23 and 24 of 24,
and 23 of 24 stream chunks are rotation candidates that would need **no coroutine
intrinsics and no arena-resident frames at all**. If the rotation equivalence
holds, both B and C shrink to a one-chunk minority.

The boundary condition remains genuinely open and is the whole of the residual
risk, so this is a case for restating the row, not deleting it.

### Deliberately NOT edited

I have not touched `V0_3_X_ROADMAP.md`. The `v0.2.3` session stated they intend
to restate its Order-1 gate row and had not done so because the file sat inside
their running gate. Editing the same table while that is outstanding manufactures
a conflict in a shared planning document for no benefit. The proposals are
recorded here and raised in the mailbox instead; whoever edits it should carry
both.

## SCOPED: Workstream E's WCET half, and why it is NOT symmetric with the WCMU half

The WCMU half is closed — a native stack bound derived end to end from LLVM's
own `.stack_sizes`. The natural assumption is that the WCET half closes the same
way. **It does not, and the reason is worth stating before anyone spends a week
on it.**

### What already exists is attestation, not derivation

`NativeIterationBound::per_call_wcet_cycles` (`src/verify.rs:1688`) carries a
**host-attested** per-call cost, supplied through `Vm::set_native_bounds` with a
`DEFAULT_NATIVE_WCET` fallback. The WCET pass adds it for a native's body,
summed over static call sites for a verified native and multiplied by
`max_invocations` for an external one.

So the runtime's existing answer for native code is *"the host asserts a number
and the verifier trusts it."* That is a trust boundary, not a proof, and it is
strictly weaker than what the bytecode enjoys.

### Why the WCMU half was derivable

A stack frame size is a **static** property that LLVM computes exactly and can be
made to emit. Nothing about the microarchitecture changes it. That is why
`llc --stack-size-section` plus `llvm-readobj --stack-sizes` produced a real
bound rather than an estimate.

### Why WCET is not the same problem

A cycle count is **not** a static property of the code. It depends on cache
state, branch prediction, and pipeline occupancy — none of which the emitted
object records.

`llvm-mca` is present in the same toolchain (verified: LLVM 22.1.8 at
`/opt/local/libexec/llvm-22/bin/llvm-mca`), and it is the obvious thing to reach
for. **It must not be used as a WCET tool.** It is a *throughput* analyzer: it
models steady-state execution with no cache misses and no mispredictions. Its
output is therefore closer to a **lower** bound on cost than an upper one, and a
WCET bound built from it would be unsound in the direction that matters. Naming
it here specifically so the next person does not discover this after wiring it in.

### Where the derivation IS sound, and it falls the right way again

On an **in-order embedded core with no cache** — `thumbv7em-none-eabihf` and its
relatives — a per-instruction cycle table *is* sound, because the dynamic
behaviour the host CPU has is absent by construction. The route is then:

1. Emit the object out of process, as the WCMU half already does.
2. Recover the instruction sequence per basic block (`llvm-objdump`).
3. Apply a per-target worst-case cycle table.
4. **Take the maximum over the same worst-case path the bytecode pass already
   proved**, rather than re-deriving the path.

Step 4 is the load-bearing simplification: the lowering builds its basic blocks
directly from the bytecode's branch targets, so the control-flow structure is
preserved and the *path* argument does not have to be redone — only the cost
table is substituted.

**This mirrors the `.stack_sizes` finding exactly, including the caveat.** That
one is ELF-only, absent on Mach-O, present on `thumbv7em`, and was recorded as
falling the right way because the embedded targets are where a stack overflow is
unrecoverable. The same is true here: WCET is derivable precisely where it is
needed and undecidable-in-practice precisely where it is not.

### The constraint this places on optimisation, which is already familiar

Step 4 holds only while the lowering's control flow survives to the object. At
`-O2` LLVM inlines, unrolls, if-converts and merges blocks, and the proven path
no longer corresponds to anything. **This is the same tension the WCMU half
already hit**, where `default<O2>` inlined everything and zeroed all frames, so
`mem2reg` alone was used.

Two ways out, and the choice is a real decision rather than a detail: constrain
the optimisation level so the CFG is preserved, or derive the bound
post-codegen from the emitted machine code where whatever LLVM did is already
reflected. The second is better and is what the WCMU half effectively does.

### Status

Design only. Nothing measured, and the measurement wants a quiet machine, which a
running gate is not. Recorded so the WCET half is not started on the false
premise that it is the WCMU half again.

## APPLIED: the three fixes, and one of my own controls was vacuous

All three queued fixes are in, with both controls. 68 tests green, `fmt` clean,
`clippy` zero at `-D warnings`.

### The must-fire check I nearly skipped, and what it caught

Both controls passed immediately after the fixes. **That proves nothing**, so the
lowering was reverted and the controls re-run against the unfixed code. Results:

| Control | Against unfixed code | Verdict |
|---|---|---|
| Missing trailing `Return` | **FAILED** — `"Basic Block in function 'kel_chunk_0' does not have terminator!"` | A real control |
| Unwritten local | **PASSED** | **VACUOUS** |

The second control did not fire. An uninitialised `alloca` loaded immediately
reads whatever occupies the slot, and a fresh frame slot is usually zero, so
`undef` materialised as `0` and matched the expected value **by accident**. It
would have sat in the suite looking like protection while testing nothing.

This is the same class as the null mutation recorded earlier in this document —
where changing an arithmetic shift to a logical one fired nothing because the
following truncate discarded the difference. The difference is that the null
mutation was a property that *could not* be observed, whereas this was a real
defect the test simply failed to detect.

### The rewrite, and why structural was the right form

The control is now a **structural assertion on the emitted IR**: the store of
zero into the non-parameter local either appears or it does not, and no
accidental stack contents can fake it. `differential.rs` already had
`lowered_ir` for exactly this purpose — "assertions about structure that runtime
behaviour cannot demonstrate" — so the tool existed and I had reached for the
wrong one.

Re-verified after the rewrite: **both controls now fail against the unfixed
lowering**, the second with its own message rather than an incidental mismatch.
The behavioural comparison is retained beside it, explicitly labelled a
regression check and not a control, since it agrees with the VM but cannot fail
when the fix is absent.

### The `verify()` finding is now DEMONSTRATED, not read-derived

`control_chunk_without_trailing_return_falls_off_the_end` builds a module whose
chunk has no trailing `Return` and hands it to `Vm::new`, **which runs
`verify()`**. It is accepted and executes, returning the top of stack. The
inventory recorded this as derived from both depth passes discarding their
terminal result; it is now shown. The item reported to the `v0.2.3` session
stands, and its "no proof of concept built" caveat can be dropped.

### Fix 3 justified itself immediately

The missing-terminator control fails through `lm.verify()`. Before Fix 3, that
call existed **only in the test harness** — so a consumer calling `lower_module`
would have received the malformed module with no error at all. The fix that
found this defect is the same fix that would have surfaced it in production.

## Input to the roadmap's OPEN DECISIONS, from today's evidence

`V0_3_X_ROADMAP.md` carries three open decisions. Today's work bears on two.
Recorded here rather than edited into the roadmap, for the coordination reason
given above: the `v0.2.3` session has an unfinished restatement of that same
table.

### Decision 1, "WCET on native is hard or best-effort", is posed as a false dichotomy

The decision offers two answers. The evidence says the right answer is
**neither, because it is target-dependent** — and the split falls along a line
this branch has now hit twice, independently.

| Target class | WCET derivable? | Why |
|---|---|---|
| In-order embedded, no cache (`thumbv7em-none-eabihf`) | **Hard bound achievable** | A per-instruction cycle table is sound; the dynamic behaviour that defeats it is absent by construction |
| Out-of-order superscalar host | **Best-effort at most** | Cycle counts depend on cache state, branch prediction and pipeline occupancy, none of which the emitted object records |

**The same split governs the WCMU half**, which is the part that makes this
structural rather than coincidental. `.stack_sizes` is ELF-only: absent on
Mach-O, present on `thumbv7em`. That was recorded as "falling the right way,
since the embedded targets are where a stack overflow is unrecoverable". The
WCET half falls the same way for an unrelated reason — one is a file-format
question, the other a microarchitecture question — and they agree.

**So the guarantees are derivable precisely on the targets that need them, and
best-effort precisely on the targets that do not.** That is a better answer than
either option the decision offers, and it means the decision can be resolved
without picking a side: hard where it is provable, best-effort where it is not,
with the boundary stated per target rather than per release.

The V0.5.0 host strategy already treating native WCET as best-effort is
consistent with this and does not settle it, because that strategy is about a
host, which is exactly the class where best-effort is the only honest answer.

### Decision 2 gains an argument from an unexpected direction

The AOT-versus-JIT question was already answered on feasibility — both shapes
work, so it is a support-and-maintain decision. Today adds two asymmetries that
were not visible before.

**Verification asymmetry.** Fix 3 made `lower_module` verify its own module,
closing a hole where malformed IR reached a consumer. That hole is **materially
worse on the JIT path**: malformed IR handed to an execution engine is executed
in-process, whereas the same IR on the AOT path fails at object emission with a
diagnostic. The missing check was therefore a JIT-path safety issue specifically,
and a JIT path carries a standing obligation to verify that the AOT path gets
partly for free from the toolchain.

**WCET-derivation asymmetry.** The route sketched for Workstream E's WCET half
works on the emitted machine code — recover per-block instruction sequences,
apply a target cycle table. That is naturally an **ahead-of-time** activity. It
is not impossible under a JIT, since the object can be emitted to a memory
buffer, but the analysis wants a fixed artefact and a known target, and a JIT has
neither by design.

**Net:** if hard WCET on embedded targets is wanted, that argues for AOT as the
primary shape, with JIT retained as a development and testing convenience — which
is how this package already uses it, since every differential test JITs at
`OptimizationLevel::None` and only the linkage test exercises the pipeline that
ships.

### Decision 3 gets nothing from me

Flat-machine ISA timing. No evidence produced today bears on it, and saying so is
worth more than manufacturing a position.

## READING THE AUTHORITATIVE ARCHITECTURE: a symbol-scheme conflict, and a correction to my last entry

`V0_4_0_NATIVE_CODEGEN.md` is named by `V0_3_X_ROADMAP.md` as the authoritative
architecture for the milestone this line targets. **I had never read it**, which
was a mistake worth naming: four provisional application-binary-interface
decisions were made on this branch without checking whether the architecture
already settled them. One of them was already settled, and differently.

### CONFLICT: the symbol scheme is a RESOLVED design question, and mine contradicts it

R4.2, recorded at **high confidence**, specifies:

```
_K<v>_<purity><category>_<module_path>_<function_name>[_<typeargs>]
```

Versioned at `v=1`; purity `P`/`I`/`T`; category `F`/`Y`/`L` for fn, yield and
loop; module path separated by `__`; type arguments as a 16-hex-digit SHA-256
truncation; demangleable through a `keleusma demangle` tool.

This branch emits **`kel_chunk_<index>`**, and it fails the scheme's own stated
constraints in two ways that are not stylistic:

- **It is not stable across compiler versions.** The name is a chunk INDEX, so
  adding, removing or reordering a chunk silently renames every symbol after it.
  R4.2 lists stability as a constraint precisely because cross-module references
  and hot replacement resolve by name.
- **It is not demangleable to anything.** No module path, no function name, no
  purity, no category. `keleusma demangle` would have nothing to return.

**Not yet harmful, and the reason matters.** The lowering is single-module today;
every test resolves within one object, by JIT symbol lookup or intra-object
linkage. The index scheme works exactly as long as that holds and breaks the
moment there are two modules or a hot swap — which are Workstream D and
Workstream H respectively.

**There is no design work to do here, only adoption.** The answer exists at high
confidence and predates this branch. Recording it as a debt against the
provisional label rather than a question: `kel_chunk_<index>` must not survive
into any artefact another object or tool consumes. Note also that category
`Y`/`L` encodes whether a symbol is a coroutine, which touches the Workstream B
rotation directly — a rotated stream chunk lowered as a plain function has a
category question attached to it.

### CORRECTION to the open-decisions entry immediately above

That entry claimed "**the same split governs the WCMU half**", presenting the
embedded-versus-host boundary as common to both halves of Workstream E. **That is
too strong, and reading the target tiers shows why.**

The tiers (R4.5): **Tier 1** is x86-64 Linux, AArch64 Linux, and macOS on both
architectures. **Tier 2** includes Cortex-M55 and Cortex-M4.

| Half | Derivable where | Against the tiers |
|---|---|---|
| WCMU (`.stack_sizes`) | **Any ELF target** | Both Tier 1 Linux targets, plus Tier 2 embedded. Fails on macOS only. |
| WCET (cycle table) | **In-order, no cache** | **No Tier 1 target at all.** Tier 2 embedded only. |

So the two halves do **not** split along the same line. They agree only at the
extremes — both work on Cortex-M, both fail on macOS — and diverge on Tier 1
Linux, where the memory bound is derivable and the timing bound is not. The
earlier claim conflated a file-format constraint with a microarchitecture
constraint because both happened to favour embedded.

### The uncomfortable consequence, stated plainly

**Every Tier 1 ship target for V0.4.0 is one where a hard WCET bound is not
derivable.** The ecosystem's stated value proposition is definitive WCET and
WCMU. On the targets V0.4.0 ships first, the WCMU half survives on Linux and the
WCET half degrades to attestation or best-effort everywhere.

That is not an argument against the tier order, which follows back-end maturity
and platform relevance and is sensible on those grounds. It is an argument that
**open decision 1 cannot be resolved without saying which tier it is being
resolved for**, and that the answer for Tier 1 is materially weaker than the
answer for Tier 2. Better surfaced now than discovered when someone asks what the
guarantee means on the platform they actually shipped to.

## Provisional ABI decisions audited against the architecture: two conflict, two are open

Having found the R4.2 symbol conflict, the honest move was to check the other
three rather than stop at the first hit.

| # | Provisional decision | Architecture says | Verdict |
|---|---|---|---|
| 1 | `kel_chunk_<index>` symbols | R4.2 mangling scheme, high confidence | **CONFLICT** |
| 2 | Trailing shared-buffer pointer | Not addressed | Genuinely open |
| 3 | Flat private region, 8 bytes/slot | Not addressed | Genuinely open |
| 4 | `i64 kel_yield(i64)` | `coro.id.retcon` continuation model | **CONFLICT** |

Decisions 2 and 3 are a clean result rather than a lucky one: the Architecture
section specifies the pipeline and the linkage story but says nothing about the
data-segment boundary, so those two remain this branch's to settle and the
provisional label is accurate.

### CONFLICT 4: the yield ABI is a callback where the architecture specifies a coroutine

The architecture lowers each sub-coroutine as an **LLVM coroutine in the
returned-continuation kind**: `Yield` becomes `@llvm.coro.suspend` returning the
yielded value *plus the next continuation pointer*, and `Resume` is an indirect
call through the pointer held in an arena slot. Control sits with the host, which
drives resumption.

This branch emits `i64 kel_yield(i64)` — a **synchronous host callback**. Control
sits with the script: the native function calls out, the host returns the resume
value, and the chunk runs to completion inside a single host call.

The difference is not stylistic, and the sharpest consequence is about memory:

**The callback keeps the native frame live across the suspension, on the C
stack.** The architecture puts coroutine frames in the arena precisely so that
suspension state is arena-accounted — that is the entire purpose of Workstream C,
"preserving the bounded-WCMU model in native code". My model puts that state
somewhere the arena does not see.

In mitigation, and stated so this is not read as worse than it is: the C-stack
usage is **bounded and measured**, by the `.stack_sizes` derivation already
closed for the WCMU half. So the state is accounted, just not where the
architecture accounts it. On a host that is defensible; on an embedded target
with a small C stack and a large managed arena it is the wrong side of the line.

### Where the rotation finding lands, and a tension in my own position

The Workstream B rotation makes this much less alarming — and exposes an
inconsistency in what I have argued versus what I have built.

`Op::Reset` clears every local, truncates the operand stack, and reclaims the
ephemeral arena, so **no state crosses a suspension** in 23 of 24 stream chunks.
For those, there is no frame to keep anywhere, and the architecture's coroutine
machinery is not needed at all. That is what the rotation section argues, and it
makes a simple ABI legitimate for the overwhelming majority.

**But the rotated form yields by RETURNING, not by calling out.** A rotated chunk
is a plain function that computes and returns the yielded value; the host calls
it again for the next one. `kel_yield` is the opposite shape — a call made from
the middle of a live frame. **I have implemented the mechanism the rotation would
replace, while arguing for the rotation.** Both cannot be the destination.

So the ABI question resolves into three cases rather than one, which is a better
frame than "callback versus coroutine":

1. **Rotation-eligible chunks (23 of 24)** — lower as plain functions returning
   the yielded value. No `kel_yield`, no coroutine intrinsics, no frame.
2. **The multi-yield chunk (1 of 24)** — genuinely needs the architecture's
   `coro.id.retcon` model with arena-allocated frames.
3. **`kel_yield` as it stands** — a development scaffold that is neither, and
   should be retired into case 1 rather than promoted.

Recording this as a debt against the provisional label, not as a defect: the
callback was a deliberate placeholder that let `Yield` lower at all and let the
order-blind oracle be closed. It did its job. It should not survive contact with
Workstream D.

### The meta-point

Four decisions were labelled provisional and documented at their definitions,
which was the right discipline. **What was missing was checking them against the
architecture that already existed.** Two were already answered — one at high
confidence, one in a full design section — and neither answer was hard to find. A
provisional label records that a decision is unsettled; it does not establish
that nobody else has settled it.

## R4.2 is not currently CONSTRUCTIBLE, but its worst defect is fixable today

Having found that `kel_chunk_<index>` contradicts resolved design question R4.2,
the next question is what adopting R4.2 would take. Checked against the fields
the lowering actually receives. The answer splits three ways.

### One component maps exactly, which is a good sign about R4.2

R4.2's category `F`/`Y`/`L` (fn, yield, loop) corresponds **one to one** with
`Chunk::block_type`:

| `BlockType` | Source form | R4.2 category |
|---|---|---|
| `Func` | `fn` — atomic total function | **F** |
| `Reentrant` | `yield fn` — non-atomic total function | **Y** |
| `Stream` | `loop fn` — productive divergent function | **L** |

An exact correspondence, available in the bytecode today. R4.2 was evidently
written against this language rather than adapted from another, which raises
confidence in the rest of it.

### Two components DO NOT EXIST, and not merely in the bytecode

- **Purity `P`/`I`/`T`.** Not in `bytecode.rs`, and not in `ast.rs` or
  `typecheck.rs` either. **Purity is not a language feature yet.** The
  architecture refers to "possibly Keleusma `impure fn`" as a prospect, so R4.2
  anticipates an annotation nobody has built.
- **Module path.** No `module_path` concept anywhere in the compile pipeline, and
  `Module` carries no name field at all.

So R4.2 is a **resolved design for a future state of the language**, not an
implementable specification today. That is not a defect in R4.2 and this entry
should not be read as one. It does mean the conflict recorded above cannot be
closed by simply adopting the scheme: two of its five components have nothing to
compute from.

### But the WORST property of the current scheme is fixable now, cheaply

The serious defect in `kel_chunk_<index>` is not that it lacks purity or a module
path. It is that **the name is an index**, so adding, removing or reordering a
chunk silently renames every symbol after it — which breaks cross-module linkage
and hot replacement, the two things a stable name exists for.

`Chunk::name` is carried in the bytecode and is not positional. **Switching the
symbol from the index to the name fixes the stability defect immediately**,
independently of R4.2 compliance, and a forward-compatible shape costs nothing
extra:

```
_K1_<category>_<chunk_name>        // purity and module path omitted, not faked
```

Adopting the `_K1_` prefix and the real category now means the eventual purity
and module-path components are an **extension** rather than a rename, so the
symbols do not churn twice.

### The one thing to verify before doing it, and it is a real risk

**`Chunk::name` uniqueness is unverified.** Two functions in different scopes
could share a name, and monomorphisation may or may not encode type arguments
into the name it emits. If names collide, swapping index for name trades an
unstable-but-unique symbol for a stable-but-ambiguous one, which is worse: a
duplicate symbol is a link error at best and a silently wrong call at worst.

Queued as a corpus measurement with the others: count distinct `chunk.name`
values against chunk count across the corpus. **Do not make this change before
that count comes back**, which is exactly the discipline this document failed at
when it quoted a chunk-level coverage figure without checking the module level.

## The four queued corpus counts are WRITTEN, and one bug was caught before running

All four questions this document deferred rather than guessed are now a single
spike, prepared while another session's gate held the machine. One `cargo test`
answers all four, so the machine is occupied once instead of four times.

| # | Question | Decides |
|---|---|---|
| 1 | How many composite constants exist, and do they co-occur with construction? | Whether refusing them costs real coverage |
| 2 | Coverage restricted to `src/selfhost/kel/` | Whether roadmap Order 1 sits behind the width stack |
| 3 | Is `Chunk::name` unique, within and across modules? | Whether the symbol stability fix is safe to make |
| 4 | Does the corpus use fixed-point at all? | Whether the scoped fixed-point design is worth building |

Each carries its own guard against the failure it is most likely to hide:

- **Count 1 is labelled a PROXY in the test itself.** It counts chunks holding
  both a composite constant and a construction, which over-approximates.
  Establishing that a *specific* constant reaches a *specific* `NewComposite`
  needs the shape stack — the very thing the measurement exists to scope. Saying
  so in the test rather than in a commit message keeps the caveat attached to the
  number.
- **Count 3 separates within-module from cross-module collisions**, because they
  have different consequences: a within-module collision is an immediate
  duplicate symbol, while a cross-module one is the hazard R4.2's module-path
  component exists to prevent.
- A shared **must-fire guard** asserts the corpus is actually being read, since
  every conclusion above is of the form "if this is zero, the work is
  unnecessary" — and a zero from a broken path reader looks identical to a real
  one.

### The bug caught before it ran

`is_composite_const` omitted **`ConstValue::Enum`**. Flat enums are composites and
the reference compiler emits enum constants, so the predicate would have
undercounted — **and the undercount ran in the direction that licenses skipping
work**, toward "composite constants cost nothing". A false zero on count 1 would
have removed a real coverage cost from consideration.

Found by verifying the variant list against `bytecode.rs` rather than trusting
recall. That verification step is exactly what was missing last night, when a
waiter was written, relied upon, and never checked for its ability to fire. The
difference between the two outcomes is one read of the source.

An unused `NewCompositeOperand` import was removed in the same pass; it would
have failed `clippy -D warnings` and cost a compile cycle on a machine that is
currently contended.

## The architecture already answers WCET, my design is its DEFERRED posture, and it leaves a real gap

Having been caught once by scoping a symbol scheme without reading the resolved
design question, the same check was applied to Workstream E. **The architecture
has a full section on WCET and WCMU preservation and I had not read it.** Same
mistake, caught one step earlier this time.

### The roadmap's open decision is an impoverished version of a settled answer

`V0_3_X_ROADMAP.md` open decision 1 asks whether native WCET is "hard or
best-effort". I called that a false dichotomy. **It is worse than that: the
architecture offers THREE postures and has already chosen one.**

| Posture | Content | Status |
|---|---|---|
| 1 | Best-effort. Bytecode WCET is a soft upper bound; operators needing hard real time use the **bytecode shape**. | **Ships with V0.4.0** |
| 2 | Measurement-based. Benchmark the native artefact on target under worst-case input. | V0.4.x, "the rigorous path" |
| 3 | Per-target cycle analysis of the native output. | **V0.5+ research, explicitly not V0.4.0** |

**What I designed is posture 3** — recover per-block instruction sequences, apply
a target cycle table, maximise over the proven path. Technically sound as far as
it goes, and scoped to the posture the architecture defers *past* the milestone
this line targets. It is not the near-term path and I presented it as though it
were.

What survives from that work is genuinely useful **to posture 3 whenever it is
picked up**, and neither point is in the architecture: that `llvm-mca` must not
be used for it, being a throughput analyser whose output is nearer a lower bound
than an upper one; and that posture 3 is only *sound* on in-order cores with no
cache, so it is a Tier 2 technique and no Tier 1 target admits it.

### The gap: "WCMU is preserved" covers the arena, not the C stack

The architecture states WCMU is the easier half and is preserved across native
compilation, on the grounds that "the master arena layout is fixed at compile
time and does not change under LLVM optimisation. Native code accesses the same
arena structure the bytecode would."

**That argument is sound for the arena and does not cover the C stack.** The VM's
operand stack and call frames live *in the arena*; native code instead consumes a
**machine stack the bytecode model never accounts for**. LLVM decides frame sizes,
and spills, inlining and register pressure change them — precisely the
optimisation-driven variability the section invokes to explain why WCET is hard.

So native compilation introduces a bounded resource the architecture's WCMU
argument does not reason about. **The `.stack_sizes` derivation already closed on
this branch is exactly the missing piece**, and it is not redundant with the
architecture's claim — it covers a different resource.

This also sharpens the `kel_yield` conflict recorded above. A synchronous
callback keeps the native frame live across a suspension **on that same
unaccounted C stack**, whereas the `coro.id.retcon` model the architecture
specifies puts the frame in the arena, where the preserved-WCMU argument applies.
The two findings are one finding seen twice.

### Also noted: Workstream F has a complete specification already

Partial-operation lowering (B35 P8) is not a design problem. The normative
contract — including per-target hardware basis and the canonical-zero and
lowest-valid resolutions the guards consult — is in
[`../spec/RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md), described as "complete
and reviewable now; only the lowering is deferred". The defaults are specified
concretely: zero for integer division by zero, the numerator for modulo by zero,
the canonical zero or lowest-valid value for an out-of-bounds index, and the
zero-discriminant variant for an invalid discriminant, with a native-call failure
trapping on both backends because a host failure has no safe default.

Worth recording against the ordering: **Workstream F is implementation against a
finished spec**, which makes it cheaper than its position in the workstream list
suggests, and cheaper than anything still requiring design.

## The architecture's ONE unaudited item is only half-closed, and my own test is why it looks closed

R4.4 records the toolchain at **high** confidence and, notably, credits this
branch: the inkwell 0.8 → 0.9 correction, the LLVM 22.1 pin, and the MacPorts
`-L native=/opt/local/lib` link path were all fed back from here. The
boundary-polling protocol worked in that direction and there is no conflict to
report — my toolchain matches the resolved decision exactly.

R4.4 leaves precisely one thing open, at **medium** confidence:

> *"Whether inkwell exposes `coro.id.retcon` with a safe wrapper still requires a
> source-tree audit when implementation begins."*

### The audit, done

**inkwell 0.9.0 has no coroutine wrapper at all.** Grepping its source for
`coro` returns exactly one file, `passes.rs`, and that is a pass-pipeline name
rather than an intrinsic surface. There is no `coro_id`, no `retcon`, no
`coro_begin`, no `coro_suspend`.

That is not fatal, and the reason is already recorded in
`tests/coroutine_feasibility.rs`: the generic `Intrinsic::find` plus
`get_declaration` route sidesteps the missing wrapper entirely, by asking LLVM
for the intrinsic by name rather than needing inkwell to model its `token` type.

### But my own test is weaker than it looks, in the way that matters

Two tests cover this and they are **not equally strong**:

| Family | `Intrinsic::find` | `get_declaration` |
|---|---|---|
| Switch-resume (`coro.id`, `begin`, `suspend`, …) | yes | **yes** |
| Returned-continuation (`coro.id.retcon`, `.retcon.once`) | yes | **NO** |

`find` proves LLVM knows the **name**. `get_declaration` proves inkwell can
actually **emit a declaration**, which is the thing R4.4 is asking about and the
thing Workstream B would fail on. `coro.id.retcon` takes allocator and
deallocator function pointers, so it is a strong candidate for being overloaded —
exactly the case where `find` succeeds and declaration does not, since
`get_declaration` returns `None` for an overloaded intrinsic given no overload
types.

So the family the **architecture actually specifies** is validated only by a name
lookup, while the family it does *not* specify is validated properly. That is the
wrong way round, and it is my test that makes the item look closed when it is
half open.

**This is the must-fire problem again.** A test asserting `find(...).is_some()`
cannot fail for the reason that would actually block the workstream. It is not
vacuous — the name genuinely might have been absent — but it does not test
declarability, and declarability is the question.

### Queued, small, and it settles a medium-confidence item in the architecture

Extend `the_returned_continuation_family_exists_as_well` to attempt
`get_declaration` for both retcon intrinsics, supplying overload types where
required, and to say plainly in its failure message that a `None` here forces
either the `coro_intrinsics.rs` llvm-sys escape hatch R4.4 anticipates or the
switched-resume form.

Whichever way it goes is worth knowing before Workstream B starts, and it
converts R4.4's confidence on that clause from medium to measured. Needs
compilation, so it joins the queue.

## OUT OF SCOPE settles decision 2, and my oracle runs on the excluded path

`V0_4_0_NATIVE_CODEGEN.md`'s **Out of scope** list is unambiguous:

> **"JIT compilation. V0.4.0 is AOT only."**

### Roadmap open decision 2 is not open

The roadmap asks "whether V0.3.x pursues ahead-of-time only or admits a JIT
path". The architecture it names as authoritative answers it: **AOT only, JIT
explicitly excluded** — permitted later "if a use case demanded it", not
delivered.

That is now the **third** of three roadmap "open decisions" found already settled
in the source document. Decision 1 is resolved to posture 1; decision 2 is
resolved here; and the section header itself claims these were "carried from
`V0_4_0_NATIVE_CODEGEN.md`'s open questions". **They were carried and then
resolved upstream, and the roadmap was not updated.** The `v0.2.3` session
independently reached the same conclusion about the Order-1 gate row. The
open-decisions section is stale as a class, not in one entry.

My earlier contribution to decision 2 — that both shapes work so it is a
support-and-maintain question — was arguing a case that had already been decided.
The AOT/JIT feasibility evidence stands; the framing was uninformed.

### The consequence for testing is real, and it is uncomfortable

Measured across the test files:

| Path | Files | Optimisation |
|---|---|---|
| **JIT** — `create_jit_execution_engine` | 3 (`differential`, `shared_data`, `yield_sequence`) | `OptimizationLevel::None` |
| **AOT** — object emission and linker | 4, but only `aot_linkage` links a real program | `default<O2>` |

**The differential oracle — the correctness signal for this whole workstream —
runs almost entirely on the JIT at `OptimizationLevel::None`, which is the
configuration the architecture excludes from the deliverable.** The shipped shape
is AOT at `O2`, and it is covered by a single end-to-end test.

### And the vacuous control proves the gap MATTERS

This is not a theoretical concern about fidelity. Earlier today the must-fire
check found that the unwritten-local control **passed against the unfixed
lowering**, because an uninitialised `alloca` loaded immediately read zero and
matched by accident.

**That accident is a property of the O0 JIT path.** At `O2` LLVM does not merely
leave `undef` alone — it *exploits* it, propagating and deleting branches on the
assumption it can take any convenient value. The same defect that was invisible
at O0 could produce actively wrong control flow at O2.

So the coverage gap already hid one real defect from its own control, and the
place it hid it is exactly the place the product ships. That converts this from a
tidiness observation into a soundness one.

### Queued, and it changes priority

Add an AOT-and-`O2` arm to the differential oracle rather than leaving
`aot_linkage` as the sole representative of the shipped configuration. The
harness for it already exists — `aot_linkage.rs` writes an object, links against
a C `main`, and runs it as a separate process — so this is extension rather than
invention.

**This outranks the width stack.** More coverage of a configuration that is not
shipped is worth less than any coverage of the one that is, and the argument for
that is no longer abstract: the O0 path demonstrably concealed a defect from the
control written to catch it.

Also noted from the same list, against future work: **LTO is permitted within a
hot-replacement boundary and suppressed across boundaries**, and per-target WCET
analysis is confirmed V0.5+ — the posture-3 design recorded above is out of scope
for this milestone, not merely deferred within it.

## The O2 arm is written, and the earlier recommendation was over-specified

The entry above recommended "an AOT-and-`O2` arm" for the differential oracle.
Reading `aot_linkage.rs`'s harness before writing it showed that phrase conflates
**two independent dimensions with very different cost and risk**:

| Dimension | What it catches | Cost per case |
|---|---|---|
| **Optimisation level** (O0 → `default<O2>`) | `undef`/poison exploitation, pass miscompilation | One `run_passes` call |
| **Delivery shape** (JIT → object + linker) | Calling convention, symbol emission, real linkage | Object write, link, subprocess |

**The soundness risk lives entirely in the first**, and it is nearly free. The
second carries integration risk and is already sampled by `aot_linkage.rs`.
Closing dimension 1 across the corpus and sampling dimension 2 is the right
split; routing every differential case through a linker would buy little for a
large cost.

So the prepared arm runs the real `default<O2>` pipeline and then JITs, rather
than emitting objects. Recording the correction because the earlier phrasing
would have produced a slower, more fragile test for no additional soundness.

### One property is genuinely O2-only, and the arm pins it

An arm that re-runs passing cases at a second optimisation level can look like
coverage while being unable to fail differently from the O0 arm. The arm
therefore **re-verifies the module after `run_passes`**, which no O0 test can do
because no O0 test runs a pass. That is the one assertion here that is
structurally impossible to make elsewhere in the suite.

### Two API errors caught before compiling, both by reading the existing harness

- I drafted `TargetMachine::get_host_cpu_name()` / `get_host_cpu_features()`.
  Those return `LLVMString`, not `&str`, and would not have compiled.
  `aot_linkage.rs` uses `"generic"` and `""` — and matching the shipped emitter's
  settings is the *point* of this arm, so copying it was right on both counts.
- I assumed `a + b` at `i64::MAX` might trap and invalidate a case. Checked:
  `wrapping_addition_agrees_with_the_vm` already uses `[i64::MAX, 1]` and
  `[i64::MIN, -1]`, so addition **wraps** here and the case tests the triple's low
  word rather than failing for an unrelated reason.

Both found by reading the file the new code sits beside, which is the same
one-command check that separated today's caught bugs from last night's
uncaught one.

## The retcon declarability probe is written; prep is now complete

The last identified-but-unwritten item. It closes R4.4's one medium-confidence
clause by measuring, rather than by reading further.

**Reporting rather than asserting, and that is a deliberate shape.** Whether LLVM
22.1 lets inkwell declare `coro.id.retcon` is a fact about the toolchain, not
about our code. A red test would block the suite over something no change of ours
could fix. The probe therefore prints the answer and asserts only what a defect
on our side could break — that the intrinsic NAMES are still present, whose
absence would contradict the existing test and mean the toolchain moved under us.

The printed outcome decides Workstream B's shape three ways: declarable means the
architecture's design is reachable through inkwell alone; overloaded means supply
the types, extra work rather than a blocker; neither means the `coro_intrinsics.rs`
`llvm-sys` escape hatch R4.4 anticipates, or the switched-resume form instead.

It also prints the switch-resume family beside it, so a reader sees both rather
than correlating two tests — the asymmetry between them being the finding that
prompted this.

One more compile error caught by reading first: `get_declaration(&m, &[])` does
not infer `BasicTypeEnum` from a bare empty slice. The existing test in the same
file types it explicitly, and copying that was the difference between compiling
and not. **That is the fourth API error this session found by reading the
neighbouring code instead of trusting recall**, against one — last night's
waiter — that was not checked and cost two sessions a night.

### Prep is complete

Everything identified is now written and dry-checked:

| Artefact | State |
|---|---|
| Three lowering fixes | **Applied and committed**, 68 tests green |
| Two must-fire controls | **Applied**, both verified to fire against unfixed code |
| Four corpus counts | Written, `ConstValue::Enum` undercount caught |
| O2 differential arm | Written, two API errors caught |
| Retcon declarability probe | Written, one API error caught |

Nothing further can be prepared without the machine. The remaining work is to
gate, run the three prepared spikes, and act on what they report — and the
measurements are deliberately front-loaded, because three of the six queued items
are "do X only if the count says X is worth doing".

## CORRECTION to "prep is complete": the mislabel fix was queued and never written

The entry above declared prep complete. **It was not.** The Workstream C
mislabelling was recorded as "queued with the other fixes" and no patch existed
for it — a queued item counted as prepared because it had been *decided*, which
is precisely the distinction that entry was drawing.

Now written and dry-verified: six sites, all six anchors unique.

| Site | Kind |
|---|---|
| `src/lib.rs:463` shared composite body | **Shipped `LowerError` string** |
| `src/lib.rs:481` Text slot | **Shipped `LowerError` string** |
| `src/lib.rs:521` shared composite array | **Shipped `LowerError` string** |
| `src/lib.rs:1099` composite constants | Comment |
| `tests/spike_corpus_coverage.rs:79` | Bucket label in a report |
| `tests/differential.rs:160` | Comment |

Two choices in the replacement worth stating rather than leaving to be inferred:

- It reads **"Workstream A (full pass)"**, not bare "A". A lone letter is exactly
  what made the original error easy to commit and hard to spot; spelling out the
  phase costs nothing and removes the ambiguity that caused this.
- The two comment sites gain an explicit **"NOT Workstream C, which is
  arena-resident coroutine frames"**. A correction that only removes the wrong
  label invites the same mistake again, since the letter is adjacent and the
  distinction is not obvious from the surrounding code.

No behavioural change: three diagnostic strings, two comments, one report label.

### Why this kept slipping

It was recorded three times as queued and never converted into a patch, while
smaller items were. The reason is that it looked like tidying, and tidying loses
to anything that looks like a defect. **But four of the six sites are strings a
consumer reads when the lowering refuses their program**, and they currently
direct that consumer to the wrong workstream. Misrouting a user is a defect that
happens to be spelled like a typo.

## ARTEFACT LEDGER: which prepared patches are SPENT and which are PENDING

Auditing the queue rather than asserting completeness a third time turned up a
hazard that was not recorded anywhere: **two of the six prepared artefacts have
already been applied**, and one of them presents a *mixed* anchor state that
invites misreading.

| Artefact | State | Notes |
|---|---|---|
| `apply_queued_fixes.py` | **SPENT** | Applied in `ce77aa1`. **Do not re-run.** |
| `queued_controls.rs` | **SPENT** | Appended, then the first control was rewritten as structural |
| `spike_queued_counts.rs` | **SPENT** | Installed and run in `ea69282`; produced the four counts |
| `o2_differential_arm.rs` | PENDING | Optimisation-level arm |
| `retcon_declarability.rs` | PENDING | R4.4's open clause |
| `fix_workstream_label.py` | PENDING | Six sites, all anchors re-verified against current source |
| `spike_stream_sufficiency.rs` | PENDING | Does `Stream` alone free the stages, and the bytecode yield shape |

> **THE PENDING ARTEFACTS NOW LIVE IN THE REPOSITORY**, at
> `native_codegen/pending/`, with a `README.md` that records install targets and
> the two API mistakes this branch has already made writing code without
> compiling it. They were held in a session scratchpad until 2026-08-10, which
> **does not survive the session**, and four had accumulated there. Cargo does not
> build that directory, so parking an uncompiled artefact there cannot make the
> tree red. **This ledger remains the only authority on what is spent**; the
> directory deliberately carries no state.

### The mixed-anchor trap, and why re-running is safe anyway

`apply_queued_fixes.py` has four anchors. Against the current source, **two still
match and two do not**:

- Anchors 1 and 2 survive because those fixes *inserted* text **after** the
  anchor. The anchor text is still there; the change sits beside it.
- Anchors 3 and 4 are gone because those fixes *replaced* their anchor text.

So a reader checking "do the anchors still match?" gets `2 of 4` and no clean
signal either way. **Re-running would double-apply the first two fixes** — a
second zero-initialisation loop and a second implicit-`ret` block.

It does not, and the reason is structural rather than lucky in the way that
matters: the script performs every replacement in memory and writes **once at the
end**, so the failing assertion on anchor 3 aborts before any write occurs. The
`assert old in s` discipline, adopted after an edit script silently matched
nothing and cost five diagnostic rounds, turns out to also prevent partial
re-application. **A guard written for one failure mode covered a second one it
was not designed for**, which is an argument for guards that fail loudly rather
than guards that are narrowly targeted.

### Why this belongs in the record

A resuming session — or this one after a compaction — would find six scripts in a
scratchpad with no indication that two are spent. The scratchpad is not durable
and carries no state. **This table is the durable part**, and it is the piece
that was missing when "prep is complete" was declared twice.

The general form, since this is the third instance today: *decided* is not
*written*, and *written* is not *unapplied*. Each transition needs its own
record, because none of them is visible from the artefact alone.

## FALSE PREMISE, caught by the `v0.2.3` session: stream chunks do NOT end in `Return`

They fixed the `verify()` finding reported from here, demonstrated it
independently first, and in doing so **falsified a claim this document made
twice**. Recording it in full because the reasoning error matters more than the
fact.

### What I claimed, and what is true

I wrote that "the reference compiler always emits a trailing `Op::Return`
(`src/compiler.rs:5342`, `5414`)", and concluded the end-of-chunk hazard was
unreachable from reference output — exposure limited to hot swap and precompiled
bytecode. **Both cited lines are inside the `else` branch.** Three lines above:

```rust
if block_type == BlockType::Stream {
    fc.emit(Op::Stream);  compile_block(..)?;  fc.emit(Op::PopN(1));
    fc.emit(Op::Reset);          // <- a stream chunk ends HERE
} else {
    compile_block(..)?;  fc.emit(Op::Return);
}
```

**Every `loop fn` ends in `Op::Reset`, never `Return`.**

### The reasoning error, which is the transferable part

I grepped for `Op::Return` emission, found two sites, and concluded *always*. The
contradicting branch was in the same `if`/`else`. **A grep confirms presence; it
cannot establish universality**, and I used it for the latter. This is the same
shape as the chunk-versus-module coverage error and the `count * word_bytes`
coincidence: a measurement that supports the expected answer, accepted without
asking what it excludes.

Their evidence was better than mine and cost more: rejecting fall-through broke
**37 library tests at once**, on real compiler output.

### What survives, and what does not

- **The conclusion survives, with corrected reasoning.** The runtime fall-off
  path is still not reachable from reference output — not because every chunk
  ends in `Return`, but because **every compiler-emitted chunk ends in a path
  exit**: `Return` for `Func`/`Reentrant`, `Reset` for `Stream`. `Reset` rewinds
  `ip` to just after `Stream` and returns `VmState::Reset`, so control never runs
  past the final op.
- **The scope statement does not survive.** "Exposure is hot swap and precompiled
  bytecode only" was right by accident. The verifier's depth pass *did* walk past
  `Reset` — it did not know `Reset` is a path exit — so it treated every stream
  chunk as falling through. Real compiler output was affected all along, at the
  verifier rather than at run time.

Their fix is the correct one: teach the depth pass that `Reset` exits the path,
and stop discarding the terminal depth both passes already compute.

### The warning they attached, and whether it lands

> *"if any lowering assumes a chunk ends in `Return`, the stream form does not."*

**Checked. No current impact, and one future trap.** Queued Fix 2 emits an
implicit `ret` when the final block has no terminator, which cannot misfire on a
stream chunk today because `Stream`/`Reset` are refused outright — the chunk never
reaches the lowering. **But when Workstream B lands the rotation, a rotated stream
body must not receive an implicit `ret` on the strength of "no terminator seen".**
`Reset` is the terminator, and it means *rewind and yield control*, not *return*.
That is now written down before the code exists rather than after it misbehaves.

## THE FOUR COUNTS CAME BACK, and they reorder the roadmap

Run on a free machine immediately after the gate went green. 73 tests, fmt clean,
clippy zero. Every one of these was deferred rather than guessed, and **three of
the four removed work**.

| # | Question | Answer |
|---|---|---|
| 1 | Composite constants in the corpus | **0** |
| 2 | Self-hosted stages that lower | **1 of 11**, and the blocker is **`Stream`**, not composites |
| 3 | `Chunk::name` uniqueness | 821 chunks, 725 names, **0 within-module collisions** |
| 4 | Fixed-point opcodes in the corpus | **0** |

### Count 2 falsifies my prior, and it moves Order 1

I predicted the self-hosted stages were composite-heavy and that **Order 1 sat
behind the width stack**, while flagging it as a prior rather than a measurement.
**It is wrong.** All ten refusals read:

```
native lowering does not yet support opcode Stream
```

**Order 1 sits behind Workstream B — sub-coroutines — not behind composites.**
The compiler's own stages are stream-driven, so `Stream` is hit before any
composite op. That is the workstream whose three preconditions are already
established here: P1 at 23 of 24, P2 verified from the `Op::Reset` mechanism, P3
at 24 of 24, with only the rotation's boundary condition open.

**One caveat, stated because the measurement cannot exclude it.** `lower_module`
refuses on the FIRST unsupported opcode, so "blocked on `Stream`" means `Stream`
is hit first, **not** that it is the only blocker. Composites may well block
these modules too, behind it. What the count establishes is that `Stream` is
*necessary*, not that it is *sufficient*.

### Counts 1 and 4 delete work

- **Composite constants: zero.** The coverage cost of refusing them under the
  width stack's unknown-is-refused rule is **nil**. The `AbsVal::Top` limitation
  recorded as a real cost costs nothing on this corpus.
- **Fixed-point: zero occurrences.** The scoped design is correct and worth
  **zero coverage today**. It was explicitly recorded as "unmeasured, and
  deliberately not guessed"; the measurement says do not build it. That caution
  paid for itself.

### Count 3 clears the symbol fix, with a boundary

**0 within-module collisions** across 821 chunks, so switching the symbol from
chunk index to `Chunk::name` is safe for the single-module lowering that exists
today, and it fixes the stability defect immediately.

**96 names are shared ACROSS modules** (821 − 725). That is precisely the hazard
R4.2's module-path component exists to prevent, and it becomes live the moment
cross-module linkage does. So the interim scheme is sound now and has a known
expiry, which is the honest way to hold it.

### Net effect on ordering

1. **Workstream B (the rotation) now outranks the width stack**, because it gates
   Order 1 and the width stack does not.
2. **The fixed-point family drops off** the list entirely.
3. **The composite-constant caveat disappears** from the width stack's design.
4. **The symbol fix is unblocked** and is the cheapest item remaining.

Three of four counts subtracted work. That is the argument for measuring before
building, made in numbers rather than in principle.

## DESIGN: the rotation, now that the counts make it the top item

Count 2 put Workstream B ahead of the width stack — ten of eleven self-hosted
stage modules refuse on `Stream`, so **Order 1 is gated here**. The rotation has
been a hypothesis with preconditions since yesterday; this specifies it.

### The structural facts it rests on, each established rather than assumed

1. **A stream chunk's shape is fixed by the compiler** (`compiler.rs:5334`):
   `[prologue] Stream [body] PopN(1) Reset`. The `else` branch emits `Return`
   instead; only `Stream` chunks get this form.
2. **`Reset` rewinds to just after `Stream`**, so the prologue runs exactly once
   and is *outside* the repeating region.
3. **`Reset` clears everything but the data segment** — locals to `Unit`, operand
   stack truncated, ephemeral arena reclaimed with an epoch bump, opaque registry
   cleared. Verified from the `Op::Reset` mechanism, not inferred.
4. **A top-level `Yield` delimits a segment.** `wcet_stream_iteration` tracks
   block-nesting depth and **bails to `None` if any `Yield` is nested** inside an
   `If` or `Loop`. So the analysable case is exactly the flat one.

> **FACT 4 IS FALSE, AND SO IS THE SECTION BELOW THAT RESTS ON IT.** See
> "FALSIFIED: `wcet_stream_iteration` computes no segments" at the end of this
> document. `wcet_stream_iteration` performs a single whole-body computation with
> no depth tracking and no yield scan of any kind. The depth-tracking bail exists,
> but in a different private function serving a different block type, and it has
> never run on a `Stream` chunk.

### The insight fact 4 supplies

**The rotation is a permutation of the segments the WCET pass already computes.**
That pass splits a stream body at its top-level yields to bound each resumption
separately; the rotation reorders those same segments. It is not a new program
analysis — it reuses one that exists, and it inherits that pass's own
admissibility condition, which is a better place to draw the line than any I
would have invented.

### The transformation

For a single top-level yield — 23 of 24 stream chunks — the body is
`A ; Yield ; B`, and the VM's cycle is:

```
prologue ; [ A ; Yield ; B ; PopN(1) ; Reset ]*
```

The rotated native form runs `B` for the PREVIOUS resume value, then `A`:

```
kel_chunk_N_init()            -> prologue ; A ; return yielded
kel_chunk_N_step(prev_resume) -> B(prev_resume) ; clear locals ; A ; return yielded
```

**Two entry points rather than a flag**, because that is what makes the boundary
condition disappear instead of being handled: `init` has no previous resume value
*by construction*, so there is no first-call branch to get wrong. The host calls
`init` once and `step` thereafter — which is exactly the shape the existing
`Vm::call` / `Vm::resume` pair already has, so the ABI mirrors the runtime's own
rather than inventing a convention.

`clear locals` is the emulated `Reset`, and by fact 3 it is the *whole* of what
`Reset` does that the rotated form must reproduce: the operand stack is
function-local in native code, the ephemeral arena has no native analogue yet,
and the data segment is meant to survive.

### What this does NOT resolve, stated plainly

- **Equivalence is still unproven.** The preconditions are necessary; this
  specifies the transformation but does not prove `B;A` observationally equals
  the suspended form. The oracle for that exists — `tests/yield_sequence.rs`
  compares whole yield sequences — and it is the only thing that can settle it.
- **The nineteen-yield chunk is out of scope** and needs real coroutine frames,
  which is where the architecture's `coro.id.retcon` design applies.
- **`kel_yield` is superseded by this**, not extended. The callback keeps a frame
  live across the suspension on the unaccounted C stack; the rotated form returns
  instead, so no frame persists and the WCMU gap closes with it. That is the
  three-case resolution recorded earlier, now with the middle case specified.
- **Whether `Stream` alone unblocks the ten stage modules is unknown.**
  `lower_module` refuses on the first unsupported opcode, so composites may sit
  behind it. The rotation is necessary for Order 1; sufficiency is unmeasured.

## FALSIFIED: `wcet_stream_iteration` computes no segments, and the corpus does not need them

Yesterday's rotation design rested on fact 4, that the segmentation it needed
already existed and only had to be permuted. **Fact 4 is false.** I recorded it
from the shape of the surrounding documentation rather than from the function
body, which is the same failure that produced "the reference compiler always
emits a trailing `Return`", falsified by the `v0.2.3` session two increments ago.

### What the function actually does

`wcet_stream_iteration_with_cost_model` locates `Stream` and `Reset`, calls
`wcet_region` **once** over the half-open range between them, adds the two
overhead costs and the once-per-chunk external-native term, and returns. There is
no depth counter, no scan for `Yield`, and no partition. It is one number over
one region.

The depth-tracking bail I attributed to it is real, but it lives in
`reentrant_segmented_wcet`, and every property of that placement works against
the reuse I assumed.

| Property | What I assumed | What is true |
|---|---|---|
| Function | `wcet_stream_iteration` | `reentrant_segmented_wcet` |
| Reached from | the `Stream` WCET path | `wcet_whole_chunk`, which **errors on a `Stream` chunk** |
| Block type served | `Stream` | `Reentrant` only |
| Visibility | usable | private `fn`, no caller outside the module |
| Return value | the segment partition | `Option<u32>`, a single **maximum** |

The last row is the one that survives even if the others are worked around. The
boundaries live in two local variables that are dropped on return, so the
partition is not exposed for `Reentrant` chunks either. **Nothing in the codebase
today computes a reusable yield-delimited partition of anything.**

The favourable half is that the algorithm is thirty lines and its precondition is
exactly the rotation's own, namely that every `Yield` sits at nesting depth zero.
Its decline path already carries must-fire coverage at two call sites in the
verifier test module. So the correct statement is **port a small private
function**, not **call an existing computation**. That is a larger increment than
I recorded and a much smaller one than inventing the analysis.

### One soundness check on the port, which passes

A depth counter is only sound if it counts every block-opening opcode. The
block-structured set is `If`, `Else`, `EndIf`, `Loop`, `EndLoop`, `Break` and
`BreakIf`. `reentrant_segmented_wcet` increments on `If` and `Loop`, decrements on
`EndIf` and `EndLoop`, and ignores the other three. That is correct rather than
merely adequate. `Else` is a jump inside an already-open `If` and opens nothing,
while `Break` and `BreakIf` transfer control without nesting. **No block-opening
opcode is missed**, so a nested `Yield` cannot be misread as top level.

### THE MEASUREMENT THAT CHANGES THE INCREMENT, taken without compiling

The ten stage modules were read at source level for the shape of their `loop`
block. The result is far more lopsided than the design assumed.

| Class | Stages | Shape |
|---|---|---|
| **Trivial** | **8** | one top-level `yield`, and it is the **final statement** |
| Delegated | 1 | `codegen.kel`, body is `emit_next(resume)` with **no `Yield` at all** |
| Nested | 1 | `lexer.kel`, yields inside `if` and `else` |

The trivial class is `parse.kel`, `analyze.kel`, `reconstruct.kel`, and the five
`verify_*.kel` modules. Seven of the eight are literally `loop main(resume) {
yield run() }` or its equivalent over an ordinary `fn`, confirmed by checking each
callee's declared category rather than assuming it. `reconstruct.kel` runs
straight-line field assignments and then yields once at the end.

**For a body whose single top-level `Yield` is the last statement, the rotation is
the identity.** The partition has one segment, the trailing segment is empty, and
a permutation of one element reorders nothing. The transformation collapses to
something that needs no partition, no permutation, no `kel_yield` callback and no
coroutine intrinsic:

```
kel_chunk_N_step(resume) -> i64     // the body, with the trailing Yield as the return
```

That is the rotation hypothesis in its strongest form, stream chunks as plain
functions, holding by measurement for eight of the ten stages rather than by
argument.

`codegen.kel` is a genuinely different case and I had it in neither class. Its
stream body contains **no `Op::Yield`**, because the yield is delegated to a
multiheaded `yield emit_next`, and the verifier counts a call to an
always-yielding chunk as a yield. A yield-position scan over that chunk finds
nothing, so any segment-based treatment sees one segment covering the whole body
and is silently wrong about where suspension happens.

### Consequences for the plan

- **The general rotation is not the next increment.** The degenerate case is, and
  it is a much smaller piece of work that unblocks eight of the ten stages.
- **`Op::Yield` already lowers.** Only `Op::Stream` and `Op::Reset` are refused,
  and the refusal comment states the reason, which is that a divergent `loop fn`
  driven through the `kel_yield` callback would spin inside native code with no
  way for the host to stop it. The degenerate form returns to the host every
  iteration, so it **dissolves that reason** rather than working around it.
- **`Reset` still has to be reproduced.** By the earlier finding it rewinds the
  top arena region only, not both, and the data segment is meant to survive. For
  `reconstruct.kel`, which writes data-block fields before yielding, the data
  segment **is** the state carried across iterations. This is the interaction this
  branch is named for and it is not yet verified.
- **`lexer.kel` and `codegen.kel` are deferred**, not solved, and neither is
  covered by the degenerate case.

### What is NOT established, stated so a later reader does not inherit it as fact

- **This is a source-level reading, not a bytecode count.** That the eight trivial
  stages compile to exactly one top-level `Op::Yield` with only `PopN(1)` between
  it and `Reset` is a strong expectation and an unmeasured one. It needs a corpus
  count, which needs the machine, which currently belongs to the other session.
- **Sufficiency remains unmeasured.** `lower_module` refuses on the first
  unsupported opcode, so composites may sit behind `Stream` in these same modules.
  Handling `Stream` is necessary for Order 1. Nothing here shows it is enough.
- **Observational equivalence is still unproven even in the degenerate case.**
  It is far more plausible when the permutation is the identity, but plausible is
  not proven, and `tests/yield_sequence.rs` remains the only thing that settles it.

## The degenerate case's op shape is now DERIVED, and a hazard beside it does not bite

Two of the three items left unestablished above were addressed by reading the
emission path rather than by counting, which is worth separating from the
measurement because a derivation and a count fail in different ways.

### Derived, not expected

`Expr::Yield` emits exactly `compile_expr(value)` followed by `Op::Yield`, with
nothing after it. A single-headed stream chunk emits `Op::Stream`, the body,
`Op::PopN(1)`, `Op::Reset`. A block's tail expression is its value, and `Yield` is
pop-one push-one, so for `loop main(resume) { yield run() }` the whole chunk is

```
Stream ; Call(run) ; Yield ; PopN(1) ; Reset
```

and the `PopN(1)` discards the resumed value that `Yield` pushed. **The claim that
only `PopN(1)` separates `Yield` from `Reset` is therefore structural**, holding
for any single-headed stream body whose tail expression is a `yield`, rather than
an accident of these particular modules. A corpus count would still be worth
having, since it would also catch anything a later pass appends, but the shape no
longer rests on expectation.

### A hazard that would have voided the trivial class, checked and clear

A **multiheaded** stream chunk takes a different emission path entirely. The
dispatch is wrapped in `Op::Loop` and `Op::EndLoop`, so **every `Yield` in a
multiheaded stream chunk sits at nesting depth one or greater and is nested by
construction.** No such chunk can ever be in the degenerate class, whatever its
source looks like.

All ten stage modules declare `loop main(resume: Word) -> Word` with no `when`
clause, so none takes that path. Had even one been multiheaded, the trivial class
would have been smaller than measured and the measurement above would have been
wrong in the optimistic direction.

This is the more durable half of the finding. The eight-of-ten split is a fact
about today's corpus and will drift. **Single-headed is a precondition of the
degenerate lowering in the language**, and it belongs in the admissibility check
rather than being rediscovered when a stage gains a second head.

All five `verify_*.kel` modules were read individually rather than sampled, and
all five are byte-identical in this respect.
