# Brief — half the bound transfers, and the example presents both halves alike

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01.**

---

## The goal set

| goal | owner | state |
|---|---|---|
| **G6** the worst-case-execution-time half of the bound transfer | this line | **unblocked, and the subject of this brief** |
| **G5** the unwind personality | this line | implemented; awaits the behavioural falsifier |
| **G2** absorption 43 | this line | still gated on a solo machine; the peer's release gate is running |

## The finding, stated before any fix is proposed

`native_codegen/examples/emit_object.rs` prints, in one block, under one heading:

```text
PROVEN BOUNDS, from the verifier rather than from prose:
  chunk 0 (derate_for  ) WCET     19 cost units
  chunk 1 (main        ) WCET    134 cost units
  worst case over all chunks: stack 352 B, heap 24 B
  shared segment 58 B, preallocated by the host and never grown
```

and `motor_policy/README.md` reproduces it verbatim above the line **"The point is not that the
policy runs. It is that its worst case was known before it ran."**

**The memory figures transfer and are measured.** `bound_transfer.rs` is Workstream E's instrument and
compares exactly two things: the operand-stack slots the backend provisions against
`RuntimeFootprint::max_operand_slots`, and `region_total_bytes` against
`RuntimeFootprint::max_heap_bytes`.

**The time figure is not measured against anything, and `bound_transfer.rs` contains no time
comparison at all.** Established by search, not assumed: the files mentioning worst-case execution
time in this package are `corpus_differential`, `stage_differential`, `spike_bounds_transfer`,
`spike_stream_sufficiency`, `emit_object` and the `motor_policy` README. **The bound-transfer
instrument is not among them.**

## Why this matters more than an ordinary documentation defect

The cost-unit figure is a **bytecode-level** count under a cost model calibrated for the **virtual
machine**. It is a true statement about the bytecode. Printed beside memory bounds that genuinely
transfer, in an example whose entire subject is a C host linking a **native object**, under the
heading **PROVEN BOUNDS**, it will be read as a bound on the native execution. Nothing in the output
distinguishes the two halves.

**And this line has just measured why the gap is not hypothetical.**
[`NARROW_TARGET_LINKAGE.md`](./NARROW_TARGET_LINKAGE.md) shows the native code calls into a compiler
runtime — `__adddf3`, `__divdi3`, `__gedf2` and others on `thumbv8m.main-none-eabihf`. **Those calls
have execution times that nothing in this project bounds.** A per-opcode VM cost model cannot describe
them, because at the bytecode level they do not exist.

**The project's stated value proposition is definitive worst-case execution time and memory usage.**
An asymmetry between the two halves, hidden by presenting them together, is exactly the kind of claim
this line exists to check.

## What the work is, and what it is NOT

**It is to narrow a claim to what is measured, and to name precisely what a native time bound would
require.** It is *not* to build a native cost model — that is a workstream, not an increment, and
proposing it here would be scope this brief cannot honestly size.

**Do not "fix" this by deleting the WCET line.** The figure is true about the bytecode and it is
useful. The defect is that its subject is unstated, not that it is wrong.

**Do not overstate the fault either.** Nothing measured says the native code is SLOWER than the
bound, or that any bound is violated. What is established is that **no measurement relates them**, and
that is a different and smaller claim than "the bound is wrong". Say the smaller one.

## The specific wrong turns

**1. Do not infer that memory transfer implies time transfer.** They are separate instruments and
only one exists. This brief's whole content is that they were presented as one.

**2. Do not claim the compiler-runtime calls are unbounded in fact.** They are unbounded *by this
project*. A vendor runtime may well have published timings. **"Nothing here bounds them" is the
supportable claim.**

**3. A test that asserts a document contains a sentence is weak, but it is not nothing.** If the
narrowing is pinned, pin the *distinction* rather than a phrase, and expect the phrase to be reworded.

**4. Verify at the level of the artefact.** The claim lives in program output and in a README that
quotes that output. **If the output changes and the README is not regenerated, the README becomes the
stale copy** — which is the same failure shape as a handoff banner disagreeing with its body, recorded
three times on this line.

**5. Do not edit test sources while a suite runs.** Broken once today.
