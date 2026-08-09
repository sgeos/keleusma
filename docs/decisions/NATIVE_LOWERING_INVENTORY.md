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

**Lowered (22).** `GetLocal`, `SetLocal`, `PopN`, `Dup`, `CheckedAdd`,
`CmpEq`, `CmpNe`, `CmpLt`, `CmpGt`, `CmpLe`, `CmpGe`, `Not`, `BitAnd`,
`BitOr`, `BitXor`, `Shl`, `Shr`, `If`, `Else`, `EndIf`, `Return`, `Trap`.

**Remaining (44),** grouped below by what they actually cost.

## The next increment is structural, not another opcode

`Loop`, `EndLoop`, `Break` and `BreakIf` introduce **backward jumps**, and the
current lowering cannot express one. Its merge-depth algorithm walks the opcode
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

### `for` is not the test vehicle, and the keyword is overloaded three ways

The grammar distinguishes three constructs that are easy to conflate:

| Form | What it is |
|---|---|
| `loop { block }` | The divergent loop block. |
| `loop name(..) -> T` | A **coroutine definition**, not a loop at all. |
| `for x in it [limit n] on { .. }` | Bounded iteration, and what real code uses. |

Measured from the `for` lowering in `src/compiler.rs`, a `for` statement emits
`Loop` and `EndLoop` **together with** `Stream`, `Reset`, `Yield`,
`NewComposite`, `Len`, `GetIndex`, `IsEnum`, `IsStruct` and `GetField`. So a
`for` loop is not a small program: it drags in coroutines and composites, most
of Group 4.

**Consequence for ordering.** The loop opcodes cannot be exercised in isolation
through `for`. The only isolated vehicle is the `loop { }` block, which emits
`Loop`, `EndLoop` and `Break` alone. Attempting to validate loop lowering with a
`for` loop would fail on eight unrelated unsupported opcodes and prove nothing.

None of the ten stage sources under `src/selfhost/kel/` contains a `loop { }`
block; they iterate with `for`. So the divergent block needs purpose-written
test programs rather than a corpus example.

## Group 1 — mechanical, no new design

Same shape as what already lowers. Each needs differential cases that
distinguish it, not merely exercise it.

| Opcode | Note |
|---|---|
| `Add` `Sub` `Mul` `Neg` | Direct LLVM integer operations. Confirm the VM's overflow behaviour first; do not assume it matches `CheckedAdd`'s discard-the-flag wrap. |
| `CheckedSub` `CheckedNeg` | The three-slot `low, high, flag` pattern already implemented for `CheckedAdd`. |
| `PushImmediate(u8)` | Encoding is documented: `0 = Unit`, `1 = true`, `2 = false`, `3 = None`, `4..19 = Int(operand - 4)`. **Blocks on one decision**: how `Unit` and `None` are represented in a flat i64 world. Refusing them is a legitimate first answer. |
| `WordToByte` `ByteToWord` | Truncate and extend. Needs the `Byte` representation settled, including whether the extension is signed. |

## Group 2 — one design decision each

| Opcode | The decision |
|---|---|
| `Div` `Mod` `CheckedDiv` `CheckedMod` | **Division by zero is undefined behaviour in LLVM** and a `VmError::DivisionByZero` in the VM. A guard branch to the trap block is mandatory, not optional. The same trap applies to `i64::MIN / -1`, which is also UB in LLVM. |
| `CheckedMul(u8)` | Carries a `u8` operand whose meaning has not been probed. |
| `Const(u16)` | Scalar constants are easy. Composite constants are not, and route into Group 4. |
| `BoundsCheck(u16)` | A compare and a branch to trap. Cheap once the trap path carries a reason code. |
| `Loop` `EndLoop` `Break` `BreakIf` | The structural work above. |

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

### Negative controls, run 2026-08-08, and what they found

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
