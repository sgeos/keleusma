# Chapter 14. Multiheaded Functions and Guards

## Goal

By the end of this chapter you will be able to write a function as
several heads, each handling a different case.

## A prepared response for each cue

A performer rehearses a prepared response for each cue the conductor
might give. The responses are written out separately, one per cue, rather
than as one tangled instruction. Keleusma allows a function to be written
the same way. A single function name may have several heads, each with
its own case, and the right head is chosen when the function is called.

## Heads that match a value

Here a function gives back the MIDI number of a fret on a guitar's high E
string. The open string, fret zero, is treated as its own case:

```
fn fret_note(0) -> Word { 64 }
fn fret_note(n: Word) -> Word { 64 + n }

fn main() -> Word {
    fret_note(5)
}
```

Run it with `keleusma run`. The output is `69`.

There are two heads for `fret_note`. The first head matches only the
exact argument `0`. The second head, with the binding `n`, matches any
argument. The heads are tried in the order written, and the first that
fits is the one used. The call `fret_note(5)` does not match `0`, so it
falls to the second head, which gives `64 + 5`, that is `69`, the MIDI
number of A4.

The order matters. The specific case, `0`, is written before the general
case, `n`. Written the other way round, the general head would catch
every call and the `0` head would never be reached.

## Heads with guards

A head may instead carry a `when` guard, the same guard seen on `match`
arms in Chapter 13. The head is used only when its guard is true:

```
fn tempo_class(bpm: Word) -> Word when bpm < 60 { 0 }
fn tempo_class(bpm: Word) -> Word when bpm < 120 { 1 }
fn tempo_class(bpm: Word) -> Word { 2 }

fn main() -> Word {
    tempo_class(90)
}
```

Run it. The output is `1`. The call `tempo_class(90)` tries the first
head, whose guard `90 < 60` is false, then the second, whose guard
`90 < 120` is true. The second head runs and gives `1`, the class for a
moderate tempo. The final head has no guard and catches everything that
reached it.

## Multiheaded functions or match

A multiheaded function and a `match` express related ideas. A `match`
chooses inside one function body. A multiheaded function chooses which
body to enter at all. Use a multiheaded function when the cases are
substantial enough to deserve separate definitions, and a `match` when
the choice is a small step within a single computation.

## What you now know

- A function name may have several heads, each handling one case.
- A head may match a literal argument or bind it with a name.
- A head may carry a `when` guard.
- Heads are tried in source order, and the first that fits is used, so
  specific cases come before general ones.

That completes Part III. You can now shape data as structs, enums,
tuples, and arrays, take it apart with `match`, and dispatch a function
across several heads. Part IV turns to the heart of the language: the
three kinds of function and the way a program talks to its host.
