# Chapter 8. Bounded Repetition

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

```
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
```

Run it with `keleusma run`. The output is:

```
8
```

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

```
for beat in 0..16 {
    if beat == 4 {
        break;
    }
}
```

When `beat` reaches 4, `break` ends the loop at once. Note the semicolon
after `break`. Because `break` can stop the loop early, the count is
still bounded: the loop runs at most its full count, and possibly fewer
passes, but never more.

## A cap for a runtime range

Every loop so far has had a count known before the loop begins, either a
literal like `0..8` or the fixed length of an array. A loop whose end
comes from a value computed at run time, like `0..n` where `n` is a
parameter, is rejected, because the tool cannot see how many passes it
will take and so cannot bound its time and memory.

You can supply that bound yourself with a `limit`. The loop then runs
over its real range, but never more times than the cap you name:

```
for i in 0..n limit 64 {
    // runs at most 64 times, and stops early when i reaches n
}
```

The cap must be a constant the tool can read at compile time, so a plain
number, a const-data field, or a const parameter. The worst-case count is
the cap, which is what makes the loop admissible even though `n` is only
known at run time.

If the range is longer than the cap, the loop stops at the cap. By
default that is treated as a mistake and the program stops with a loud
error, rather than quietly leaving work undone. If stopping at the cap is
something you want to handle, add an `on` block that names the outcomes:

```
for i in 0..n limit 64 {
    // body, may break
} on {
    ok(count)   => { /* the range finished */ },
    break(at)   => { /* the body ran break; at is the index */ },
    limit(at)   => { /* the cap was reached first; at is the index */ },
}
```

An `on` block always names an `ok` arm, the same catch-all every other
handling block in the language requires; the `break` and `limit` arms are
optional, and a `break` you do not name runs the `ok` arm. Without an `on`
block at all, reaching the cap is the loud error just described, while
finishing the range or breaking simply ends the loop. The bound is the cap
in every case, so the loop always has a worst-case count the tool can prove.

## What you now know

- `for name in 0..n { ... }` loops over a range of numbers.
- `for name in array { ... }` loops over the entries of an array.
- Every loop count is known before the loop begins.
- Inside an atomic `fn`, a loop cannot accumulate a result, because
  bindings do not change. The loop does real work in Part IV.
- `break;` leaves a loop early.
- `limit` puts a compile-time cap on a runtime range, and an `on` block
  captures how the loop ended (`ok`, `break`, or `limit`).

The next chapter threads a value through a chain of functions.
