# Chapter 30. A Tour of the Song Roster

> Part VIII, The Capstone: Making Music. Chapter 30 of 40.
> Previous: [Chapter 29, Writing and Modifying a Song](./29_writing_a_song.md).
> Next: [Chapter 31, Embedding Keleusma: Orientation](./31_embedding_orientation.md).

## Goal

By the end of this chapter you will have heard the full range of what a
Keleusma song can do, and understood why that range is so wide.

## A song is a program

The reason the roster is worth a tour is a single fact established across
the whole guide. A song is not a list of notes. A song is a program. It
can compute, branch, count, and change its own behaviour over time,
because it is built from functions, `match`, the data segment, and a
`loop`. Anything a bounded program can do, a song can do. The ten bundled
songs are chosen to show that range.

Run the piano roll from Chapter 28 and press `s`, or a number key, to
move through them.

## The roster

- **Songs 0 and 1** are the introductory pieces. Three channels, a
  four-bar chord progression. They are the ones to read first, and the
  ones Chapter 29 modified.
- **Song 2** is an arrangement of a Bach prelude across five channels. It
  shows the host's envelope and retrigger handling on real repertoire.
- **Song 3** is an eight-channel piece in D minor that exercises every
  host native, shifts into a seven-beat meter partway through, and ramps
  its tempo continuously.
- **Song 4** runs its tempo under a slow sine wave, between 60 and 300
  beats per minute, and varies its material every time its loop counter
  advances, in the manner of an algorithmic variation form.
- **Song 5** is a process piece. Eight voices play the same twelve-note
  pattern, but each advances at a different rate, so the voices drift
  into and out of alignment over minutes and hours.
- **Song 6** is a canon whose four voices share one melody but move at
  four different speeds at once, producing genuine four-voice
  counterpoint.
- **Song 7** is a microtonal drone. Its eight voices are tuned to the
  natural harmonic series, using fine pitch offsets the host supports
  directly.
- **Song 8** is a conventional pop song, with a bridge and a key change.
  It is here to show that the same engine that runs the experimental
  pieces also handles ordinary songwriting.
- **Song 9** is a long experimental loop that cycles through sixteen
  variations of scale and instrument, running about fifty minutes before
  it repeats.

## Why this matters

Several of these songs do things a conventional music program, a digital
audio workstation, would struggle with. A tempo shaped by a continuous
sine wave, four meters running at once, a tuning drawn from the harmonic
series, a piece that restructures itself on a counter: these are not
points on a menu. They are consequences of the song being a program.

That is the demonstration. The experimental songs are not there because
they are pleasant. They are there because they could not easily exist any
other way, and the fact that they run, within the same proved time and
memory bounds as song 0, is the language showing what it is for.

## What you now know

- Every song in the roster is a program, which is why the roster ranges
  so widely.
- The bundled songs span introductory progressions, a Bach arrangement,
  several experimental process and tuning pieces, and a conventional pop
  song.
- Techniques that are awkward or impossible in a conventional music
  program follow naturally when the song is a program.

That completes Part VIII, and with it the part of the guide written for
someone learning the language. Part IX is a separate track, for a
developer who wants to host Keleusma inside a Rust program of their own.
