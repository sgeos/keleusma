# AGENTS.md

This file provides guidance to AI coding assistants (Codex, Claude Code, Cursor, Aider, and similar tools) when working with code in this repository.

## Authoritative source

The full project context, coding conventions, and per-session protocol live in [`CLAUDE.md`](./CLAUDE.md) at the project root. Despite the filename, the content is agent-agnostic. Read `CLAUDE.md` before making any non-trivial change.

## Quick orientation

Keleusma is a Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. It targets `no_std + alloc` environments. The ecosystem value proposition is definitive worst-case execution time and worst-case memory usage. Programs whose bounds cannot be statically computed are rejected by the safe verifier.

**Status**. V0.2.2 published to crates.io. Seven workspace crates: `keleusma`, `keleusma-arena`, `keleusma-macros`, `keleusma-bench`, `keleusma-cli`, `keleusma-wire`, `keleusma-wire-derive`.

## ⚠ TWO DEVELOPMENT LINES — check which one you are on BEFORE the reading order below

The reading order and the verification commands in this file describe the **`v0.2.3` line**. A
second line runs concurrently and **neither its channel nor its gate appears anywhere else in this
file**, so following the sections below on that line will point you at the wrong documents and run
a verification that cannot see the code you are changing.

### If you are on `v0.3.0` — the V0.3.X native-code-generation line

| | |
|---|---|
| Where | worktree `keleusma-worktrees/arena-composites`, branch `v0.3.0` |
| The package | `native_codegen/` — **a DETACHED workspace root**, not one of the seven workspace crates |
| Start here | [`docs/process/handoffs/v0.3.0-BRIEF.md`](./docs/process/handoffs/v0.3.0-BRIEF.md) — bounded. The full channel is `docs/process/handoffs/v0.3.0.md`, which is over ten thousand lines and is an ARCHIVE |
| **Not** its channel | `docs/process/TASKLOG.md` and `docs/process/REVERSE_PROMPT.md` belong to the `v0.2.3` line |

**The verification commands in "Build, test, lint" below DO NOT REACH THIS PACKAGE.** A detached
workspace root is invisible to a root `cargo test`, and continuous integration never builds it —
measured, zero occurrences of `native_codegen` across every job in `ci.yml`. Its only gate is:

```sh
cd native_codegen
tools/backend-gate.sh            # default features
tools/backend-gate.sh --narrow   # narrow-float-32
```

**One invocation covers ONE float configuration, and a single run is not both.** Each takes roughly
ten to twenty minutes and must run with the tree STILL — the gate freeze-checks itself, and editing
anything, documentation included, while it runs voids that phase. The result is written to
`native_codegen/GATE_RECORD.md`, which records the commit, whether the worktree was clean, whether
the phases stayed frozen, and the verdict. **That file, not a claim in prose, is this line's
evidence that it is green.**

**A detached package shares nothing with the parent workspace unless it declares it** — it has its
own `Cargo.lock`, its own `[profile.dev]`, its own `rust-version`. Ask that question whenever
workspace-level configuration changes; it has already cost three separate rediscoveries.

### Constraints on the `v0.3.0` line specifically

- **Root `src/` and root `tests/` are READ-ONLY to this line.** Read them freely; report problems
  rather than repairing them. Verify with the commits **not reachable from `origin/v0.2.3`**, or the
  check counts the other line's absorbed work as this line's violation.
- **No new opcodes**, and **no `BYTECODE_VERSION` change without operator authorisation.**
- **Publication is held.** Nothing is published from this line.
- **The byte-identical differential against the reference VM is the correctness signal.** A lowering
  that "looks right" is not evidence; agreement with the virtual machine is.
- **Absorb first.** `git rev-list --count HEAD..origin/v0.2.3` — derive it, never quote a figure for
  it — and run the conflict check in its OWN command before writing any prediction about the merge.
- **Commits**: the global rule below says no commits without explicit authorisation. **This line is a
  deliberate exception**: it commits per increment and pushes after a green gate. Publishing, and
  anything outward-facing, still requires confirmation.

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
| Guide | [`book/src/`](./book/src/introduction.md) | Onboarding for new users and embedders |
| Architecture | [`docs/architecture/`](./docs/architecture/README.md) | Narrative descriptions of the implemented system |
| Spec | [`docs/spec/`](./docs/spec/README.md) | Authoritative specifications: grammar, type system, standard library, instruction set, structural ISA, wire format |
| Decisions | [`docs/decisions/`](./docs/decisions/README.md) | Architectural and design decisions |
| Process | [`docs/process/`](./docs/process/README.md) | Development workflow and task tracking |
| Reference | [`docs/reference/`](./docs/reference/README.md) | Glossary, citations, prior art |
| Roadmap | [`docs/roadmap/`](./docs/roadmap/README.md) | Development phases V0.3.0 through V0.5.0 |
| Extras | [`docs/extras/`](./docs/extras/README.md) | Supplementary references for specific examples |
