# Chapter 11. Enums

> Part III, Shaping Data. Chapter 11 of 40.
> Previous: [Chapter 10, Structs](./10_structs.md).
> Next: [Chapter 12, Tuples and Arrays](./12_tuples_and_arrays.md).

## Goal

By the end of this chapter you will be able to describe a value that is
exactly one of a fixed set of choices.

## One of a fixed set

A struct bundles several facts that are all present at once. Sometimes a
value is instead exactly one of a small, fixed set of possibilities. The
articulation of a note is one of staccato, legato, or accent. It is
always exactly one. A value like that is described by an enum.

## Declaring and matching an enum

Each choice in an enum is called a variant:

````
enum Articulation {
    Staccato,
    Legato,
    Accent,
}

fn length_percent(a: Articulation) -> Word {
    match a {
        Articulation::Staccato => 50,
        Articulation::Legato => 100,
        Articulation::Accent => 90,
    }
}

fn main() -> Word {
    length_percent(Articulation::Staccato)
}
````

Run it. The output is `50`. A staccato note is held for about half its
written length.

A variant is named with the enum name, two colons, and the variant name,
as in `Articulation::Staccato`. The `match` checks which variant the
value is and chooses the matching arm.

Notice that the `match` has no `_` catch-all. It does not need one. The
language knows every variant of `Articulation`, and all three are listed,
so the `match` is complete. If a variant were left out, the program would
be rejected before it ran. This is a real safety net. Add a fourth
articulation later, and every `match` that forgot to handle it is caught
at once.

## Variants that carry a value

A variant can also carry a value of its own. An interval might be a
unison, or a rise of some number of semitones, or a fall:

````
enum Interval {
    Unison,
    Up(Word),
    Down(Word),
}

fn semitone_shift(i: Interval) -> Word {
    match i {
        Interval::Unison => 0,
        Interval::Up(n) => n,
        Interval::Down(n) => 0 - n,
    }
}

fn main() -> Word {
    semitone_shift(Interval::Up(7))
}
````

Run it. The output is `7`. The variants `Up` and `Down` each carry a
`Word`. When a `match` arm names that carried value, as `Interval::Up(n)`
does, the value becomes available as `n` inside the arm. Building such a
variant looks like a function call: `Interval::Up(7)`.

## What you now know

- `enum Name { Variant, ... }` declares a value that is one of a fixed
  set.
- `Name::Variant` names a variant, and `match` chooses on it.
- A `match` over an enum must cover every variant, and the language
  checks this.
- A variant may carry a value, written `Variant(Type)`, and a `match` arm
  can name and use that carried value.

The next chapter groups values by position rather than by name.
