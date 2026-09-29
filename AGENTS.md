# AGENTS.md

This file provides guidance to AI coding assistants (Codex, Claude Code, Cursor, Aider, and similar tools) when working with code in this repository.

## Authoritative source

The full project context, coding conventions, and per-session protocol live in [`CLAUDE.md`](./CLAUDE.md) at the project root. Despite the filename, the content is agent-agnostic. Read `CLAUDE.md` before making any non-trivial change.

## Quick orientation

Keleusma is a Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. It targets `no_std + alloc` environments. The ecosystem value proposition is definitive worst-case execution time and worst-case memory usage. Programs whose bounds cannot be statically computed are rejected by the safe verifier.

**Status**. V0.2.2 published to crates.io. Seven workspace crates: `keleusma`, `keleusma-arena`, `keleusma-macros`, `keleusma-bench`, `keleusma-cli`, `keleusma-wire`, `keleusma-wire-derive`.

## Reading order for new sessions

1. [`CLAUDE.md`](./CLAUDE.md) for project conventions and protocol.
2. [`docs/process/HANDOFF.md`](./docs/process/HANDOFF.md) — **the resume prompt**. Run the validity check its own Validity section defines, and report the handoff as valid, or as invalid-and-stale, on that outcome. This entry was missing until 2026-09-28, so an assistant following this list alone never reached the file `CLAUDE.md`'s startup protocol names first.
3. [`docs/architecture/LANGUAGE_DESIGN.md`](./docs/architecture/LANGUAGE_DESIGN.md) for the why behind the unusual design choices.
4. [`docs/decisions/RESOLVED.md`](./docs/decisions/RESOLVED.md) for the historical record of architectural decisions.
5. [`docs/process/TASKLOG.md`](./docs/process/TASKLOG.md) for the current sprint state.
6. [`docs/process/REVERSE_PROMPT.md`](./docs/process/REVERSE_PROMPT.md) for the most recent AI-to-human handoff.
7. [`docs/roadmap/`](./docs/roadmap/) for the V0.3.0, V0.4.0, V0.5.0, and IMPLEMENTATION_ORDER strategy documents.

## Conventions worth flagging up front

Items that an AI assistant trained on general Rust code is likely to get wrong on first attempt unless flagged explicitly.

- **`no_std + alloc` only.** Do not reach for `std::collections::HashMap`, `std::fs`, `std::sync`, or `Box::leak`. Use `alloc::collections::BTreeMap`, `alloc::vec::Vec`, and equivalents.
- **Determinism matters.** Use `BTreeMap` rather than `HashMap` even where `std` is in scope, because hash-map iteration order would break the byte-identical bytecode property that the test suite enforces.
- **Conservative-verification stance.** The safe verifier rejects recursion, closures, `dyn Trait` dispatch, and other constructs that defeat the WCET and WCMU analyses. The compile pipeline admits a broader surface than the verifier accepts; both rejection paths are intentional. Do not silence verifier diagnostics by relaxing the checks.
- **Trait-bounded generics over trait objects.** Prefer `fn foo<T: Trait>(x: T)` to `fn foo(x: &dyn Trait)`. The latter is rejected by the verifier in most positions.
- **No flat jumps.** Control flow uses block-structured instructions (`If`, `Else`, `EndIf`, `Loop`, `EndLoop`, `Break`, `BreakIf`). Flat `Jmp` and `Branch` opcodes are not present in the ISA.
- **Per-session protocol.** Read `docs/process/TASKLOG.md` for current task state and `docs/process/REVERSE_PROMPT.md` for the last AI-to-human handoff before proceeding. After completing a task, update `TASKLOG.md` and overwrite `REVERSE_PROMPT.md`.
- **HARD CONSTRAINT: before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** It holds operator-stated context held outside the repository deliberately. **Do not copy any of it into tracked files, commit messages, or public documentation; reference it only as "Appendix B".** Until 2026-09-28 this constraint appeared only in `docs/process/HANDOFF.md`, which was absent from the reading order above — so an assistant could follow this file faithfully and never learn it. The failure mode is disclosure, which is irreversible.

- **Three limits are the operator's, not yours.** `BYTECODE_VERSION` moves only on explicit operator authorisation; it is 2. The rad-hard minimal-ISA constraint means the opcode count is itself a design limit, so prefer reusing an opcode over adding one; the count is 66. And a crates.io publication needs in-session confirmation. If a change appears to require any of these, stop and record the question in `REVERSE_PROMPT.md` rather than proceeding.

- **Scratch directories.** Use `tmp/` for transient files (drafts, probe outputs, scratch scripts). Contents of `tmp/` are gitignored by convention; do not commit them.
- **Commit once per prompt. Only feature branches may commit on red.** The operator confirmed this policy on 2026-09-28. Include the `REVERSE_PROMPT.md` update in the scoped conventional commit. Version branches and `main` must remain green. Use relevant subsets of gates during feature development and reserve complete suites for important integration and release checkpoints. Record what ran and what remains unverified. See `CLAUDE.md` and `docs/process/GIT_STRATEGY.md` for integration requirements. Publication requires explicit in-session confirmation.

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

**These are the everyday commands and they are not what gates a merge.** Continuous integration
runs the suite across five feature sets and adds Miri, two minimum-supported-version checks, a
`no_std` build, a cross-target build, the book, the language server, the playground, a shell-script
analysis and a dependency-advisory scan. `scripts/release-gate.sh` is a subset of it, kept for a
pre-publication run and for working offline. `CLAUDE.md` carries two catalogues of the ways a
green local run has nonetheless been misleading here; **they are the most useful pages in the
repository for anyone working in it, and they are worth reading before trusting your own green
run.**

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
