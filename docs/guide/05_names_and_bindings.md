# Chapter 5. Names and Bindings

> Part II, The Building Blocks. Chapter 5 of 40.
> Previous: [Chapter 4, Values and Types](./04_values_and_types.md).
> Next: [Chapter 6, Functions](./06_functions.md).

## Goal

By the end of this chapter you will be able to give a value a name, and
you will understand an important rule about those names.

## Naming a value

A composer who writes a motif gives it a name, so that the rest of the
score can refer back to it without writing the notes out again. A program
does the same with a value. Giving a value a name is called binding it,
and the name is called a binding.

A binding is made with the word `let`:

````
fn main() -> Word {
    let beats_per_bar = 4;
    let bars = 8;
    beats_per_bar * bars
}
````

Save that as `phrase.kel` and run it with `keleusma run phrase.kel`. The
output is:

````
32
````

The program names two values, `beats_per_bar` and `bars`, and then uses
both names in the final line. A piece of eight bars in four-four time has
thirty-two beats.

## Stating the type

The language works out the type of a binding on its own. `4` is a whole
number, so `beats_per_bar` is a `Word`. The type may also be stated
plainly, after a colon:

````
let beats_per_bar: Word = 4;
````

Stating the type is optional. It is useful when the value is
complicated, or when writing the type down makes the program clearer to a
reader.

## Bindings do not change

Here is the important rule. Once a value has a name, that name keeps that
value. A binding cannot be reassigned. Writing `let total = 32;` and then
later trying to make `total` equal something else is not allowed.

This may sound limiting, and in one specific way it is. A binding cannot
serve as a running total that a loop adds to, because adding to it would
mean changing it. Chapter 8 returns to this point, and Part IV shows
where changing state is actually done.

The benefit is large. When a name is read further down the program, it
still holds exactly the value it was given. Nothing reassigned it in
between. A reader, and the language, can trust the name. This is the same
discipline as a written score, where a motif marked in the margin means
the same thing every time the score points back to it.

## What you now know

- `let name = value;` binds a value to a name.
- The type can be stated as `let name: Type = value;`, but the language
  can also work it out.
- A binding cannot be reassigned. A name keeps its value.

The next chapter groups statements into functions.
