# Chapter 8. Bounded Repetition

> Part II, The Building Blocks. Chapter 8 of 40.
> Previous: [Chapter 7, Making Decisions](./07_making_decisions.md).
> Next: [Chapter 9, The Pipeline Operator](./09_pipeline.md).

## Goal

By the end of this chapter you will be able to write a loop, and you will
understand the one rule that makes Keleusma loops different from loops in
most other languages.

## A loop with a known count

A repeat sign in sheet music says how many bars to repeat. It never says
"repeat for a while, and we shall see." Keleusma loops are the same. Every
loop has a count that is known before the loop begins. This is what the
word bounded means in the chapter title.

A loop is written with `for`. It walks through a range of numbers, or
through the entries of an array:

````
fn main() -> Word {
    let durations = [4, 4, 8, 2];

    for d in durations {
        let _step = d * 2;
    }

    for beat in 0..4 {
        let _tick = beat;
    }

    durations[2]
}
````

Run it with `keleusma run`. The output is:

````
8
````

The first loop walks through the four entries of the array `durations`,
binding each in turn to `d`. The second loop walks through the range
`0..4`, which is the four numbers 0, 1, 2, and 3. Both loop counts are
fixed before the loop starts: an array has a known length, and a range
states its bounds.

## The honest limitation

Look closely at that program. The two loops run, but the result, `8`,
does not depend on them. It is `durations[2]`, the entry at position 2 of
the array, computed without any loop at all.

This is deliberate, and it follows directly from Chapter 5. A binding
cannot be reassigned. A loop therefore cannot keep a running total,
because a running total is a value that changes on every pass. Inside an
atomic function, the kind declared with `fn`, a loop can walk through
values but cannot accumulate a result from them.

So why learn the loop now? Because the loop earns its place in Part IV,
inside a different kind of function called a `loop` function. There, each
pass of the loop can hand a value to the host or update stored state, and
the repetition does real work. This chapter teaches the shape of the loop
so that it is already familiar when it matters.

## Leaving a loop early

A loop can stop before its count is reached, with `break`:

````
for beat in 0..16 {
    if beat == 4 {
        break;
    }
}
````

When `beat` reaches 4, `break` ends the loop at once. Note the semicolon
after `break`. Because `break` can stop the loop early, the count is
still bounded: the loop runs at most its full count, and possibly fewer
passes, but never more.

## What you now know

- `for name in 0..n { ... }` loops over a range of numbers.
- `for name in array { ... }` loops over the entries of an array.
- Every loop count is known before the loop begins.
- Inside an atomic `fn`, a loop cannot accumulate a result, because
  bindings do not change. The loop does real work in Part IV.
- `break;` leaves a loop early.

The next chapter threads a value through a chain of functions.
