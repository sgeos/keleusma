# BRIEF — the entry ABI, which is four boundary points and nothing else

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why it is smaller than it looked

The operand stack already carries a float **as its bit pattern in an `i64`**, tagged `Float`. So the
internals need no change at all. The ABI matters at exactly **four boundary points**:

| point | change |
|---|---|
| function **declaration** | `f64` in the positions the signature says are floats |
| **prologue** | a float parameter arrives as `f64`; bitcast to `i64` before storing, and tag the local `Float` |
| **`Op::Return`** | bitcast back to `f64` when the signature's return is a float |
| **`Op::Call`** | bitcast arguments in, and the result back out |

## Why `lower_module` and not `lower_chunk`

`lower_chunk` receives `chunk.param_types`, so it knows the parameters — but **the chunk carries no
RETURN type**. That lives in module-level `ChunkSignature`. A single-chunk lowering cannot build a
correct function type, so **the entry ABI is a module-level feature** and `lower_chunk` keeps refusing
a float signature.

## Prior failures to avoid repeating

- **Three increments running, my brace-matching script has misplaced a closing brace** when `rustfmt`
  reflowed between edits. **Do not use it for match-arm surgery here.** Edit with explicit anchors, or
  insert whole functions rather than splicing into arms.
- **Do not open the signature route until the differential agrees.** Every previous float route was
  opened only after execution matched; a half-built ABI that is *accepted* is worse than one refused,
  because a wrong float is a plausible number.
- **The 67 lowering modules are the risk.** Changing declaration types touches every chunk, not only
  float ones. The corpus differential is the check; a regression there is the signal, not a nuisance.
- **Do not assume the return shape is in `param_types`.** Parameters and return come from different
  places, and conflating them is exactly the error that made the previous increment defer this.
- **Do not verify with a constant-folded probe.** A float parameter must be passed at runtime through
  the real calling convention, so the test must call the JIT-ed symbol with an `f64` argument.
- **Expect censuses to move only if a corpus module gains a float signature — none has.** So the
  headline figures should NOT move, and a movement needs explaining.

## What good looks like

A hand-built module with a float parameter and a float return lowers, is called through the real C
calling convention with an `f64`, and agrees with the reference. The 67 existing modules still lower
and still agree.

## Outcome (2026-08-30, written after the build)

Landed as planned, at the four boundary points and `lower_module` only; `lower_chunk` keeps refusing
a float signature because a chunk carries no return type. Two details the plan did not name:

- **The parameter's local must be TAGGED `Float` after the prologue bitcast**, or every operation on
  a float parameter refuses with the ABI itself correct — the seeding reads the declared parameter
  type, and the call-result twin reads the callee's declared return.
- **`Op::Call` is float-aware only positionally.** Each argument is converted to the callee's
  DECLARED parameter type, and a disagreement between an operand's kind and the declared position is
  refused rather than reinterpreted, in either direction.

What stays refused: a float of any width other than 8 bytes (the guard reads the module's
`float_bits_log2`), float shared slots, floats inside composites, and a native declaring a float
return. `native_codegen/tests/entry_abi_float.rs` calls the JIT-ed symbol through the C convention
with runtime `f64` arguments, bit-compares against the virtual machine, and covers NaN, signed
zero, infinities, a cross-call round trip, and a mixed float-parameter integer-return signature.

Four tests rotated their subjects because this route opened, each by its own standing instruction:
the backend-support census pin, the module-level-refusal visibility pin (now the word-width guard),
the unsupported-subset boundary (now `Op::Len`), and the float whitelist subject (now route 3, the
native float return).
