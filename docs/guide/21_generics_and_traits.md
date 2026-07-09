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

## A const generic: a compile-time number

A type parameter stands in for a type. A const parameter stands in for a
number fixed at compile time. It is written `const n: Word` in the angle
brackets, and inside the body `n` is an ordinary `Word` value:

````
fn plus<const n: Word>() -> Word {
    n + 10
}

fn main() -> Word {
    plus::<7>()
}
````

Run it. The output is `17`. The `::<7>` after the name is a turbofish,
and it supplies the const value for this call. A const value is always
written out this way, never inferred, because there is no value argument
for the compiler to read it from.

A const parameter can set the length of an array, so a function can take
a fixed-size buffer whose size is part of its signature:

````
fn first<const n: Word>(a: [Word; n]) -> Word {
    a[0]
}

fn main() -> Word {
    first::<3>([10, 20, 30])
}
````

The output is `10`. Structs take const parameters too, mixed after any
type parameters, and construction supplies the const with the same
turbofish:

````
struct Buf<const n: Word> {
    items: [Word; n],
}

fn get(b: Buf<3>) -> Word {
    b.items[2]
}

fn main() -> Word {
    get(Buf::<3> { items: [10, 20, 30] })
}
````

The output is `30`. A const value can be built from other const values
with `+`, `-`, and `*`, as in `Buf<n + 1>` or `Multiword<2 * n>`. There is
no const division, so const arithmetic is always total.

Every const parameter is replaced by its concrete number when the program
is specialized, before the worst-case bounds are computed. The verifier
therefore never sees a symbolic size; a `[Word; n]` has become a
`[Word; 3]` by the time its memory is measured. This is why a const
generic keeps the definitive time and memory bounds intact.

## What you now know

- A `trait` declares a named behavior, and an `impl` block provides that
  behavior for one type.
- `value.method()` calls a behavior, acting on the value before the dot.
- A generic function uses a type parameter, written `<T>`, to work for
  many types at once.
- A const parameter, written `<const n: Word>`, stands in for a
  compile-time number, supplied by the turbofish `f::<7>()` and usable as
  an array length, a `Multiword` dimension, or a `Word` value. It is
  erased to its concrete number before the bounds are computed.

The next chapter gives a type a distinct name and a rule.
