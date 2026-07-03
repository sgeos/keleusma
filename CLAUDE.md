# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Keleusma is a Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. It is a lightweight, embeddable scripting language targeting `no_std+alloc` environments. The ecosystem value proposition is **definitive WCET and WCMU**. Programs whose worst-case execution time or worst-case memory usage cannot be statically bounded are rejected by the safe verifier. Without host-plugged functions, the language admits only pure total functions and the productive divergent `loop` block. All domain functionality is provided by native Rust functions registered by the host application.

**Status**: V0.2.1 in development on branch `feat-flat-const-pool`, the nominal next publication and not yet released. V0.2.0 published cryptographic module signing (Ed25519), the V0.2.0 ISA reset (fixed-size opcode records, a separately addressed operand pool, a section-partitioned body), information-flow labels including negative labels, calibrated WCET cost models via the `keleusma-bench` crate, and a docs/spec/ reorganization that consolidates authoritative specifications. V0.2.1 completes the B28 flat-byte composite representation: composite `Value` bodies are pure flat bytes resident in the host arena (no global-heap `Vec`/`String` indirection), the `Value` slot is 32 bytes (down from 40, pinned by a `const` size assertion), the P4 `NewComposite` consolidation took the instruction set from 69 to 66 opcodes, the `shared data` segment became a host-owned borrowed `&mut [u8]` buffer driven through `call_with_shared`/`resume_with_shared` and `get_shared`/`set_shared` (the `set_data`/`get_data` slot vector is removed), private composite data persists in the arena's persistent region across RESET, and worst-case-memory-usage bounds are correspondingly tighter; B28 also subsumes B26 and B27. V0.1.x retired surface features (closures, f-strings, `text` bundled DSL) are gone; programs that used them must be rewritten under host-registered natives. The runtime is at `BYTECODE_VERSION = 1` (reset for V0.2.0 and unchanged through V0.2.1; V0.1.x bytecode does not load). Approximately 1149 keleusma lib tests plus 274 integration tests across 23 files (including 53 rogue-script, 31 marshall, the 59-test multi-word fixed-point suite, and the flat-composite, persistent-data, and narrow-word VM suites, the last also covering multi-word arithmetic at a 16-bit word width), 42 keleusma-arena, and 6 keleusma-bench tests across the workspace, all passing under default features, default+signatures, and `--all-features`. Hindley-Milner inference, generics with traits and bounds, compile-time monomorphization, target descriptor for cross-architecture portability, hot code swap, and the conservative-verification stance remain in place.

**Conservative-verification stance.** The compile pipeline admits a broader surface than the WCET and WCMU analyses can prove bounded. The verifier rejects programs whose bound is unprovable (first category) or whose bound is provable in principle but the analysis is not yet implemented (second category). See [`docs/architecture/LANGUAGE_DESIGN.md`](docs/architecture/LANGUAGE_DESIGN.md#conservative-verification) for the full statement. `Vm::new_unchecked` exists for trust-skip of precompiled bytecode and is intentional misuse if used to admit programs that would fail verification.

**Engineering Classification**: Library. See `docs/process/PROCESS_STRATEGY.md`.

## Repository Structure

```
keleusma/
├── CLAUDE.md                  # AI agent instructions
├── Cargo.toml                 # Workspace + runtime package definition
├── src/                       # Runtime package source
│   ├── lib.rs                 # Crate root (no_std, module declarations, re-exports)
│   ├── token.rs               # Token definitions and keyword recognition
│   ├── lexer.rs               # Tokenization (public API: tokenize), includes f-string desugaring
│   ├── ast.rs                 # Abstract Syntax Tree node definitions
│   ├── parser.rs              # Recursive descent parser (public API: parse)
│   ├── visitor.rs             # MutVisitor and Visitor traits with default walk methods over Block, Stmt, Expr, Iterable
│   ├── typecheck.rs           # Hindley-Milner type checker (public API: check), generics, traits, impl method validation
│   ├── monomorphize.rs        # Compile-time monomorphization for generic functions, structs, enums (public API: monomorphize)
│   ├── target.rs              # Target descriptor for cross-architecture portability (public API: Target)
│   ├── bytecode.rs            # Runtime values, instruction set, wire format, target-aware width fields
│   ├── compiler.rs            # Source-to-bytecode compilation (public API: compile, compile_with_target)
│   ├── vm.rs                  # Stack-based VM with coroutine support (public API: Vm), per-op decode cache
│   ├── verify.rs              # Structural verifier (public API: verify, wcet_stream_iteration, wcmu_stream_iteration, verify_resource_bounds, module_wcmu)
│   ├── marshall.rs            # KeleusmaType trait and IntoNativeFn family
│   ├── audio_natives.rs       # Built-in audio and math native functions
│   └── utility_natives.rs     # to_string, length, concat, slice, println, math utilities
├── tests/                     # Integration tests
│   └── marshall.rs            # KeleusmaType derive and register_fn end-to-end
├── keleusma-macros/           # Proc-macro crate (workspace member)
│   ├── Cargo.toml
│   └── src/lib.rs             # #[derive(KeleusmaType)]
├── keleusma-arena/            # Standalone arena allocator (workspace member)
│   ├── Cargo.toml
│   ├── README.md
│   └── src/lib.rs             # Arena, BottomHandle, TopHandle, Budget, marks
├── examples/
│   ├── rogue/                 # Roguelike example (workspace [[example]])
│   ├── piano_roll.rs          # SDL3 audio + hot-swap (workspace [[example]])
│   ├── rtos/                  # Cooperative RTOS microkernel (standalone crate, not a workspace member)
│   │   ├── Cargo.toml         # Detached [workspace]; embassy git deps under stm32n6570dk-platform feature
│   │   ├── README.md          # Overview, quick-start commands, file table
│   │   ├── MANUAL.md          # Operator manual: hardware setup, build matrix, troubleshooting
│   │   ├── SPEC.md            # Architectural rationale and roadmap
│   │   ├── memory.x           # AXISRAM2 layout for the STM32N6570-DK bin
│   │   ├── build.rs           # Target-conditional link args (no_std target only)
│   │   ├── .cargo/config.toml # probe-rs runner for thumbv8m.main-none-eabihf
│   │   ├── scripts/           # Keleusma scripts (prelude, led, sensor, heartbeat)
│   │   └── src/               # Kernel core, platform impls, natives, bins
│   └── …                      # Other Rust embedding examples and standalone .kel scripts
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
| Guide | [`docs/guide/`](docs/guide/README.md) | Onboarding for new users and embedders |
| Architecture | [`docs/architecture/`](docs/architecture/README.md) | Narrative descriptions of the implemented system |
| Spec | [`docs/spec/`](docs/spec/README.md) | Authoritative specifications: grammar, type system, standard library, instruction set, structural ISA, wire format |
| Decisions | [`docs/decisions/`](docs/decisions/README.md) | Architectural and design decisions |
| Process | [`docs/process/`](docs/process/README.md) | Development workflow and task tracking |
| Reference | [`docs/reference/`](docs/reference/README.md) | Glossary and citations |
| Roadmap | [`docs/roadmap/`](docs/roadmap/README.md) | Development phases |
| Extras | [`docs/extras/`](docs/extras/README.md) | Supplementary references for specific examples |

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
- **allocator-api2 0.4** (stable polyfill of the unstable allocator API, used by `keleusma-arena`)
- **syn 2, quote 1, proc-macro2 1** (compile-time only, used by `keleusma-macros`)
- **rkyv 0.8** (zero-copy archived bytecode format)
- Cargo workspace with members: `keleusma` (runtime), `keleusma-macros` (proc-macro), `keleusma-arena` (standalone arena allocator), `keleusma-bench` (cost-model calibration), and `keleusma-cli` (CLI frontend).
- Approximately 1149 keleusma lib tests plus 274 integration tests across 23 files (including 53 rogue-script, 31 marshall, the 59-test multi-word fixed-point suite, and the flat-composite, persistent-data, narrow-word VM, and zero-copy suites), 42 keleusma-arena, and 6 keleusma-bench tests across the workspace covering lexer, parser, type checker, monomorphizer, compiler, VM, verifier, marshall, flat-byte composites, multi-word fixed-point arithmetic, arena, audio natives, utility natives, target descriptor, visitor pattern, signing, IFC labels, cost-model calibration, and integration tests.
- The `examples/rtos/` directory carries a standalone crate (not a workspace member) implementing a cooperative RTOS microkernel; it depends on the parent `keleusma` runtime by path and ships its own toolchain pin, build.rs, memory.x, and probe-rs runner. Run with `cd examples/rtos && cargo run --release --bin three-task-std` (host) or `cd examples/rtos && cargo run --release --bin three-task-n6 --target thumbv8m.main-none-eabihf --no-default-features --features stm32n6570dk-platform` (STM32N6570-DK). See `examples/rtos/MANUAL.md`.
