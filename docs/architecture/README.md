# Architecture

> **Navigation**: [Documentation Root](../README.md)

Language design and compilation pipeline for Keleusma.

## Contents

| Document | Description |
|----------|-------------|
| [LANGUAGE_DESIGN.md](./LANGUAGE_DESIGN.md) | Design philosophy, target applications, function categories, guarantees, memory model, coroutine model, native function interface |
| [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) | Target execution model with two temporal domains, arena memory, hot code swapping, structural verification |
| [COMPILATION_PIPELINE.md](./COMPILATION_PIPELINE.md) | Four-stage pipeline from source to execution |
| [WIRE_FORMAT.md](./WIRE_FORMAT.md) | Bytecode wire format: framing header, sections, signatures, interface fingerprint |
| [SUB_COROUTINES.md](./SUB_COROUTINES.md) | Preliminary: asymmetric sub-coroutine primitive (call down, yield up), arena slot reservation, ephemeral pools and persistent slots, V0.5.0-gated |
