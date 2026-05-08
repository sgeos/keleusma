# Keleusma

A Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. Keleusma is designed for embedded scripting in audio engines, game simulations, and domains where deterministic execution, bounded-step guarantees, and coroutine-based stream processing are required. The crate targets `no_std+alloc` environments.

The name derives from the Greek word for a command or signal, specifically the rhythmic calls used by ancient Greek rowing masters to coordinate oar strokes.

## Features

- **Three function categories** with static guarantees: `fn` (atomic total), `yield` (non-atomic total), `loop` (productive divergent)
- **Coroutine model** with typed yield/resume for host-driven stream processing
- **Multiheaded functions** with pattern matching and guard clauses
- **Pipeline expressions** (`|>`) with placeholder support
- **Block-structured ISA** enabling single-pass structural verification
- **Productivity verification** ensuring every stream iteration yields observable output
- **WCET analysis** providing worst-case execution time bounds per yield slice
- **Native function binding** for host-defined domain-specific operations
- **`no_std+alloc` compatible** with a single external dependency (`libm`)

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
keleusma = { path = "path/to/keleusma" }
```

Compile and run a script:

```rust
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::compiler::compile;
use keleusma::vm::{Vm, VmState};
use keleusma::bytecode::Value;

let source = r#"
    fn double(x: i64) -> i64 { x * 2 }
    fn main(n: i64) -> i64 { n |> double() }
"#;

let tokens = tokenize(source).expect("lex error");
let program = parse(&tokens).expect("parse error");
let module = compile(&program).expect("compile error");
let mut vm = Vm::new(module).expect("verification error");

match vm.call(&[Value::Int(21)]).unwrap() {
    VmState::Finished(value) => println!("result: {:?}", value),
    _ => unreachable!(),
}
// Output: result: Int(42)
```

## Language Overview

### Three Function Categories

```
// Atomic total: must terminate, no yields.
fn clamp(val: i64, lo: i64, hi: i64) -> i64 {
    if val < lo { lo }
    else if val > hi { hi }
    else { val }
}

// Non-atomic total: may yield, must eventually return.
yield prompt(question: String) -> String {
    let answer = yield question;
    answer
}

// Productive divergent: never returns, must yield every iteration.
loop main(input: i64) -> i64 {
    let result = input * 2;
    let input = yield result;
    input
}
```

### Pattern Matching and Guard Clauses

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

### Pipeline Expressions

```
fn process(x: i64) -> String {
    x |> double() |> clamp(_, 0, 100) |> to_string()
}
```

### Coroutine Yield and Resume

```
loop audio_processor(sample: f64) -> f64 {
    let output = sample * 0.5;
    let sample = yield output;
    sample
}
```

The host drives execution:

```rust
// Start the stream with an initial sample.
let state = vm.call(&[Value::Float(1.0)]).unwrap();
// VmState::Yielded(Float(0.5))

// Resume with a new sample.
let state = vm.resume(Value::Float(0.8)).unwrap();
// VmState::Reset -- iteration complete, arena cleared.

// Resume after reset with the next sample.
let state = vm.resume(Value::Float(0.8)).unwrap();
// VmState::Yielded(Float(0.4))
```

## Native Function Registration

Register host functions before calling the VM:

```rust
use keleusma::vm::{Vm, VmError};
use keleusma::bytecode::Value;

// Function pointer.
vm.register_native("square", |args: &[Value]| {
    match &args[0] {
        Value::Int(x) => Ok(Value::Int(x * x)),
        _ => Err(VmError::TypeError("expected Int".into())),
    }
});

// Closure capturing state.
let scale_factor = 2.0;
vm.register_native_closure("scale", move |args: &[Value]| {
    match &args[0] {
        Value::Float(x) => Ok(Value::Float(x * scale_factor)),
        _ => Err(VmError::TypeError("expected Float".into())),
    }
});
```

Scripts declare native function usage with `use`:

```
use square
use scale

fn main() -> f64 {
    let n = square(5);
    n as f64 |> scale()
}
```

## Provided Native Functions

### Audio and Math (`register_audio_natives`)

| Function | Description |
|----------|-------------|
| `audio::midi_to_freq` | MIDI note number to frequency in Hz |
| `audio::freq_to_midi` | Frequency in Hz to nearest MIDI note |
| `audio::db_to_linear` | Decibels to linear amplitude |
| `audio::linear_to_db` | Linear amplitude to decibels |
| `math::clamp` | Clamp value to range |
| `math::lerp` | Linear interpolation |
| `math::sin`, `math::cos` | Trigonometric functions |
| `math::pow` | Exponentiation |
| `math::abs` | Absolute value |
| `math::min`, `math::max` | Minimum and maximum |

### Utility (`register_utility_natives`)

| Function | Description |
|----------|-------------|
| `to_string` | Convert any value to string |
| `length` | Length of array, string, or tuple |
| `println` | Debug print (no-op in `no_std`) |
| `math::sqrt` | Square root |
| `math::floor` | Floor rounding |
| `math::ceil` | Ceiling rounding |
| `math::round` | Nearest integer rounding |
| `math::log2` | Base-2 logarithm |

## Compilation Pipeline

```
Source Code -> Lexer -> Tokens -> Parser -> AST -> Compiler -> Module -> Verifier -> VM
```

1. **Lexer** (`tokenize`): Source text to tokens with source locations.
2. **Parser** (`parse`): Tokens to abstract syntax tree.
3. **Compiler** (`compile`): AST to bytecode module.
4. **Verifier** (runs automatically in `Vm::new`): Structural verification, productivity checking, WCET analysis.
5. **VM** (`call`, `resume`): Bytecode execution with yield/resume protocol.

## Error Handling

Each pipeline stage produces typed errors with source locations:

- `LexError`: Invalid characters, unterminated strings or comments.
- `ParseError`: Syntax errors with line and column.
- `CompileError`: Undefined variables, undefined functions, break outside loop.
- `VmError`: Type errors, division by zero, index out of bounds, verification failures.

## Documentation

See [docs/README.md](docs/README.md) for the full documentation knowledge graph, including:

- [Language Design](docs/architecture/LANGUAGE_DESIGN.md): Design philosophy, guarantees, memory model.
- [Grammar Specification](docs/design/GRAMMAR.md): Formal EBNF grammar and design decisions.
- [Execution Model](docs/architecture/EXECUTION_MODEL.md): Temporal domains, structural verification.
- [Instruction Set](docs/reference/INSTRUCTION_SET.md): Complete bytecode reference with costs.
- [Related Work](docs/reference/RELATED_WORK.md): Academic and industrial context with citations.

## License

MIT
