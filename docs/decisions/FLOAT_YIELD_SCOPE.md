# What implementing float yields would actually take

**Status: SCOPED, NOT IMPLEMENTED.** Written 2026-09-17 after assessing the work
and stopping at a boundary set before the assessment began.

## Why this document exists rather than a commit

The brief for this increment committed, in advance, to a stop condition: *if the
change reaches beyond the yield arm into the reply parameter or the resume
restore, stop and report the shape rather than half-implementing a feature across
three sites at the end of a long session.*

**It does reach the reply parameter.** So this is the report.

## The gap

`Op::Yield` is absent from the emitter's `float_aware` set, so a generic guard
refuses any float operand reaching it:

> *"would consume a float operand, and this arm was not written for one.
> Interpreting a double's bit pattern as an integer is a plausible wrong number
> rather than a fault, so the operand kind fails closed here"*

**The reference RUNS float streams** — `Value::Float(3.0)` in, `Yielded(Float(3.0))`
out. This backend declines to lower one. The asymmetry is pinned from both sides
by `float_stream_differential.rs`, which also carries the differential ready to
switch on.

## What is already in place

Three things make this more tractable than it looks, and all three landed today:

1. **The general-stream yield already returns through `build_typed_return`**,
   which is width-aware: it converts bits to a four- or eight-byte float according
   to the function's declared return type. `Op::Return` is already float-aware for
   exactly this reason.
2. **The spill slice carries `(Width, OperandKind)` pairs**, so the kinds of
   operands beneath the yielded value already survive a suspension.
3. **The resume-parameter restore handles a float parameter**, as of the panic fix
   this morning. Both of its two sites now route through `float_to_bits`.

## What is NOT in place, and why it is a third site

**`push_w` deliberately marks its slot `Int`.** Its own comment says why: the
operand stack reuses slots, so a `Float` tag left by an earlier operand would
otherwise leak into a later integer one. That is correct defensive behaviour.

The resumed value is pushed with `push_w`. **So after a `yield`, the reply is an
integer-kinded operand even in a float stream**, and the next float operation on it
sees a kind mismatch. Admitting `Op::Yield` to `float_aware` without addressing
this would produce a module that lowers and computes on a float's bit pattern as an
integer — **precisely the silent wrong number the refusal prevents.**

## The shape of the work, for whoever takes it

1. **Admit `Op::Yield` only for a general stream.** The degenerate form calls the
   host hook `kel_yield(i64) -> i64`, and carrying a float through that is a
   HOST-FACING ABI change, not an emitter change. A blanket entry in `float_aware`
   would admit both and silently mispass the degenerate one.
2. **Give the resumed value the right kind**, from the stream's declared parameter
   type rather than from the LLVM value — a `Fixed` is an `i64` too, so the LLVM
   type cannot distinguish every case, which this package has already recorded
   about `Fixed`.
3. **Switch the differential on in the same increment.** `float_stream_differential.rs`
   is written and compiling for this. A lowered float stream with nothing comparing
   it is the state that let a panic sit unnoticed on that path until this morning.

## The recommendation

**Do it as its own increment, with the differential switched on in the same
change**, not as an addendum to unrelated work. The refusal is safe and the
reference-versus-backend asymmetry is now pinned from both sides, so nothing
degrades while it waits.
