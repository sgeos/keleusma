# Chapter 23. Handling Partial Operations

## Goal

By the end of this chapter you will be able to handle the operations
that can fail, namely arithmetic that does not fit, division by zero,
an index past the end of an array, and a few others, so that your
program stays total and never fails silently.

## Total and partial operations

Most operations always produce a value. Adding two small numbers, taking
a struct field, comparing two values: these are total, defined for every
input. A few operations are different. They are mathematically partial,
meaning undefined on some inputs. Arithmetic can overflow the range of a
`Word`. Division by zero has no answer. An index can point past the end
of an array. A refinement can reject its value. Each of these is a real
input the language must do something with.

Keleusma does not let a partial operation pass silently or crash. It
gives each one a defined outcome and a construct that performs the
operation and reports which case happened, so your program decides what
to do. This chapter covers that family of constructs. We begin with
arithmetic.

## The checked arithmetic construct

The construct is an arithmetic expression followed by arms in braces:

```
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
```

Run it with `keleusma run`. The output is `42`.

The expression `a + b` is performed, and the result is routed to one of
the arms.

- `ok(v)` runs when the true result fits in a `Word`. The result is bound
  to `v`.
- `overflow` runs when the true result is too large.
- `underflow` runs when the true result is too far below zero.

For `20 + 22`, the result `42` fits, so the `ok` arm runs.

## When the result does not fit

Change `main` to add the largest `Word` and one more:

```
fn main() -> Word {
    add_checked(9223372036854775807, 1)
}
```

That sum is one past the largest `Word`. Now the `overflow` arm runs
instead, and the function returns `0`. The arithmetic did not fail
silently and did not produce a quietly wrong answer. The construct
reported the overflow, and the program decided what to do about it.

## The high and low halves

The `overflow` and `underflow` arms were written `overflow(_, _)` above,
ignoring what they carry. On a `Word` they carry two values, the high
half and the low half of the true result, computed in a number twice as
wide as a `Word`:

```
overflow(high, low) => ...
```

These two halves are the foundation of big-number arithmetic. A number
too large for one `Word` is carried as a pair, a high half and a low
half, and the carry from one position threads into the next. The bundled
example `examples/scripts/09_big_numbers.kel`, and the guide page
`BIG_NUMBERS.md`, work this technique in full.

## The first-class multi-word type

You do not have to thread the carry by hand for the common case. The
`Multiword<N, F>` type is a fixed-width multi-word fixed-point value, `N`
words wide with `F` fractional bits, that carries the halves for you. The
form `Multiword<N>` is the integer case, equal to `Multiword<N, 0>`. You
construct one from a tuple of its words, least significant first, and
index its words back out:

```
fn main() -> Word {
    let a = (9223372036854775807, 0) as Multiword<2>;
    let b = (1, 0) as Multiword<2>;
    let s = a + b;
    s[1]
}
```

The low word of `a` is the largest `Word`. Adding `1` sets that word's
top bit, turning it into the smallest `Word`, but no bit carries out of
the low word, so the high word `s[1]` stays `0`. This is the correct
unsigned multi-word carry, which is not the same as the signed-overflow
report of the checked construct above. Addition, subtraction,
and the six comparisons are lowered to the very carry and borrow cascade
this chapter describes, so those operations add no new instructions.
Integer and fixed-point multiplication, division, and modulo, which
apply the fractional scale `F`, along with the four shifts `lsl`, `asl`,
`lsr`, and `asr` and the per-limb bitwise operators `band`, `bor`,
`bxor`, and `bnot`, are also available. The type was delivered as B19.

## Optional arms and the wrapping default

The `overflow` and `underflow` arms are optional. When you omit them, an
out-of-range result wraps around in two's complement, the same as bare
machine arithmetic. So a construct with only an `ok` arm is exactly the
ordinary wrapping operation, written out so the intent is visible:

```
let total = a + b { ok(v) => v };
```

You add the arms only for the cases you want to handle. The `ok` arm is
the one you must always write.

## Division by zero

Division and modulo have a different failure, a zero divisor, with no
result at all. The `zero_divisor` arm handles it and binds the numerator:

```
fn safe_div(a: Word, b: Word) -> Word {
    a / b {
        ok(q) => q,
        zero_divisor(n) => 0,
    }
}

fn main() -> Word {
    safe_div(10, 0)
}
```

The output is `0`. Without the `zero_divisor` arm, a division by zero
stops the program with a recoverable error rather than producing a
silent wrong answer.

## The other number types

The construct works on the four numeric types, not only `Word`. On
`Byte`, `Float`, and `Fixed<N>` an overflow or underflow arm binds a
single result rather than two halves, because those types do not carry
the big-number high half:

```
fn main() -> Byte {
    200Byte + 100Byte {
        ok(v) => v,
        overflow(w) => w,
    }
}
```

The sum `300` does not fit in a `Byte`, so the `overflow` arm runs and
binds the wrapped result `w`, which is `44`; a `Byte` result prints as
`Byte(44)`, the value tagged with its type. The supported operators are
`+`, `-`, `*`, `/`, `%`, the arithmetic left shift `asl`, and unary `-`,
with the admissible arms depending on the type. An unsigned `Byte`, for
instance, can overflow on addition but can only go below zero on
subtraction. The arithmetic left shift `asl` is the value `x * 2^k`, so
on a `Word` it can overflow or go below zero exactly as a multiply does,
and it takes the same `overflow` and `underflow` arms.

## Saturating to the edge

Inside an arm body, the keywords `saturate_max` and `saturate_min` stand
for the largest and smallest value of the construct's type. They let you
clamp an out-of-range result to the edge of the range instead of choosing
a number by hand:

```
fn main() -> Byte {
    200Byte + 100Byte {
        ok(v) => v,
        overflow(_) => saturate_max,
    }
}
```

The output is `Byte(255)`, the largest `Byte`. On `Word` the keywords are the
word bounds, on `Float` the largest and most-negative finite value, and
on `Fixed<N>` the extremal fixed-point value. When the result type is a
refined newtype that declared a `with saturate_max` or `with
saturate_min` value, the keyword resolves to that declared bound.

## A family of constructs

The same brace-and-arms shape handles every partial operation in the
language, each with its own arm keywords.

**Indexing.** An array index can point past the end. The `invalid_index`
arm binds the offending index, and `ok` binds the element:

```
fn main() -> Word {
    let a = [10, 20, 30];
    a[9] {
        ok(v) => v,
        invalid_index(_) => 0,
    }
}
```

The index `9` is out of range, so the result is `0`.

**Newtype construction.** Constructing a refined newtype can fail when
the value breaks the rule. The `invalid_newtype` arm binds the value the
predicate rejected:

```
fn is_positive(x: Word) -> bool { x > 0 }
newtype Positive = Word where is_positive;

fn main() -> Word {
    let p = Positive(0 - 4) {
        ok(v) => v as Word,
        invalid_newtype(_) => 1,
    };
    p
}
```

The value `-4` fails the rule, so the `invalid_newtype` arm runs and the
result is `1`.

**Discriminant to enum.** A `Word` can be turned back into an enum value,
the reverse of casting an enum to its discriminant. A unit variant
converts to itself, the `payload_discriminant` arm supplies a
payload-bearing variant's payload, and `invalid_discriminant` catches a
`Word` that matches no variant:

```
enum Signal { Stop = 0, Go = 1 }

fn main() -> Word {
    let s = 1 as Signal {
        invalid_discriminant(_) => Signal::Stop,
    };
    s as Word
}
```

The discriminant `1` is the `Go` variant, so the result is `1`.

**Native call.** A native function provided by the host can report a
failure. The `error` arm binds the `Word` error code the native reports,
and `ok` binds the success value:

```
let row = host::lookup(id) {
    ok(v) => v,
    error(code) => code,
};
```

A native call is exercised from an embedding host rather than from
`keleusma run`. [Chapter 33, Registering Natives](./33_registering_natives.md)
shows the host side, including how a host reports the error code.

## Two backends, one contract

Every construct here shares one contract. The bytecode virtual machine,
the verifying interpreter you run with `keleusma run`, traps on any
unhandled partial operation. A trap is a recoverable error the host
receives, not a crash. A future native build of the same program, the
subject of a later milestone, instead produces a defined, non-crashing
value, using the hardware result where the hardware does not fault and a
small inserted check where it would. The two builds can differ only on a
partial operation you did not handle. Handle every outcome through these
constructs and your program is total: it produces the same result on
both backends and never traps. The full contract, including the value
each backend produces for each operation, is specified in
[RUNTIME_FAULTS.md](../../docs/spec/RUNTIME_FAULTS.md).

## What you now know

- A few operations are partial, undefined on some inputs. The language
  gives each a defined outcome and a construct to handle it.
- Checked arithmetic, `a + b { ok(v) => ..., overflow(...) => ... }`,
  reports overflow, underflow, and the zero divisor. The `overflow` and
  `underflow` arms are optional and default to wrapping; `ok` is
  required. On `Word` the arms carry the high and low halves, the
  foundation of big-number arithmetic; on `Byte`, `Float`, and
  `Fixed<N>` they carry a single result.
- `saturate_max` and `saturate_min` clamp to the edge of the type's
  range.
- The same shape handles indexing (`invalid_index`), newtype
  construction (`invalid_newtype`), the discriminant-to-enum conversion
  (`payload_discriminant`, `invalid_discriminant`), and native calls
  (`error`).
- An unhandled partial operation traps on the virtual machine. Handling
  every outcome makes a program total.

The next chapter, the last of Part VI, marks data as confidential and
lets the language track where it flows.
