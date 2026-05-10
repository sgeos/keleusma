# Keleusma

A Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. Keleusma targets `no_std + alloc` environments.

The ecosystem value proposition is **definitive WCET and WCMU**. Programs whose worst-case execution time or worst-case memory usage cannot be statically bounded are rejected by the safe verifier. Programs that pass verification carry a definitive bound on stream-iteration execution time and memory consumption.

The name derives from the Greek word for a command or signal, specifically the rhythmic calls used by ancient Greek rowing masters to coordinate oar strokes.

## Conservative-Verification Stance

The compile pipeline (parse, type-check, monomorphize, hoist, emit) admits a broader surface than the WCET and WCMU analyses can prove bounded. The verifier rejects programs whose bound is unprovable or whose bound is provable in principle but the analysis is not yet implemented. This rejection is intentional and defines the language's contract. See [`docs/architecture/LANGUAGE_DESIGN.md`](docs/architecture/LANGUAGE_DESIGN.md#conservative-verification) for the full statement.

`Vm::new_unchecked` exists for trust-skip of precompiled bytecode validated during the build pipeline. Using it to admit programs that would fail verification is intentional misuse outside the WCET contract.

## Features

- **Five static guarantees.** Totality, productivity, bounded-step, bounded-memory, safe swapping.
- **Three function categories** with static enforcement: `fn` (atomic total), `yield` (non-atomic total), `loop` (productive divergent).
- **Coroutine model** with typed yield and resume for host-driven stream processing.
- **Multiheaded functions** with pattern matching, guard clauses, and pipeline expressions.
- **Block-structured ISA** enabling single-pass structural verification.
- **WCMU and WCET analysis** providing worst-case bounds at module load. WCMU is reported in bytes; WCET is reported in pipelined cycles. The pipelined-cycle bound is order-of-magnitude correct relative to actual wall-clock execution time on real hardware. Hosts apply a platform-specific calibration factor to convert pipelined cycles to wall-clock time. See [`docs/architecture/LANGUAGE_DESIGN.md`](docs/architecture/LANGUAGE_DESIGN.md#wcet-and-wcmu-analysis) for the full unit conventions and caveats.
- **Target descriptor** for cross-architecture portability across word and address widths.
- **Hot code swap** at RESET boundaries with persistent data segment.
- **Hindley-Milner type inference** with generics, traits, and bounds.
- **Compile-time monomorphization** of generic functions, structs, and enums.
- **f-string interpolation** desugared at lex time to concat and to_string.
- **Static marshalling** through `KeleusmaType` derive for ergonomic native registration.
- **`no_std + alloc` compatible** with a minimal dependency set.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
keleusma = "0.1"
```

Compile and run a script:

```rust
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::compiler::compile;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

let source = r#"
    fn double(x: i64) -> i64 { x * 2 }
    fn main(n: i64) -> i64 { n |> double() }
"#;

let tokens = tokenize(source).expect("lex");
let program = parse(&tokens).expect("parse");
let module = compile(&program).expect("compile");
let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
let mut vm = Vm::new(module, &arena).expect("verify");

match vm.call(&[Value::Int(21)]).unwrap() {
    VmState::Finished(value) => println!("result: {:?}", value),
    _ => unreachable!(),
}
// Output: result: Int(42)
```

The `Vm` borrows a host-owned `Arena` for the operand stack, call frames, and dynamic-string allocations. The arena's bottom region holds the operand stack; the top region holds `KString` allocations. The verifier checks worst-case memory usage against the arena's capacity at construction.

## Language Overview

### Three function categories

```
// Atomic total. Must terminate, no yields, no recursion.
fn clamp(val: i64, lo: i64, hi: i64) -> i64 {
    if val < lo { lo }
    else if val > hi { hi }
    else { val }
}

// Non-atomic total. May yield, must eventually return.
yield prompt(question: String) -> String {
    let answer = yield question;
    answer
}

// Productive divergent. Never returns, must yield every iteration.
loop main(input: i64) -> i64 {
    let result = input * 2;
    let input = yield result;
    input
}
```

### Pattern matching and guard clauses

```
fn classify(0) -> String { "zero" }
fn classify(x: i64) -> String when x > 0 { "positive" }
fn classify(x: i64) -> String { "negative" }

fn describe(msg: Message) -> String {
    match msg {
        Message::Text(s) => s,
        Message::Code(n) => to_string(n),
        _ => "unknown",
    }
}
```

### Generics and traits

```
trait Doubler { fn double(x: i64) -> i64; }
impl Doubler for i64 { fn double(x: i64) -> i64 { x + x } }

fn use_doubler<T: Doubler>(x: T) -> i64 { x.double() }
fn main() -> i64 { use_doubler(21) }
```

### f-string interpolation

```
fn greet(name: String) -> String {
    f"hello, {name}!"
}
```

### Coroutine yield and resume

```
loop audio_processor(sample: f64) -> f64 {
    let output = sample * 0.5;
    let sample = yield output;
    sample
}
```

The host drives execution:

```rust
let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
let mut vm = Vm::new(module, &arena).expect("verify");

let state = vm.call(&[Value::Float(1.0)]).unwrap();
// VmState::Yielded(Float(0.5))

let state = vm.resume(Value::Float(0.8)).unwrap();
// VmState::Reset

let state = vm.resume(Value::Float(0.8)).unwrap();
// VmState::Yielded(Float(0.4))
```

## Native Function Registration

The ergonomic typed registration uses `KeleusmaType`-implementing argument and return types:

```rust
vm.register_fn("square", |x: i64| -> i64 { x * x });
vm.register_fn("scale", |x: f64| -> f64 { x * 2.0 });
```

Lower-level paths exist for functions that inspect arbitrary `Value` variants. See `docs/architecture/LANGUAGE_DESIGN.md` for the full registration contract.

Scripts declare native usage with `use`:

```
use square
use scale

fn main() -> f64 {
    let n = square(5);
    n as f64 |> scale()
}
```

## Cross-Architecture Targeting

The compiler accepts a `Target` descriptor that bakes word, address, and float widths into the bytecode wire format. The verifier rejects programs that use features unsupported by the target.

```rust
use keleusma::compiler::compile_with_target;
use keleusma::target::Target;

let module = compile_with_target(&program, &Target::embedded_16())
    .expect("compile");
```

Presets include `host`, `wasm32`, `embedded_32`, `embedded_16`, and `embedded_8` (8-bit word with 16-bit address per the 6502 class). See `docs/decisions/BACKLOG.md` entry B10 for the portability story.

## Compilation Pipeline

```
Source Code -> tokenize -> parse -> typecheck -> monomorphize -> hoist -> emit -> Module
Module -> verify (structural + WCMU + WCET) -> Vm
```

Stages:

1. **Lexer** (`tokenize`). Source text to tokens with source locations. f-strings desugar at this layer.
2. **Parser** (`parse`). Tokens to abstract syntax tree.
3. **Type checker** (`typecheck::check`). Hindley-Milner inference with generics, traits, and bounds.
4. **Monomorphization** (`monomorphize::monomorphize`). Specializes generic functions, structs, and enums per concrete instantiation.
5. **Closure hoisting** (`hoist_closures`). Lifts closure literals to top-level synthetic chunks.
6. **Emission** (`compile`). Lowers the AST to bytecode.
7. **Verifier** (runs automatically in `Vm::new`). Structural verification, productivity check, WCMU and WCET bounds.
8. **VM** (`call`, `resume`). Bytecode execution with the yield-and-resume protocol.

## Error Handling

Each pipeline stage produces typed errors with source locations.

- `LexError` for tokenization failures.
- `ParseError` for syntax errors.
- `CompileError` for type-check, monomorphization, hoisting, and emission failures.
- `VerifyError` for structural and resource-bound failures.
- `VmError` for runtime errors during `call` and `resume`.

## Workspace

Five crates:

- `keleusma`. The runtime crate.
- `keleusma-macros`. Compile-time proc macro for `#[derive(KeleusmaType)]`.
- `keleusma-arena`. Standalone dual-end bump allocator. Published on crates.io as `keleusma-arena`.
- `keleusma-bench`. Cost-model calibration tool that emits a measured `CostModel` for the host CPU.
- `keleusma-cli`. Standalone command-line frontend providing `run`, `compile`, and `repl` subcommands.

## Documentation

See [docs/README.md](docs/README.md) for the full documentation knowledge graph.

**Onboarding**

- [Getting Started](docs/guide/GETTING_STARTED.md). Install the CLI, write a first script, embed it in a Rust host.
- [Embedding](docs/guide/EMBEDDING.md). Native function registration, arena sizing, call and resume protocol, error recovery.
- [Why Was My Program Rejected](docs/guide/WHY_REJECTED.md). Verifier rejection messages mapped to root causes and rewrites.
- [Script Examples](examples/scripts/README.md). Standalone `.kel` files demonstrating language features.

**Reference**

- [Language Design](docs/architecture/LANGUAGE_DESIGN.md). Design philosophy, guarantees, conservative-verification stance, memory model.
- [Execution Model](docs/architecture/EXECUTION_MODEL.md). Temporal domains, structural verification, indirect-dispatch rejection contract, hot code swap.
- [Compilation Pipeline](docs/architecture/COMPILATION_PIPELINE.md). Stage-by-stage description.
- [Grammar](docs/design/GRAMMAR.md). Formal EBNF grammar.
- [Type System](docs/design/TYPE_SYSTEM.md). Static type discipline, data segment fixed-size constraint.
- [Instruction Set](docs/reference/INSTRUCTION_SET.md). Bytecode reference with costs.
- [Related Work](docs/reference/RELATED_WORK.md). Academic and industrial context with citations.
- [Decisions](docs/decisions/). Resolved, priority, and backlog decisions.

## License

0BSD. See LICENSE file.
