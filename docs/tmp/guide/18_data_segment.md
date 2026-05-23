# Chapter 18. The Data Segment

> Part IV, The Heart of Keleusma. Chapter 18 of 40.
> Previous: [Chapter 17, The loop Function](./17_loop_function.md).
> Next: [Chapter 19, Why Was My Program Rejected?](./19_why_rejected.md).

## Goal

By the end of this chapter you will be able to keep state that survives
from one cycle of a `loop` function to the next.

## The problem of memory

A `loop` function runs the same body, cycle after cycle. Suppose it needs
to remember something: which beat of the bar it is on, which note comes
next. Chapter 5 established that a binding cannot change, and a binding
made inside the body is gone by the time the next cycle begins. As the
language stands so far, a `loop` function cannot remember anything.

## The data segment

The answer is the data segment. It is the one region of a program's
memory that may be changed, and whose values survive from one cycle to
the next. It is declared with the word `data`:

````
data state {
    steps: [Word; 4],
}

loop main(input: Word) -> Word {
    for i in 0..4 {
        state.steps[i] = state.steps[i] + 1;
    }
    let _ = yield state.steps[0];
    0
}
````

A `data` block looks like a struct, and its fields are read with a dot,
as `state.steps`. The difference is that its fields may be assigned, with
`=`, and what is assigned is still there on the next cycle.

The data segment begins with every field zeroed. The program above has
four counters. On the first cycle each becomes 1. On the second cycle,
having survived RESET, each becomes 2. The counters climb, cycle after
cycle, because the data segment remembers.

## This is where loops do real work

Look again at the `for` loop above, and recall Chapter 8. There, a `for`
loop inside an atomic `fn` could not build a result, because bindings do
not change. Here the same `for` loop does real work. Each pass writes to
`state.steps`, and the data segment does change. This is the place the
loop earns its keep, exactly as Chapter 8 promised.

## Three kinds of data block

A data block may be marked with its visibility.

- A bare `data` block, as above, is shared. The host may read and write
  it too. It is how the host and the program pass state between them.
- A `private data` block is the program's own memory. It persists across
  cycles, but the host does not see it.
- A `const data` block is read-only configuration, fixed for the life of
  the program and never assigned.

Shared is the common case and the one to start with.

## Checking the program

Save the program as `counters.kel` and check it:

````
keleusma compile counters.kel -o counters.bin
````

The tool prints a line such as `wrote counters.bin (532 bytes)`. As in
the previous two chapters, running it for real needs a host, and Part VIII
runs one.

## What you now know

- The data segment is the one region of memory that may change and that
  survives from one cycle to the next.
- `data name { field: Type, ... }` declares it; `name.field` reads a
  field; `name.field = value;` writes one.
- A `for` loop inside a `loop` function does real work by writing to the
  data segment.
- A data block is `shared` by default, or `private`, or `const`.

That completes Part IV, the heart of the language. You have seen the
three kinds of function, the `yield` exchange with the host, the `loop`
function and its cycle, and the data segment that gives it memory. Part V
explains the checks a program must pass before it is allowed to run at
all.
