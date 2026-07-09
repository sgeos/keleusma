# Chapter 4. Values and Types

## Goal

By the end of this chapter you will know the kinds of value Keleusma
works with, and the name of the type that each kind belongs to.

## A type is a set of values that make sense together

A frequency, such as 261.6 hertz, is a fractional number. A count of
beats is a whole number. Whether a note is sounding right now is a plain
yes or no. These are different kinds of value, and a kind of value is
called a type. The type is how the language knows what makes sense for a
value and what does not.

This chapter uses the interactive prompt. Start it with `keleusma repl`
and type along.

## Word, a whole number

A `Word` is a whole number. Counts are words: a number of beats, a number
of semitones, a MIDI note number.

```
> 12
12
> 7 + 5
12
```

One result will surprise you. Dividing one `Word` by another throws away
any remainder:

```
> 7 / 2
3
```

Seven divided by two is three, with one left over, and the leftover is
discarded. Whole-number division always rounds toward zero. When a
fraction is needed, the next type is the one to reach for.

## Float, a fractional number

A `Float` is a number that can have a fractional part. Frequencies are
floats. A float is written with a decimal point, and the point is what
tells the language the value is a float and not a word.

```
> 3.5
3.5
> 7.0 / 2.0
3.5
```

Note the `.0` in `7.0` and `2.0`. Dividing two floats keeps the
fractional part, so `7.0 / 2.0` is `3.5`, not `3`.

## bool, true or false

A `bool` is the answer to a yes-or-no question. It has exactly two
values, `true` and `false`. A comparison produces a `bool`:

```
> 3 < 5
true
```

## Text, written words

A `Text` value is a piece of writing. It is written between double
quotes.

```
> "middle C"
middle C
```

## Unit, no value at all

`Unit` is the type of `()`, which is read aloud as "unit." It means there
is no meaningful value. A function that does something useful but has
nothing to hand back returns `()`.

```
> ()
()
```

## A few more number types, met later

Keleusma has three further number types. None is needed in Part II, so
they are only named here.

- `Byte` is an eight-bit whole number, used for byte-level work. It
  appears in Chapter 23.
- `Fixed` is a fractional number with fully deterministic, repeatable
  arithmetic, used where audio code must produce the exact same result
  every time. The piano roll uses it.
- `Multiword<N, F>` is a fixed-width multi-word number, `N` words wide
  with `F` fractional bits, for values too large for a single `Word`. It
  appears in Chapter 23.

## Why types matter

Every value in a Keleusma program has a type, and the language checks,
before the program runs, that values are only used where their type makes
sense. Handing a frequency to a function that expects a count of beats is
caught at that check, not discovered later as a wrong note. The types are
a safety net stretched under the whole program.

## What you now know

- `Word` is a whole number, and whole-number division drops the
  remainder.
- `Float` is a fractional number, written with a decimal point.
- `bool` is `true` or `false`.
- `Text` is writing in double quotes.
- `Unit`, written `()`, means no value.
- `Byte`, `Fixed`, and `Multiword<N, F>` are further number types, met
  later.

The next chapter gives values names.
