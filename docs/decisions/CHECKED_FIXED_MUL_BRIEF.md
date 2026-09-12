# BRIEF — the checked fixed-point multiply

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## The gap, and why it is a gap rather than a decision

`Op::CheckedMul(n)` with a non-zero fraction count is refused. `Op::FixedMul` and `Op::FixedDiv` —
the bare `a * b` and `a / b` forms — **both lower**. What does not is the *checked* form, the one with
`ok` / `overflow` arms.

The refusal's own test says: *"If `Fixed` arithmetic ever enters the subset, this test must be changed
deliberately — as the division case above had to be when multiplication arrived — rather than
deleted."* **That boundary has moved before, on purpose, when capability arrived.** This is the same
kind of move, not a contradiction of a standing decision.

## The semantics, read from `src/vm.rs` rather than inferred

This line has twice written a refusal describing its own lowering while reading as a fact about the
language, and both were settled by reading the runtime at the same opcode. So:

1. **Guard first.** `frac_bits >= word_bits` is `InvalidBytecode` in the runtime. `FixedMul` already
   refuses that; the checked form must too.
2. `product = widen(x) * widen(y)`, then `shifted = product >> frac_bits`, an **arithmetic** shift.
3. `fixed_checked_outputs` gives `(low, flag)`: `flag` is **0 in range, 1 above max, 2 below min**,
   and `low` is `from_wide_wrap` — **wrapping, not saturating.**
4. **Three values are pushed**: `Fixed(low)`, `Fixed(0)`, `Int(flag)`. The middle slot is always zero,
   where the integer arm pushes the high half.

**That middle slot is the trap.** The integer `CheckedMul(0)` lowering pushes a real high half; a
Fixed one pushing the same would hand the overflow arm a number the runtime never produces.

## The wrong turns

1. **Do not reuse the integer arm by parameterising the shift.** The low slot SATURATES nowhere and
   WRAPS here, the middle is zero rather than the high half, and the guard differs. Sharing the code
   would make one of those wrong silently.
2. **Do not confuse this with `FixedMul`.** That one saturates; this one wraps. They are different
   opcodes with different contracts and the same arithmetic in the middle.
3. **Do not lower `CheckedDiv(n)` on the strength of this.** Division shifts the dividend LEFT into
   128 bits and then divides, which reaches for `__divti3` — a compiler-runtime routine the
   bare-metal census already flagged. It stays refused, with that as the stated reason.
4. **Do not assume the operand is Fixed because the count is non-zero without saying so.** It is true
   — the compiler emits a non-zero count only for the Fixed arm — and it is exactly the kind of
   upstream premise this package censuses. State it where the code relies on it.
5. **Do not leave the refusal test deleted.** It must be changed to assert the new behaviour, as its
   own comment instructs.

## What done looks like

A checked fixed-point multiply agrees with the reference on ordinary values, on a product that
overflows, and on one that underflows; an out-of-range fraction count is still refused; `CheckedDiv`
with a non-zero count is still refused with the linkage reason; and the test that asserted the
multiply refusal asserts agreement instead of being removed.

---

## OUTCOME — 2026-09-12

**It lowers, and agrees on ok, overflow and underflow.** The refusal test was inverted as its own
comment instructed, not deleted.

Every wrong turn the brief named was live:

- **Not reused from the integer arm.** The middle slot is zero, the low slot wraps, and the guard
  differs; sharing would have made one of those wrong silently.
- **`CheckedDiv(n)` stays refused.** It shifts the dividend LEFT into 128 bits and divides, reaching
  for `__divti3`, which the bare-metal census already flagged.
- **The type premise is stated where the code relies on it**, because the count being non-zero is the
  only static signal that the operand is `Fixed`.

### The test's own arithmetic was wrong before the lowering was

`the_overflow_arm_is_actually_selected` expected `wrapped_only - 1` and measured `-65536`. **`w - 1`
is fixed-point subtraction**: the literal is one UNIT, `1 << 16` in Q16 raw bits. The lowering was
right and the expectation was wrong — the more common way round, and worth recording because the first
instinct on a red differential is to suspect the backend.

### ⚠ AND THE PREMISE CENSUS SHOULD HAVE REFUSED THIS INCREMENT, BUT COULD NOT

The new comment states an upstream premise — *"The compiler emits a non-zero count only for the Fixed
arm"* — and `upstream_premise_census.rs` did not fire.

**Its phrases are lowercase and were matched case-SENSITIVELY.** A premise opening a sentence never
matched. **Three did exactly that**, two predating this increment, and one of those — the `break;`
dead-code entry — **was already dispositioned in the census's own table while never once being
matched.**

> **The start of a sentence is where a premise naturally appears.** This was the common case, not an
> edge case. The count of 29 the file defended so carefully was measuring two thirds of a phrase.

Found by writing a premise, expecting a refusal, and noticing none came — **the same shape as the word
boundary that hid `debug_assert_eq!` from the panic census one increment earlier.** Two matchers, two
silent narrowings, both found by watching for a guard that should have fired and did not.
