# Chapter 3. A Complete First Program: A Note of the Major Scale

## Goal

By the end of this chapter you will have written and run one complete
program that is larger than a single line. The program computes the
frequency of a note of the major scale. You are not expected to
understand every detail yet. Each idea used here has its own chapter
later. The purpose of this chapter is to see a whole program work, end to
end.

## The idea: a note is a frequency

Every musical note is a frequency, a number of vibrations per second,
measured in a unit called the hertz. The A above middle C vibrates at 440
hertz. That note is the reference the rest of this chapter measures
from.

Two facts connect notes to numbers.

- Going up one octave doubles the frequency.
- An octave is divided into twelve equal steps, called semitones.

Because twelve equal steps must multiply the frequency by two in total,
each single step multiplies the frequency by the same fixed amount, the
twelfth root of two. Going up `n` semitones therefore multiplies the
frequency by two raised to the power `n / 12`.

Musicians and instruments often refer to a note by a whole number called
its MIDI number. A4, the 440 hertz reference, is MIDI number 69. Middle C
is MIDI number 60. The frequency of MIDI number `m` is:

```
frequency = 440 * 2 raised to the power ((m - 69) / 12)
```

## The idea: the major scale is a pattern of steps

A major scale does not use all twelve semitones. Starting from a root
note, it rises by a fixed pattern of semitone counts:

```
0, 2, 4, 5, 7, 9, 11, 12
```

The first note is the root itself, zero semitones up. The last note is
the octave, twelve semitones up. The pattern in between is what gives the
major scale its sound.

## Building the program

The program is built from three functions. Read the whole program first,
then the explanation that follows.

```
use math::pow

fn midi_to_hz(m: Word) -> Float {
    440.0 * math::pow(2.0, ((m - 69) as Float) / 12.0)
}

fn scale_degree_hz(root: Word, degree: Word) -> Float {
    let steps = [0, 2, 4, 5, 7, 9, 11, 12];
    midi_to_hz(root + steps[degree])
}

fn main() -> Float {
    scale_degree_hz(60, 4)
}
```

Consider each part.

- `use math::pow` borrows a function from the host. Raising a number to a
  power is provided by the host's math library, and `use` brings it into
  the program by name. Chapter 6 returns to functions, and Part IX
  explains where host functions come from.
- `fn midi_to_hz(m: Word) -> Float` is the frequency formula from above.
  It takes a MIDI number `m`, a `Word`, and returns a `Float`. A `Float`
  is a number that can have a fractional part, which a frequency needs.
  The expression `(m - 69) as Float` converts the whole number `m - 69`
  into a `Float` so it can be divided by `12.0`. That conversion is
  called a cast. Chapter 4 covers `Word`, `Float`, and casts.
- `fn scale_degree_hz(root: Word, degree: Word) -> Float` puts the scale
  pattern in a list called an array, named `steps`. Writing `steps[degree]`
  reads the entry at position `degree`. The function adds that semitone
  count to the root and asks `midi_to_hz` for the frequency. Chapter 12
  covers arrays.
- `fn main` runs the program. It asks for degree `4` of the major scale
  built on MIDI number `60`, which is middle C.

## Running it

Save the program as `scale.kel` and run it:

```
keleusma run scale.kel
```

The output is:

```
391.99543598174927
```

Position `4` in the array `0, 2, 4, 5, 7, 9, 11, 12` is the value `7`, so
the note is seven semitones above middle C. That note is G4, the fifth
note of the C major scale, and its frequency is just under 392 hertz. The
program computed it from first principles.

## Change it and run it again

The array `steps` has eight entries, numbered 0 through 7. Change the
second argument of `scale_degree_hz` in `main` and run again:

- degree `0` gives the root, middle C itself, near 261.63 hertz,
- degree `7` gives the octave above the root, near 523.25 hertz,
- the degrees in between give the rest of the scale.

Change the first argument to move the whole scale to a different root.
MIDI number 69 puts the scale on A.

The program returns one frequency each time it runs, because the
command-line tool prints the single value that `main` returns, as
Chapter 2 described. Computing a whole scale at once, and hearing it
played, is the work of the piano roll in Part VIII.

## What you now know

This one program already used a great deal of the language:

- functions with parameters and return types,
- the `Word` and `Float` types,
- a cast from one type to another,
- an array and reading an entry from it,
- a function borrowed from the host with `use`.

Every one of these has its own chapter ahead, starting with values and
types in Chapter 4. You have now seen a complete Keleusma program work.
That is the goal of Part I.
