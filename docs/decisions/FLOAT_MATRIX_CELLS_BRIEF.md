# BRIEF — the scalar-operator matrix covers three of four scalar types

**Filed 2026-09-18, against `da6e7fdb`, backlog 2.**

## What is being closed

`native_codegen/tests/scalar_operator_matrix.rs` documents itself as enumerating
*"every cell, from the types and the operators"*. Its type list is `byte`, `word`,
`fixed`. **`Float` is not in it**, and the file says so in a comment added after
the cost was paid.

The cost: on 2026-09-17 `Op::Neg` on a `Float` was found emitting **invalid
intermediate representation** under `narrow-float-32`, a bitcast changing the bit
width. Float negation was broken outright in a configuration the gate runs. It was
found by a composite-field probe. **No cell in the matrix could have seen it**,
and the matrix is the instrument whose job that was.

`float_ir_validity.rs` was written in response and covers the float surface for
**lowering plus `verify()`**. That is real coverage and it is not execution. A
float operator that lowers to valid IR computing the **wrong number** is invisible
to it. Closing the matrix cells closes that.

## The fact that governs the design, and it is not obvious

```
src/vm.rs:957: pub type Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;
```

**The reference virtual machine these tests drive is instantiated at `f64` in BOTH
float configurations.** The backend lowers `Float` at the configured width, which
is `f32` under `narrow-float-32`.

So a float differential under `narrow-float-32` compares an **f64 reference**
against an **f32 backend**. For any operand or result not exactly representable in
four bytes, the two legitimately differ, **and that is not a backend defect.**

This is the single most likely way to produce a confident false finding here, and
this line has already recorded the shape: a divergence report that is really a
statement about the harness. The corrective is not care. It is:

- Choose operands whose every result is **exact in four bytes**. `14.0` and `4.0`
  give `18`, `10`, `56`, `3.5`, `2.0` and the six comparisons, all exact.
- **Guard that choice.** A check in the tree must fail if a subject value or an
  observed result stops round-tripping through `f32` unchanged. Otherwise a later
  edit picking `0.1` produces a phantom divergence and someone goes looking in the
  emitter.

A tolerance-based comparison is the wrong answer. The correctness signal on this
line is a byte-identical differential; introducing an epsilon here would weaken
the one property the whole package rests on, to work around a harness artefact.

## The wrong turns, named

1. **Do not hand-name the call signature.** A float-in, float-out chunk lowers
   with a floating-point parameter and return, not `i64`. Calling through the
   wrong shape is undefined behaviour that surfaces as a SIGBUS inside JIT code
   with no usable stack. **This package has already paid for that once.** Read the
   parameter and return types off the lowered function, assert them, and dispatch.
   An unanticipated shape must **panic with a message**, never fall through to a
   call.

2. **Do not pass a float as an `i64` bit pattern into the existing driver.** A
   recorded probe defect here passed a float as `i64::MIN`. The existing `raw()`
   helper panics on `Value::Float` calling it a non-scalar; widening it by
   transmuting bits would reintroduce exactly that.

3. **Do not edit a recorded class to match a run.** The file says so itself. A
   cell that moves class gets a stated reason, or the emitter gets fixed.

4. **The floor and the record must both move**, and the comment declaring `Float`
   absent must be retired. `comment_citations.rs` fails on a citation naming
   something that no longer exists, so a half-retired comment is caught — but a
   comment that is merely *false* is not. Retire it deliberately.

5. **`src/` and `tests/` at the repository root belong to the `v0.2.3` line.**
   Read-only. If a float cell exposes a reference defect, it is a report, not a
   repair.

6. **Absorption is measured alone**, with the prediction filed first and stamped
   with the tree it was computed against. `prediction_stamp.rs` requires the
   unabsorbed count and the commit. Absorption 59 predicted zero conflicts against
   a stale `merge-tree`.

7. **Run the gate script end to end.** The ninth row of `CLAUDE.md`'s catalogue is
   a clippy failure that stood four commits behind a green suite figure, because
   the verdict was assembled from phases by hand and the clippy phase was not on
   the assembler's list. The script accumulates `fail=1`; an assembly does not.

8. **Expect refusals and do not read them as gaps.** `bnot`, the shifts and the
   bitwise pair operators on `Float` are very likely refused by the reference
   compiler. `RefRejects` is a classification, not a finding. The finding would be
   a cell where the backend **answers where the reference errors**.

## Pre-commitment

If the new cells show a `Disagree` under `narrow-float-32` only, and the operands
are f32-exact, **the first hypothesis is the emitter, not the harness** — because
the harness constraint above is guarded and the guard would have fired first. If
the operands are not f32-exact, the finding is void and the subject is wrong.

If closing the cells produces no finding at all, **that is the expected outcome
and it is still worth the increment**: the gap is named in three documents, and an
enumeration that covers its stated population is worth more than one that covers
three quarters of it and says so in a comment.
