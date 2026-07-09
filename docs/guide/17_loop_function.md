# Chapter 17. The loop Function

> Part IV, The Heart of Keleusma. Chapter 17 of 40.
> Previous: [Chapter 16, Yield: Talking to the Host](./16_yield.md).
> Next: [Chapter 18, The Data Segment](./18_data_segment.md).

## Goal

By the end of this chapter you will understand the function that never
finishes, and the rule that keeps it honest.

## A program for a stream

A `yield` function pauses and resumes, but in the end it finishes. A
`loop` function never finishes. It is the right shape for anything that
goes on as long as the host keeps it running: an audio engine, a game, a
control loop. It runs, and runs, and runs.

````
loop main(input: Word) -> Word {
    let _ = yield input;
    0
}
````

## The return to the top

When the last statement of a `loop` body has run, the program does not
stop. Execution returns to the top of the body and runs the whole thing
again. This return to the top is called RESET. RESET is the only point in
a Keleusma program where execution jumps backward.

Each pass through the body is one cycle. The program above yields `input`,
ignores the value it is resumed with by binding it to `_`, reaches the
final `0`, and then RESET carries it back to the top for the next cycle.

## The parameter is refreshed

The parameter `input` is not asked for. At the top of every cycle the host
hands in the value of `input` for that cycle. A game might hand in the
latest controller state. An audio sequencer might hand in the current
tick number. The program reads `input` and responds, every cycle, with
fresh data from the host.

## The productivity rule

A `loop` function must hand a value to the host, with `yield`, on every
cycle. This is not advice. It is enforced. A `loop` whose body could run a
whole cycle without reaching a `yield` is rejected before it ever runs.
Chapter 19 shows that rejection.

The musical reading is exact. A player holding down an ostinato must
sound something every bar. A player who falls silent, with no plan to
return, has broken the groove and stopped the music. The productivity
rule is the language refusing to let that happen.

## One loop per program

A program has at most one `loop` function, and when it has one, that
function is the entry point. The piece has one groove at its center.

## Running a loop program

Save the program above as `pulse.kel` and run it:

````
keleusma run pulse.kel --tick-interval 1s
````

The command-line tool drives the loop through the same tick-counter
protocol as a `yield` program, except that a `loop` never finishes. The
tool calls the script with `tick = 1`, accepts each yielded `Word`,
sleeps until the next tick interval, and resumes the script with the
next tick number. The `--tick-interval` flag accepts humanized durations
such as `100ms`, `1s`, `1m`, `1h`, `1d`, or `1w`. Without the flag the
loop runs as fast as the script yields. To stop a running loop press
Control-C. A loop can also stop itself by calling `shell::exit(code)`.

The same program can be compiled to a bytecode file for later execution:

````
keleusma compile pulse.kel -o pulse.bin
````

The tool prints `wrote pulse.bin (2372 bytes)` confirming the program is
valid. Part VIII runs a more elaborate `loop` program, a song, inside
the piano roll.

## What you now know

- A `loop` function never finishes. Each pass of its body is one cycle.
- RESET is the return to the top of the body at the end of each cycle.
- The parameter is refreshed by the host at the top of every cycle.
- A `loop` must `yield` on every cycle. This productivity rule is
  enforced.
- A program has at most one `loop` function, and it is the entry point.

The loop above does the same thing every cycle, because it remembers
nothing from one cycle to the next. The next chapter gives it a memory.
