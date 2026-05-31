# Chapter 24. Information-Flow Labels

> Part VI, Going Deeper. Chapter 24 of 40.
> Previous: [Chapter 23, Handling Partial Operations](./23_big_numbers.md).
> Next: [Chapter 25, From Source to Bytecode](./25_from_source_to_bytecode.md).

## Goal

By the end of this chapter you will be able to mark a value as
confidential and let the language check that it does not flow somewhere
it should not.

This is an advanced chapter, and the last of Part VI. The features in it
are not needed for everyday programs. They are here for programs that
must keep some data confidential.

## A label rides on a type

A master recording is not meant to leave the studio. A program may handle
data with the same quality: a value that must not reach a public output.
Keleusma lets a type carry a label, a tag that marks the value and rides
along with it. A label is written after the type with an `@`:

````
fn main() -> Word@Master {
    classify 42@Master
}
````

Run it with `keleusma run`. The output is `42`.

`Word@Master` is a `Word` carrying the label `Master`. The operator
`classify 42@Master` takes the plain value `42` and attaches the `Master`
label to it. The label names are chosen by the programmer; `Master` here
is one such name.

The label exists only while the program is being checked. Once the
program runs, the label is gone, and the value is an ordinary `42`. The
label costs nothing at run time. It is purely a check the language
performs beforehand.

## A label blocks a leak

The point of a label is that the language follows it. A labelled value
may not flow into a place that does not accept the label. Here a function
`broadcast` sends a plain `Word` to a public output:

````
fn broadcast(x: Word) -> Word {
    x
}

fn main() -> Word {
    let take = classify 42@Master;
    broadcast(take)
}
````

Run it, and there is no result:

````
error: compile: type error: argument to `broadcast` expects Word, got Word@Master
````

The value `take` carries the `Master` label. The parameter of `broadcast`
is a plain `Word`, with no label, so it does not accept `Master`-labelled
data. Handing `take` to `broadcast` would let the master recording reach
a public output. The language calls that a leak and rejects the program
before it runs.

## Declassify: a deliberate release

Sometimes confidential data genuinely should be released, by an explicit
decision. The operator `declassify` removes a label:

````
fn broadcast(x: Word) -> Word {
    x
}

fn main() -> Word {
    let take = classify 42@Master;
    broadcast(declassify take@Master)
}
````

Run it. The output is `42`. The `declassify take@Master` removes the
`Master` label, producing a plain `Word`, which `broadcast` accepts.

The two operators are not equal in weight. `classify` only adds a
restriction, and is always safe. `declassify` removes one, and so it is
the single, visible place in the program where confidential data is
released. A reviewer reading the program can find every release by
searching for `declassify`.

## What you now know

- A type can carry a label, written `T@Label`, that marks a value.
- `classify expr@Label` adds a label; `declassify expr@Label` removes
  one.
- The language tracks labelled values and rejects, before the program
  runs, a flow into a place that does not accept the label.
- Labels are erased before the program runs and cost nothing at run
  time.
- `declassify` is the deliberate, visible point where confidential data
  is released.

That completes Part VI. You have now seen the whole language a script
author writes. Part VII turns to what happens to a program after it is
written: how it is compiled, signed, and swapped.
