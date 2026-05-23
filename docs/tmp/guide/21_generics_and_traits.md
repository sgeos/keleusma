# Chapter 21. Generics and Traits

> Part VI, Going Deeper. Chapter 21 of 40.
> Previous: [Chapter 20, Time and Memory Budgets](./20_time_and_memory_budgets.md).
> Next: [Chapter 22, Newtypes and Refinement Types](./22_newtypes_and_refinement.md).

## Goal

By the end of this chapter you will be able to attach behavior to a type
with a trait, and write a function that works for many types with a
generic.

## A trait: a named role

In an ensemble, a role such as "the instrument carrying the melody" can
be filled by a flute one night and a violin the next. The role is named;
the instrument that fills it varies. A trait is a named role for a type.

A trait declares behavior. An `impl` block provides that behavior for one
particular type:

````
trait Transpose {
    fn up_octave(x: Word) -> Word;
}

impl Transpose for Word {
    fn up_octave(x: Word) -> Word {
        x + 12
    }
}

fn main() -> Word {
    let n: Word = 60;
    n.up_octave()
}
````

Run it with `keleusma run`. The output is `72`.

The `trait Transpose` declares that a type filling this role has an
`up_octave` behavior. The `impl Transpose for Word` provides it: for a
`Word`, raising by an octave is adding twelve. The call `n.up_octave()`
uses it. The value before the dot, `n`, is the one the behavior acts on.

Calling one method and then another on the result, as in
`n.up_octave().up_octave()`, needs a typed binding in between for now.
Bind the intermediate result with `let m: Word = n.up_octave();` and call
the next method on `m`.

## A generic: a function for many types

A generic function is written once and works for many types. The type it
works on is left as a parameter, a stand-in name in angle brackets:

````
fn first<T>(a: T, b: T) -> T {
    a
}

fn main() -> Word {
    first(64, 67)
}
````

Run it. The output is `64`.

The `<T>` introduces a type parameter named `T`. Inside `first`, both
parameters and the result are `T`, whatever `T` turns out to be. The call
`first(64, 67)` uses `Word` values, so for that call `T` is `Word`. The
same function would serve `Float` values or any other type. A generic
function is a phrase written so that it works whatever the instrument.

## What you now know

- A `trait` declares a named behavior, and an `impl` block provides that
  behavior for one type.
- `value.method()` calls a behavior, acting on the value before the dot.
- A generic function uses a type parameter, written `<T>`, to work for
  many types at once.

The next chapter gives a type a distinct name and a rule.
