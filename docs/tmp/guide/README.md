# Guide

> **Navigation**: [Documentation Root](../README.md)

Onboarding-oriented documentation for new users and embedders. Where the
[architecture](../architecture/README.md), [spec](../spec/README.md), and
[reference](../reference/README.md) sections describe what Keleusma is,
this section describes how to use it.

The guide is in two layers. The first is a linear course of forty
chapters that teaches Keleusma from scratch, sized for video presentation
and ordered as a single learning arc. The second is a set of reference
pages for lookup and deeper study. The course and the reference pages
overlap by design: the course is for first learning, the reference pages
are for going back.

## The Course

Forty chapters in ten parts. Each chapter is self-contained and
demo-driven, sized to roughly one short video. Parts I through VIII teach
the language; Part IX teaches embedding into a Rust host; Part X points
onward.

### Part I — Setting Out

| Chapter | Title |
|---------|-------|
| 1 | [What Keleusma Is, and What It Is Not](./01_what_keleusma_is.md) |
| 2 | [Installing Keleusma and the Interactive Prompt](./02_installing_and_running.md) |
| 3 | [A Complete First Program](./03_first_complete_program.md) |

### Part II — The Building Blocks

| Chapter | Title |
|---------|-------|
| 4 | [Values and Types](./04_values_and_types.md) |
| 5 | [Names and Bindings](./05_names_and_bindings.md) |
| 6 | [Functions](./06_functions.md) |
| 7 | [Making Decisions](./07_making_decisions.md) |
| 8 | [Bounded Repetition](./08_bounded_repetition.md) |
| 9 | [The Pipeline Operator](./09_pipeline.md) |

### Part III — Shaping Data

| Chapter | Title |
|---------|-------|
| 10 | [Structs](./10_structs.md) |
| 11 | [Enums](./11_enums.md) |
| 12 | [Tuples and Arrays](./12_tuples_and_arrays.md) |
| 13 | [Pattern Matching in Depth](./13_pattern_matching.md) |
| 14 | [Multiheaded Functions and Guards](./14_multiheaded_functions.md) |

### Part IV — The Heart of Keleusma

| Chapter | Title |
|---------|-------|
| 15 | [The Three Function Categories](./15_three_function_categories.md) |
| 16 | [Yield: Talking to the Host](./16_yield.md) |
| 17 | [The loop Function](./17_loop_function.md) |
| 18 | [The Data Segment](./18_data_segment.md) |

### Part V — The Verifier and the Guarantees

| Chapter | Title |
|---------|-------|
| 19 | [Why Was My Program Rejected?](./19_why_rejected.md) |
| 20 | [Time and Memory Budgets](./20_time_and_memory_budgets.md) |

### Part VI — Going Deeper

| Chapter | Title |
|---------|-------|
| 21 | [Generics and Traits](./21_generics_and_traits.md) |
| 22 | [Newtypes and Refinement Types](./22_newtypes_and_refinement.md) |
| 23 | [Big Numbers: The Overflow Construct](./23_big_numbers.md) |
| 24 | [Information-Flow Labels](./24_information_flow_labels.md) |

### Part VII — Shipping a Program

| Chapter | Title |
|---------|-------|
| 25 | [From Source to Bytecode](./25_from_source_to_bytecode.md) |
| 26 | [Signed Modules and Hot Code Swap](./26_signed_modules_and_hot_swap.md) |

### Part VIII — The Capstone: Making Music

| Chapter | Title |
|---------|-------|
| 27 | [The Piano Roll: How It Works](./27_piano_roll.md) |
| 28 | [Setting Up Your Own Song Playground](./28_song_playground.md) |
| 29 | [Writing and Modifying a Song](./29_writing_a_song.md) |
| 30 | [A Tour of the Song Roster](./30_song_roster.md) |

### Part IX — Embedding Keleusma in a Rust Program

| Chapter | Title |
|---------|-------|
| 31 | [Embedding Keleusma: Orientation](./31_embedding_orientation.md) |
| 32 | [Constructing a VM and Running a Module](./32_constructing_a_vm.md) |
| 33 | [Registering Native Functions](./33_registering_natives.md) |
| 34 | [The Coroutine Protocol from the Host Side](./34_coroutine_protocol.md) |
| 35 | [Sizing the Arena and Reading the Bounds](./35_arena_sizing.md) |
| 36 | [Loading Precompiled and Signed Bytecode](./36_loading_bytecode.md) |
| 37 | [Hot Code Swap from the Host](./37_hot_swap_host.md) |
| 38 | [Calibrated WCET and Cost Models](./38_cost_models.md) |
| 39 | [A Full Host, End to End](./39_full_host.md) |

### Part X — Where to Go Next

| Chapter | Title |
|---------|-------|
| 40 | [Further Reading](./40_further_reading.md) |

## Reference Pages

The reference pages are the topic-organized companions to the linear
course. Each is a self-contained document that the corresponding course
chapters draw on and that a reader returns to when looking up a specific
question.

| Document | Audience | Purpose |
|----------|----------|---------|
| [GETTING_STARTED.md](./GETTING_STARTED.md) | First-time user | Install the CLI, write a first script, run it, embed it in a twenty-line Rust host. Course coverage: Chapter 2, Chapter 31. |
| [EMBEDDING.md](./EMBEDDING.md) | Rust host author | The complete host-facing reference: `Vm` construction, native function registration, arena sizing, the call and resume protocol, error recovery, opaque host types, the `Library` trait, signed modules. Course coverage: Part IX (Chapters 31 through 39). |
| [PIANO_ROLL.md](./PIANO_ROLL.md) | Song author, host lifter, or host architect | The long-form manual for the piano-roll example: composing songs, lifting the host loop into another application, and using the example as an architectural reference for other control-loop domains. Course coverage: Part VIII (Chapters 27 through 30). |
| [ROGUE.md](./ROGUE.md) | Game author or host architect | The long-form manual for the roguelike example: gameplay rules, the host and multi-script architecture, dungeon generation, the artificial-intelligence archetypes, item effects, and exercises. Not covered by a chapter; pointed to from Chapter 40. |
| [WHY_REJECTED.md](./WHY_REJECTED.md) | Anyone whose program failed verification | The full catalogue of verifier rejection messages, mapped to root causes and proposed rewrites. Course coverage: Chapter 19 introduces the three most common rejections; this document is the full list. |
| [FAQ.md](./FAQ.md) | Anyone who hit a surprise | Common rough edges in V0.2.0, including string handling, escape sequences, the immutable-locals constraint, and migration notes from the V0.1.x line. |
| [COOKBOOK.md](./COOKBOOK.md) | Embedder reaching for a known-good pattern | Working recipes for embedding patterns: the data-loader pattern, auto-sizing the arena from a module's WCMU, narrow-runtime type aliasing, signed bytecode distribution, calibrated WCET with a measured cost model. Course coverage: Chapter 39 names the patterns; this document is the recipes. |
| [BIG_NUMBERS.md](./BIG_NUMBERS.md) | Author needing multi-digit arithmetic | The full worked example for the overflow construct: 64-by-64 to 128-bit multiplication via the high half, and addition with explicit carry-out propagation for chained multi-digit arithmetic. Course coverage: Chapter 23 introduces the construct; this document is the full technique. |
| [LLM_USAGE.md](./LLM_USAGE.md) | Operator using AI coding assistants | Patterns AI tools tend to get wrong, the read-AGENTS-first session protocol, prompt patterns that reduce iteration time. |
| [SECURITY_POLICY.md](./SECURITY_POLICY.md) | Operator deploying `keleusma-cli` in constrained environments | The strict-mode signing and encryption policies introduced in V0.2.1: key generation, policy activation, deployment scenarios, the trust model. |

## Companion Material

| Path | Content |
|------|---------|
| [`examples/scripts/`](../../examples/scripts) | Standalone `.kel` files demonstrating language features. Run any of them with `keleusma run examples/scripts/<file>.kel`. |
| [`examples/`](../../examples) | Rust embedding examples. Run with `cargo run --example <name>`. |
| [`examples/piano_roll.rs`](../../examples/piano_roll.rs) | End-to-end SDL3 audio host with hot code swap across a song roster. Run with `cargo run --release --example piano_roll --features sdl3-example`. |
| [`examples/rogue/`](../../examples/rogue) | End-to-end SDL3 video host driving a roguelike. Run with `cargo run --release --example rogue --features sdl3-example`. |
| [`keleusma-cli/`](../../keleusma-cli) | The standalone command-line frontend installed by `cargo install --path keleusma-cli --bin keleusma`. |

## Reference Cross-Links

When a term is unfamiliar:

- [GLOSSARY.md](../reference/GLOSSARY.md) defines core terms.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the
  function categories, the five guarantees, and the conservative-
  verification stance.
- [GRAMMAR.md](../spec/GRAMMAR.md) is the formal syntax reference.
- [TYPE_SYSTEM.md](../spec/TYPE_SYSTEM.md) describes primitive types,
  string discipline, and composite types.
- [STANDARD_LIBRARY.md](../spec/STANDARD_LIBRARY.md) lists the bundled
  native functions in the `audio::` and `math::` namespaces.

## Planning Document

[OUTLINE.md](./OUTLINE.md) is the planning document the course was
drafted against. It records the pedagogical decisions, the music-to-
Keleusma concept map, and the open questions still under review. It is
working material rather than user-facing documentation.
