# BRIEF — the type × operator matrix, and the cell where the reference traps

## Present goals

The previous increment gave the opcode census a denominator and generalised the
finding: **the minimal-ISA constraint manufactures name-keyed blind spots**, so
key on the operand or enumerate the variant space from the types. That closed one
of two variant axes — the opcode's own operand field.

**The other axis is the runtime TYPE of the operands, which has no field in the
bytecode at all.** `Op::Add`, `Op::Mod`, `Op::Shl` and the rest carry nothing; the
virtual machine dispatches on the values it pops. That axis produced the three
defects this line has already fixed.

## The finding

`operand_variant_sweep.rs` covers that axis with **27 hand-written cases** and no
denominator — the same shape as the census before the last increment.

Driving the matrix properly — three scalar types times twelve binary operators,
two unary operators and six comparisons, plus the boolean family, **66 cells** —
found exactly one divergence:

**`Fixed % Fixed`.**

- The reference **compiler accepts it** and emits a plain `Op::Mod`.
- The reference **virtual machine traps at runtime**:
  `TypeError("cannot modulo Fixed by Fixed")`. Its `Op::Mod` arm handles
  `Int`/`Int`, `Byte`/`Byte` and `Float`/`Float`; `Fixed` falls to the catch-all.
- The **backend lowers it without refusal and returns a value** — `200.0 % 7.0`
  yields `4.0` in Q16.16, mathematically right and *not what the reference does*.

**This is two findings, and they belong to different lines.**

1. **Upstream (`v0.2.3`).** The type checker admits a program the virtual machine
   refuses. The operand types are statically known, so a static type error is
   escaping to runtime. `%` has no Fixed-aware opcode the way `*` and `/` have
   `FixedMul` and `FixedDiv`. **Report; do not repair** — `src/` is read-only to
   this line.
2. **Here.** The backend produces a value where the reference produces an error.
   The differential oracle is the correctness signal and it says they disagree.
   The conservative stance is to fail loudly rather than emit a module neither
   side vouches for.

## Why the sweep could never have found this

`run_both` panics on any virtual-machine outcome that is not `Finished`. **A
trapping cell cannot be represented in that matrix** — adding one would panic the
harness rather than record a result.

The last increment's defect hid behind an instrument's *keying*. This one hides
behind an instrument's **outcome type**: the harness could express *agree* and
*disagree*, but not *the reference refuses*. An instrument can only find defects
it has a vocabulary for.

## What to build

1. A matrix with a denominator, enumerated from the scalar types and the operator
   set rather than hand-listed, with every cell classified — including the classes
   the old sweep could not express: the reference compiler rejects it, the
   reference traps and the backend refuses, and **the reference traps and the
   backend computes**, which is the defect class.
2. The backend fix. `OperandKind` is `Int | Float | Unknown`, so `Fixed` is
   indistinguishable from `Word` on the operand stack — **which is exactly why the
   backend got this wrong.** The signature table carries `ScalarKind::Fixed`, and
   parameter kinds are already seeded from the declaration for floats. Extend that
   lattice and refuse `Op::Mod` on a Fixed operand.
3. The upstream report, watched by a guard that fails when the behaviour changes,
   so the report cannot outlive its subject.

## Prior failures to avoid repeating

- **A rejection recorded without checking WHY it was rejected.** `Bool` was filed
  as "comparisons are refused" when the real cause was an unknown type name. In
  this matrix, `byte lsl` with a `Byte` shift amount is rejected but the same
  shift with a `Word` amount and with a literal both compile and agree. **Record
  the amount type, not "byte shifts are refused."**
- **A count cannot see a cell change class.** Classify every cell; do not total
  them.
- **Do not "fix" the mathematics.** The native answer for `Fixed % Fixed` is
  arithmetically correct. It is still wrong, because the reference's behaviour is
  to trap. Matching the reference is the contract.
- **Do not repair `src/`.** The type-checker hole is upstream's to rule on.
- **Prefer precision to a coarse refusal** if the signature makes it available,
  but a coarse refusal beats a wrong number. Refusing more than necessary is
  within the conservative stance; emitting a fabricated value is not.

---

## OUTCOME

**Built, and it found a defect.** `native_codegen/tests/scalar_operator_matrix.rs`
enumerates **90 cells** from the type list and the operator list: 69 agree, 20 are
refused by the reference compiler, and **one is the defect** —

**`Fixed % Fixed`: the reference traps, the backend returned `4.0`.**

Fixed in the emitter. `OperandKind` gained a `Fixed` variant, seeded from the
chunk signature's `ScalarKind::Fixed` tag — the LLVM type cannot carry it, since a
`Fixed` parameter *is* an `i64`, **which is exactly why the backend got this
wrong**. `Op::Mod` and `Op::Div` now refuse a `Fixed` operand, naming the
reference's `TypeError` as the reason. `word %` still lowers, so the refusal is
precise rather than coarse.

**Reported upstream as report 4**, guarded from both sides: if the compiler starts
rejecting the program the first assertion fires, and if the virtual machine starts
running it the second does. This line asserts the observation, **not which repair
is right** — rejecting `%` on `Fixed` in the type checker and adding a Fixed-aware
modulo are both coherent, and the second needs an opcode.

**The shift rows were recorded by amount type**, not by the shifted type: a `Byte`
value shifts by a `Word` amount or a literal and agrees in both; a `Byte` *amount*
is refused; `Fixed` is refused by the shift operators in all three forms. Filing
these as "byte shifts are refused" would have repeated this morning's `Bool`
conflation exactly.

**The methodological finding.** The previous defect hid behind an instrument's
*keying*. This one hid behind an instrument's **outcome type**: `run_both` panics
on any virtual-machine outcome that is not `Finished`, so a trapping cell could
not be represented in that matrix at all. **An instrument can only find defects it
has a vocabulary for.** The old sweep now carries a statement of what it cannot
express and points here.

Every guard was mutation-checked: a flipped cell class fires, and reverting the
emitter fix reproduces `fixed %: VmTrapsNativeComputes` — the assertion catches the
original defect rather than merely passing beside it. The `ScalarKind::Fixed` tag
is pinned, because a silent renumbering upstream would fail **open**, back to the
divergence.


---

## CORRECTION, 2026-09-12 — the fixed-point scale named above

This brief describes the divergence as *"the backend returned `4.0`"* for
`200.0 % 7.0`. **Bare `Fixed` is `Fixed<32>`, not Q16.16**; the compiler emits
`FixedMul(32)` for it. The raw operands used were `200 << 16` and `7 << 16`, which
as Q32 values are approximately `0.003052` and `0.0001068`, giving approximately
`0.000061`.

**Nothing else changes.** The integer remainder of the raw words is identical under
either reading, the reference traps regardless of scale, and the refusal is
correct. Only the decimal figures in prose were wrong, and the default is now
pinned by a test so it cannot be assumed again.
