# Chapter 13. Pattern Matching in Depth

> Part III, Shaping Data. Chapter 13 of 40.
> Previous: [Chapter 12, Tuples and Arrays](./12_tuples_and_arrays.md).
> Next: [Chapter 14, Multiheaded Functions and Guards](./14_multiheaded_functions.md).

## Goal

By the end of this chapter you will understand `match` thoroughly: the
kinds of pattern it accepts, the rule that it must be complete, and the
guard that refines an arm.

## A recap

Earlier chapters used `match` to choose among cases. Chapter 7 matched a
`Word` against literal numbers. Chapter 11 matched an enum against its
variants. This chapter gathers the full picture.

A `match` has a value and a list of arms. Each arm is a pattern, then
`=>`, then a result. The first arm whose pattern fits the value is the
one that runs.

## The kinds of pattern

Three kinds of pattern appear in an arm.

- A literal, such as `3`, fits only that exact value.
- A binding, such as `midi`, fits any value and gives it that name
  inside the arm.
- The wildcard, `_`, fits any value and names nothing. It is the
  catch-all.

An enum variant pattern, such as `Signal::Note(midi)`, fits one variant
and binds the value the variant carries.

## A worked example

````
enum Signal {
    Rest,
    Note(Word),
}

fn loudness(s: Signal) -> Word {
    match s {
        Signal::Rest => 0,
        Signal::Note(midi) when midi >= 60 => 2,
        Signal::Note(midi) => 1,
    }
}

fn main() -> Word {
    loudness(Signal::Note(72))
}
````

Run it with `keleusma run`. The output is:

````
2
````

The value is `Signal::Note(72)`. The first arm wants `Signal::Rest`, and
does not fit. The second arm wants a `Signal::Note`, binds its carried
value as `midi`, and then asks a further question with `when midi >= 60`.
A `when` on an arm is a guard. The arm runs only if its pattern fits and
its guard is true. Here `72 >= 60` is true, so the arm runs and the
result is `2`.

If the note had been below 60, the guarded arm's guard would be false,
and matching would fall through to the third arm, `Signal::Note(midi)`,
which has no guard and produces `1`.

## A match must be complete

Every `match` must account for every possible value. A `match` over an
enum is complete when every variant is covered, and the language checks
this for you. A `match` over a `Word`, which has far too many values to
list, is completed with a `_` wildcard arm.

Completeness is not a formality. It is the guarantee that the `match`
produces a result no matter what value arrives. There is no case the
program forgot.

## Taking apart tuples and structs

`match` is at its best on enums, where the language can check
completeness precisely. For the other shapes from this part, simpler
tools were already given. A tuple is taken apart with `let (a, b) = ...`,
shown in Chapter 12. A struct's fields are read with `value.field`, shown
in Chapter 10. Reach for those first, and reserve `match` for choosing
among an enum's variants and among literal values.

## What you now know

- A `match` arm is a pattern, `=>`, and a result.
- Patterns are literals, bindings, the wildcard `_`, and enum variant
  patterns that bind carried values.
- A `when` guard refines an arm with a further condition.
- Every `match` must be complete, and the language enforces it.

The next chapter spreads a single function across several heads.
