# BRIEF — the checked fixed-point divide

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## The refusal, and why its stated reason does not distinguish

`Op::CheckedDiv(n)` with a non-zero fraction count is refused. **The reason given — that it would
reach for `__divti3`, the compiler-runtime routine for 128-bit signed division — is a cost the tree
already pays.**

`linkage_symbol_census.rs` establishes it directly: *"`Fixed` division. It is the only construct in
the sweep that reaches for a compiler-runtime symbol"* — and **the bare `Op::FixedDiv` LOWERS.** So any
object performing a fixed-point division already depends on `__divti3`. Refusing the checked form on
that ground protects nothing.

**I wrote that reason myself, one increment ago**, in the brief for the checked multiply. It was true
and not a distinguishing fact, which is a weaker thing than it sounded.

## The semantics, read from `src/vm.rs`

| case | result |
|---|---|
| zero divisor | flag **3**, low is the NUMERATOR, middle zero |
| `frac_bits >= word_bits` | `InvalidBytecode` — refuse, as the bare form and the checked multiply do |
| otherwise | `dividend = widen(x) << frac_bits`; `quotient = dividend / widen(y)`; classify and **wrap** |

Three values are pushed: `Fixed(low)`, `Fixed(0)`, `Int(flag)` — the middle is zero, not a high half.

**`i64::MIN / -1` needs no special case**: in 128 bits the quotient is representable, which is how the
integer arm already gets flag 1 rather than a fault.

## What already exists and must be reused rather than rebuilt

The integer `CheckedDiv(0)` arm already: excludes the zero divisor from the `sdiv` by substituting a
non-zero one, divides in 128 bits, and then OVERRIDES the resulting triple for the zero case. That is
precisely the shape this needs. **The difference is the left shift of the dividend and the Fixed
triple.**

## The wrong turns

1. **Do not skip the divisor substitution.** A zero divisor in LLVM's `sdiv` is undefined behaviour,
   not a fault, so the substitution must happen before the division and the result must be discarded
   for that path.
2. **Do not reuse `checked_triple`.** It puts the quotient's high half in the middle slot; the Fixed
   contract puts zero there. Same trap as the multiply.
3. **Do not shift the dividend in 64 bits.** `x << frac_bits` overflows a word for exactly the inputs
   that make the operation interesting; the widening is what makes the quotient meaningful.
4. **Do not invert the divide's refusal test without driving the arms.** A zero divisor, an
   out-of-range quotient and an ordinary one are three distinct paths, and flag 3 is reachable only
   by the first.
5. **Do not state `__divti3` as the reason for anything again** without checking whether the
   construct it is offered against already appears in the same object.

## What done looks like

A checked fixed-point divide agrees with the reference on an ordinary quotient, on a zero divisor,
and on a quotient outside the word range; an out-of-range fraction count is still refused; and the
refusal's stated linkage reason is corrected where it was recorded rather than left standing.

---

## OUTCOME — 2026-09-12

**It lowers, and agrees on all three paths**: an ordinary quotient, one outside the word range, and a
zero divisor — which the runtime reifies as flag 3 with the NUMERATOR in the low slot rather than
faulting.

Every wrong turn the brief named was live:

- **The divisor substitution was required.** A zero divisor is undefined in LLVM's `sdiv`, not a
  fault, so it is excluded before the division and overridden afterwards by selects — branch-free, as
  the integer arm already is, because a new block would have to be reconciled with the per-block
  operand-depth bookkeeping.
- **`checked_triple` was not reused.** It puts the quotient's high half in the middle slot; the Fixed
  contract puts zero there.
- **The dividend is widened before shifting**, because `x << frac_bits` overflows a word for exactly
  the inputs that make the operation interesting.
- **The zero-divisor path is driven and shown DISTINCT** from an ordinary quotient. Agreement alone
  would hold even if the flag were never set.

### The withdrawn reason, and where it was withdrawn

`CHECKED_FIXED_MUL_BRIEF.md` — written by this line one increment earlier — gave `__divti3` as the
ground for keeping the divide refused. **That reason is now marked withdrawn in the document that
made it**, rather than quietly superseded here.

> **A cost already paid by supported code cannot justify refusing more of it.** The bare `Op::FixedDiv`
> lowers and `linkage_symbol_census.rs` measured it as the one construct in its sweep needing a
> compiler-runtime symbol. The checked form adds no dependency that the object did not already have.

The refusal test was inverted rather than deleted, and its new message says not to cite `__divti3`
again if the capability is ever withdrawn.
