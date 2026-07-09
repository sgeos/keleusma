# Chapter 7. Making Decisions

> Part II, The Building Blocks. Chapter 7 of 40.
> Previous: [Chapter 6, Functions](./06_functions.md).
> Next: [Chapter 8, Bounded Repetition](./08_bounded_repetition.md).

## Goal

By the end of this chapter you will be able to write a program that
chooses between possibilities.

## Asking questions: comparison

A decision starts with a question, and a question in a program is a
comparison. A comparison comes out as a `bool`, either `true` or `false`.
Open the prompt with `keleusma repl` and try some:

````
> 3 < 5
true
> 7 == 7
true
````

The comparisons are `<` less than, `>` greater than, `<=` less than or
equal, `>=` greater than or equal, `==` equal, and `!=` not equal. Note
that asking whether two values are equal uses a doubled `==`, because a
single `=` is already used to bind a name.

## Combining questions: and, or, not

Questions combine. Keleusma writes the combining words as words, not as
symbols. This is worth fixing in memory now, because many other languages
use symbols here and the habit carries over wrongly.

- `and` is true when both sides are true.
- `or` is true when at least one side is true.
- `xor` is true when the two sides differ.
- `not` flips true and false.

````
> (3 < 5) and (7 == 7)
true
> not (3 < 5)
false
````

There is no `&&` and no `||` in Keleusma. The words are `and`, `or`,
`xor`, and `not`.

These four evaluate both sides. Two more words, `andalso` and `orelse`,
are their short-circuit forms. `andalso` produces `false` without looking
at its right side once the left side is `false`, and `orelse` produces
`true` without looking at its right side once the left side is `true`.
Reach for them when the right side is only meaningful after the left has
been checked, and for the everyday case reach for `and` and `or`.

## Working with bits

The words above act on a whole `bool`. Their bit-level cousins act on
every bit of a `Word` or a `Byte` at once. `band`, `bor`, and `bxor`
combine two values bit by bit, and `bnot` flips every bit of one value:

````
> 12 band 10
8
> 12 bor 10
14
> 12 bxor 10
6
````

Four shifts move the bits of a value left or right by a count, named by
their assembly mnemonics. `lsl` and `asl` shift left. `lsr` shifts right
filling with zeros, the unsigned form; `asr` shifts right copying the
sign bit, the signed form. The count may be a constant or a runtime
value.

````
> 1 lsl 4
16
> 48 lsr 2
12
````

These operators work on `Word` and `Byte` here, and on the multi-word
`Multiword<N, F>` type of [Chapter 23](./23_big_numbers.md), which
carries the same names.

## Choosing a value: if and else

An `if` expression chooses between two values based on a question:

````
fn louder_of(a: Word, b: Word) -> Word {
    if a > b { a } else { b }
}

fn main() -> Word {
    louder_of(80, 100)
}
````

Run it. The output is `100`. The function compares two note velocities,
two measures of loudness, and gives back the larger one. If `a > b` is
true, the `if` produces `a`; otherwise it produces `b`. The whole `if`
is one value, and that value is what `louder_of` returns.

## Choosing among many: match

When there are more than two possibilities, `match` compares one value
against a list of cases:

````
fn third_quality(semitones: Word) -> Word {
    match semitones {
        3 => 1,
        4 => 2,
        _ => 0,
    }
}

fn main() -> Word {
    third_quality(4)
}
````

Run it. The output is `2`. The interval that defines a triad's quality is
its third. A third of three semitones is a minor third, written here as
`1`. A third of four semitones is a major third, written `2`. The
underscore `_` is the catch-all case, matching anything not listed above
it, and it produces `0`.

Every `match` must cover every possibility. The `_` case guarantees that.
Chapter 13 returns to `match` in depth.

## Checking an assumption: assert

A `bool` question can also guard an assumption during development. The
`assert` statement checks a condition and, when it is false, stops the
program with an assertion failure:

````
assert count > 0;
assert index < length, "index past the end of the buffer";
````

`assert` is a debug aid, and it follows a deliberate rule. The check is
present only in a debug build, produced with `keleusma compile --debug`.
An ordinary build compiles the assertion away entirely, so it costs
nothing in a shipped program. A debug build and an ordinary build are
therefore separate compilations rather than one artefact, and the
optional message is recorded as removable debug information that
`keleusma strip` can take out. Use `assert` to state what you believe is
true while developing; rely on the type system and the partial-operation
constructs of [Chapter 23](./23_big_numbers.md) for checks that must
hold in a shipped program.

`assert` is not a reserved word. Written before an expression it is the
assertion statement; written as `assert(...)` it is an ordinary call to
a function you happen to have named `assert`.

## What you now know

- Comparisons (`<`, `>`, `<=`, `>=`, `==`, `!=`) produce a `bool`.
- Questions combine with the words `and`, `or`, `xor`, and `not`, never
  with symbols; `andalso` and `orelse` are the short-circuit forms.
- Bit-level operators act on every bit of a `Word` or `Byte`: `band`,
  `bor`, `bxor`, `bnot`, and the shifts `lsl`, `asl`, `lsr`, and `asr`.
- `if condition { ... } else { ... }` chooses between two values.
- `match` chooses among many cases, and `_` is the catch-all.
- `assert condition` checks a development-time assumption; it is present
  only in a `--debug` build and compiles away otherwise.

The next chapter repeats an action a fixed number of times.
