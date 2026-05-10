# Getting Started

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This document walks a new user through installing the Keleusma command-line frontend, writing and running a first script, and embedding the same script in a Rust host program. The walkthrough assumes a working Rust toolchain at edition 2024 and minimum supported Rust version 1.87.

## Install the CLI

Keleusma ships a standalone CLI binary called `keleusma`. The CLI provides a script runner, a bytecode compiler, and an interactive REPL. Install it from the workspace root.

````
git clone https://github.com/sgeos/keleusma
cd keleusma
cargo install --path keleusma-cli --bin keleusma
````

Verify the installation.

````
keleusma --help
````

If the command is not found, ensure Cargo's bin directory is on the shell `PATH`. The default location is `~/.cargo/bin`.

## A First Script

Create a file called `hello.kel` with the following contents.

````
fn double(x: i64) -> i64 {
    x + x
}

fn main() -> i64 {
    double(21)
}
````

Run the script.

````
keleusma run hello.kel
````

Expected output.

````
42
````

The runner parses, compiles, verifies, and executes the script. Atomic total functions declared with `fn` may not yield to the host or contain unbounded recursion. The `main` function is the entry point. The return type appears in the function signature and is required.

## Compile to Bytecode

The CLI can serialize a script to bytecode. The serialized form is loadable through the embedding API.

````
keleusma compile hello.kel -o hello.kel.bin
````

The output file uses the framed wire format with magic, length, version, target word and address widths, body, and CRC trailer. A host loads the file through `Vm::load_bytes`.

## Interactive REPL

Start the REPL to explore the language interactively.

````
keleusma repl
````

The REPL accumulates declarations into a session prefix and evaluates expressions against the current prefix. The REPL supports the colon-prefixed commands `:help`, `:quit`, `:reset`, and `:show`.

````
> 1 + 2
3
> fn double(x: i64) -> i64 { x + x }
defined: double
> double(21)
42
> :quit
````

The REPL wraps each expression as `fn main() -> T { <expression> }` and tries return types `i64`, `f64`, `bool`, `String`, and `()` in order. The first type that compiles is used. Expressions whose type lies outside this list require an explicit function declaration.

## Embed in a Rust Host

The same script runs from a Rust host program. Create a new Cargo project.

````
cargo new --bin keleusma-hello
cd keleusma-hello
````

Add Keleusma to `Cargo.toml`.

````
[dependencies]
keleusma = { path = "../keleusma" }
keleusma-arena = { path = "../keleusma/keleusma-arena" }
````

Replace `src/main.rs` with the following.

````rust
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

const SCRIPT: &str = "
    fn double(x: i64) -> i64 { x + x }
    fn main() -> i64 { double(21) }
";

fn main() {
    let tokens = tokenize(SCRIPT).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");

    match vm.call(&[]).expect("call") {
        VmState::Finished(Value::Int(n)) => println!("{}", n),
        other => panic!("unexpected: {:?}", other),
    }
}
````

Build and run.

````
cargo run
````

Expected output.

````
42
````

The host code performs the same compile-verify-run pipeline as the CLI runner. The four steps are visible in the source: lex, parse, compile, and execute. The `Arena` is the bounded-memory region the VM borrows for its operand stack and dynamic-string allocations. `DEFAULT_ARENA_CAPACITY` is sixty-four kilobytes, sufficient for most scripts and the value used by the bundled examples.

## Next Steps

The walkthrough above produces a running Keleusma host. Common next steps include the following.

- Read [EMBEDDING.md](./EMBEDDING.md) for the full embedding surface, including native function registration, arena sizing, the call and resume loop for stream-classified scripts, and error recovery.
- Read [WHY_REJECTED.md](./WHY_REJECTED.md) when the verifier rejects a program. The document maps error messages to root causes and proposes rewrites.
- Explore [`examples/scripts/`](../../examples/scripts) for short scripts demonstrating common language features. Each script runs through `keleusma run`.
- Explore [`examples/`](../../examples) for Rust embedding examples that demonstrate WCMU computation, native attestation, error propagation through yield, and string interoperability.
- Run [`examples/piano_roll.rs`](../../examples/piano_roll.rs) for a feature-gated end-to-end SDL3 audio demonstration. Three voices sequenced by a Keleusma tick loop with hot code swap between two precompiled songs. Run with `cargo run --release --example piano_roll --features sdl3-example`. Press `s` then Enter to swap; press Enter alone to quit.
