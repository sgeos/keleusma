# Chapter 15. The Three Function Categories

> Part IV, The Heart of Keleusma. Chapter 15 of 40.
> Previous: [Chapter 14, Multiheaded Functions and Guards](./14_multiheaded_functions.md).
> Next: [Chapter 16, Yield: Talking to the Host](./16_yield.md).

## Goal

By the end of this chapter you will know the three kinds of function
Keleusma has, and what each kind is allowed to do.

## Three kinds, three words

Every Keleusma function is exactly one of three kinds. The kind is fixed
by the word that begins the declaration: `fn`, `yield`, or `loop`. Every
function in the guide so far has been an `fn`. This chapter introduces all
three, and the chapters after it take the other two in turn.

## fn, a finished calculation

A function declared with `fn` is an atomic total function. Atomic means
it runs in one piece, start to finish, without pausing. Total means it
always finishes. An `fn` function takes its inputs, computes, and returns
a result. It may not pause to talk to the host, it may not run forever,
and it may not call itself.

````
fn perfect_fifth(root: Word) -> Word {
    root + 7
}

fn main() -> Word {
    perfect_fifth(60)
}
````

Run it with `keleusma run`. The output is `67`. An `fn` function is like
working out the notes of a chord: a definite question, a definite answer,
and then it is done.

## yield, a phrase that pauses

A function declared with `yield` is a non-atomic total function.
Non-atomic means it may pause partway through. A `yield` function can hand
a value to the host and pause, then continue when the host resumes it. It
may pause many times, but it must eventually finish.

````
yield main(input: Word) -> Word {
    let reply = yield input;
    reply
}
````

A `yield` function is like a phrase that pauses for the conductor's cue
and then, after however many cues, comes to an end. Chapter 16 is about
the pause itself.

## loop, the piece that never ends

A function declared with `loop` is a productive divergent function.
Divergent means it never finishes. A `loop` function repeats forever. The
word productive is the condition attached: it must hand a value to the
host on every single cycle.

````
loop main(input: Word) -> Word {
    let _ = yield input;
    0
}
````

A `loop` function is the piece itself, an ostinato that grooves on and on
for as long as the host keeps it running. Chapter 17 is about it.

## The rules between them

- A program has at most one `loop` function. If it has one, that `loop`
  function is the program's entry point.
- A `yield` function may be an entry point, or a helper.
- An `fn` function is a pure calculation, used by any of the three.

The kind of a function is a promise written into its first word. An `fn`
will finish. A `loop` will keep producing. The language relies on these
promises to make the guarantees of Chapter 1.

## Running yield and loop programs

The two programs above were shown but not run. A `yield` or `loop`
program talks to a host, and `keleusma run` does not play the part of the
host. Chapters 16 through 18 show how to check such a program with
`keleusma compile`, and Part VIII runs a real `loop` program, a song,
inside the piano roll.

## What you now know

- `fn` is an atomic total function: it runs straight through and
  finishes.
- `yield` is a non-atomic total function: it may pause and resume, and
  must eventually finish.
- `loop` is a productive divergent function: it never finishes and must
  yield on every cycle.
- A program has at most one `loop` function, and it is the entry point.

The next chapter examines the pause itself: `yield`.
