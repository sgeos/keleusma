# Chapter 16. Yield: Talking to the Host

> Part IV, The Heart of Keleusma. Chapter 16 of 40.
> Previous: [Chapter 15, The Three Function Categories](./15_three_function_categories.md).
> Next: [Chapter 17, The loop Function](./17_loop_function.md).

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

````
let reply = yield question;
````

Read it as: hand `question` to the host, pause, and when the host
resumes, let `reply` be the value it hands back.

## A program that uses yield

````
yield main(input: Word) -> Word {
    let reply = yield input;
    reply
}
````

This program is started with a value, `input`. It yields `input` to the
host and pauses. The host resumes it with some value, which becomes
`reply`. The program then returns `reply` and finishes.

## The dialogue

Two types are in play at every `yield`. There is the type of the value
handed out, and the type of the value handed back. Together they form the
program's dialogue with the host, the agreed shape of the conversation.
In the program above both are `Word`: the program yields a `Word` and is
resumed with a `Word`.

## Checking a yielding program

Save the program above as `echo.kel`. It cannot be run with `keleusma
run`, because `keleusma run` does not play the host's part, and there is
no one to resume the program after it yields. What you can do is check
that the program is valid:

````
keleusma compile echo.kel -o echo.bin
````

The tool prints a line such as:

````
wrote echo.bin (204 bytes)
````

That line means the program lexed, parsed, type-checked, and passed the
structural verifier. It is a correct Keleusma program. Running a yielding
program for real needs a host, and Part VIII runs one, a song, inside the
piano roll.

## What you now know

- A Keleusma program runs in a conversation with its host.
- `yield value` hands `value` to the host and pauses the program.
- When the host resumes, the value it hands back is the result of the
  `yield` expression.
- The pair of types, yielded out and resumed in, is the dialogue.
- `keleusma compile` checks that a yielding program is valid, since
  `keleusma run` cannot drive it.

The next chapter turns to the function that never finishes: `loop`.
