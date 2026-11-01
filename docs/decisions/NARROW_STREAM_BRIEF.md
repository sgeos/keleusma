# BRIEF — a Byte and bool stream differential

**Filed 2026-09-23, against `70163c1f`, backlog 0, suite 658.**

> ⚠ **STATUS.** Filed before its own work lands. The gap it states in the present
> tense is open at filing by construction.

## The gap, found by the method rather than from a list

Every named gap on the pickup list is closed. This was found by asking the question
that has worked all session — **what lowers with nothing comparing it?** — applied to
stream ENTRY types rather than to operators.

Measured: `loop main(t: Byte) -> Byte` and `loop main(t: bool) -> bool` both lower
with **zero refusals**, and **no file in the package declares either shape.** Every
stream subject and driver uses `Word`, `Float` or `Fixed`.

Two refusals bound the surface and are sound, so they are not gaps: a stream with
**two** parameters and one with **zero** are both refused, because *"resume defines
slot 0 only, so any further parameter would have no defined value on re-entry."*

## Why the narrow types are the interesting ones

**A `Byte` is one byte and a `bool` is one byte**, where every driven stream so far
carried eight. `width_of_tag` gives both `Scalar(1)`, and `Byte` arithmetic is
promote-operate-truncate MASKED — so a yielded `Byte` must come back masked to eight
bits. **A wrong mask on the resume path is a wrong NUMBER, not a fault.**

And the ABI is already known compatible, which is what makes this cheap: all three
of `Byte`, `bool` and `Word` lower to `(i64, ptr, ptr, ptr) -> i64`. **Measured, not
assumed** — that was the SIGBUS hazard and it is absent here.

## ⚠ THE TRAP HAZARD IS REAL AND DIFFERENT FROM THE FLOAT CASE

`Byte` arithmetic is **CHECKED**: the reference errors on overflow and the native
side executes `llvm.trap`, which **kills the process with SIGTRAP** — not a failed
comparison, a dead test binary. `generated_expressions.rs` bounds its byte leaves at
**3, not 9**, for exactly this reason, and records that depth 3 reaches 6561 and does
not fit.

A stream feeds back, so the reply becomes the parameter on reset. Values must stay
inside a byte across every tick, and **the bound must be asserted rather than
argued**.

## Wrong turns, named

1. **Drive the REFERENCE first, and only then the backend.** A trapping input kills
   the binary with no usable result. Both `corpus_differential` and the streaming
   arena census order their runs this way.

2. **Pass `Value::Byte` and `Value::Bool`, not `Value::Int`.** The runtime
   type-checks the entry parameter and rejects a mismatched value — the float stream
   file records exactly that for `Value::Int` against a `Float` parameter.

3. **Do not copy the float exactness machinery.** There is no configured width here.

4. **Compare the whole sequence.** The float stream perturbation diverged at the
   loop-back ticks while both sequences ended identically.

5. **If a file is added, check the host-buffer census BEFORE committing**, and do not
   create probe files in `tests/` while a gate runs — the eleventh catalogue row is
   mine from this session, and `frozen-run.sh` cannot see a transient change.

6. **`src/` and `tests/` at the repository root are read-only to this line.**

## Pre-commitment

- **Prove reach by perturbation before believing a green run**, and perturb something
  stream-specific rather than byte arithmetic, which the operator matrix already
  covers. The reset-leg resume restore is the one that has worked twice.
- **If the mask on the resume path turns out not to be load-bearing, say so** — that
  is the same shape as the `Fixed` result, and a measured negative is worth more than
  an implied positive.
- **If either shape traps despite the bound, tighten the bound and record the
  arithmetic**; do not widen the comparison.
