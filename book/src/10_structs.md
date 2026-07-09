# Chapter 10. Structs

## Goal

By the end of this chapter you will be able to bundle several related
values into one named shape.

## A note is more than a pitch

A single note carries several facts at once. It has a pitch. It has a
loudness, often called velocity. It might have a duration. These facts
belong together. Passing them around as three separate values, always in
the right order, is error-prone. A struct bundles them into one value
with named parts.

## Declaring a struct

A struct declaration lists the parts, each with a name and a type:

```
struct Note {
    pitch: Word,
    velocity: Word,
}
```

This declares a new type, `Note`. A `Note` value has two parts, called
fields: a `pitch` and a `velocity`, each a `Word`.

## Building and using a struct

Here is a complete program that builds a `Note` and reads its fields:

```
struct Note {
    pitch: Word,
    velocity: Word,
}

fn brightness(n: Note) -> Word {
    n.pitch + n.velocity
}

fn main() -> Word {
    let middle_c = Note { pitch: 60, velocity: 90 };
    brightness(middle_c)
}
```

Run it with `keleusma run`. The output is:

```
150
```

Three things happen.

- `Note { pitch: 60, velocity: 90 }` builds a `Note`. Each field is given
  a value by name. This is called construction.
- `brightness` takes one parameter, a whole `Note`, rather than two loose
  numbers. The two facts travel together.
- `n.pitch` and `n.velocity` read the fields. A field is reached by
  writing the value, a dot, and the field name.

## Why bundle

A struct lets a function signature say what it really means. `brightness`
takes a `Note`, not a pair of numbers that the caller must remember to
pass in the correct order. The structure of the data is written down once,
in the declaration, and every part of the program then agrees on it.

## What you now know

- `struct Name { field: Type, ... }` declares a new bundled type.
- `Name { field: value, ... }` constructs a value of it.
- `value.field` reads a field.

The next chapter describes a value that is one of a fixed set of
choices.
