# Chapter 16. Yield: Talking to the Host

## Goal

By the end of this chapter you will understand the exchange between a
program and its host, and the `yield` expression that carries it out.

## A program does not run alone

Chapter 1 described a Keleusma program as a score and the host as the
orchestra. The picture is now exact. A program does not simply run from
start to finish on its own. It runs in a conversation with its host, and
`yield` is one turn of that conversation.

## The exchange

`yield` does two things in a single step. It hands a value out to the
host, and it pauses the program. The host then does whatever it does, and
when it is ready it resumes the program, handing a value back. That
returned value is the result of the `yield`.

This is the metronome tick. On the tick, the program hands the host a
value and stops. The host acts. On the next tick, the host hands the
program a value and the program continues.

In a program, the exchange is written as part of a `let`:

```
let reply = yield question;
```

Read it as: hand `question` to the host, pause, and when the host
resumes, let `reply` be the value it hands back.

## A program that uses yield

```
yield main(input: Word) -> Word {
    let reply = yield input;
    reply
}
```

This program is started with a value, `input`. It yields `input` to the
host and pauses. The host resumes it with some value, which becomes
`reply`. The program then returns `reply` and finishes.

## The dialogue

Two types are in play at every `yield`. There is the type of the value
handed out, and the type of the value handed back. Together they form the
program's dialogue with the host, the agreed shape of the conversation.
In the program above both are `Word`: the program yields a `Word` and is
resumed with a `Word`.

## Running a yielding program

Save the program above as `echo.kel` and run it:

```
keleusma run echo.kel
```

The output is:

```
1
```

The command-line tool drives a yielding program through a tick-counter
protocol. It calls the script with `tick = 1`, the script yields `input`
which is `1`, the host resumes with the next tick which is `2`, and the
script returns the resumed value. The tool prints the returned value and
the program ends. A `yield` program may pause and resume many times
before finishing. Part VIII runs a more elaborate one, a song, inside
the piano roll.

The same program can also be compiled to a bytecode file for later
execution:

```
keleusma compile echo.kel -o echo.bin
```

The tool prints a line such as `wrote echo.bin (2316 bytes)`. That line
means the program lexed, parsed, type-checked, and passed the structural
verifier.

## What you now know

- A Keleusma program runs in a conversation with its host.
- `yield value` hands `value` to the host and pauses the program.
- When the host resumes, the value it hands back is the result of the
  `yield` expression.
- The pair of types, yielded out and resumed in, is the dialogue.
- The command-line tool drives `yield main` programs through a
  tick-counter protocol; the program finishes when control returns from
  the entry function.
- `keleusma compile` produces a bytecode file for later execution.

The next chapter turns to the function that never finishes: `loop`.
