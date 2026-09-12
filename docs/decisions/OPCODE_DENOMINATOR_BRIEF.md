# BRIEF — give the support census a denominator

## Present goals

Workstream A's milestone is that **the whole language lowers**. The instrument
that reports progress toward it is `native_codegen/tests/backend_support_census.rs`,
titled *"WHICH OPCODES CAN THE BACKEND ACTUALLY LOWER?"*.

## The finding

**That census has no denominator.** Measured on this tree: the `Op` enum declares
**66** opcodes; the census probes **17**. Nothing in the file relates its probe
table to the instruction set, so an opcode's absence is unclassified — it could
mean covered elsewhere, unprobeable, or forgotten, and the file cannot tell a
reader which.

This is not a claim that 49 opcodes are untested. Most are driven by the
differential corpus. The claim is narrower and worse: **the census cannot say
which**, and therefore cannot report progress toward the milestone it exists to
measure.

## Why this instrument specifically

Three defects have already hidden behind this file's keying: the `Fixed` variants
of `CheckedMul` and `CheckedDiv` were refused outright, and the `Byte` variants of
`CheckedMul` and `CheckedAdd` returned untruncated values, all while the census
reported **0 refused**. Its header now records that, and closes with the honest
admission that *"this file cannot know which variants exist, only which it was
given."*

**This line has an idiom for exactly this** — pin a population, fail on growth,
demand classification rather than a patched number, always include a non-vacuity
check. It has been applied to test functions, pointer offsets, panic sites, value
movement, host buffers, and upstream premises. **It was never applied to the
census whose blind spot produced three defects.**

## The `NewComposite` case, and why the ISA constraint manufactures this

`NewComposite` carries `NewCompositeOperand`: two forms (`Flat`, `Boxed`) times
four `CompositeKind`s = **eight operand variants under one opcode name**. The
census has **no row for it at all**.

That is not an oversight peculiar to this opcode. The P4 consolidation took the
instruction set from 69 opcodes to 66 by moving a discriminant **out of the opcode
name and into an operand field** — and the rad-hard minimal-ISA constraint means
every future consolidation does the same. **The constraint that keeps the ISA
small systematically manufactures the blind spot that a name-keyed census has.**
So the fix must be keyed on something the constraint cannot move.

## What to build

A denominator derived by MEASUREMENT, not by hand:

1. Enumerate opcode names from `src/bytecode.rs` at test time. That file is the
   V0.2.3 line's and read-only to this line, which is what makes it a source of
   truth rather than a copy that rots.
2. Compile the shared corpus and record, per opcode, whether it is **emitted** and
   whether the lowering **visits** it.
3. Classify every opcode into that partition and pin it. A new opcode, or one that
   changes class, fails with a message demanding classification.
4. Do the same for the eight `NewComposite` operand variants, which are
   enumerable from the same source.

Then **read the result**, because a class boundary is where a defect sits.

## Prior failures to avoid repeating

- **A regex over a formatted file silently matched nothing.** Extracting the probe
  names with `grep -oE '\("[A-Za-z]+"'` returned **0 rows**, because `cargo fmt`
  had split the tuples across lines. A count of zero was the only reason it was
  caught. Every enumeration here asserts a non-vacuous floor.
- **A count cannot see a site change class.** Widening the pointer census to a
  bare matcher took it 22 to 38 and was reverted. Classify, do not just count.
- **A patched number hides the event.** When a population moves, the message must
  demand that the mover be named, not that the constant be edited.
- **Do not claim the unmeasured.** "Covered elsewhere" is only assertable where
  this test observed the opcode lowered. Where it observed nothing, the honest
  class is *no evidence from this corpus*, and it must be spelled that way.
- **Do not edit `src/` or `tests/` at the repository root.** Read only; report.

---

## OUTCOME

**Built and measured.** `native_codegen/tests/opcode_denominator.rs` parses the
opcode names from `src/bytecode.rs` at test time (**66**, matching an independent
count), compiles **74** corpus modules with the shipping host's own prelude
composition, and classifies every opcode by observation.

**63 Lowered. 1 EmittedNotLowered. 2 NotEmitted.**

The three non-`Lowered` entries are the result, and all three have an account:

- **`Reset`** is emitted by the corpus and never reached by the lowering. The
  support census had already recorded this from the other direction. The native
  stream rewinds its arena at the host boundary rather than at the instruction,
  so the instruction has no work to do.
- **`Len`** — B28 P2 deleted every fall-back that emitted it, because a flat
  array body cannot answer a length query. Upstream asserts its absence in two
  tests. `length(a)` is rejected by the reference outright.
- **`IsStruct`** — upstream records a **bounded search that found no producer**,
  and deliberately declines to call it unreachable, because *"the first
  producerless claim made here was falsified by another line within the hour"*.
  **That discipline is copied here**: the class says this corpus emitted none,
  and nothing stronger.

`NotEmitted` proves nothing about the backend on its own, so both opcodes are
driven into it by mutation: neither has an emitter arm at all, and both produce a
loud `UnsupportedOp` refusal rather than lowering to something. If either grows an
arm, the test fails.

**The variant axis.** All four `Flat` composite kinds lower. **No `Boxed` form is
emitted by the corpus at all**, and the emitter matches only `Flat` — so a `Boxed`
construction appearing in the corpus would be a construction the backend refuses.
That is now a failure rather than a silence. The four `NotEmitted` rows are the
expected reading of a form upstream documents as transitional.

**No lowering defect was found.** The finding is about the instrument: the census
that produced three defects through its keying had no denominator, and now the
denominator exists and is derived rather than transcribed.

Each of the five guards was mutation-checked — unclassified opcode, stale row,
moved class, moved variant, and a broken parse hitting the floor — and every one
fires. The floor check is not decoration: extracting the census's probe names
during this increment returned **0 rows** because `cargo fmt` had split the
tuples, and only the count gave it away.
