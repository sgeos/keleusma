# BRIEF — the float yield arm

**Filed 2026-09-18. Supersedes nothing; it executes `FLOAT_YIELD_SCOPE.md`, which

> ⚠ **STATUS, ADDED BY THE ARTIFACT AUDIT 2026-09-18.** A BRIEF DESCRIBES THE TREE AT FILING, BEFORE ITS OWN WORK LANDS. Every gap it states in the present tense was, by construction, still open when written. **LANDED `142d9b68`.** `Op::Yield` IS float-aware for a general stream now, so this brief's opening — *"`Op::Yield` is absent from the emitter's `float_aware` set"* — describes the tree BEFORE the work. One premise in it was also false when written; see the box below.

scoped this deliberately and stopped.**

## Why this and why now

`Op::Yield` is absent from the emitter's `float_aware` set, so a generic guard
refuses any float operand reaching it. **The reference RUNS float streams.** This
backend declines to lower one. `float_stream_differential.rs` pins the asymmetry
from both sides and carries the differential already written and compiling,
waiting to be switched on.

The refusal is *sound* — it fails closed on a value whose bit pattern would
otherwise be read as an integer — so nothing degrades while it waits. What it
costs is capability, and this line has now spent two increments finding that its
instruments were narrower than their descriptions. **This is the opposite case: a
capability narrower than the reference's, written down, with its oracle ready.**

## The three sites, confirmed in the code rather than recalled

1. **`float_aware`** (`src/lib.rs`, the per-op guard). `Op::Yield` must be admitted
   **only for a general stream**. The degenerate form lowers to the host callback
   `kel_yield(i64) -> i64`; carrying a float through that is a **host-facing ABI
   change**, not an emitter change. A blanket entry admits both and silently
   mispasses the degenerate one. The lowering arm itself is already spelled
   `Op::Yield if general_stream`, so the guard must carry the same condition.

> ⚠ **ONE PREMISE IN THIS BRIEF WAS FALSE, AND IT WAS INHERITED.** The brief was
> written believing the missing reply kind would cause a silent wrong number. It
> does not: operating on the reply REFUSES loudly, and passing it through gives the
> right bits. Measured in both configurations after the work; see the correction
> box in `FLOAT_YIELD_SCOPE.md`. **The brief repeated the scope document's claim
> without measuring it**, which is how a premise survives being wrong — the
> instrument to distrust is the one you copied.

2. **The resume push.** In the general-stream yield arm the restored reply is
   pushed with `st.push_w(rv, ..)`. **`push_w` deliberately marks the slot `Int`**,
   and its comment says why: the operand stack reuses slots, so a stale `Float`
   tag would leak into a later integer. That behaviour is correct and must not be
   changed. The fix is to push with `push_k` and the kind derived from the
   stream's **declared parameter tag**, not from the LLVM value — a `Fixed` is an
   `i64` too, so the LLVM type cannot discriminate it, which this package has
   already recorded about `Fixed`.

3. **The differential.** `float_stream_differential.rs` currently asserts the
   asymmetry and will FAIL by design the moment the module lowers, with a message
   saying so. Switching it on is part of this change, not a follow-up. Its own
   note: *"a lowered float stream with nothing comparing it is exactly the state
   that let a panic sit on this path unnoticed."*

## Wrong turns, named

1. **Do not add `Op::Yield` unconditionally to `float_aware`.** See site 1. If the
   guard cannot see `general_stream`, that is a reason to thread it, not a reason
   to widen the admission.

2. **Do not derive the resumed value's kind from `is_float_value()`.** It answers
   the wrong question. `Fixed` and `Word` are both integers at the LLVM level and
   only the declared tag separates a float from them.

3. **Do not "fix" a differential disagreement by adjusting the comparison.** The
   byte-identical differential is this line's correctness signal. If the reference
   and the backend disagree on a float stream, the finding is that they disagree.

4. **Do not relax the `Width::Body` refusal in the same arm.** It is a separate
   hazard — a pointer into region storage the next iteration overwrites — and it
   exists to stop the yield-escape route. Touching it is out of scope.

5. **The f64-versus-f32 constraint applies here too.** `keleusma::vm::Vm` is
   instantiated at `f64` in BOTH configurations while the backend lowers at the
   configured width. The differential's subjects and replies must be four-byte
   exact, or `narrow-float-32` reports rounding as divergence. **This exact
   mistake was made and caught earlier today** in the scalar matrix, where a
   result-only check let a phantom `Disagree` through: **the operands must be
   checked, not only the results.**

6. **`src/` and `tests/` at the repository root are read-only to this line.**

7. **Run the gate script end to end, once per configuration.** One invocation
   covers ONE configuration; it takes `--narrow`. A single run is not both, and
   assuming otherwise was caught earlier today only because the script was read.

## Pre-commitment, written before the work

- **If the differential disagrees on any subject, revert the admission** and
  record the disagreement. Do not debug toward agreement inside this increment.
- **If the change reaches a fourth site** — beyond the guard, the resume push and
  the differential — stop and report the shape, exactly as the scope document did
  at three. A capability spread across four sites at the end of a session is how a
  silent wrong number gets shipped.
- **If admitting the arm makes the degenerate yield lower a float**, that is a
  host ABI change and it stops immediately, regardless of whether tests pass.
