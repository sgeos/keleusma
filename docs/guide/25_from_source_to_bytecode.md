# Chapter 25. From Source to Bytecode

> Part VII, Shipping a Program. Chapter 25 of 40.
> Previous: [Chapter 24, Information-Flow Labels](./24_information_flow_labels.md).
> Next: [Chapter 26, Signed Modules and Hot Code Swap](./26_signed_modules_and_hot_swap.md).

## Goal

By the end of this chapter you will understand what `keleusma run` has
been doing all along, and you will be able to compile a program into a
file that runs directly.

## Source and bytecode

A `.kel` file holds source: the text a person writes and reads. The
virtual machine does not run source. It runs bytecode, a compact form
produced from the source by the compiler. Every time `keleusma run` has
been used in this guide, it has quietly done four steps in a row: read
the source, compile it to bytecode, verify the bytecode, and run it.

Those steps can be separated. The compiling can be done once, ahead of
time, and the result saved.

## Compiling ahead of time

Write a small program and save it as `tune.kel`:

````
fn main() -> Word { 60 + 7 }
````

Compile it:

````
keleusma compile tune.kel -o tune.kel.bin
````

The tool prints:

````
wrote tune.kel.bin (2400 bytes)
````

`tune.kel.bin` is the compiled bytecode. Run it directly:

````
keleusma run tune.kel.bin
````

The output is `67`. The tool recognized the file as bytecode and ran it
without compiling, because the compiling was already done.

## What is in a bytecode file

A bytecode file is a self-describing package. It begins with a short
marker so the runtime can recognize it as Keleusma bytecode. After the
marker comes a header carrying the program's facts, then the program
body, and at the end a checksum. The runtime reads the header before
anything else and refuses a file that is not genuine Keleusma bytecode or
that is built for an incompatible machine. The checksum lets it detect a
file that was damaged in storage or transit.

A beginner does not need the byte-by-byte layout. The point is that a
bytecode file is checked, by the runtime, before a single instruction of
it runs.

## A compiled file can carry a shebang

Chapter 2 added a shebang line to a source file on macOS and Linux. A
compiled bytecode file can carry one too, so a finished, compiled program
can be made directly runnable in the same way.

## Why compile ahead

Compiling once and shipping the bytecode has two benefits. The machine
that runs the program does not need the compiler, only the runtime. And
the program starts at once, with no compile step first. Bytecode is the
finished, engraved score, ready to hand to a player, as distinct from the
working manuscript that the source is.

## Selecting a target

The default compile targets the same machine running the compiler. To
build a bytecode artefact for a different machine, pass `--target`:

````
keleusma compile tune.kel --target embedded_16 -o tune.kel.bin
````

The recognised target names are `host` (the default), `wasm32`,
`embedded_32`, `embedded_16`, and `embedded_8`. The chosen target
controls word, address, and float widths and validates the program
against the configuration. A program that uses literals or constants
outside the target's representable range is rejected at compile time.

## What you now know

- Source is the text you write; bytecode is the compact form the runtime
  executes.
- `keleusma run` compiles and runs in one step; `keleusma compile`
  produces a bytecode file you can run later.
- A bytecode file is self-describing, version-checked, and protected by a
  checksum.
- Compiling ahead of time means the running machine needs only the
  runtime, and the program starts immediately.

The next chapter covers two more things that happen to a finished
program: it can be signed, and it can be swapped.
