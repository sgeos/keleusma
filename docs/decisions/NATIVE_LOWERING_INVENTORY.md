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

**Lowered (31).** `GetLocal`, `SetLocal`, `PopN`, `Dup`, `Const` (scalars),
`PushImmediate`, `CheckedAdd`, `CheckedSub`, `CheckedNeg`, `CheckedMul(0)`,
`CmpEq`, `CmpNe`, `CmpLt`, `CmpGt`, `CmpLe`, `CmpGe`, `Not`, `BitAnd`, `BitOr`,
`BitXor`, `Shl`, `Shr`, `If`, `Else`, `EndIf`, `Loop`, `EndLoop`, `Break`,
`BreakIf`, `Return`, `Trap`.

**Remaining (35),** grouped below by what they actually cost.

Two entries in that list are **partial**, and the count treats them as lowered
because the unsupported case is refused rather than mislowered: `Const` handles
scalar constants only, and `CheckedMul` handles a zero fraction-bit count only.
A reader taking the count as a completeness measure would overstate coverage by
two.

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
| `PushImmediate(u8)` | Encoding is documented: `0 = Unit`, `1 = true`, `2 = false`, `3 = None`, `4..19 = Int(operand - 4)`. **Blocks on one decision**: how `Unit` and `None` are represented in a flat i64 world. Refusing them is a legitimate first answer. |
| `WordToByte` `ByteToWord` | Truncate and extend. Needs the `Byte` representation settled, including whether the extension is signed. |

## Group 2 — one design decision each

| Opcode | The decision |
|---|---|
| `Div` `Mod` `CheckedDiv` `CheckedMod` | **THE NEXT INCREMENT.** Division by zero is undefined behaviour in LLVM and a `VmError::DivisionByZero` in the VM. A guard branch to the trap block is mandatory, not optional. The same trap applies to `i64::MIN / -1`, which is also UB in LLVM. Note the checked forms do **not** trap on a zero divisor: the VM reifies it as flag `3` with the numerator in the low slot, so the handled `zero_divisor(n)` arm can bind it. Read the VM arm before lowering; the two forms differ. |
| ~~`CheckedMul(u8)`~~ | **DONE for `0`.** The operand is the Q-format fraction-bit count; zero is integer multiply. A non-zero count is fixed-point and is refused. |
| `Const(u16)` | Scalar constants are easy. Composite constants are not, and route into Group 4. |
| `BoundsCheck(u16)` | A compare and a branch to trap. Cheap once the trap path carries a reason code. |
| ~~`Loop` `EndLoop` `Break` `BreakIf`~~ | **DONE.** The structural work above. |

## Group 3 — the host and call boundary

| Opcode | Depends on |
|---|---|
| `Call(u16, u8)` | Multi-chunk lowering and the symbol mangling scheme resolved as R4.2 in the V0.4.0 strategy. Until then only single-chunk programs lower. |
| `CallVerifiedNative` `CallExternalNative` | The native application binary interface, Workstream D. Not a Workstream A item. |

## Group 4 — the workstreams that own them

These are not deferred out of convenience; each belongs to a workstream with its
own design.

- **`Stream`, `Yield`, `Reset`** — Workstream B, sub-coroutine lowering through
  the returned-continuation intrinsic family. The roadmap identifies this as
  where the risk concentrates, and it is the load-bearing primitive for V0.5.0.
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
