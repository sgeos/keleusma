# Chapter 2. Installing Keleusma and the Interactive Prompt

> Part I, Setting Out. Chapter 2 of 40.
> Previous: [Chapter 1, What Keleusma Is, and What It Is Not](./01_what_keleusma_is.md).
> Next: [Chapter 3, A Complete First Program](./03_first_complete_program.md).

## Goal

By the end of this chapter you will have the Keleusma tool installed, you
will have run code in the interactive prompt, and you will have saved your
first program to a file.

## What you need first

Keleusma is built with the Rust toolchain, so the toolchain must be
present before Keleusma can be installed. The standard installer for the
Rust toolchain is `rustup`, available from the official Rust website.
Install it for your operating system, then confirm it works:

````
cargo --version
````

If that command prints a version number, the toolchain is ready.

## Installing the Keleusma command-line tool

Keleusma ships a command-line tool, also named `keleusma`. Install it from
a copy of the Keleusma source repository:

````
git clone https://github.com/sgeos/keleusma
cd keleusma
cargo install --path keleusma-cli --bin keleusma
````

Confirm the installation:

````
keleusma --help
````

If the shell reports that the command is not found, the directory for
installed Rust programs is not on the search path. That directory is
named `.cargo/bin` inside your home folder. Add it to the path and try
again.

## The interactive prompt

The fastest way to try the language is the interactive prompt, called the
REPL. Start it:

````
keleusma repl
````

The prompt is a `> `. Type an expression, press Enter, and the answer
appears on the next line. There is no file to create and no program to
structure. Try some arithmetic:

````
> 7 + 5
12
> 12 * 2
24
````

A piano octave has seven white keys and five black keys, twelve in all,
and two octaves span twenty-four semitones. Numbers with a fractional
part work too:

````
> 261.6
261.6
````

That happens to be close to the frequency of middle C, in hertz, a number
Chapter 3 returns to.

The prompt can also remember a function for the rest of the session.
Define one, then call it:

````
> fn semitones_in(octaves: Word) -> Word { octaves * 12 }
defined: semitones_in
> semitones_in(3)
36
````

The word `fn` begins a function, `semitones_in` is its name, `octaves` is
the input it expects, and `octaves * 12` is what it computes. Functions
have their own chapter later. For now it is enough to see that the prompt
accepted the definition and then used it.

Type `:help` to list the prompt commands, and `:quit` to leave:

````
> :quit
````

The interactive prompt is ideal for trying a small idea quickly. It has
one limit: it forgets everything when you quit. To keep a program, save
it in a file.

## Saving a program in a file

Create a file named `octave.kel` in any folder. The `.kel` ending marks
it as Keleusma source. Put one line in it:

````
fn main() -> Word { 7 + 5 }
````

A program saved in a file must be written as a function named `main`,
because a program starts at `main`. The `-> Word` states that the
function gives back a whole number. The interactive prompt wrapped your
expressions in a `main` for you. A file is explicit about it.

Run the file:

````
keleusma run octave.kel
````

The output is:

````
12
````

A Keleusma program does not print text on its own. The tool prints the
single value that `main` hands back. As a shorthand, the tool also
accepts the file without the word `run`, as `keleusma octave.kel`.

## An optional step for macOS and Linux

On macOS and Linux a file can be made to run on its own, like any other
command. Add one line to the very top of `octave.kel`, so the file reads:

````
#!/usr/bin/env keleusma
fn main() -> Word { 7 + 5 }
````

That first line is called a shebang. Mark the file as runnable and run it
directly:

````
chmod +x octave.kel
./octave.kel
````

The output is again `12`. The file now behaves like a small program of
its own.

This step is specific to macOS and Linux. On Windows there is no shebang
mechanism, and the file is run with `keleusma run octave.kel`, which works
on every operating system. The shebang line is harmless on Windows, so a
file that carries it stays usable everywhere.

## What you now know

- The Keleusma tool is installed and runs from the command line.
- The interactive prompt evaluates expressions immediately and can
  remember functions for a session.
- A program saved in a file is a function named `main`, run with
  `keleusma run <file>`.
- A Keleusma program produces output by returning a value.
- On macOS and Linux a shebang line plus `chmod +x` makes a script run on
  its own.

The next chapter writes a complete program with several functions, and it
computes something a musician will recognize.
