# Chapter 28. Setting Up Your Own Song Playground

## Goal

By the end of this chapter you will have the piano roll built and running
on your own machine.

## You already have the code

Chapter 2 installed the Keleusma command-line tool from a clone of the
Keleusma source repository. That same clone contains the piano roll
example and all ten songs. There is nothing new to download. The work of
this chapter happens inside that repository folder.

## One extra requirement: CMake

The piano roll produces sound, and for that it uses an audio library
called Simple DirectMedia Layer 3, or SDL3. On its first build, SDL3 is
compiled from source, and compiling it needs a tool called CMake.

Install CMake for your operating system before continuing. This is the
one real setup cost in the whole guide. It is heaviest on Windows, where
neither CMake nor a C build toolchain is present by default. On macOS and
Linux a C toolchain is usually already installed, and only CMake needs
adding.

## Building and running

From inside the Keleusma repository folder, run:

```
cargo run --release --example piano_roll --features sdl3-example
```

Read the command in parts. `cargo run` builds and runs. `--release`
builds the fast version, which audio needs. `--example piano_roll`
selects the piano roll. `--features sdl3-example` switches on the SDL3
audio support.

The first time, this takes a few minutes, because SDL3 is being built
from source. That happens once. Every later run reuses the built SDL3 and
starts quickly.

## What you will see and hear

When it starts, the piano roll prints its commands, begins playing the
first song, and listens for single-key commands typed into the terminal:

- `s` swaps to the next song.
- `r` restarts the current song.
- `p` pauses, and `p` again resumes.
- A number key jumps straight to that song.
- Pressing Enter alone quits.

Sound should be coming from your speakers. If the build succeeded but you
hear nothing, check that the terminal program is allowed to use the
audio device, and that the system volume is up.

## The songs are right here

The ten songs live in the repository at
`examples/scripts/piano_roll/`, named `piano_roll_0.kel` through
`piano_roll_9.kel`. They are ordinary Keleusma source files. The next
chapter opens one and changes it.

## What you now know

- The piano roll example is part of the repository clone from Chapter 2.
- Building it needs CMake, because SDL3 is compiled from source on the
  first build.
- `cargo run --release --example piano_roll --features sdl3-example`
  builds and runs it.
- The songs are `.kel` files in `examples/scripts/piano_roll/`.

The next chapter changes a song and hears the difference.
