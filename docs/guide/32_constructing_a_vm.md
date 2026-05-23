# Chapter 32. Constructing a VM and Running a Module

> Part IX, Embedding Keleusma in a Rust Program. Chapter 32 of 40.
> Previous: [Chapter 31, Embedding Keleusma: Orientation](./31_embedding_orientation.md).
> Next: [Chapter 33, Registering Native Functions](./33_registering_natives.md).

## Goal

By the end of this chapter you will understand each phase of VM
construction and the lifetime relationship between the VM and the arena.

## The four phases

Constructing a VM from source is four phases, each producing a distinct
type.

````rust
let tokens  = tokenize(SOURCE)?;   // Vec<Token>
let program = parse(&tokens)?;     // Program
let module  = compile(&program)?;  // Module
let arena   = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
let mut vm  = Vm::new(module, &arena)?;
````

- `tokenize` turns source text into a `Vec<Token>`.
- `parse` turns the tokens into a `Program`, the syntax tree.
- `compile` turns the program into a `Module`, the bytecode object.
- `Vm::new` consumes the module, borrows the arena, runs structural
  verification and resource-bounds verification, and returns a VM ready
  to call.

Each phase returns a `Result`. A failure in any phase is a typed error:
`LexError`, `ParseError`, `CompileError`, or, from `Vm::new`,
`VmError::VerifyError`.

## Verification happens at construction

`Vm::new` is where the guarantees of Part V are enforced. It runs the
structural verifier and the resource-bounds verifier before returning. A
program the verifier rejects never yields a VM; `Vm::new` returns
`Err(VmError::VerifyError(message))`, and the message is the one
documented in `WHY_REJECTED.md`. No script code has run at that point.
The host learns a program is unacceptable at construction, not partway
through execution.

## The arena and its lifetime

The arena is the bounded working memory the VM uses for its operand stack
and for dynamic strings. The host creates it and the VM borrows it.

The borrow is enforced by the Rust borrow checker. `Vm` carries an
`'arena` lifetime parameter, and the arena must outlive the VM. In
practice this means the arena is declared before the VM and dropped
after it, which ordinary block scoping gives for free:

````rust
let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
let mut vm = Vm::new(module, &arena)?;
// vm is used here; arena outlives it
````

Chapter 35 covers how to choose the arena capacity. For now,
`DEFAULT_ARENA_CAPACITY`, sixty-four kilobytes, serves.

## Running an atomic module

Once constructed, an atomic `fn main` module is run with `call`:

````rust
match vm.call(&[])? {
    VmState::Finished(value) => { /* use value */ }
    other => panic!("expected Finished, got {:?}", other),
}
````

`call` takes a slice of arguments. An `fn main` that takes no parameters
is called with `&[]`. The atomic module runs to completion and `call`
returns `VmState::Finished(value)`, carrying the return value.

A `yield` or `loop` module does not finish on the first `call`. It
returns `VmState::Yielded` instead, and driving it requires the resume
protocol. Chapter 34 covers that. Native functions, which most real
scripts need, come first, in the next chapter.

## What you now know

- VM construction is four phases: `tokenize`, `parse`, `compile`,
  `Vm::new`.
- `Vm::new` runs verification; a rejected program fails here, before any
  code runs.
- The VM borrows the arena, and the arena must outlive the VM.
- `call(&[])` runs an atomic module and returns `VmState::Finished`.

The next chapter registers the host functions a script calls.
