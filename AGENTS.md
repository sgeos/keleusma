# AGENTS.md

This file provides guidance to AI coding assistants (Codex, Claude Code, Cursor, Aider, and similar tools) when working with code in this repository.

## Authoritative source

The full project context, coding conventions, and per-session protocol live in [`CLAUDE.md`](./CLAUDE.md) at the project root. Despite the filename, the content is agent-agnostic. Read `CLAUDE.md` before making any non-trivial change.

## Quick orientation

Keleusma is a Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. It targets `no_std + alloc` environments. The ecosystem value proposition is definitive worst-case execution time and worst-case memory usage. Programs whose bounds cannot be statically computed are rejected by the safe verifier.

**Status**. V0.2.0 published to crates.io. Five workspace crates: `keleusma`, `keleusma-arena`, `keleusma-macros`, `keleusma-bench`, `keleusma-cli`.

## Reading order for new sessions

1. [`CLAUDE.md`](./CLAUDE.md) for project conventions and protocol.
2. [`docs/architecture/LANGUAGE_DESIGN.md`](./docs/architecture/LANGUAGE_DESIGN.md) for the why behind the unusual design choices.
3. [`docs/decisions/RESOLVED.md`](./docs/decisions/RESOLVED.md) for the historical record of architectural decisions.
4. [`docs/process/TASKLOG.md`](./docs/process/TASKLOG.md) for the current sprint state.
5. [`docs/process/REVERSE_PROMPT.md`](./docs/process/REVERSE_PROMPT.md) for the most recent AI-to-human handoff.
6. [`docs/roadmap/`](./docs/roadmap/) for the V0.3.0, V0.4.0, V0.5.0, and IMPLEMENTATION_ORDER strategy documents.

## Conventions worth flagging up front

Items that an AI assistant trained on general Rust code is likely to get wrong on first attempt unless flagged explicitly.

- **`no_std + alloc` only.** Do not reach for `std::collections::HashMap`, `std::fs`, `std::sync`, or `Box::leak`. Use `alloc::collections::BTreeMap`, `alloc::vec::Vec`, and equivalents.
- **Determinism matters.** Use `BTreeMap` rather than `HashMap` even where `std` is in scope, because hash-map iteration order would break the byte-identical bytecode property that the test suite enforces.
- **Conservative-verification stance.** The safe verifier rejects recursion, closures, `dyn Trait` dispatch, and other constructs that defeat the WCET and WCMU analyses. The compile pipeline admits a broader surface than the verifier accepts; both rejection paths are intentional. Do not silence verifier diagnostics by relaxing the checks.
- **Trait-bounded generics over trait objects.** Prefer `fn foo<T: Trait>(x: T)` to `fn foo(x: &dyn Trait)`. The latter is rejected by the verifier in most positions.
- **No flat jumps.** Control flow uses block-structured instructions (`If`, `Else`, `EndIf`, `Loop`, `EndLoop`, `Break`, `BreakIf`). Flat `Jmp` and `Branch` opcodes are not present in the ISA.
- **Per-session protocol.** Read `docs/process/TASKLOG.md` for current task state and `docs/process/REVERSE_PROMPT.md` for the last AI-to-human handoff before proceeding. After completing a task, update `TASKLOG.md` and overwrite `REVERSE_PROMPT.md`.
- **Scratch directories.** Use `tmp/` for transient files (drafts, probe outputs, scratch scripts). Contents of `tmp/` are gitignored by convention; do not commit them.
- **No commits without explicit authorisation.** Even when work is complete, do not run `git commit` unless the human operator explicitly asks.

## Build, test, lint

```sh
cargo build
cargo test
cargo fmt
cargo clippy --tests -- -D warnings
```

Full verification before considering work complete:

```sh
cargo test && cargo clippy --tests -- -D warnings
```

## Other documentation entry points

| Section | Path | Description |
|---------|------|-------------|
| Guide | [`docs/guide/`](./docs/guide/README.md) | Onboarding for new users and embedders |
| Architecture | [`docs/architecture/`](./docs/architecture/README.md) | Narrative descriptions of the implemented system |
| Spec | [`docs/spec/`](./docs/spec/README.md) | Authoritative specifications: grammar, type system, standard library, instruction set, structural ISA, wire format |
| Decisions | [`docs/decisions/`](./docs/decisions/README.md) | Architectural and design decisions |
| Process | [`docs/process/`](./docs/process/README.md) | Development workflow and task tracking |
| Reference | [`docs/reference/`](./docs/reference/README.md) | Glossary, citations, prior art |
| Roadmap | [`docs/roadmap/`](./docs/roadmap/README.md) | Development phases V0.3.0 through V0.5.0 |
| Extras | [`docs/extras/`](./docs/extras/README.md) | Supplementary references for specific examples |
