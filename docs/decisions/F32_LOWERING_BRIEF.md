# BRIEF — lowering `f32`, and the bit-pattern convention that decides it

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The ruling and the instruction

The operator confirmed that **the floating-point type matches the runtime's float width**, converting
what this line had flagged as its own reading into a ruling of record. Separately: *"The codebase
needs to be fixed so that the `f32` configuration is no longer red"*, and *"tests need to meaningfully
pass, and not be cheated into passing."*

**Those two together forbid the cheap repair.** Making the nine failing backend tests derive the width
would turn them green by having them assert a refusal, which is a green that means nothing. The
configuration stops being red by the backend LOWERING `f32`, and the tests then have something to
compare.

## The measured surface

Seven width guards reading `float_bytes != 8`, six hardcoded `f64_type()` uses, plus the float widths
this line added today for shared slots, composite fields and array elements.

## THE ONE DECISION THAT CAN PRODUCE A PLAUSIBLE WRONG NUMBER

The operand stack carries a float **as its bit pattern in an `i64`**, and at eight bytes that is a
direct `bitcast`. **At four bytes that bitcast is illegal**, so a convention is required, and the
wrong one is not a fault but a wrong value:

> **A 32-bit float occupies the LOW 32 BITS of the operand, zero-extended.** Converting to a float is
> `trunc i64 -> i32` then `bitcast i32 -> float`; converting back is `bitcast float -> i32` then
> `zext i32 -> i64`.

**Zero-extension rather than sign-extension**, and the difference is observable: a negative float has
its sign bit set, so sign-extension fills the upper word with ones and any later comparison or store
of the raw operand sees a different number. The reference stores four bytes little-endian, so the
buffer comparison is what pins it.

## Prior failures to avoid repeating

- **Do not verify in one configuration.** The whole point is that two builds exist. The default build
  must stay exactly where it is, at 430 passed over 83 binaries, and the narrow build must go green.
  A change that fixes one and breaks the other is not progress.
- **Values that discriminate.** A float whose `f32` and `f64` roundings agree proves nothing. Use a
  value that is not representable in 24 bits of mantissa, and a negative one for the extension
  question.
- **`llvm.fptosi.sat` is declared against the float type**, so its declaration changes with the
  width. A stale `f64` overload there would be a type error rather than a silent wrong answer, but
  only if the declaration is actually rebuilt rather than reused.
- **Confirm mutations APPLIED by printing the changed line.** `\b` is a GNU extension and this is a
  Darwin box.
- **Stage explicitly.** `git add -A` swept an unverified test file into a documentation commit today.
- **Check the binary count, not just the pass count.**

## What a green narrow build must NOT mean

**That the tests skipped.** Any test that becomes width-aware must still assert something in both
configurations. A test that says "this build's float is not eight bytes" and returns is exactly the
cheat the operator ruled out, and the skippable-tests pin exists to catch it.

## The configuration must be selectable

`narrow-float-32` is a feature of the `keleusma` crate, so this package needs its own forwarding
feature for the build to be reachable at all. Without it the narrow configuration can only be
produced by editing the manifest by hand, which is how it went unmeasured until now.

## Outcome — the backend half is done, and the configuration cannot be made honestly green

**The lowering works and agrees.** Composites, nested composites, arrays, shared slots, tuple members
and enum payloads all agree with the reference under a four-byte float, including NaN, both
infinities and negative zero. The convention holds: a 32-bit float occupies the low 32 bits of the
operand, zero-extended.

### Four defects, all mine, all plausible wrong numbers rather than faults

| defect | what it did |
|---|---|
| the composite packer handled 8 and 1 bytes only | a four-byte float field was REFUSED — the conservative direction, and why it surfaced at all |
| the indexed shared-slot path mapped any non-8 width to ONE byte | a float array element loaded a single byte; native returned 11 where the reference returned 22 |
| `GetIndex` had the same two-way branch | same shape, different arm |
| float constants took the `f64` bit pattern unconditionally | the low 32 bits of a small double are ZERO, so `x * 2.0 + 1.0` silently lost its `+ 1.0` |

**Every one was found by the differential and none by review**, which is the argument for exercising a
configuration rather than compiling it.

### THE BLOCKER IS NOT IN THIS PACKAGE

**Under `narrow-float-32` the module declares a four-byte float and the virtual machine computes in
`f64`.** Measured: `x * 2.0 + 1.0` at `1e10` returns 20000000001, the `f64` answer, where `f32` gives
20000000000.

**The mechanism was reported by the `v0.2.3` line and VERIFIED HERE rather than relayed**:
`pub type Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>` carries no `#[cfg]`, while
`RUNTIME_FLOAT_BITS_LOG2` drops to 5 under the feature. That one line explains both the witness and
the fact that `Value::Float(1.0e10_f64)` compiles in a build whose float should be `f32`. They have
escalated it, because changing a public type's meaning for every embedder in that configuration is
the operator's call.

### Why the remaining red is not closed from here

**`GenericVm` is public and parameterisable, so this package COULD drive the oracle at `f32` and go
green.** That was considered and rejected. Embedders get the bundled `Vm`, so a suite that passes
against a differently-parameterised machine would pass while the shipping runtime still disagrees
with this backend. **That hides the defect**, and choosing values where the two widths agree is the
cheating the operator ruled out explicitly.

**So exactly one test stays red in the narrow configuration, and its cause is named rather than
re-pointed.** The operator's instruction that the configuration stop being red is blocked on a file
this line does not own.
