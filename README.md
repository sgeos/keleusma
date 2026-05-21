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
- **Static marshalling** through `KeleusmaType` derive for ergonomic native registration.
- **`no_std + alloc` compatible** with a minimal dependency set.

### Cargo features

The runtime crate exposes orthogonal feature gates so hosts can strip pipeline stages they do not need from the flash image.

| Feature | Default | What it adds | Drop to save flash when |
|---------|:-------:|--------------|-------------------------|
| `compile` | on | Lexer, parser, type checker, monomorphizer, compiler. The source-to-bytecode pipeline. | The host ships precompiled bytecode and loads through `Module::from_bytes` or `Vm::view_bytes_zero_copy`. |
| `verify` | on | Structural verifier, WCET and WCMU resource-bounds pass. Used inside `Vm::new` at load time. | An equivalent verification ran at artefact-ingestion time; `Vm::new` then degrades to a trust-load equivalent to `Vm::new_unchecked`. |
| `floats` | on | Surface syntax for the `Float` type and float literals, `Value::Float` and `ConstValue::Float` variants, `Op::IntToFloat` and `Op::FloatToInt` opcode bodies, the f64 arm in `Vm::binary_arith`, the `KeleusmaType` impl for `f64`, the `audio_natives` and `stddsl` bundles. | Scripts use only integer, byte, and fixed-point arithmetic. Dropping `floats` removes the soft-float `compiler_builtins` routines (`__divdf3`, `__adddf3`, `__muldf3`) from the runtime image; on the bare-metal STM32N6570-DK build this is roughly 12 KB. |
| `shell` | off | The `stddsl::Shell` bundle, which forwards `println` and a handful of shell-style natives onto the host. | The host registers its own diagnostics natives. |

Text support is unconditional in V0.2.0: `Value::StaticStr` literals and arena-resident `Value::KStr` strings are always available, and the surface syntax accepts the `Text` type. Dynamic-text composition (`to_string`, `concat`, `slice`, `length`) is the host's responsibility through `register_verified_native` or the `register_fn` marshalling layer. The bundled `register_utility_natives` registers `println` only.

The features compose freely. The `examples/rtos/` cooperative microkernel disables `floats` and uses precompiled bytecode under either `keleusma-verify` only (157 KB `.text`) or trust-load (137 KB `.text`) on the STM32N6570-DK; see [`examples/rtos/MANUAL.md`](examples/rtos/MANUAL.md) for the measured flash-size table.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
keleusma = "0.2"
```

Compile and run a script:

```rust
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::compiler::compile;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

let source = r#"
    fn double(x: Word) -> Word { x * 2 }
    fn main(n: Word) -> Word { n |> double() }
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
fn clamp(val: Word, lo: Word, hi: Word) -> Word {
    if val < lo { lo }
    else if val > hi { hi }
    else { val }
}

// Non-atomic total. May yield, must eventually return.
yield prompt(question: Text) -> Text {
    let answer = yield question;
    answer
}

// Productive divergent. Never returns, must yield every iteration.
loop main(input: Word) -> Word {
    let result = input * 2;
    let input = yield result;
    input
}
```

### Pattern matching and guard clauses

```
fn classify(0) -> Text { "zero" }
fn classify(x: Word) -> Text when x > 0 { "positive" }
fn classify(x: Word) -> Text { "negative" }

use format

fn describe(msg: Message) -> Text {
    match msg {
        Message::Body(s) => s,
        Message::Code(n) => format(n),
        _ => "unknown",
    }
}
```

### Generics and traits

```
trait Doubler { fn double(x: Word) -> Word; }
impl Doubler for Word { fn double(x: Word) -> Word { x + x } }

fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }
fn main() -> Word { use_doubler(21) }
```

### Coroutine yield and resume

```
loop audio_processor(sample: Float) -> Float {
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

fn main() -> Float {
    let n = square(5);
    n as Float |> scale()
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
Source Code -> tokenize -> parse -> typecheck -> monomorphize -> emit -> Module
Module -> verify (structural + WCMU + WCET) -> Vm
```

Stages:

1. **Lexer** (`tokenize`). Source text to tokens with source locations.
2. **Parser** (`parse`). Tokens to abstract syntax tree.
3. **Type checker** (`typecheck::check`). Hindley-Milner inference with generics, traits, and bounds. Closure-shaped expressions and first-class function references are rejected here with a diagnostic that names the construct.
4. **Monomorphization** (`monomorphize::monomorphize`). Specializes generic functions, structs, and enums per concrete instantiation.
5. **Emission** (`compile`). Lowers the AST to bytecode.
6. **Verifier** (runs automatically in `Vm::new`). Structural verification, productivity check, WCMU and WCET bounds.
7. **VM** (`call`, `resume`). Bytecode execution with the yield-and-resume protocol.

## Error Handling

Each pipeline stage produces typed errors with source locations.

- `LexError` for tokenization failures.
- `ParseError` for syntax errors.
- `CompileError` for type-check, monomorphization, and emission failures.
- `VerifyError` for structural and resource-bound failures.
- `VmError` for runtime errors during `call` and `resume`.

## Workspace

Five crates:

- `keleusma`. The runtime crate.
- `keleusma-macros`. Compile-time proc macro for `#[derive(KeleusmaType)]`.
- `keleusma-arena`. Standalone dual-end bump allocator. Published on crates.io as `keleusma-arena`.
- `keleusma-bench`. Cost-model calibration tool that emits a measured `CostModel` for the host CPU.
- `keleusma-cli`. Standalone command-line frontend providing `run`, `compile`, and `repl` subcommands.

## Examples

Rust embedding examples live under [`examples/`](examples). Run any of them with `cargo run --example <name>`.

A larger end-to-end example, [`piano_roll`](examples/piano_roll.rs), is a three-channel SDL3 audio host driven by a Keleusma tick-based control loop. It exercises the principal capabilities Keleusma is designed for: bounded-step execution under a real-time deadline (audio rendering), thread-safe handoff between the Keleusma main thread and the SDL3 audio callback, multi-voice control flow through the data segment, and hot code swap between two precompiled songs at the reset boundary.

The example is gated behind the `sdl3-example` feature because SDL3 is built from source via CMake. Run with:

```sh
cargo run --release --example piano_roll --features sdl3-example
```

Controls:

- Press `s` then Enter to swap songs.
- Press Enter alone to quit.

### RTOS microkernel ([`examples/rtos/`](examples/rtos))

A draft cooperative-scheduling microkernel where every task is a Keleusma `loop main` script. The kernel core is `no_std + alloc`; the same kernel runs on a host through a `StdPlatform` for development and on the STM32N6570-DK through an embassy-backed `Stm32N6570DkPlatform` for hardware. Three tasks (LED blinker, sensor poller, heartbeat) are dispatched cooperatively. Verified on hardware on 2026-05-18.

The RTOS example is a standalone Rust crate (its own `Cargo.toml`, toolchain pin, `build.rs`, `memory.x`, and `.cargo/config.toml`) intentionally detached from the parent workspace because its bare-metal dependencies (embassy git pins, defmt, cortex-m-rt, an embedded heap allocator) are heavy and orthogonal to the parent crate's normal build. Run from inside `examples/rtos/`:

```sh
cd examples/rtos
# Host demonstrator
cargo run --release --bin three-task-std

# STM32N6570-DK demonstrator (BOOT0 in dev position, ST-LINK V3-EC attached)
cargo run --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform
```

See the example's [`README.md`](examples/rtos/README.md), [`MANUAL.md`](examples/rtos/MANUAL.md) (operator manual), and [`SPEC.md`](examples/rtos/SPEC.md) (architectural rationale) for the full story.

## Documentation

See [docs/README.md](docs/README.md) for the full documentation knowledge graph.

**Onboarding**

- [Getting Started](docs/guide/GETTING_STARTED.md). Install the CLI, write a first script, embed it in a Rust host.
- [Embedding](docs/guide/EMBEDDING.md). Native function registration, arena sizing, call and resume protocol, error recovery.
- [Why Was My Program Rejected](docs/guide/WHY_REJECTED.md). Verifier rejection messages mapped to root causes and rewrites.
- [FAQ](docs/guide/FAQ.md). Common rough edges and surprises: string handling, escape sequences, the immutable-locals constraint, and what changed from the V0.1.x pre-release line.
- [Script Examples](examples/scripts/README.md). Standalone `.kel` files demonstrating language features.

**Reference**

- [Language Design](docs/architecture/LANGUAGE_DESIGN.md). Design philosophy, guarantees, conservative-verification stance, memory model.
- [Execution Model](docs/architecture/EXECUTION_MODEL.md). Temporal domains, structural verification, indirect-dispatch rejection contract, hot code swap.
- [Compilation Pipeline](docs/architecture/COMPILATION_PIPELINE.md). Stage-by-stage description.
- [Grammar](docs/spec/GRAMMAR.md). Formal EBNF grammar.
- [Type System](docs/spec/TYPE_SYSTEM.md). Static type discipline, data segment fixed-size constraint.
- [Instruction Set](docs/spec/INSTRUCTION_SET.md). Bytecode reference with costs.
- [Related Work](docs/reference/RELATED_WORK.md). Academic and industrial context with citations.
- [Decisions](docs/decisions/). Resolved, priority, and backlog decisions.

## License

0BSD. See LICENSE file.
