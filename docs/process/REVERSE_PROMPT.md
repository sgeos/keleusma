# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T42 Standalone CLI: keleusma compiler, runner, REPL.
**Status**: Complete. New `keleusma-cli` workspace member provides a `keleusma` binary modeled after Rhai's CLI ergonomics, allowing users to work with Keleusma scripts without writing Rust host code.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p keleusma-cli
./target/release/keleusma run keleusma-cli/examples/hello.kel
./target/release/keleusma compile keleusma-cli/examples/hello.kel -o tmp/hello.kel.bin
printf 'fn double(x: i64) -> i64 { x + x }\ndouble(21)\n:quit\n' | ./target/release/keleusma repl
```

**Results**:

- Workspace tests pass. 519 tests across the workspace.
- Format clean.
- Clippy clean across `--workspace --all-targets`.
- `keleusma run hello.kel` prints `42`.
- `keleusma compile hello.kel -o tmp/hello.kel.bin` produces a 220-byte file consumable by `Vm::load_bytes`.
- The REPL session defines `double` and evaluates `double(21)` to `42`, then exits cleanly.

## Summary

The user observed that no standalone Keleusma compiler or REPL existed and asked for one whose ergonomics match Rhai's CLI tooling, so that a user could install Keleusma and run scripts without authoring Rust host code. The result is a new workspace member `keleusma-cli` that publishes the binary `keleusma`.

### Subcommand surface

The CLI provides three subcommands and a shorthand.

- `keleusma run <file>` parses, compiles, verifies, and executes a script through the safe `Vm::new` constructor with `register_utility_natives` and `register_audio_natives` pre-installed. The script's `main` function is invoked with no arguments and the return value is printed.
- `keleusma compile <file> [-o <output>]` runs the full compile pipeline and serializes the resulting `Module` to disk through `Module::to_bytes`. The default output path is `<input>.kel.bin`. A host can load the file through `Vm::load_bytes`.
- `keleusma repl` opens an interactive prompt. The prompt distinguishes declarations from expressions: declarations matching one of `fn`, `yield`, `loop`, `struct`, `enum`, `trait`, `impl`, `use`, `data` are appended to a session prefix and acknowledged with `defined: <name>`, while expressions are wrapped as `fn main() -> T { <expr> }` and evaluated against the current prefix. Return-type inference iterates a fixed list `i64`, `f64`, `bool`, `String`, `()` and uses the first type that compiles. REPL commands are `:help`, `:quit`, `:reset`, `:show`.
- Shorthand: any first argument ending in `.kel` is treated as `run`, so `keleusma hello.kel` is equivalent to `keleusma run hello.kel`.

### Known limitations

Documented in the README:

- The runner does not drive `yield` and `resume` interactively; stream-classified `main` functions are not directly runnable.
- The REPL does not persist data-segment values across evaluations; any `data` block declared in the prefix is allocated freshly per evaluation.
- The REPL's return-type inference uses a fixed list; expressions producing custom enums, structs, or tuples require explicit function wrapping.
- Cross-target compilation through CLI flags is future work.

### Installation

```sh
cargo install --path . --bin keleusma
```

The binary lands in the user's Cargo bin directory and is invoked as `keleusma`.

## Trade-offs and Properties

The choice to keep the CLI single-file (one `src/main.rs`) reflects the small surface area; pulling in a clap-style argument parser would inflate the dependency tree without proportional benefit. The argument parsing is hand-written and limited to the documented subcommands and flags. Future feature growth (cross-target flags, watch mode, multi-file projects) may justify migrating to a structured argument parser.

The REPL's expression-vs-declaration heuristic is keyword-prefix based rather than syntactic. This is intentional: parsing the input twice (once as a declaration, once as an expression) would double the error surface and complicate diagnostics. The current heuristic correctly classifies all declared keywords; a non-declaration line is treated as an expression and wrapped in a `main` function. Edge cases like `let` at the top level are not yet supported but can be added by extending the keyword list.

The return-type inference list is fixed because Keleusma does not currently expose a typecheck-only entry point that returns the inferred type of a top-level expression. The fixed list covers the common cases. Adding a typecheck-only API would let the REPL infer the type before wrapping, eliminating the iteration. This is recorded as future work.

The compile subcommand uses the runtime's target descriptors (word size, address size, float size). Cross-target compilation requires exposing `Target` selection through CLI flags and threading the target through the compiler's `compile_with_target` entry point. The infrastructure is in place; only the CLI surface is missing.

## Files Touched

- **`Cargo.toml`** at workspace root. Added `keleusma-cli` as a workspace member.
- **`keleusma-cli/Cargo.toml`** (new). Crate metadata; depends on `keleusma` and `keleusma-arena`. Declares the `keleusma` binary at `src/main.rs`.
- **`keleusma-cli/README.md`** (new). Usage, installation, REPL command reference, limitations.
- **`keleusma-cli/src/main.rs`** (new). Single-file implementation of the `run`, `compile`, and `repl` subcommands.
- **`keleusma-cli/examples/hello.kel`** (new). Sample script demonstrating a function and `main`.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T42 in the Task Breakdown table and a new History row.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The CLI is functional and ergonomic for the common cases. Several refinements are tracked but not blocking.

- Typecheck-only entry point exposed through the public API so the REPL can infer the type of an expression before wrapping it. This would replace the fixed return-type iteration.
- Watch mode that re-runs a script on file change, useful for iterative development.
- Cross-target compilation flags (`--target native64`, `--target embedded32`) threaded through `compile_with_target`.
- REPL persistence: writing the session prefix to a file on `:save` and restoring on `:load`. Data-segment value persistence across evaluations would require a richer state model.
- Streaming `main` support: a CLI flag that drives `resume` in a loop with a step budget, surfacing yielded values to stdout.
- Argument parsing migration to a structured library (clap or argh) once feature surface grows beyond what hand-written parsing handles cleanly.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session delivered the standalone CLI that the user identified as missing. The choice to model the ergonomics after Rhai means a user can install Keleusma through `cargo install` and immediately run scripts, compile bytecode, and explore the language interactively without writing any Rust host code. The CLI uses the safe `Vm::new` constructor end to end so all WCET and WCMU verification fires; the bytecode written by `compile` is the standard rkyv-framed wire format with magic, length, version, word and address sizes, body, and CRC trailer. The CLI is positioned as the user-facing entry point that complements the embeddable runtime.
