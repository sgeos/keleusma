# Chapter 29. Writing and Modifying a Song

## Goal

By the end of this chapter you will have changed a song and heard the
change.

## A song is everything you have learned

Open `examples/scripts/piano_roll/piano_roll_0.kel` in a text editor. It
is the simplest song in the roster, and every part of it is something
this guide has already covered.

- It begins with `use` lines, importing the host natives from Chapter 16.
- It declares an `enum Pitch`, from Chapter 11, listing the twelve pitch
  classes and a `Rest`.
- It has ordinary `fn` helpers, from Chapter 6, that look up notes. Each
  uses a `match`, from Chapter 13.
- It declares a `data state` block, from Chapter 18.
- Its entry point is a `loop main`, from Chapter 17, with the init block
  and per-tick body that Chapter 27 described.

A song is not a special kind of file. It is a Keleusma program, built
from the pieces of Parts I through VII.

## The note tables

The notes a channel plays are listed in the helper function
`channel_note`. For channel 0, the melody, it holds a `match` on the note
position. Its first arm is the first melody note:

```
0 => (Pitch::C,  5, 4),  // C major: C E G E
```

The tuple means pitch C, octave 5, duration 4 sixteenths, a quarter note.
This is the note the melody opens on.

## Make a change

Change that first note. Edit the arm to open the melody on E instead of
C:

```
0 => (Pitch::E,  5, 4),  // C major: C E G E
```

Before running, check that the song still compiles:

```
keleusma compile examples/scripts/piano_roll/piano_roll_0.kel -o /tmp/song.bin
```

The tool prints a `wrote ... bytes` line. The change is valid Keleusma,
because `Pitch::E` is a real variant of the `Pitch` enum and the tuple
shape is unchanged. Had the edit broken the program, this step would have
reported the error before any sound was attempted.

## Hear it

Run the piano roll again:

```
cargo run --release --example piano_roll --features sdl3-example
```

Song 0 now opens its melody on E. The host reads the song file fresh when
it builds, so editing the file and rerunning is the whole loop. This is
the loop of composition: change the score, hear the result, change it
again.

## Going further

The same `channel_note` function holds the bass line, in channel 1, and
the harmony, in channel 2. Every note of song 0 is an arm in one of those
`match` blocks. Change pitches, change octaves, change durations. Change
the `host::set_waveform` calls in the init block to give a channel a
different instrument. Each change is checked by `keleusma compile` and
then heard by running the example.

## What you now know

- A song is an ordinary Keleusma program, assembled from the features of
  the whole guide.
- A song's notes are tuples in the `match` arms of its note-table
  functions.
- The edit loop is: change the `.kel` file, check it with
  `keleusma compile`, run the piano roll, listen.

The next chapter tours the full roster of ten songs.
