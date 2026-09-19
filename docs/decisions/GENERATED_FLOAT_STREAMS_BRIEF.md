# BRIEF — generated breadth over float streams

**Filed 2026-09-18, against `2c90a176`, backlog 0, suite 644.**

> ⚠ **STATUS, ADDED BY THE ARTIFACT AUDIT 2026-09-18.** A BRIEF DESCRIBES THE TREE AT FILING, BEFORE ITS OWN WORK LANDS. Every gap it states in the present tense was, by construction, still open when written. **LANDED `a4bb539a`, AND ONE OF ITS CLAIMS WAS FALSIFIED THE SAME DAY.** It argues the generator reaches the operand spill — *"spilled to an ephemeral slice as `(Width, OperandKind)` pairs"*. **It does not.** All 240 subjects carry values in LOCALS, `deep` is zero, and the spill loop never runs. Corrected in `float_stream_differential.rs` and in `FLOAT_SPILL_BRIEF.md`; this brief was left stale until the artifact audit found it.


## Why this, and why now

`Op::Yield` became float-aware for a general stream earlier today. That arm is
**the newest code in the backend** and it rests on **four hand-written subjects**,
all chosen by me, after I had already formed a hypothesis about what could go
wrong. `generated_floats.rs` showed the value of the other kind of instrument on
the straight-line float surface the same afternoon.

The stream path is where it matters most. Everything beneath a yielded value is
**spilled to an ephemeral slice as `(Width, OperandKind)` pairs and restored at the
resume point**. That is precisely the interaction — width and kind, across a
suspension — where this session found six width arm-groups silently refusing, and
where a panic sat unnoticed until 2026-09-17.

## ⚠ THE HAZARD THAT MAKES THIS DIFFERENT FROM THE EXPRESSION GENERATOR

A stream **feeds its own output back**. The reply `r` at tick *n* is whatever the
host sent, and a realistic generated body computes the next yield from `r`.

If the body is an expression tree of depth *d* over `{+, -, *}` with leaves
bounded by *M*, one tick's output is bounded by `M^(2^d)`. **With feedback, the
next tick's bound is that value raised to the same power.** The magnitude is
**doubly exponential in the tick count**, and it leaves the four-byte
exact-integer range `2^24` almost immediately.

`generated_floats.rs` has no feedback, so its single bound `LEAF_MAX^(2^DEPTH)`
suffices. **Copying that reasoning here is unsound and it is the obvious thing to
do.**

The design that fixes it structurally:

- **The reply enters only ADDITIVELY.** `r` may appear as `r + E` or `r - E` at the
  top level and nowhere else; it is never a multiplicand. Growth is then linear in
  the tick count rather than doubly exponential.
- **The inner expression `E` is over `t` and literals only**, with its own depth
  and leaf bounds, so its worst case is the familiar `M^(2^d)`.
- **The total bound is `ticks * (M^(2^d) + max_reply)`**, which is arithmetic a
  test can assert rather than prose a reader must trust.
- **And every yielded value is checked at every tick on both sides anyway**,
  because a construction argument rots when someone edits a bound.

## Wrong turns, named

1. **Do not let `r` under a multiplication.** See above. This is the whole reason
   the file needs a different shape from its sibling.

2. **Do not reuse `generated_floats.rs`'s bounds or its single-value magnitude
   argument.** It has no feedback term. Its argument is correct for it and wrong
   here.

3. **Do not compare only the final value.** A stream yields a sequence. Comparing
   the last element would pass while every intermediate diverged — and the
   sequence is the thing the spill and restore path actually affects.

4. **Do not drive the reference through `common::general_vm_sequence`.** It passes
   `Value::Int`, and the reference rejects an `Int` for a `Float` parameter with
   *"parameter 0 expected Float, got Int"*. `float_stream_differential.rs` carries
   float-aware drivers written for exactly this; reuse their shape.

5. **Do not name the native signature by hand without asserting it.** A float
   stream lowers with a floating-point parameter and return plus three trailing
   pointers. A wrong shape is undefined behaviour surfacing as a SIGBUS inside JIT
   code with no usable stack, which this package has already paid for once. Assert
   the parameter count, the parameter kind and the return kind before the call.

6. **Do not treat a refusal as a divergence.** A generated body may stack a
   composite beneath the yielded value, which is refused deliberately. If the
   backend refuses, the program is outside the generator's intended set — tighten
   the generator, and say so, rather than recording a finding.

7. **`src/` and `tests/` at the repository root are read-only to this line.**

8. **One gate invocation covers ONE float configuration.** The script takes
   `--narrow`.

## Pre-commitment, written before the work

- **If a generated stream diverges and every value on both sides is four-byte
  exact, it is a backend finding** and gets reported with its source and its
  sequence. It is not resolved by removing the program.
- **If the exactness bound forces the tick count so low that the loop and the
  reset leg are not exercised, stop and say the surface is not reachable this
  way**, rather than shipping a generator that drives one yield and calls it a
  stream.
- **If the generator cannot produce a nesting deeper than the four hand-written
  subjects already reach**, it adds nothing and should be abandoned rather than
  committed for the sake of a count.
