# Chapter 31. Embedding Keleusma: Orientation

> Part IX, Embedding Keleusma in a Rust Program. Chapter 31 of 40.
> Previous: [Chapter 30, A Tour of the Song Roster](./30_song_roster.md).
> Next: [Chapter 32, Constructing a VM and Running a Module](./32_constructing_a_vm.md).

## Goal

By the end of this chapter you will understand what this part covers, who
it is for, and you will have a minimal Keleusma host compiling and
running.

## Who this part is for

Parts I through VIII were written for someone learning the Keleusma
language. This part is different. It is for a Rust developer who wants to
embed Keleusma inside a Rust program of their own. It assumes a working
knowledge of Rust: cargo, dependencies, traits, closures, and `Result`.
The music framing of the earlier parts is set aside. The prose here is
plain and technical.

The worked example throughout this part is the piano roll. Part VIII met
the piano roll from the song side. This part builds the host behind it.

## The host and the script

A Keleusma deployment has two halves.

- The script is a `.kel` program. It carries the bounded, verified logic.
- The host is a Rust program. It compiles the script, verifies it,
  constructs a virtual machine, drives that machine, and supplies the
  native functions the script calls.

The host is where Keleusma is embedded. Everything in this part is host
code.

## A minimal host

A host is an ordinary Rust binary project with `keleusma` as a
dependency. The `Cargo.toml` needs one line:

````
[dependencies]
keleusma = "0.2"
````

The `src/main.rs` below is a complete host. It compiles a one-line
script, constructs a VM, runs it, and prints the result:

````rust
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

const SCRIPT: &str = "fn main() -> Word { 60 + 7 }";

fn main() {
    let tokens = tokenize(SCRIPT).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");

    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => println!("result: {}", n),
        other => panic!("unexpected: {:?}", other),
    }
}
````

Run it with `cargo run`. The output is:

````
result: 67
````

Every later chapter of this part builds on this skeleton. The next
chapter examines the construction phases in detail. The remaining
chapters add native functions, the resume protocol, arena sizing,
bytecode loading, hot swap, and cost models, and the part closes with a
full walkthrough of the piano roll host.

## What you now know

- This part is for a Rust developer embedding Keleusma in a host program.
- A Keleusma deployment is a host and a script; the host is the Rust
  side.
- A minimal host adds `keleusma` as a dependency and runs the
  lex, parse, compile, construct, call sequence.

The next chapter takes that sequence apart.
