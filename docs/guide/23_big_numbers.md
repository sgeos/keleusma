# Chapter 23. Big Numbers: The Overflow Construct

> Part VI, Going Deeper. Chapter 23 of 40.
> Previous: [Chapter 22, Newtypes and Refinement Types](./22_newtypes_and_refinement.md).
> Next: [Chapter 24, Information-Flow Labels](./24_information_flow_labels.md).

## Goal

By the end of this chapter you will be able to perform arithmetic that
checks, safely, whether its result fits.

## A number has a range

A `Word` holds whole numbers, but only up to a limit. Most arithmetic
stays well within that limit. Now and then it does not: add or multiply
large enough values and the true result is too big for a single `Word`
to hold. This is called overflow, and its opposite, a result too far
below zero, is called underflow.

Keleusma does not let overflow pass silently. It offers a construct that
performs one arithmetic operation and reports which of three things
happened.

## The overflow construct

The construct is an arithmetic expression followed by three arms in
braces:

````
fn add_checked(a: Word, b: Word) -> Word {
    a + b {
        ok(v) => v,
        overflow(_, _) => 0,
        underflow(_, _) => 0,
    }
}

fn main() -> Word {
    add_checked(20, 22)
}
````

Run it with `keleusma run`. The output is `42`.

The expression `a + b` is performed, and the result is routed to one of
the three arms.

- `ok(v)` runs when the true result fits in a `Word`. The result is bound
  to `v`.
- `overflow` runs when the true result is too large.
- `underflow` runs when the true result is too far below zero.

For `20 + 22`, the result `42` fits, so the `ok` arm runs and the
function returns `42`.

## When the result does not fit

Change `main` to add the largest `Word` and one more:

````
fn main() -> Word {
    add_checked(9223372036854775807, 1)
}
````

That sum is one past the largest `Word`. Now the `overflow` arm runs
instead, and the function returns `0`. The arithmetic did not fail
silently and did not produce a quietly wrong answer. The construct
reported the overflow, and the program decided what to do about it.

## The high and low halves

The `overflow` and `underflow` arms were written `overflow(_, _)` above,
ignoring what they carry. They actually carry two values, the high half
and the low half of the true result, computed in a number twice as wide
as a `Word`:

````
overflow(high, low) => ...
````

These two halves are the foundation of big-number arithmetic. A number
too large for one `Word` is carried as a pair, a high half and a low
half, and the carry from one position threads into the next. The bundled
example `examples/scripts/09_big_numbers.kel`, and the guide page
`BIG_NUMBERS.md`, work this technique in full.

The construct guards a single operation. It supports `+`, `-`, `*`, `/`,
`%`, and the negation `-` on `Word` values.

## What you now know

- A `Word` has a range, and arithmetic can produce a result outside it.
- The overflow construct, `a + b { ok(v) => ..., overflow(...) => ...,
  underflow(...) => ... }`, performs one operation and reports which case
  occurred.
- `ok` carries the result; `overflow` and `underflow` carry the high and
  low halves of the true result.
- The construct works on `+`, `-`, `*`, `/`, `%`, and unary `-`.

The next chapter, the last of Part VI, marks data as confidential and
lets the language track where it flows.
