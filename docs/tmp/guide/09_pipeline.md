# Chapter 9. The Pipeline Operator

> Part II, The Building Blocks. Chapter 9 of 40.
> Previous: [Chapter 8, Bounded Repetition](./08_bounded_repetition.md).
> Next: [Chapter 10, Structs](./10_structs.md).

## Goal

By the end of this chapter you will be able to write a chain of
transformations that reads from left to right.

## A chain of transformations

A guitarist sends a signal through a chain of effects pedals. The sound
leaves the guitar, enters the first pedal, leaves changed, enters the
next, and so on. The chain reads in one direction, and each stage feeds
the next.

A program often does the same with a value: take a starting value, pass
it through one function, pass that result through another. Written as
ordinary calls, the chain nests inside out, and the reader has to start
from the middle. Keleusma offers a clearer way to write it.

## The pipeline operator

The pipeline operator is written `|>`. The expression `x |> f(args)` means
"call `f`, with `x` as its first argument, followed by `args`." It takes
the value on the left and threads it into the call on the right as that
call's first argument.

````
fn up(note: Word, semitones: Word) -> Word {
    note + semitones
}

fn main() -> Word {
    60
    |> up(7)
    |> up(5)
}
````

Run it with `keleusma run`. The output is:

````
72
````

Read the chain from the top. The starting value is `60`, the MIDI number
of middle C. The line `|> up(7)` calls `up(60, 7)`, raising the note by a
perfect fifth to `67`. The next line `|> up(5)` calls `up(67, 5)`, raising
that by a perfect fourth to `72`. A fifth stacked on a fourth spans an
octave, and `72` is indeed middle C raised by one octave.

## Why the pipeline helps

The same program without the pipeline would be written `up(up(60, 7), 5)`.
That is correct, but it reads from the inside out. The starting value
`60` is buried in the middle, and the first step applied, `up(7)`, sits
inside the second. The pipeline version places `60` first and lists each
step in the order it happens. It reads the way the music moves, one
transformation after another.

## What you now know

- `x |> f(args)` calls `f` with `x` as its first argument.
- A pipeline chains transformations so they read top to bottom in the
  order they occur.
- The pipeline is a clearer way to write what would otherwise be nested
  calls.

That completes Part II. You can now name values, write functions, make
decisions, repeat actions, and chain transformations. Part III turns to
building larger shapes of data.
