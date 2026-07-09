# Chapter 19. Why Was My Program Rejected?

## Goal

By the end of this chapter you will understand what it means for a
program to be rejected, you will have seen it happen, and you will know
how to respond.

## The verifier

Chapter 1 made a promise: before a program runs, the language guarantees
that each tick finishes within bounded time and memory. The part of the
language that keeps that promise is the verifier. Every program passes
through it. A program the verifier cannot prove bounded, it rejects, and
the program does not run.

A rejection is not a malfunction. It is the promise doing its work. A
rejected program is simply one the language was unable to vouch for.

## A worked rejection: recursion

Anyone who has seen a little programming reaches, sooner or later, for a
function that calls itself. It is a natural way to express "do this
again." Here is one that counts down from a number:

```
fn count_down(n: Word) -> Word {
    if n <= 0 { 0 } else { count_down(n - 1) }
}

fn main() -> Word {
    count_down(5)
}
```

Run it with `keleusma run`. There is no result, only an error:

```
error: verify: VerifyError("count_down: recursive call detected during WCMU topological sort")
```

The phrase that matters is recursive call detected. The rest names the
internal check that found it.

## Why recursion is rejected

A function that calls itself could call itself any number of times. The
depth depends on the input. The language cannot see, before the program
runs, how deep the calls will go, so it cannot promise a bound on the
work or the memory. Chapter 1 listed "no recursion" among the things
Keleusma leaves out. This error is that rule being enforced.

## The rewrite

The instinct behind the recursive `count_down` was "repeat five times."
Keleusma expresses a fixed number of repetitions with a `for` loop whose
count is written as a plain constant:

```
fn repeat_five() -> Word {
    for _i in 0..5 {
        let _step = 1;
    }
    0
}

fn main() -> Word {
    repeat_five()
}
```

This runs, and returns `0`. The count, `5`, is written into the program
and visible to the verifier, so the verifier can prove the loop is
bounded. Recall from Chapter 8 that such a loop cannot accumulate a
result inside an `fn`. When a running total is genuinely needed, the data
segment of a `loop` function holds it, as Chapter 18 showed.

## Two more rejections

A `for` loop whose count is not a constant is also rejected:

```
fn process(n: Word) -> Word {
    for i in 0..n {
        let _step = i;
    }
    0
}
```

This produces an error containing `no statically extractable iteration
bound`. The count `n` arrives at runtime, and the verifier cannot see it
in advance. The fix is the same: a constant bound, or iteration over an
array whose length is fixed.

A `loop` function with no `yield` is rejected with `Stream block must
contain at least one Yield`. That is the productivity rule from Chapter
17, enforced by the verifier.

## Two categories of rejection

Rejections fall into two groups.

- Some programs are rejected because no bound exists at all. Recursion is
  one. No future improvement to the language will admit it, because there
  is nothing to prove. The only response is to rewrite the program.
- Some programs are rejected because, although a bound exists, the
  present analysis cannot yet work it out. The loop with a runtime count
  is one. A future, sharper verifier might admit it unchanged.

Either way, the response a beginner needs is the same: rewrite the
program into a form the verifier accepts. The repository document
`WHY_REJECTED.md` lists the rejection messages and their rewrites in
full.

## What you now know

- The verifier checks every program and rejects any it cannot prove
  bounded.
- Recursion is rejected, because its depth cannot be known in advance.
- A loop with a non-constant count is rejected, for the same reason.
- A `loop` with no `yield` is rejected by the productivity rule.
- A rejection is the language keeping its promise, not a failure.

The next chapter explains the promise itself: the budgets a program is
proved to fit within.
