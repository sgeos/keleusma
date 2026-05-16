# Guide

> **Navigation**: [Documentation Root](../README.md)

Onboarding-oriented documentation for new users and embedders. Where the [architecture](../architecture/README.md), [design](../design/README.md), and [reference](../reference/README.md) sections describe what Keleusma is, this section describes how to use it.

## Sequence

| Document | Audience | Purpose |
|----------|----------|---------|
| [GETTING_STARTED.md](./GETTING_STARTED.md) | First-time user | Install the CLI, write a first script, run it, embed it in a twenty-line Rust host |
| [EMBEDDING.md](./EMBEDDING.md) | Rust host author | Construct a `Vm`, register native functions, size the arena, drive the call and resume loop, recover from errors |
| [WHY_REJECTED.md](./WHY_REJECTED.md) | Anyone whose program failed verification | Map verifier error messages to the conservative-verification taxonomy and propose rewrites |
| [FAQ.md](./FAQ.md) | Anyone who hit a surprise | Common rough edges in V0.1.x, including string handling, escape sequences, and the immutable-locals constraint |

## Companion Material

| Path | Content |
|------|---------|
| [`examples/scripts/`](../../examples/scripts) | Standalone `.kel` files demonstrating language features. Run any of them with `keleusma run examples/scripts/<file>.kel` |
| [`examples/`](../../examples) | Rust embedding examples. Run with `cargo run --example <name>` |
| [`examples/piano_roll.rs`](../../examples/piano_roll.rs) | End-to-end SDL3 audio host with hot code swap between two songs. Feature-gated. Run with `cargo run --release --example piano_roll --features sdl3-example` |
| [`keleusma-cli/`](../../keleusma-cli) | The standalone command-line frontend |

## Reference Cross-Links

The guide assumes some familiarity with Keleusma's vocabulary. When a term is unfamiliar:

- [GLOSSARY.md](../reference/GLOSSARY.md) defines core terms.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the function categories, the five guarantees, and the conservative-verification stance.
- [GRAMMAR.md](../design/GRAMMAR.md) is the formal syntax reference.
- [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md) describes primitive types, string discipline, and composite types.
- [STANDARD_LIBRARY.md](../design/STANDARD_LIBRARY.md) lists the bundled native functions in the `audio::` and `math::` namespaces.
