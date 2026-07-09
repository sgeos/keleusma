# Keleusma Documentation

> A Total Functional Stream Processor that compiles to bytecode.

This documentation is structured as a **knowledge graph** encoded in the file system. Each file contains one atomic concept. Navigate by following links from section indexes.

## Sections

| Section | Path | Description |
|---------|------|-------------|
| Guide | [guide/](../book/src/introduction.md) | Onboarding for new users and embedders |
| Architecture | [architecture/](./architecture/README.md) | Narrative descriptions of the implemented system |
| Spec | [spec/](./spec/README.md) | Authoritative specifications: grammar, type system, standard library, instruction set, structural ISA, wire format |
| Decisions | [decisions/](./decisions/README.md) | Resolved, priority, and backlog decisions |
| Process | [process/](./process/README.md) | Development workflow and task tracking |
| Reference | [reference/](./reference/README.md) | Glossary and citations |
| Roadmap | [roadmap/](./roadmap/README.md) | Development phases |
| Extras | [extras/](./extras/README.md) | Supplementary references for specific examples |

## Quick Reference

| If you need... | Start here |
|----------------|------------|
| First-time setup and a working example | [guide/GETTING_STARTED.md](../book/src/GETTING_STARTED.md) |
| Linear learning course (forty chapters) | [guide/README.md](../book/src/introduction.md) |
| Embedding Keleusma in a Rust host | [guide/EMBEDDING.md](../book/src/EMBEDDING.md) |
| Recipes for common embedding patterns | [guide/COOKBOOK.md](../book/src/COOKBOOK.md) |
| A program rejected by the verifier | [guide/WHY_REJECTED.md](../book/src/WHY_REJECTED.md) |
| Surprises and rough edges in V0.2.x | [guide/FAQ.md](../book/src/FAQ.md) |
| Strict-mode policies and daemon deployments | [guide/SECURITY_POLICY.md](../book/src/SECURITY_POLICY.md) |
| Binary, memory, and CPU footprint | [guide/METRICS.md](../book/src/METRICS.md) |
| Language overview | [architecture/LANGUAGE_DESIGN.md](./architecture/LANGUAGE_DESIGN.md) |
| Execution model and two temporal domains | [architecture/EXECUTION_MODEL.md](./architecture/EXECUTION_MODEL.md) |
| Compilation pipeline | [architecture/COMPILATION_PIPELINE.md](./architecture/COMPILATION_PIPELINE.md) |
| Bytecode wire format | [spec/WIRE_FORMAT.md](./spec/WIRE_FORMAT.md) |
| Sub-coroutine primitive (preliminary, V0.5.0-gated) | [architecture/SUB_COROUTINES.md](./architecture/SUB_COROUTINES.md) |
| Formal grammar | [spec/GRAMMAR.md](./spec/GRAMMAR.md) |
| Type system | [spec/TYPE_SYSTEM.md](./spec/TYPE_SYSTEM.md) |
| Built-in functions | [spec/STANDARD_LIBRARY.md](./spec/STANDARD_LIBRARY.md) |
| Bytecode instruction reference | [spec/INSTRUCTION_SET.md](./spec/INSTRUCTION_SET.md) |
| Structural ISA description | [spec/STRUCTURAL_ISA.md](./spec/STRUCTURAL_ISA.md) |
| Terminology | [reference/GLOSSARY.md](./reference/GLOSSARY.md) |
| Related work and citations | [reference/RELATED_WORK.md](./reference/RELATED_WORK.md) |
| Design decisions | [decisions/RESOLVED.md](./decisions/RESOLVED.md) |
| Open questions | [decisions/PRIORITY.md](./decisions/PRIORITY.md) |
| Deferred items | [decisions/BACKLOG.md](./decisions/BACKLOG.md) |
| Current task | [process/TASKLOG.md](./process/TASKLOG.md) |
| Development roadmap | [roadmap/README.md](./roadmap/README.md) |
| Standalone scripts to run | [`examples/scripts/`](../examples/scripts) |
| Rust embedding examples | [`examples/`](../examples) |
| End-to-end SDL3 audio demo with hot swap | [`examples/piano_roll.rs`](../examples/piano_roll.rs) |
| Manual for the piano-roll example | [guide/PIANO_ROLL.md](../book/src/PIANO_ROLL.md) |
| Cooperative RTOS microkernel running on STM32N6570-DK | [`examples/rtos/`](../examples/rtos/README.md) |
| Operator manual for the RTOS example | [`examples/rtos/MANUAL.md`](../examples/rtos/MANUAL.md) |
| Architectural rationale for the RTOS microkernel | [`examples/rtos/SPEC.md`](../examples/rtos/SPEC.md) |

## Meta

See [DOCUMENTATION_STRATEGY.md](./DOCUMENTATION_STRATEGY.md) for conventions and navigation guidance.
