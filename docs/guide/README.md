# Guide

> **Navigation**: [Documentation Root](../README.md)

Onboarding-oriented documentation for new users and embedders. Where the [architecture](../architecture/README.md), [design](../spec/README.md), and [reference](../reference/README.md) sections describe what Keleusma is, this section describes how to use it.

## Sequence

| Document | Audience | Purpose |
|----------|----------|---------|
| [GETTING_STARTED.md](./GETTING_STARTED.md) | First-time user | Install the CLI, write a first script, run it, embed it in a twenty-line Rust host |
| [EMBEDDING.md](./EMBEDDING.md) | Rust host author | Construct a `Vm`, register native functions, size the arena, drive the call and resume loop, recover from errors |
| [PIANO_ROLL.md](./PIANO_ROLL.md) | Song author, host lifter, or host architect | Long-form companion to the `piano_roll` example. Covers writing songs, lifting the host loop into another application, and using the example as a pattern reference for embedding Keleusma in other control-loop domains |
| [ROGUE.md](./ROGUE.md) | Game author or host architect | Long-form companion to the `rogue` example. Covers gameplay rules, the host and twelve-script architecture, the dungeon generator, the eight artificial-intelligence archetypes, and the item-effect scripts |
| [WHY_REJECTED.md](./WHY_REJECTED.md) | Anyone whose program failed verification | Map verifier error messages to the conservative-verification taxonomy and propose rewrites |
| [FAQ.md](./FAQ.md) | Anyone who hit a surprise | Common rough edges in V0.2.0, including string handling, escape sequences, the immutable-locals constraint, and migration notes from the V0.1.x pre-release line |
| [COOKBOOK.md](./COOKBOOK.md) | Embedder reaching for a known-good pattern | Working recipes for embedding patterns. Starts with the data-loader pattern for shipping designer-editable configuration tables in script form |
| [BIG_NUMBERS.md](./BIG_NUMBERS.md) | Author needing multi-digit arithmetic | Worked example for the V0.2 pattern-arm checked construct. Demonstrates full 64x64 -> 128-bit multiplication via the high half and addition with explicit carry-out propagation for chained multi-digit arithmetic |

## Companion Material

| Path | Content |
|------|---------|
| [`examples/scripts/`](../../examples/scripts) | Standalone `.kel` files demonstrating language features. Run any of them with `keleusma run examples/scripts/<file>.kel` |
| [`examples/`](../../examples) | Rust embedding examples. Run with `cargo run --example <name>` |
| [`examples/piano_roll.rs`](../../examples/piano_roll.rs) | End-to-end SDL3 audio host with hot code swap across a song roster. Feature-gated. Run with `cargo run --release --example piano_roll --features sdl3-example`. See [PIANO_ROLL.md](./PIANO_ROLL.md) for the manual |
| [`examples/rogue/`](../../examples/rogue) | End-to-end SDL3 video host driving a roguelike. Nineteen Keleusma scripts for dungeon generation, player and monster artificial intelligence, combat resolution, and item effects. Feature-gated. Run with `cargo run --release --example rogue --features sdl3-example`. See [ROGUE.md](./ROGUE.md) for the manual |
| [`keleusma-cli/`](../../keleusma-cli) | The standalone command-line frontend |

## Reference Cross-Links

The guide assumes some familiarity with Keleusma's vocabulary. When a term is unfamiliar:

- [GLOSSARY.md](../reference/GLOSSARY.md) defines core terms.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the function categories, the five guarantees, and the conservative-verification stance.
- [GRAMMAR.md](../spec/GRAMMAR.md) is the formal syntax reference.
- [TYPE_SYSTEM.md](../spec/TYPE_SYSTEM.md) describes primitive types, string discipline, and composite types.
- [STANDARD_LIBRARY.md](../spec/STANDARD_LIBRARY.md) lists the bundled native functions in the `audio::` and `math::` namespaces.
