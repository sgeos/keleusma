# LLM Usage

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Guidance for operators who use AI coding assistants (Claude Code, Codex, Cursor, Aider, and similar) on Keleusma source or scripts. The advice below addresses patterns that AI tools trained on general Rust code tend to get wrong on first attempt, plus practical prompts that reduce iteration time.

This document is written for the operator driving the AI. It is also useful as direct context to feed an AI session at the start of work. The convention for direct-to-AI context files in the broader ecosystem is [`AGENTS.md`](../../AGENTS.md) and [`llms.txt`](../../llms.txt) at the project root. Both reference this document for deeper guidance.

## Audience

Two operator profiles benefit from this guide.

- **Embedders writing Keleusma scripts** for an embedded application. Keleusma's surface is unusual enough that LLM-generated scripts fail verification frequently on first attempt.
- **Contributors editing the Keleusma runtime** in Rust. The runtime's `no_std + alloc` posture and conservative-verification stance push back against several common Rust idioms.

Both profiles need similar context. The patterns flagged below apply across the divide.

## Reading order for any AI session

Have the AI read these documents at the start of a session before making changes.

1. [`AGENTS.md`](../../AGENTS.md) for the project's conventions and the per-session protocol.
2. This document for the AI-specific pattern gotchas.
3. [`docs/architecture/LANGUAGE_DESIGN.md`](../architecture/LANGUAGE_DESIGN.md) for the why behind the design.
4. [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) for the historical record of architectural decisions. Most "why was it done this way" questions are answered here.
5. [`docs/process/TASKLOG.md`](../process/TASKLOG.md) for current sprint state and [`docs/process/REVERSE_PROMPT.md`](../process/REVERSE_PROMPT.md) for the most recent AI-to-human handoff.

A useful first prompt to give any AI session:

```
Please read AGENTS.md, llms.txt, docs/guide/LLM_USAGE.md, the process documents
at docs/process/, then walk the knowledge graph under docs/. Summarise what
you learned in three to five paragraphs before proceeding to any task.
```

The "summarise before proceeding" forces the AI to demonstrate that it actually read rather than skimmed.

## Patterns AI tools tend to get wrong

The items below have surfaced repeatedly in AI-assisted work on this codebase. The pattern is documented; the AI is expected to read it.

### `no_std + alloc` only

Keleusma's runtime crate targets `no_std` with `alloc`. The standard library is not available.

The AI's first reflex tends to be wrong on several fronts. Replace each with the indicated `alloc` equivalent.

| Wrong (std-only) | Right (no_std + alloc) |
|------------------|-----------------------|
| `std::collections::HashMap` | `alloc::collections::BTreeMap` |
| `std::collections::HashSet` | `alloc::collections::BTreeSet` |
| `std::fs`, `std::io`, `std::process` | Not available; surface as a host-registered native function |
| `std::sync::Mutex`, `std::sync::RwLock` | Not available; the runtime is single-threaded |
| `Box::leak`, `Box::pin` | Available through `alloc::boxed::Box` but the leak pattern is rejected |
| `std::time::Instant`, `std::time::SystemTime` | Not available; clock access is host-provided through a native |
| `println!`, `eprintln!` | Not available; the `println` native is host-registered |

The `keleusma-cli` crate links against `std` and can use these APIs at the binary layer. The runtime crate cannot.

### Determinism matters even where `std` is in scope

Even in tests and the CLI, prefer `BTreeMap` over `HashMap`. The byte-identical bytecode property the test suite enforces depends on stable iteration order. Hash-map iteration order is not stable across Rust versions, and even within a single version it can drift when capacity grows. Tests that pass on one machine may fail on another if iteration order leaks into observable output.

The same applies to any other source of non-determinism: floating-point reductions, `std::hash::DefaultHasher`, system time, random number generators not explicitly seeded, file iteration order via `read_dir`. If the AI introduces any of these, the change is wrong regardless of what the prompt asked for.

### Conservative-verification stance

The safe verifier rejects constructs that defeat the worst-case execution time (WCET) and worst-case memory usage (WCMU) analyses. Specifically:

- All recursion is rejected at compile time, even when the recursive call is provably terminating. The analyses require a directed acyclic call graph.
- Closures are rejected at the type-checker stage because dynamic dispatch through closure invocation breaks the per-call cost model.
- `dyn Trait` is rejected in most positions because virtual dispatch defeats the per-call-site cost analysis.
- Flat control-flow opcodes (`Jmp`, `Branch`) are not present in the instruction set; only block-structured forms (`If`, `Else`, `EndIf`, `Loop`, `EndLoop`, `Break`, `BreakIf`) are admitted.
- Loops whose iteration count cannot be statically extracted are rejected by the strict-mode bounded-iteration analysis (R38).

The AI's natural reflex is to use any of these constructs when the algorithm seems to call for them. The right response is to rewrite the algorithm to use the work-stack pattern (explicit stack rather than recursion), enum-based dispatch (rather than `dyn Trait`), or a fixed-iteration loop (rather than an unbounded one).

The verifier's diagnostic message names the rejected construct and points to the corresponding guidance in [`docs/guide/WHY_REJECTED.md`](./WHY_REJECTED.md). Do not silence the diagnostic by relaxing the verifier; the rejection is by design.

### Trait-bounded generics over trait objects

Where the AI would reach for `&dyn Trait`, prefer `fn foo<T: Trait>(x: T)` or `fn foo<T: Trait>(x: &T)`. The monomorphizing form is admitted; the dynamic form is rejected by the conservative-verification stance in most positions.

This is the same posture as Rust embedded best practice, but the AI may not recognise the pattern. Cite the convention up front in the prompt if the work involves new trait surfaces.

### Persistent state lives in the `data` block, not in module-level statics

A common AI reflex when porting Rust code is to introduce a module-level `static` or `lazy_static` for per-run state. In Keleusma, persistent state across loop iterations belongs in the program's declared `data` block, accessed through `GetData` and `SetData`. The host owns the underlying storage; the script reads and writes by slot index. There is no module-level mutable state at the language level.

### Per-session protocol

Per [`docs/process/PROCESS_STRATEGY.md`](../process/PROCESS_STRATEGY.md), each session has a specific shape:

1. Read [`docs/process/TASKLOG.md`](../process/TASKLOG.md) for current task state.
2. Read [`docs/process/REVERSE_PROMPT.md`](../process/REVERSE_PROMPT.md) for the last AI-to-human handoff.
3. Wait for human prompt before proceeding.

After completing each task:

1. Update task status in `TASKLOG.md`.
2. Overwrite `REVERSE_PROMPT.md` with verification, questions, concerns, and intended next step.
3. Commit only if the operator explicitly asks. Otherwise leave changes uncommitted.

The protocol is documented in [`AGENTS.md`](../../AGENTS.md) and in [`CLAUDE.md`](../../CLAUDE.md) but is worth restating: AI sessions in this project run on a strict per-session contract.

### Scratch directories

Use `tmp/` for transient files (drafts, probe outputs, scratch scripts, design specs awaiting integration). The contents of `tmp/` are gitignored by convention and are never committed.

A common AI reflex is to put work in the repository root or in a new subdirectory of `docs/`. For draft material, `tmp/` is correct. For finished material, the appropriate `docs/` subdirectory is correct. The pattern is: drafts in `tmp/`, then promote to `docs/` once reviewed.

## Useful prompt patterns

The patterns below produce higher-quality output than open-ended prompts.

### The "read first, summarise, then proceed" pattern

```
Please read [specific document paths]. Summarise what you learned in three
to five paragraphs. Then [the actual task].
```

The summarise-first step forces the AI to demonstrate that it actually read the documents. It also gives the operator visible signal that the relevant context was loaded.

### The "design before implementation" pattern

```
Please produce a design document at tmp/<topic>.md before writing any code.
The design should cover [the specific concerns]. Length budget two pages.
Identify any places where the design does not map cleanly onto existing
patterns in the codebase.
```

For non-trivial work, design-before-implementation produces a reviewable artefact and reduces the cost of changes-of-direction. The pattern is established with `tmp/enrolled_keys_execution.md` and `tmp/call_site_identifier.md` as worked examples.

### The "structural-bound" pattern for algorithm work

```
Please implement [algorithm] using the work-stack pattern documented in
docs/research/r3_1_recursion_to_iteration.md. The verifier rejects
recursion; do not use recursive helper functions. Declare the stack
capacity statically.
```

Explicit reference to the work-stack pattern (or other Keleusma-specific patterns) prevents the AI from defaulting to recursive Rust idioms.

### The "verify before claiming done" pattern

```
After implementing, run the full verification suite:
  cargo test && cargo clippy --tests -- -D warnings
Do not report the task complete until both pass.
```

Forces the AI to actually run the validation rather than claim success based on inspection.

## What this guide is not

This guide is not a tutorial on Keleusma. Operators new to the language should start with [`GETTING_STARTED.md`](./GETTING_STARTED.md). This guide is also not a tutorial on AI tools; it assumes operator familiarity with whichever AI assistant is in use.

This guide is not exhaustive. New patterns surface with each AI-assisted session. When a recurring AI failure-mode is identified that is not documented here, add a section. This is a living document.

## Related prior art

The idea of publishing an LLM-targeted guidance document is borrowed from the Rex project (https://github.com/peterkelly/rex), which publishes [`docs/src/LLMS.md`](https://peterkelly.github.io/rex/LLMS.html) for the same purpose. The two projects share architectural patterns (pure functional language embedded in Rust via host-injected natives) and the LLM-targeting framing translates cleanly. Operators familiar with Rex's `LLMS.md` will recognise the shape of this guide.

The broader `llms.txt` convention was proposed by Jeremy Howard et al. in 2024 as a structured-markdown convention for declaring an AI-readable project entry point. Keleusma's [`llms.txt`](../../llms.txt) at the project root follows that convention.
