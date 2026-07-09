# Chapter 6. Functions

## Goal

By the end of this chapter you will be able to write a function of your
own, give it inputs, and use its result.

## A function is a named phrase

A phrase in music is a small, complete musical thought that can be played
wherever it is wanted. A function is the same idea in a program. It is a
named piece of computation. Once it has a name, it can be used anywhere,
as often as needed, without writing it out again.

Every program so far has had one function, `main`. A program may have as
many functions as it needs.

## Writing a function

Here is a function that answers a question: how many semitones are there
in a given number of octaves?

```
fn semitone_steps(octaves: Word) -> Word {
    octaves * 12
}

fn main() -> Word {
    semitone_steps(3)
}
```

Run it with `keleusma run`. The output is:

```
36
```

Three octaves span thirty-six semitones.

## The parts of a function

Read `semitone_steps` piece by piece.

- `fn` begins the function.
- `semitone_steps` is its name. A name should say what the function does.
- `(octaves: Word)` is the parameter list. A parameter is an input. This
  function takes one input, named `octaves`, of type `Word`. Each
  parameter states its type.
- `-> Word` states the type of the result the function gives back.
- `{ octaves * 12 }` is the body. The body computes the result.

The body's last expression is the result. There is no special word for
"give this back." The function `semitone_steps` ends with `octaves * 12`,
so that is what it returns.

## Calling a function

Using a function is called calling it. A call is the function's name
followed by its inputs in parentheses. The call `semitone_steps(3)` runs
the function with `octaves` set to `3`.

A function may take more than one input. The parameters are separated by
commas:

```
fn interval(low: Word, high: Word) -> Word {
    high - low
}

fn main() -> Word {
    interval(60, 67)
}
```

That program returns `7`. The distance from MIDI note 60, middle C, up to
MIDI note 67, the G above it, is seven semitones, a perfect fifth.

## What you now know

- A function is a named, reusable piece of computation.
- `fn name(parameter: Type, ...) -> ResultType { body }` declares one.
- The body's last expression is the result.
- A call is `name(inputs)`.

The next chapter lets a program make decisions.
