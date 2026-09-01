# Half the bound transfers, and the example presented both halves alike

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Found and narrowed 2026-09-01.**
Brief: [`NATIVE_WCET_ASYMMETRY_BRIEF.md`](./NATIVE_WCET_ASYMMETRY_BRIEF.md).

---

## What was wrong, and it was a presentation defect with a substantive consequence

`emit_object.rs` printed this, and `motor_policy/README.md` reproduced it above the line *"its worst
case was known before it ran"*:

```text
PROVEN BOUNDS, from the verifier rather than from prose:
  chunk 0 (derate_for  ) WCET     19 cost units
  chunk 1 (main        ) WCET    134 cost units
  worst case over all chunks: stack 352 B, heap 24 B
```

**One heading, two different kinds of claim.**

## The two halves

**Memory transfers, and is measured.** `tests/bound_transfer.rs` compares the operand-stack slots the
backend provisions against `RuntimeFootprint::max_operand_slots`, and `region_total_bytes` against
`max_heap_bytes`. Those figures describe the emitted object.

**Time is measured against nothing.** The cost-unit figure is a **bytecode-level** count under a cost
model calibrated for the **virtual machine**. Established by search rather than assumed: the files in
this package mentioning worst-case execution time are `corpus_differential`, `stage_differential`,
`spike_bounds_transfer`, `spike_stream_sufficiency`, `emit_object` and the `motor_policy` README.
**`bound_transfer.rs` — the instrument for exactly this question — is not among them.**

## Why it is not hypothetical

[`NARROW_TARGET_LINKAGE.md`](./NARROW_TARGET_LINKAGE.md) measured the emitted native code calling
`__adddf3`, `__divdi3`, `__gedf2` and others on `thumbv8m.main-none-eabihf`. **Those calls have no
counterpart at the bytecode level**, so a per-opcode cost model cannot describe them at all — not
inaccurately, but not at all.

## The claim that is actually supported, which is smaller than the one available

**No measurement in this project relates the bytecode cost model to native execution.**

**Not** that the native code is slower than the figure. **Not** that any bound is violated. **Not**
that compiler-runtime routines are unbounded in fact — they are unbounded **by this project**, and a
vendor may well publish timings for them.

## What changed

The figure is **kept**, because it is true about the bytecode and useful. Only its subject is stated.
The program's own output now leads with memory, labels it as describing the object, and follows with
the cost figure under its own heading and an explicit disclaimer. **The narrowing is in the program
output rather than only here**, because a reader linking an object meets the output and may never
open a decision record.

`motor_policy/README.md` quotes the new output and records that it previously quoted the old. **A
document that reproduces program output becomes the stale copy the moment the output changes** — the
same failure shape as a handoff banner disagreeing with its body, recorded three times on this line.

## What a native time bound would require, named and not built

A per-target cost model for the emitted instruction sequences, and a bound for every
compiler-runtime routine the code can reach — which on the measured target includes double-precision
arithmetic and 64-bit division. **That is a workstream, not an increment**, and nothing here starts
it. Naming it is the whole of what this record does about it.
