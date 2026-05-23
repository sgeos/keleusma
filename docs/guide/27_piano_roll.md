# Chapter 27. The Piano Roll: How It Works

> Part VIII, The Capstone: Making Music. Chapter 27 of 40.
> Previous: [Chapter 26, Signed Modules and Hot Code Swap](./26_signed_modules_and_hot_swap.md).
> Next: [Chapter 28, Setting Up Your Own Song Playground](./28_song_playground.md).

## Goal

By the end of this chapter you will understand how the piano roll example
works: how a Keleusma `loop` program becomes music.

## A real loop program

Part IV introduced the `loop` function, a program that never finishes and
yields to a host on every cycle, and said a real one would be run in Part
VIII. The piano roll is that real program. A song is a `loop main`, and
the piano roll example is the Rust host that drives it.

## Three pieces

The piano roll has three pieces, working at three different speeds.

- The song is a Keleusma `loop main(input: Word) -> Word`. Each cycle of
  the loop is one sixteenth-note tick. On each tick the song decides
  which notes start or stop.
- The host's main thread drives the song. It resumes the song once per
  tick. At 120 beats per minute, a tick is 125 milliseconds.
- The host's audio thread produces the sound. It runs far faster, at
  forty-eight thousand samples per second, and it never enters the
  Keleusma virtual machine.

## The song schedules, the host synthesizes

This is the key idea. The song does not make sound. It schedules events.
When the song decides that channel 0 should sound MIDI note 60, it calls
a host native:

````
host::play(0, 60)
````

That call writes into the host's voice state. The audio thread reads that
state and turns it into sound. The song's job is the timing and the note
choices. The host's job is the synthesis. The native function calls,
introduced in Chapter 16's idea of talking to the host, are the bridge.

## The song's memory

A song remembers where it is using the data segment from Chapter 18.
Every bundled song declares a `data state` block with the same shape: a
one-shot setup flag, a loop counter, a section pointer, a few
general-purpose slots, and two arrays of eight counters, one tracking
each channel's position in its note pattern and one tracking the ticks
left before that channel's next note.

## The init block

The first thing a song's `loop main` body does, on its very first cycle,
is a one-shot setup block guarded by the `state.init` flag:

````
if state.init == 0 {
    host::song_name("C major progression, four-bar loop");
    host::set_waveform(0, 0);
    host::set_adsr(0, 5, 80, 700, 150);
    host::set_enable(0, 1);
    // ... configure the other channels ...
    state.init = 1;
};
````

The data segment starts zeroed, so `state.init` is `0` on the first
cycle, the block runs, and it sets `state.init` to `1`. On every cycle
after that, the block is skipped. This is where each channel's instrument
is chosen.

## The per-tick body

After the init block, the song handles each channel the same way. For a
channel, if its remaining-ticks counter has reached zero, the song looks
up the channel's next note, calls `host::play` or `host::silence`, sets
the counter to the new note's duration, and advances the channel's
position. Otherwise it simply counts the counter down by one. Then the
song yields, and the cycle ends.

Every tick does a fixed, small amount of work. The bounded-step guarantee
from Chapter 20 holds throughout: the song cannot overrun its tick.

## Swapping songs

The piano roll holds ten songs. Pressing a key swaps the running song for
the next one. That swap is the hot code swap of Chapter 26, happening at
a RESET boundary, made audible.

## What you now know

- A song is a `loop main`, and the piano roll is its host.
- The song schedules note events with host native calls; the host's
  audio thread synthesizes the sound.
- A song keeps its position in the data segment.
- A one-shot init block configures the instruments on the first cycle.
- Each tick does a bounded amount of work.

The next chapter builds the piano roll on your own machine.
