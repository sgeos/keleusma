# BRIEF — checked arithmetic on `Byte` returns an untruncated result

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## The defect, measured

```
fn main(a: Byte, b: Byte) -> Byte { a * b { ok(v) => v, overflow(w) => w } }
```

| a, b | reference | this backend |
|---|---|---|
| 3, 4 | `Byte(12)` | 12 |
| 200, 100 | **`Byte(32)`** | **20000** |
| 255, 255 | **`Byte(1)`** | **65025** |

**And the same for `+`**: `200 + 100` is `Byte(44)` on the reference and **300** here; `255 + 255` is
`Byte(254)` against **510**.

A silently wrong value, in two operations. `-` has no subject: the compiler rejects an `overflow` arm
for `Byte` subtraction as an outcome that cannot arise.

## The semantics, read from `src/vm.rs`

The `Byte` arms of `CheckedAdd` and `CheckedMul` are **not** the integer arms with a narrower type:

- the result is computed in `i64`, then the **low eight bits** become the low slot;
- the middle slot is `Byte(0)` — **unused for Byte**, where the integer arm puts the product's high
  half;
- the flag is `1` when the result exceeds `0xFF`, and **never 2**: unsigned addition and
  multiplication of bytes cannot underflow.

The backend lowers both through the integer path, which truncates nothing and classifies against the
64-bit word range — a range a byte product cannot leave.

## How it went unseen, which is the part worth fixing

`backend_support_census.rs` reports **0 refusals** and probes `CheckedMul` with `Word` operands. **It
is keyed by opcode NAME while support and semantics are decided by the OPERAND.** The checked
fixed-point multiply and divide sat outside it for the same reason and were found by reading
refusals; this one was found by asking what else that blind spot hides.

## The wrong turns

1. **Do not truncate at the push and call it done.** The FLAG is wrong too — the integer classifier
   asks whether the result left the 64-bit range, which a byte product never does, so every overflow
   currently reports flag 0. Truncating without fixing the flag turns a wrong value into a wrong
   value with a wrong arm.
2. **Do not infer the operand type from the opcode.** `CheckedMul` is `Byte`, `Word`, `Float` or
   `Fixed` depending on what it pops. The backend tracks an operand WIDTH; `Byte` is `Scalar(1)` and
   `Word` is `Scalar(8)`, so the width discriminates here — unlike `Fixed` versus `Word`, which it
   does not.
3. **Do not assume a width is known.** An operand whose width the backend could not reconstruct must
   refuse rather than pick a path; picking the integer one is what produced this.
4. **Do not widen the flag to 2.** Unsigned byte arithmetic has no underflow, and emitting a flag the
   reference never produces would break an arm dispatch that is exhaustive over what it can see.
5. **Do not stop at `+` and `*`.** The census blind spot is the finding; the two operations are its
   instances. Probe the rest of the checked family for `Byte` operands before declaring the scope.

## What done looks like

Checked `Byte` addition and multiplication agree with the reference on a product inside the byte
range and on one outside it, in both the value and the arm taken; an operand of unknown width refuses
rather than taking the integer path; the support census probes operand variants rather than opcode
names alone; and the scope is established by probing the family rather than assumed from two
instances.

---

## OUTCOME — 2026-09-12

**Both operations now agree, in value and in arm.** `200 * 100` is `Byte(32)` on both sides;
`200 + 100` is `Byte(44)`.

Every wrong turn the brief named was live:

- **The flag was as wrong as the value.** The integer classifier asks whether the result left the
  64-bit range, which a byte product never does — so every byte overflow reported flag 0 and took the
  `ok` arm. A subject whose arms return different constants is what shows it; the natural subject,
  with both arms returning the wrapped value, agrees even when the flag is always zero.
- **The scope was probed, not inferred.** `-` has no subject at all — the reference rejects an
  `overflow` arm there — and `/` and `%` already agreed, because a byte quotient cannot leave the byte
  range. Two of five, established by measurement.
- **The width discriminates here and does not always.** `Byte` is `Scalar(1)` and `Word` is
  `Scalar(8)`, so the backend can tell them apart — unlike `Fixed` versus `Word`, where the fraction
  count is the only signal.

### The residual, stated rather than closed

The byte arm is taken when **both operand widths are known bytes**. An unknown width keeps the integer
path, which every previously lowering program relies on and which would still be wrong for a byte. No
subject here produces that shape. **Recorded as a known limit; the alternative — refusing on unknown —
would withdraw support from programs that work today.**

### THE CENSUS BLIND SPOT IS THE REAL FINDING

`backend_support_census.rs` reported **0 refused** while three variants were broken: the `Fixed` forms
of `CheckedMul` and `CheckedDiv` refused outright, and the `Byte` forms of `CheckedMul` and
`CheckedAdd` silently wrong.

**It is keyed by opcode NAME; support and semantics are decided by the OPERAND.** A row per opcode
reports whichever variant happens to be probed, and every `Checked*` row used `Word`.

> **Three defects behind one blind spot**, found across two increments: the fixed pair by reading
> refusals, the byte pair by asking what else that table could not see. The variants are now probed —
> and the file says plainly that it cannot know which variants exist, only which it was given.
