# Chapter 20. Time and Memory Budgets

> Part V, The Verifier and the Guarantees. Chapter 20 of 40.
> Previous: [Chapter 19, Why Was My Program Rejected?](./19_why_rejected.md).
> Next: [Chapter 21, Generics and Traits](./21_generics_and_traits.md).

## Goal

By the end of this chapter you will understand the two budgets every
Keleusma program is proved to fit within, and the promises that rest on
them.

## Two budgets per tick

Chapter 19 showed the verifier rejecting programs. This chapter explains
what it is protecting. The verifier holds every program to two budgets,
and it checks both before the program is allowed to run.

## The time budget

The first budget is time. Before a program runs, the language works out
the largest amount of work any single tick could possibly do, across
every path the program might take, and proves that amount is finite. This
is the worst-case execution time.

The musical reading is direct. A beat at a given tempo has only so much
room in it. A player cannot sound an unlimited number of notes inside one
beat. The verifier proves that the program's busiest tick, the one that
does the most work, still fits inside its beat.

The budget is measured in a unit called pipelined cycles. A pipelined
cycle is a measure of work, a count of small machine steps, not a count
of seconds. The language proves a bound in that measure. Turning the
measure into real seconds depends on the machine the program runs on, and
that conversion is the host's concern. What the language guarantees is
that the amount of work per tick is bounded.

## The memory budget

The second budget is memory. The language works out the largest amount of
working memory any tick could need, and proves that amount is finite too.
This is the worst-case memory usage. The host then sets aside exactly
that much memory, in a fixed region called the arena.

The arena is a music stand of a fixed size. The verifier proves that the
program never needs more paper on the stand than the stand can hold. A
program whose memory need cannot be proved finite, or whose proved need
is larger than the arena provided, does not run.

## The promises

The two budgets, together with the function categories of Chapter 15,
support a set of promises the language makes about every program it
accepts.

- Totality: every `fn` function finishes.
- Productivity: every `loop` function produces a value on every cycle.
- Bounded time: every tick fits the time budget.
- Bounded memory: every tick fits the memory budget.
- Safe swapping: a program's code can be replaced with new code at a
  RESET boundary without breaking its conversation with the host. Chapter
  26 returns to this.

These are not hopes. They are proved, before the program runs, for every
program the verifier accepts.

## Acceptance is a proof

This program is accepted:

````
fn main() -> Word {
    for _b in 0..8 {
        let _beat = 1;
    }
    0
}
````

Run it with `keleusma run` and it returns `0`. Nothing dramatic appears.
But before that `0` was produced, the verifier proved that the loop runs
exactly eight times, and therefore that the program's time and memory are
both bounded. Acceptance is quiet, but acceptance is a proof. Every
program that runs has passed it.

## The conservative stance

The verifier rejects any program it cannot prove bounded, even a program
that would in fact have behaved perfectly well. It would rather turn away
a safe program than admit an unsafe one. This is why Chapter 19's
recursive count-down was rejected even though it would have stopped at
zero. The verifier did not know that in advance, and "in advance" is the
whole point.

This is the trade Keleusma makes. The language accepts a smaller set of
programs than other languages do, and in exchange it can promise things
about every program in that set that other languages cannot promise about
any program at all. For an audio engine that must never stutter, or a
controller that must always answer in time, that trade is the reason to
reach for Keleusma.

## What you now know

- Every program is held to a time budget and a memory budget, both
  checked before it runs.
- The time budget is worst-case execution time, measured in pipelined
  cycles.
- The memory budget is worst-case memory usage, held within the arena.
- The language promises totality, productivity, bounded time, bounded
  memory, and safe swapping for every program it accepts.
- The verifier rejects whatever it cannot prove, by design.

That completes Part V. You now understand both what the language
guarantees and what it asks of a program in return. Part VI returns to
the language itself, with several features that build on everything so
far.
