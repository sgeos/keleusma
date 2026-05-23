# Architecture

> **Navigation**: [Documentation Root](../README.md)

Narrative descriptions of the implemented system: language design philosophy, execution model, compilation pipeline, and feature architecture.

For authoritative specifications (grammar, type system, instruction set, structural ISA, wire format), see [spec/](../spec/README.md).

## Contents

| Document | Description |
|----------|-------------|
| [LANGUAGE_DESIGN.md](./LANGUAGE_DESIGN.md) | Design philosophy, target applications, function categories, guarantees, memory model, coroutine model, native function interface |
| [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) | Target execution model with two temporal domains, arena memory, hot code swapping, structural verification |
| [COMPILATION_PIPELINE.md](./COMPILATION_PIPELINE.md) | Four-stage pipeline from source to execution |
| [SUB_COROUTINES.md](./SUB_COROUTINES.md) | Preliminary: asymmetric sub-coroutine primitive (call down, yield up), arena slot reservation, ephemeral pools and persistent slots, V0.5.0-gated |
| [RUN_TASKS.md](./RUN_TASKS.md) | Design proposal: multi-script runner (`keleusma run-tasks <manifest.toml>`); cooperative scheduler with event queue and supervised restart, RTOS-shaped on the desktop |
