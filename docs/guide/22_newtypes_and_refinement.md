# Chapter 22. Newtypes and Refinement Types

> Part VI, Going Deeper. Chapter 22 of 40.
> Previous: [Chapter 21, Generics and Traits](./21_generics_and_traits.md).
> Next: [Chapter 23, Big Numbers: The Overflow Construct](./23_big_numbers.md).

## Goal

By the end of this chapter you will be able to give a type a distinct
name, and attach a rule that every value of it must satisfy.

## The problem of look-alike numbers

A channel number is a `Word`. A note velocity is a `Word`. A MIDI pitch
is a `Word`. They are all whole numbers, and so the language, left to
itself, would let any of them be used where another was meant. Handing a
velocity to a function expecting a channel is a real mistake, and one the
type `Word` cannot catch, because all three are the same type.

## A newtype: a distinct name

A newtype gives an underlying type a new, distinct name:

````
newtype Channel = Word;

fn raw(c: Channel) -> Word {
    c as Word
}

fn main() -> Word {
    raw(Channel(2))
}
````

Run it with `keleusma run`. The output is `2`.

Underneath, a `Channel` is a `Word`, and runs exactly as fast. To the
type system, though, `Channel` and `Word` are different types. A
`Channel` is built by writing `Channel(2)`. The underlying `Word` is
recovered by writing `c as Word`. The language will not let a plain
`Word` be used where a `Channel` is expected, or the reverse, without one
of those explicit steps. The look-alike numbers are now kept apart.

## A refinement: a name with a rule

A newtype can carry a rule, called a refinement. The rule is an ordinary
function that takes the underlying value and answers `true` or `false`:

````
fn in_range(x: Word) -> bool {
    x >= 0 and x <= 127
}

newtype Velocity = Word where in_range;

fn raw(v: Velocity) -> Word {
    v as Word
}

fn main() -> Word {
    let soft = Velocity(40);
    raw(soft)
}
````

Run it. The output is `40`. A MIDI velocity must lie between 0 and 127.
The `where in_range` clause attaches that rule to `Velocity`. Every time
a `Velocity` is built, the rule is checked. The value `40` passes, so
`Velocity(40)` succeeds.

## A broken value is caught

Change the program to build a velocity outside the range:

````
fn main() -> Word {
    raw(Velocity(200))
}
````

Run it, and there is no result, only:

````
error: compile: refinement check `in_range` provably fails for newtype `Velocity` at compile time on argument 200
````

The value `200` is written into the program, so the language checks the
rule then and there, before the program runs, and rejects it. A
`Velocity` simply cannot hold an out-of-range number. The rule, written
once in the `where` clause, is enforced at every construction. It is a
part written so that a wrong note cannot be played.

## What you now know

- `newtype Name = Underlying;` gives an underlying type a distinct name.
- `Name(value)` builds one; `value as Underlying` recovers the
  underlying value.
- `newtype Name = Underlying where predicate;` attaches a rule, checked
  at every construction.
- A construction that provably breaks the rule is rejected before the
  program runs.

The next chapter handles arithmetic whose result may not fit.
