# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Keleusma is a Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. It is a lightweight, embeddable scripting language targeting `no_std+alloc` environments. Without host-plugged functions, Keleusma can only define pure functions that are guaranteed to yield or exit. All domain functionality is provided by native Rust functions registered by the host application.

**Status**: V0.0 Complete. Language pipeline (lexer, parser, compiler, VM) functional. Ready for V0.1 planning.

**Engineering Classification**: Library. See `docs/process/PROCESS_STRATEGY.md`.

## Repository Structure

```
keleusma/
├── CLAUDE.md                  # AI agent instructions
├── Cargo.toml                 # Workspace + runtime package definition
├── src/                       # Runtime package source
│   ├── lib.rs                 # Crate root (no_std, module declarations, re-exports)
│   ├── token.rs               # Token definitions and keyword recognition
│   ├── lexer.rs               # Tokenization (public API: tokenize)
│   ├── ast.rs                 # Abstract Syntax Tree node definitions
│   ├── parser.rs              # Recursive descent parser (public API: parse)
│   ├── bytecode.rs            # Runtime values and instruction set
│   ├── compiler.rs            # Source-to-bytecode compilation (public API: compile)
│   ├── vm.rs                  # Stack-based VM with coroutine support (public API: Vm)
│   ├── verify.rs              # Structural verifier (public API: verify, wcet_stream_iteration)
│   ├── marshall.rs            # KeleusmaType trait and IntoNativeFn family
│   ├── audio_natives.rs       # Built-in audio and math native functions
│   └── utility_natives.rs     # to_string, length, println, math utilities
├── tests/                     # Integration tests
│   └── marshall.rs            # KeleusmaType derive and register_fn end-to-end
├── keleusma-macros/           # Proc-macro crate (workspace member)
│   ├── Cargo.toml
│   └── src/lib.rs             # #[derive(KeleusmaType)]
└── docs/                      # Documentation knowledge graph
    ├── README.md              # Documentation root
    ├── DOCUMENTATION_STRATEGY.md
    ├── architecture/          # Language design and compilation pipeline
    ├── design/                # Grammar, type system, standard library
    ├── decisions/             # Resolved, priority, and backlog decisions
    ├── process/               # Workflow, communication, and task tracking
    ├── reference/             # Glossary, instruction set
    └── roadmap/               # Development phases
```

## Documentation

A knowledge graph is maintained in `docs/`. Start at [`docs/README.md`](docs/README.md) for navigation.

| Section | Path | Description |
|---------|------|-------------|
| Architecture | [`docs/architecture/`](docs/architecture/README.md) | Language design and compilation pipeline |
| Design | [`docs/design/`](docs/design/README.md) | Grammar, type system, standard library |
| Decisions | [`docs/decisions/`](docs/decisions/README.md) | Architectural and design decisions |
| Process | [`docs/process/`](docs/process/README.md) | Development workflow and task tracking |
| Reference | [`docs/reference/`](docs/reference/README.md) | Glossary, instruction set |
| Roadmap | [`docs/roadmap/`](docs/roadmap/README.md) | Development phases |

## Development Process

See `docs/process/PROCESS_STRATEGY.md` for the library engineering approach and agentic development loop.

**Session startup protocol**:
1. Read [`docs/process/TASKLOG.md`](docs/process/TASKLOG.md) for current task state.
2. Read [`docs/process/REVERSE_PROMPT.md`](docs/process/REVERSE_PROMPT.md) for last AI communication.
3. Wait for human prompt before proceeding.

**After completing each task**:
1. Update task status in `docs/process/TASKLOG.md`.
2. Overwrite `docs/process/REVERSE_PROMPT.md` with verification, questions, concerns, and intended next step.
3. Commit changes with conventional commit referencing the task.
4. If blocked or uncertain, document in REVERSE_PROMPT.md and **stop**.

**Working documents**:

| File | Purpose |
|------|---------|
| `docs/process/TASKLOG.md` | Current sprint source of truth |
| `docs/process/PROMPT.md` | Human to AI instruction staging (read-only for AI) |
| `docs/process/REVERSE_PROMPT.md` | AI to Human communication |

## Git Workflow

Trunk-based development with short-lived feature branches. See [`docs/process/GIT_STRATEGY.md`](docs/process/GIT_STRATEGY.md) for full details.

Use scoped conventional commits: `<scope>: <imperative summary>`. Common scopes: `feat`, `fix`, `docs`, `refactor`, `chore`, `test`. Include `Co-Authored-By: Claude <noreply@anthropic.com>` when AI-assisted.

The AI agent commits once after all tasks in a prompt are complete, including the `REVERSE_PROMPT.md` update. `PROMPT.md` is read-only for the AI agent but must be included in the commit if the human pilot has modified it.

## Common Commands

```bash
# Build
cargo build

# Run tests
cargo test

# Check without building
cargo check

# Format and lint
cargo fmt
cargo clippy -- -D warnings

# Full verification
cargo test && cargo clippy --tests -- -D warnings
```

## Coding Conventions

### no_std + alloc

This crate targets `no_std` with `alloc`. All allocations use `alloc` collections (`Vec`, `String`, `BTreeMap`). No standard library types.

### Generics Over Dynamic Dispatch

Prefer trait-bounded generics over dynamically dispatched trait objects (`&dyn Trait`). Define type aliases and trait bounds at the top of the file to keep generic signatures readable.

### Functional Core

Prefer pure functions that take inputs and return outputs without side effects. State mutation should be confined to the VM execution loop.

### Error Handling

All public API functions return `Result` types with error structs that include source location (`Span`) for precise error reporting. Error types: `LexError`, `ParseError`, `CompileError`, `VmError`.

## Technology Stack

- **Rust** (edition 2024)
- **no_std + alloc** (no standard library dependency)
- **libm 0.2** (math functions for no_std environments)
- **syn 2, quote 1, proc-macro2 1** (compile-time only, used by `keleusma-macros`)
- Cargo workspace with two members: `keleusma` (runtime) and `keleusma-macros` (proc-macro)
- 268 tests across lexer, parser, compiler, VM, verifier, marshall, audio natives, utility natives, and integration tests
