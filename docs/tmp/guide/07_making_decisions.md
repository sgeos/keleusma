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
- `not` flips true and false.

````
> (3 < 5) and (7 == 7)
true
> not (3 < 5)
false
````

There is no `&&` and no `||` in Keleusma. The words are `and`, `or`, and
`not`.

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

## What you now know

- Comparisons (`<`, `>`, `<=`, `>=`, `==`, `!=`) produce a `bool`.
- Questions combine with the words `and`, `or`, and `not`, never with
  symbols.
- `if condition { ... } else { ... }` chooses between two values.
- `match` chooses among many cases, and `_` is the catch-all.

The next chapter repeats an action a fixed number of times.
