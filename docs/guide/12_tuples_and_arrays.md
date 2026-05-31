# Chapter 12. Tuples and Arrays

> Part III, Shaping Data. Chapter 12 of 40.
> Previous: [Chapter 11, Enums](./11_enums.md).
> Next: [Chapter 13, Pattern Matching in Depth](./13_pattern_matching.md).

## Goal

By the end of this chapter you will be able to group values by position,
in two different ways: a tuple and an array.

## A tuple: a fixed group of values

A struct names its parts. Sometimes a small group of values does not need
names, only an order. A pair of values, written in parentheses, is a
tuple:

````
let event = (60, 4);
````

That tuple holds a pitch and a beat count, in that order. The parts of a
tuple are reached by position, starting at zero: `event.0` is `60` and
`event.1` is `4`.

A tuple can also be taken apart into named bindings in one step. This is
called destructuring:

````
let (pitch, beats) = event;
````

After that line, `pitch` is `60` and `beats` is `4`.

## An array: a fixed-length row of one type

An array is a row of values, all of the same type, with a length fixed
when the program is written. It is written in square brackets:

````
let scale = [0, 2, 4, 5, 7, 9, 11, 12];
````

An entry is read by its position, again starting at zero, as
`scale[2]`. The type of that array is written `[Word; 8]`, meaning eight
`Word` values.

The length is part of the type and never changes. An array does not grow
or shrink. This is what makes its memory use known in advance, which
Chapter 1 noted as one of the language's promises.

## A program using both

````
fn main() -> Word {
    let event = (60, 4);
    let (pitch, beats) = event;
    let scale = [0, 2, 4, 5, 7, 9, 11, 12];
    pitch + scale[2] + beats
}
````

Run it with `keleusma run`. The output is:

````
68
````

The tuple `event` is destructured into `pitch`, which is `60`, and
`beats`, which is `4`. The array `scale` holds the major-scale step
pattern from Chapter 3, and `scale[2]` is its third entry, `4`. The sum
is `60 + 4 + 4`, which is `68`.

## Tuple or struct, array or enum

A tuple and a struct both group values that are present together. Reach
for a struct when the parts deserve names, and a tuple when a short,
ordered group is clearer without them.

An array holds many values of one type. An enum holds one value out of a
fixed set of types. They are not alternatives to each other. They answer
different questions.

## What you now know

- A tuple, `(a, b)`, groups values by position, read with `.0`, `.1`,
  and so on, or destructured with `let (a, b) = ...`.
- An array, `[a, b, c]`, is a fixed-length row of one type, read with
  `array[index]`.
- An array's length is fixed and is part of its type.
- An index that points past the end is handled with the indexing
  construct `array[i] { ok(v) => ..., invalid_index(idx) => ... }`. See
  [Chapter 23](./23_big_numbers.md).

The next chapter studies `match`, the tool for choosing on the shape of a
value, in depth.
