# BRIEF — a float on the spill slice, and a correction to my own record

**Filed 2026-09-18, against `299cfd6a`, backlog 0, suite 647.**

## Part one: a claim of mine that the tree does not support

`generated_float_streams.rs`, committed hours ago, says in its header:

> *"Everything beneath a yielded value is spilled to an ephemeral slice as
> `(Width, OperandKind)` pairs and restored at the resume point. That
> width-and-kind interaction across a suspension is exactly where this line found
> six width arm-groups silently refusing."*

**Its 240 generated streams do not reach that path.** They have the shape
`let r = yield E1; yield (r op E2)`. At each `Op::Yield` the operand stack holds
only the yielded value, so `deep` is zero and the spill loop does not execute.
The values cross the suspension in LOCALS, which is a different table
(`local_widths`) and a different mechanism.

**This was already known and written down, in a file I did not re-read.**
`stream_width_survival.rs` records that its first five subjects all carried values
in locals, that corrupting the restored spill widths to `Unknown` left every one of
them passing, and that **one subject in the whole package touches the spill slice**
— a `Byte` left on the operand stack across a yield.

So the generator is good work whose header claims the wrong mechanism. **Correct
the claim; do not delete the generator.** It covers composition across a
suspension through locals, which is real. It simply is not the spill instrument.

## Part two: the gap that correction exposes

The sole spill witness is a `Byte`. **`OperandKind::Float` has no spill witness at
all**, and `Op::Yield` only became float-aware on 2026-09-18 — the newest arm in
the backend, with its least-covered path untested.

The shape that reaches it is a `yield` used as a SUBEXPRESSION, so another operand
is already on the stack:

```
loop main(t: Float) -> Float { let x: Float = ((t * 2.0) + (yield t)); yield x }
```

`t * 2.0` is computed and pushed; `yield t` then suspends with it beneath.

## Wrong turns, named

1. **Do not accept a passing test as evidence the spill ran.** That is precisely
   how five subjects sat in `stream_width_survival.rs` believing they tested it.
   **Corrupt the restore and require the new subject to fail.** If it passes with
   the restore corrupted, it is another locals test wearing a new label.

2. **Do not assume a refusal means the path is unreachable.** A float beneath a
   yield may be refused by the emitter — the arm was written days ago. A refusal
   is a capability gap to record, not a divergence, and not a reason to delete
   the subject.

3. **The `Width::Body` refusal must stay.** A composite body beneath a yield is a
   pointer into storage the next iteration overwrites, and refusing it is what
   holds the yield-escape line. Out of scope.

4. **The f64-versus-f32 constraint still binds.** The reference runs at `f64` in
   both configurations. Operands, replies and yielded values must all be
   four-byte exact, operands included — a result-only check let a phantom
   divergence through earlier today.

5. **Run the whole gate, once per configuration.** The last increment was green
   under every targeted run and RED in the gate, because `-E binary(X)` cannot see
   a census in binary Y. **If this adds a file, the host-buffer census is the
   first thing to check, not the last.**

6. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment, written before the work

- **If the corrupted-restore perturbation does not fail the new subject, the
  subject is wrong and gets replaced**, not explained. That check is the entire
  reason this increment exists.
- **If a float beneath a yield is refused**, record the refusal with its message
  and stop. Writing the arm is a separate increment with its own differential, and
  a capability added at the end of a long session is how a silent wrong number
  ships.
- **The correction to `generated_float_streams.rs` lands whatever else happens.**
  A header claiming the wrong mechanism is the failure this session keeps finding,
  and leaving it because the increment moved on would be the worst outcome here.
