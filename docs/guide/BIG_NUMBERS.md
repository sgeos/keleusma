# Big Numbers

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

The V0.2 numeric overflow construct binds the high and low halves of an `i128` intermediate result on every checked arithmetic operation. This is the load-bearing mechanism for multi-digit arithmetic against the bundled `Word` type, which is a 64-bit signed integer. This guide walks through the pattern with the worked example in [`examples/scripts/09_big_numbers.kel`](../../examples/scripts/09_big_numbers.kel).

## What the construct exposes

The construct's surface form is

````
op_expr {
    ok(v)             => arm_body,
    overflow(h, l)    => arm_body,
    underflow(h, l)   => arm_body,
}
````

The runtime computes the true result of `op_expr` in `i128`, splits it into a high and a low half, and pushes `(high, low, flag)` on the operand stack. The compiler dispatches on `flag` to one of three outcome classes (`ok`, `overflow`, `underflow`) and binds the pattern variables in that arm's body to the corresponding slot values. The `ok` arm binds a single `Word` against the in-range result; the `overflow` and `underflow` arms bind two `Word` values against the high and low halves.

The high half is the carry-out for additive operations and the upper 64 bits of the true product for multiplication. Together with the low half (the wrapped i64 result) this is sufficient to express chained multi-digit arithmetic.

## Pattern: full 64x64 -> 128-bit multiplication

````
fn mul_full(a: Word, b: Word) -> (Word, Word) {
    a * b {
        ok(v) => (0, v),
        overflow(h, l) => (h, l),
        underflow(h, l) => (h, l),
    }
}
````

When the true product fits in `Word` the `ok` arm fires and the high half is zero by definition. When the product needs more than 64 bits the construct routes to the overflow arm and binds the upper 64 bits of the true product to `h`.

Worked example: `2^32 * 2^32 = 2^64`. The true product is the bit pattern `0x0000000000000001_0000000000000000` interpreted as a 128-bit value. The high half is `1`, the low half is `0`. The script's `main` returns `1` confirming this decomposition.

## Pattern: addition with carry-out

````
fn add_with_carry(a: Word, b: Word) -> (Word, Word) {
    a + b {
        ok(v) => (0, v),
        overflow(_, l) => (1, l),
        underflow(_, l) => (1, l),
    }
}
````

The carry-out is derived from the overflow class rather than from the high half directly. For signed `Word` addition the high half of the i128 intermediate is the sign extension of the i64 wrap, not the unsigned carry; the cleaner abstraction is to read the carry from the outcome class. The wrapped result remains in the low slot.

A chained two-digit add propagates the carry to the next-higher position:

````
fn add_two_digits(a_hi: Word, a_lo: Word, b_hi: Word, b_lo: Word) -> (Word, Word) {
    let (carry_lo, sum_lo) = add_with_carry(a_lo, b_lo);
    let (_, partial_hi) = add_with_carry(a_hi, b_hi);
    let (_, sum_hi) = add_with_carry(partial_hi, carry_lo);
    (sum_hi, sum_lo)
}
````

For a full 256-bit add, repeat the same step over four `Word` positions, threading the carry through each.

## Caveats

The `Word` type is signed `i64`. Treating it as an unsigned `u64` digit in multi-digit arithmetic requires care:

1. The i128 intermediate's high half reflects signed arithmetic. For two non-negative operands whose sum exceeds `i64::MAX`, the high half is `0` and the low half is the wrap (a negative `i64` whose bit pattern matches the high bit of the unsigned sum). The carry-out is one regardless, derivable from the overflow class.

2. For two operands whose sum needs more than 65 bits (impossible for `i64` addition but reachable through multiplication), the high half carries genuine bits 64-127. The multiplication example demonstrates this case directly.

3. Division and modulo currently route to a stamped-zero-flag path: the construct produces `(high=0, low=result, flag=0)` for non-corner cases, and the `i64::MIN / -1` corner is left to the existing arithmetic. A dedicated `Op::CheckedDiv` / `Op::CheckedMod` family would close the corner; the project's backlog records the item but no consumer has demanded it yet.

## Where the pattern is and is not appropriate

The construct is appropriate when:

- The arithmetic needs to detect or recover from `Word`-range overflow at well-defined points in the program.
- The high half of a multiplication carries useful information (the load-bearing case for true 64x64 -> 128 products).
- The carry-out of an addition needs to thread into a higher-order digit.

The construct is not a substitute for an arbitrary-precision `BigInt` type. Multi-digit arithmetic at runtime through the construct works but is not ergonomically zero-cost; a future iteration may introduce a dedicated `BigInt` standard-library type with native arithmetic operators that compile to the chained checked operations under the hood.

## Cross-references

- [GRAMMAR.md, Section 7.5](../design/GRAMMAR.md) — the formal grammar of the numeric overflow construct.
- [LANGUAGE_DESIGN.md, "Numeric Overflow Construct"](../architecture/LANGUAGE_DESIGN.md) — the design rationale.
- [`examples/scripts/09_big_numbers.kel`](../../examples/scripts/09_big_numbers.kel) — the worked example this guide walks through.
- [`tests/big_number_arithmetic.rs`](../../tests/big_number_arithmetic.rs) — the integration test that compiles the example and verifies the result.
